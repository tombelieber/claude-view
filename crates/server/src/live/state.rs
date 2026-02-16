//! Live session state types and status derivation for Mission Control.
//!
//! Provides real-time session status tracking by analyzing the last JSONL line,
//! file modification time, and process presence.

use serde::{Serialize, Deserialize};
use vibe_recall_core::cost::{CacheStatus, CostBreakdown, TokenUsage};
use vibe_recall_core::live_parser::{LineType, LiveLine};

/// The universal agent state — core protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    /// Which UI group this belongs to (fixed, never changes)
    pub group: AgentStateGroup,
    /// Sub-state within group (open string — new states added freely)
    pub state: String,
    /// Human-readable label (domain layer can override)
    pub label: String,
    /// How confident we are in this classification
    pub confidence: f32,
    /// How this state was determined
    pub source: SignalSource,
    /// Raw context for domain layers to interpret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStateGroup {
    NeedsYou,
    Autonomous,
    #[allow(dead_code)]
    Delivered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSource {
    Hook,
    Jsonl,
    Fallback,
}

/// The current status of a live Claude Code session.
///
/// 3-state model: Working (actively streaming/tool use), Paused (waiting for
/// input, task complete, or idle), Done (session over).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Agent is actively streaming or using tools.
    Working,
    /// Agent paused -- reason available in pause_classification.
    Paused,
    /// Session is over (process exited + no new writes for 300s).
    Done,
}

/// A live session snapshot broadcast to connected SSE clients.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    /// Session UUID (filename without .jsonl extension).
    pub id: String,
    /// Encoded project directory name (as stored on disk).
    pub project: String,
    /// Human-readable project name (last path component, decoded).
    pub project_display_name: String,
    /// Full decoded project path.
    pub project_path: String,
    /// Absolute path to the JSONL session file.
    pub file_path: String,
    /// Current derived session status.
    pub status: SessionStatus,
    /// Universal agent state — replaces pause_classification.
    /// Always present (never null), with group/state/label/confidence.
    pub agent_state: AgentState,
    /// Git branch name, if detected.
    pub git_branch: Option<String>,
    /// PID of the running Claude process, if any.
    pub pid: Option<u32>,
    /// Session title derived from the first non-meta user message.
    pub title: String,
    /// The last user message text (truncated for display).
    pub last_user_message: String,
    /// Human-readable description of the current activity.
    pub current_activity: String,
    /// Number of user/assistant turn pairs.
    pub turn_count: u32,
    /// Unix timestamp when the session started, if known.
    pub started_at: Option<i64>,
    /// Unix timestamp of the most recent file modification.
    pub last_activity_at: i64,
    /// The primary model used in this session.
    pub model: Option<String>,
    /// Accumulated token usage for this session (cumulative, for cost).
    pub tokens: TokenUsage,
    /// Current context window fill: total input tokens from the last assistant turn.
    pub context_window_tokens: u64,
    /// Computed cost breakdown in USD.
    pub cost: CostBreakdown,
    /// Whether the Anthropic prompt cache is likely warm or cold.
    pub cache_status: CacheStatus,
    /// Unix timestamp when the current user turn started (real prompt detected).
    /// Used by frontend to compute live elapsed time for autonomous sessions.
    pub current_turn_started_at: Option<i64>,
    /// Seconds the agent spent on the last completed turn (frozen on Working->Paused).
    /// Used by frontend to show task time for needs_you sessions.
    pub last_turn_task_seconds: Option<u32>,
    /// Sub-agents spawned via the Task tool in this session.
    /// Empty vec if no sub-agents have been detected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_agents: Vec<vibe_recall_core::subagent::SubAgentInfo>,
}

/// Events broadcast over the SSE channel to connected Mission Control clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A new session JSONL file was discovered on disk.
    SessionDiscovered {
        session: LiveSession,
    },
    /// An existing session was updated (new lines appended to JSONL).
    SessionUpdated {
        session: LiveSession,
    },
    /// A session has been cleaned up (Complete for >10 min).
    SessionCompleted {
        session_id: String,
    },
    /// Periodic aggregate summary of all live sessions.
    Summary {
        #[serde(rename = "needsYouCount")]
        needs_you_count: usize,
        #[serde(rename = "autonomousCount")]
        autonomous_count: usize,
        #[serde(rename = "deliveredCount")]
        #[allow(dead_code)]
        delivered_count: usize,
        #[serde(rename = "totalCostTodayUsd")]
        total_cost_today_usd: f64,
        #[serde(rename = "totalTokensToday")]
        total_tokens_today: u64,
    },
}

