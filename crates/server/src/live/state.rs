//! Live session state types and status derivation for Mission Control.
//!
//! Provides real-time session status tracking by analyzing the last JSONL line,
//! file modification time, and process presence.

use serde::{Deserialize, Serialize};
use vibe_recall_core::cost::{CacheStatus, CostBreakdown, TokenUsage};

/// The universal agent state — driven by hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    /// Which UI group: NeedsYou or Autonomous
    pub group: AgentStateGroup,
    /// Sub-state within group (open string — new states added freely)
    pub state: String,
    /// Human-readable label for the UI
    pub label: String,
    /// Optional context (tool input, error details, etc.)
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sub_agents: Vec<vibe_recall_core::subagent::SubAgentInfo>,
    /// Task/todo progress items tracked from TodoWrite and TaskCreate/TaskUpdate.
    /// Empty vec if no progress items have been detected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub progress_items: Vec<vibe_recall_core::progress::ProgressItem>,
    /// Unix timestamp when the last cache hit or creation occurred.
    /// Set only when a turn has cache_read_tokens > 0 OR cache_creation_tokens > 0.
    /// Null if no cache activity has been detected (e.g., new session or below minimum tokens).
    pub last_cache_hit_at: Option<i64>,
}

/// Events broadcast over the SSE channel to connected Mission Control clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A new session JSONL file was discovered on disk.
    SessionDiscovered { session: LiveSession },
    /// An existing session was updated (new lines appended to JSONL).
    SessionUpdated { session: LiveSession },
    /// A session has been cleaned up (Complete for >10 min).
    SessionCompleted {
        #[serde(rename = "sessionId")]
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

/// Derive SessionStatus from AgentState. No heuristics — purely structural.
pub fn status_from_agent_state(agent_state: &AgentState) -> SessionStatus {
    match agent_state.state.as_str() {
        "session_ended" => SessionStatus::Done,
        _ => match agent_state.group {
            AgentStateGroup::Autonomous => SessionStatus::Working,
            AgentStateGroup::NeedsYou | AgentStateGroup::Delivered => SessionStatus::Paused,
        },
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_from_autonomous_acting() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: "Working".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_status_from_autonomous_thinking() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Thinking".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_status_from_autonomous_delegating() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: "Running agent".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_status_from_needs_you_idle() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Idle".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Paused);
    }

    #[test]
    fn test_status_from_needs_you_awaiting_input() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "awaiting_input".into(),
            label: "Asked a question".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Paused);
    }

    #[test]
    fn test_status_from_needs_you_needs_permission() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "needs_permission".into(),
            label: "Needs permission".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Paused);
    }

    #[test]
    fn test_status_from_session_ended() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Ended".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Done);
    }

    #[test]
    fn test_status_from_session_ended_autonomous_group() {
        // session_ended should always produce Done regardless of group
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "session_ended".into(),
            label: "Ended".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Done);
    }
}
