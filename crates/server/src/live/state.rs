//! Live session state types and status derivation for Mission Control.
//!
//! Provides real-time session status tracking by analyzing the last JSONL line,
//! file modification time, and process presence.

use serde::Serialize;
use vibe_recall_core::cost::{CacheStatus, CostBreakdown, TokenUsage};
use vibe_recall_core::live_parser::{LineType, LiveLine};

/// The current status of a live Claude Code session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Assistant is actively generating a response.
    Streaming,
    /// A tool call is in progress (Read, Write, Bash, etc.).
    ToolUse,
    /// Waiting for the user to provide input.
    WaitingForUser,
    /// No activity for more than 60 seconds.
    Idle,
    /// Session is finished (no running process and >5 min inactive).
    Complete,
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
        active_count: usize,
        waiting_count: usize,
        idle_count: usize,
        total_cost_today_usd: f64,
        total_tokens_today: u64,
    },
}

/// Derive the session status from the last parsed JSONL line, file age, and
/// process presence.
///
/// Status derivation priority (first match wins):
/// 1. No data at all -> Idle
/// 2. File inactive >5min AND no running process -> Complete
/// 3. File inactive >60s -> Idle
/// 4. Last line is Assistant with tool_use in tool_names -> ToolUse
/// 5. Last line is Assistant streaming (no "end_turn" stop_reason) -> Streaming
/// 6. Last line is User OR (Assistant with stop_reason "end_turn") OR System -> WaitingForUser
/// 7. Last line is Progress -> ToolUse
/// 8. Default -> Idle
pub fn derive_status(
    last_line: Option<&LiveLine>,
    seconds_since_modified: u64,
    has_running_process: bool,
) -> SessionStatus {
    let last_line = match last_line {
        Some(ll) => ll,
        None => return SessionStatus::Idle,
    };

    // Complete: inactive for >5 min and no process
    if seconds_since_modified > 300 && !has_running_process {
        return SessionStatus::Complete;
    }

    // Idle: inactive for >60s
    if seconds_since_modified > 60 {
        return SessionStatus::Idle;
    }

    match last_line.line_type {
        LineType::Assistant => {
            if !last_line.tool_names.is_empty() {
                SessionStatus::ToolUse
            } else if last_line.stop_reason.as_deref() == Some("end_turn") {
                SessionStatus::WaitingForUser
            } else {
                // Still streaming (no stop reason or a non-end_turn reason)
                SessionStatus::Streaming
            }
        }
        LineType::User => SessionStatus::WaitingForUser,
        LineType::System => SessionStatus::WaitingForUser,
        LineType::Progress => SessionStatus::ToolUse,
        // Summary, Other, etc. -> Idle
        _ => SessionStatus::Idle,
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
        }
    }

    // -------------------------------------------------------------------------
    // derive_status tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_status_no_data() {
        let status = derive_status(None, 0, false);
        assert_eq!(status, SessionStatus::Idle);
    }

    #[test]
    fn test_status_streaming() {
        let last = make_live_line(LineType::Assistant, vec![], None);
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::Streaming);
    }

    #[test]
    fn test_status_streaming_with_non_end_turn_stop() {
        let last = make_live_line(LineType::Assistant, vec![], Some("max_tokens"));
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::Streaming);
    }

    #[test]
    fn test_status_tool_use() {
        let last = make_live_line(
            LineType::Assistant,
            vec!["Read".to_string()],
            Some("tool_use"),
        );
        let status = derive_status(Some(&last), 3, true);
        assert_eq!(status, SessionStatus::ToolUse);
    }

    #[test]
    fn test_status_waiting_for_user_end_turn() {
        let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
        let status = derive_status(Some(&last), 10, true);
        assert_eq!(status, SessionStatus::WaitingForUser);
    }

    #[test]
    fn test_status_waiting_for_user_after_user_message() {
        let last = make_live_line(LineType::User, vec![], None);
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::WaitingForUser);
    }

    #[test]
    fn test_status_waiting_for_user_after_system() {
        let last = make_live_line(LineType::System, vec![], None);
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::WaitingForUser);
    }

    #[test]
    fn test_status_progress_means_tool_use() {
        let last = make_live_line(LineType::Progress, vec![], None);
        let status = derive_status(Some(&last), 5, true);
        assert_eq!(status, SessionStatus::ToolUse);
    }

    #[test]
    fn test_status_idle_after_60s() {
        let last = make_live_line(LineType::Assistant, vec![], None);
        let status = derive_status(Some(&last), 61, true);
        assert_eq!(status, SessionStatus::Idle);
    }

    #[test]
    fn test_status_complete_after_5min_no_process() {
        let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
        let status = derive_status(Some(&last), 301, false);
        assert_eq!(status, SessionStatus::Complete);
    }

    #[test]
    fn test_status_not_complete_with_process() {
        let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
        // >5min but process still running => Idle (not Complete)
        let status = derive_status(Some(&last), 301, true);
        assert_eq!(status, SessionStatus::Idle);
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