/// Derive the session status from the last parsed JSONL line, file age, and
/// process presence.
///
/// 3-state derivation (first match wins):
/// 1. No data at all -> Paused
/// 2. No running process AND file stale >300s -> Done
/// 3. Activity within 30s:
///    - Assistant with tools -> Working
///    - Assistant still streaming (no end_turn) -> Working
///    - Assistant with end_turn -> Paused
///    - Progress -> Working
///    - Real user prompt -> Working (Claude enters local thinking immediately)
///    - Meta/system-injected user line, System/other -> Paused
/// 4. >30s since last write -> Paused
pub fn derive_status(
    last_line: Option<&LiveLine>,
    seconds_since_modified: u64,
    has_running_process: bool,
) -> SessionStatus {
    let last_line = match last_line {
        Some(ll) => ll,
        None => return SessionStatus::Paused,
    };

    // Done: process exited AND file stale >300s
    if !has_running_process && seconds_since_modified > 300 {
        return SessionStatus::Done;
    }

    // Working: active streaming or tool use (within last 30s)
    if seconds_since_modified <= 30 {
        match last_line.line_type {
            LineType::Assistant => {
                if !last_line.tool_names.is_empty() {
                    return SessionStatus::Working;
                }
                if last_line.stop_reason.as_deref() != Some("end_turn") {
                    return SessionStatus::Working;
                }
                // end_turn = Claude finished -> Paused
                SessionStatus::Paused
            }
            LineType::Progress => SessionStatus::Working,
            LineType::User => {
                // A real user prompt means Claude is now thinking/working, even if
                // no assistant tokens have streamed yet.
                let is_real_user_prompt = !last_line.is_meta
                    && !last_line.is_tool_result_continuation
                    && !last_line.has_system_prefix;
                if is_real_user_prompt {
                    SessionStatus::Working
                } else {
                    SessionStatus::Paused
                }
            }
            // System message and other line types -> Paused
            _ => SessionStatus::Paused,
        }
    } else {
        // >30s since last write -> Paused
        SessionStatus::Paused
    }
}

