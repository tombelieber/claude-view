//! Hook-sourced fields extracted from LiveSession.

use serde::Serialize;
use ts_rs::TS;

use super::agent::{AgentState, AgentStateGroup};
use super::event::HookEvent;

/// Hook-sourced fields, grouped for merge clarity.
///
/// `#[serde(flatten)]` on the parent ensures JSON keys are identical to the old
/// flat layout. Contains agent_state, PID, title, turn count, activity tracking,
/// sub-agents, progress items, and hook event log.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct HookFields {
    /// Universal agent state -- replaces pause_classification.
    /// Always present (never null), with group/state/label/confidence.
    pub agent_state: AgentState,
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
    /// Unix timestamp of the most recent file modification.
    #[ts(type = "number")]
    pub last_activity_at: i64,
    /// Unix timestamp when the current user turn started (real prompt detected).
    /// Used by frontend to compute live elapsed time for autonomous sessions.
    #[ts(type = "number | null")]
    pub current_turn_started_at: Option<i64>,
    /// Sub-agents spawned via the Task tool in this session.
    /// Empty vec if no sub-agents have been detected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sub_agents: Vec<claude_view_core::subagent::SubAgentInfo>,
    /// Task/todo progress items tracked from TodoWrite and TaskCreate/TaskUpdate.
    /// Empty vec if no progress items have been detected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub progress_items: Vec<claude_view_core::progress::ProgressItem>,
    /// Number of context compactions in this session (compact_boundary system messages).
    pub compact_count: u32,
    /// Monotonic timestamp when `agent_state` was last set. Same semantics.
    #[serde(skip)]
    #[ts(skip)]
    pub agent_state_set_at: i64,
    /// Hook lifecycle events captured for the event log.
    /// Skipped in SSE serialization (too large); streamed via WS only.
    #[serde(skip_serializing)]
    pub hook_events: Vec<HookEvent>,
    /// Truncated preview of last assistant response (~200 chars). From Stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_assistant_preview: Option<String>,
    /// Last API error type (rate_limit, server_error, etc.). From StopFailure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Last API error details. From StopFailure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_details: Option<String>,
}

impl Default for HookFields {
    fn default() -> Self {
        Self {
            agent_state: AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "unknown".into(),
                label: "Unknown".into(),
                context: None,
            },
            pid: None,
            title: String::new(),
            last_user_message: String::new(),
            current_activity: String::new(),
            turn_count: 0,
            last_activity_at: 0,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            hook_events: Vec::new(),
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
        }
    }
}
