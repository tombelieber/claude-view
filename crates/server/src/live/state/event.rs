//! Session events and hook event types.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::core::LiveSession;

/// Events broadcast over the SSE channel to connected Live Monitor clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A new session JSONL file was discovered on disk.
    SessionDiscovered { session: LiveSession },
    /// An existing session was updated (new lines appended to JSONL).
    SessionUpdated { session: LiveSession },
    /// A session's process exited -- session moves to "recently closed" on the frontend.
    /// Carries the full session data so the frontend can display it without a REST call.
    SessionClosed { session: LiveSession },
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
        #[serde(rename = "totalCostTodayUsd")]
        total_cost_today_usd: f64,
        #[serde(rename = "totalTokensToday")]
        total_tokens_today: u64,
    },
}

/// A single hook lifecycle event, captured for the event log.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct HookEvent {
    /// Unix timestamp (seconds).
    #[ts(type = "number")]
    pub timestamp: i64,
    /// Hook event name: "PreToolUse", "PostToolUse", "Stop", etc.
    pub event_name: String,
    /// Tool name, if applicable.
    pub tool_name: Option<String>,
    /// Human-readable label (from resolve_state_from_hook).
    pub label: String,
    /// Agent state group: "autonomous" or "needs_you".
    pub group: String,
    /// Optional context JSON (tool_input, error, prompt snippet, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Origin channel: "hook" (Channel B), "hook_progress" (Channel A), "synthesized".
    pub source: String,
}

impl HookEvent {
    /// Convert to the DB row type. Maps `group` -> `group_name`.
    pub fn to_row(&self) -> claude_view_db::HookEventRow {
        claude_view_db::HookEventRow {
            timestamp: self.timestamp,
            event_name: self.event_name.clone(),
            tool_name: self.tool_name.clone(),
            label: self.label.clone(),
            group_name: self.group.clone(),
            context: self.context.clone(),
            source: self.source.clone(),
        }
    }
}

/// Maximum hook events kept in memory per session.
pub(crate) const MAX_HOOK_EVENTS_PER_SESSION: usize = 5000;

/// Append a hook event, draining oldest 100 events if at capacity.
pub(crate) fn append_capped_hook_event(dst: &mut Vec<HookEvent>, event: HookEvent, max: usize) {
    if dst.len() >= max {
        dst.drain(..100.min(dst.len()));
    }
    dst.push(event);
}

/// Append multiple hook events, draining overflow from the front.
#[allow(dead_code)]
pub(crate) fn append_capped_hook_events(
    dst: &mut Vec<HookEvent>,
    mut events: Vec<HookEvent>,
    max: usize,
) {
    if events.is_empty() {
        return;
    }
    dst.append(&mut events);
    if dst.len() > max {
        let overflow = dst.len() - max;
        dst.drain(..overflow);
    }
}