/// Derive a human-readable activity description from the tool names in use.
///
/// Returns an empty string when no tools are active and the session is not streaming.
pub fn derive_activity(tool_names: &[String], is_streaming: bool) -> String {
    if is_streaming && tool_names.is_empty() {
        return "Generating response...".to_string();
    }

    if tool_names.is_empty() {
        return String::new();
    }

    // Use the most "interesting" tool for the activity string
    for name in tool_names {
        let activity = match name.as_str() {
            "Read" => "Reading file",
            "Write" => "Writing file",
            "Edit" => "Editing file",
            "Bash" => "Running command",
            "Glob" => "Searching files",
            "Grep" => "Searching code",
            "WebFetch" => "Fetching URL",
            "WebSearch" => "Searching the web",
            "Task" => "Spawning sub-agent",
            "NotebookEdit" => "Editing notebook",
            "TodoRead" | "TodoWrite" => "Managing tasks",
            _ => "",
        };
        if !activity.is_empty() {
            return activity.to_string();
        }
    }

    // Fallback for unknown tool names
    format!("Using {}", tool_names[0])
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a minimal LiveLine for testing.
    fn make_live_line(
        line_type: LineType,
        tool_names: Vec<String>,
        stop_reason: Option<&str>,
    ) -> LiveLine {
        LiveLine {
            line_type,
            role: None,
            content_preview: String::new(),
            tool_names,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            timestamp: None,
            stop_reason: stop_reason.map(String::from),
            git_branch: None,
            is_meta: false,
            is_tool_result_continuation: false,
            has_system_prefix: false,
            sub_agent_spawns: Vec::new(),
            sub_agent_result: None,
        }
    }

    // -------------------------------------------------------------------------
    // derive_status tests (3-state: Working, Paused, Done)
    // -------------------------------------------------------------------------

    #[test]
    fn test_status_paused_no_data() {
        let status = derive_status(None, 0, false);
        assert_eq!(status, SessionStatus::Paused);
    }

    #[test]
    fn test_status_working_streaming_recent() {
        let last = make_live_line(LineType::Assistant, vec![], None);
        let status = derive_status(Some(&last), 10, true);
        assert_eq!(status, SessionStatus::Working);
    }

    #[test]
    fn test_status_working_tool_use_recent() {
        let last = make_live_line(LineType::Assistant, vec!["Bash".to_string()], None);
        let status = derive_status(Some(&last), 25, true);
        assert_eq!(status, SessionStatus::Working);
    }

    #[test]
    fn test_status_paused_end_turn_recent() {
        let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
        let status = derive_status(Some(&last), 10, true);
        assert_eq!(status, SessionStatus::Paused);
    }

    #[test]
    fn test_status_paused_at_31s() {
        let last = make_live_line(LineType::Assistant, vec![], None);
        let status = derive_status(Some(&last), 31, true);
        assert_eq!(status, SessionStatus::Paused);
    }

    #[test]
    fn test_status_paused_at_61s_with_process() {
        let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
        let status = derive_status(Some(&last), 61, true);
        assert_eq!(status, SessionStatus::Paused, "At 61s with running process, status is Paused (not Done because process is active)");
    }

    #[test]
    fn test_status_paused_at_61s_no_process() {
        let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
        let status = derive_status(Some(&last), 61, false);
        assert_eq!(status, SessionStatus::Paused); // was Done before; now in grace window
    }

    #[test]
    fn test_status_done_at_301s_no_process() {
        let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
        let status = derive_status(Some(&last), 301, false);
        assert_eq!(status, SessionStatus::Done);
    }

    #[test]
    fn test_status_working_progress_recent() {
        let last = make_live_line(LineType::Progress, vec![], None);
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::Working);
    }

    #[test]
    fn test_status_working_recent_user_prompt() {
        // After a real user prompt, Claude enters local "thinking" immediately.
        // Status should surface as Working so the UI flips to Running right away.
        let last = make_live_line(LineType::User, vec![], None);
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::Working);
    }

    #[test]
    fn test_status_paused_meta_user_message() {
        let mut last = make_live_line(LineType::User, vec![], None);
        last.is_meta = true;
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::Paused);
    }

    #[test]
    fn test_status_working_streaming_no_process() {
        // Edge case: process exits during active streaming -- recent activity keeps it Working
        let last = make_live_line(LineType::Assistant, vec![], None);
        let status = derive_status(Some(&last), 5, false);
        assert_eq!(status, SessionStatus::Working);
    }

    #[test]
    fn test_status_paused_system_message() {
        let last = make_live_line(LineType::System, vec![], None);
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::Paused);
    }

    // -------------------------------------------------------------------------
    // derive_activity tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_activity_streaming() {
        let activity = derive_activity(&[], true);
        assert_eq!(activity, "Generating response...");
    }

    #[test]
    fn test_activity_no_tools_not_streaming() {
        let activity = derive_activity(&[], false);
        assert_eq!(activity, "");
    }

    #[test]
    fn test_activity_read() {
        let tools = vec!["Read".to_string()];
        let activity = derive_activity(&tools, false);
        assert_eq!(activity, "Reading file");
    }

    #[test]
    fn test_activity_edit() {
        let tools = vec!["Edit".to_string()];
        let activity = derive_activity(&tools, false);
        assert_eq!(activity, "Editing file");
    }

    #[test]
    fn test_activity_bash() {
        let tools = vec!["Bash".to_string()];
        let activity = derive_activity(&tools, false);
        assert_eq!(activity, "Running command");
    }

    #[test]
    fn test_activity_grep() {
        let tools = vec!["Grep".to_string()];
        let activity = derive_activity(&tools, false);
        assert_eq!(activity, "Searching code");
    }

    #[test]
    fn test_activity_unknown_tool() {
        let tools = vec!["CustomMcpTool".to_string()];
        let activity = derive_activity(&tools, false);
        assert_eq!(activity, "Using CustomMcpTool");
    }

    #[test]
    fn test_activity_multiple_tools_first_wins() {
        let tools = vec!["Bash".to_string(), "Read".to_string()];
        let activity = derive_activity(&tools, false);
        assert_eq!(activity, "Running command");
    }
}
