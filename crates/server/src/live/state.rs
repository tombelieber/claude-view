//! Live session state types and status derivation for Live Monitor.
//!
//! Provides real-time session status tracking by analyzing the last JSONL line,
//! file modification time, and process presence.

use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The universal agent state — driven by hooks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum AgentStateGroup {
    NeedsYou,
    Autonomous,
}

/// The current status of a live Claude Code session.
///
/// 3-state model: Working (actively streaming/tool use), Paused (waiting for
/// input, task complete, or idle), Done (session over).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Agent is actively streaming or using tools.
    Working,
    /// Agent paused -- reason available in pause_classification.
    Paused,
    /// Session is over (process exited + no new writes for 300s).
    Done,
}

/// A tool integration (MCP server or skill) detected from actual usage in a session.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsed {
    /// Display name: "playwright", "chrome-devtools" for MCP; "commit", "review-pr" for skills.
    pub name: String,
    /// Category: "mcp" or "skill".
    pub kind: String,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// A live session snapshot broadcast to connected SSE clients.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
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
    /// Resolved branch from worktree HEAD (differs from git_branch when in a worktree).
    pub worktree_branch: Option<String>,
    /// Whether this session is running inside a git worktree.
    pub is_worktree: bool,
    /// Computed: worktree_branch ?? git_branch. Always use this for display.
    pub effective_branch: Option<String>,
    /// PID of the running Claude process, if any.
    pub pid: Option<u32>,
    /// Session title derived from the first non-meta user message.
    pub title: String,
    /// The last user message text (truncated for display).
    pub last_user_message: String,
    /// Filename from `<ide_opened_file>` tag in the last user message, if present.
    pub last_user_file: Option<String>,
    /// Human-readable description of the current activity.
    pub current_activity: String,
    /// Number of user/assistant turn pairs.
    pub turn_count: u32,
    /// Unix timestamp when the session started, if known.
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    /// Unix timestamp of the most recent file modification.
    #[ts(type = "number")]
    pub last_activity_at: i64,
    /// The primary model used in this session.
    pub model: Option<String>,
    /// Accumulated token usage for this session (cumulative, for cost).
    pub tokens: TokenUsage,
    /// Current context window fill: total input tokens from the last assistant turn.
    #[ts(type = "number")]
    pub context_window_tokens: u64,
    /// Computed cost breakdown in USD.
    pub cost: CostBreakdown,
    /// Whether the Anthropic prompt cache is likely warm or cold.
    pub cache_status: CacheStatus,
    /// Unix timestamp when the current user turn started (real prompt detected).
    /// Used by frontend to compute live elapsed time for autonomous sessions.
    #[ts(type = "number | null")]
    pub current_turn_started_at: Option<i64>,
    /// Seconds the agent spent on the last completed turn (frozen on Working->Paused).
    /// Used by frontend to show task time for needs_you sessions.
    pub last_turn_task_seconds: Option<u32>,
    /// Sub-agents spawned via the Task tool in this session.
    /// Empty vec if no sub-agents have been detected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sub_agents: Vec<claude_view_core::subagent::SubAgentInfo>,
    /// Team name if this session is a team lead.
    /// Populated from the top-level `teamName` field in the JSONL (present after TeamCreate).
    /// Frontend uses this to show team badge instead of sub-agent pills.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    /// Team members read from ~/.claude/teams/{name}/config.json.
    /// Populated after each JSONL metadata application when team_name is Some.
    /// Empty vec when not a team lead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub team_members: Vec<crate::teams::TeamMember>,
    /// Number of inbox messages for this team (0 when not a team lead).
    /// Used by frontend as a version signal to invalidate inbox queries.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub team_inbox_count: u32,
    /// Number of file-modifying tool uses (Edit + Write) in this session.
    /// Used by frontend as a version signal to invalidate file-history and plan queries.
    #[serde(default)]
    pub edit_count: u32,
    /// Task/todo progress items tracked from TodoWrite and TaskCreate/TaskUpdate.
    /// Empty vec if no progress items have been detected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub progress_items: Vec<claude_view_core::progress::ProgressItem>,
    /// Unique tool integrations detected in this session (MCP servers, skills).
    /// Discovered from actual tool_use invocations -- 100% accuracy.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools_used: Vec<ToolUsed>,
    /// Unix timestamp when the last cache hit or creation occurred.
    /// Set only when a turn has cache_read_tokens > 0 OR cache_creation_tokens > 0.
    /// Null if no cache activity has been detected (e.g., new session or below minimum tokens).
    #[ts(type = "number | null")]
    pub last_cache_hit_at: Option<i64>,
    /// Number of context compactions in this session (compact_boundary system messages).
    pub compact_count: u32,
    /// Session slug for plan file association.
    pub slug: Option<String>,
    /// Files referenced with `@filename` syntax in user messages.
    /// Deduplicated set across session lifetime (≤10, first-N-wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_files: Option<Vec<String>>,
    /// Unix timestamp when this session's process exited (None = still running).
    /// Set by reconciliation loop or SessionEnd hook. Used by frontend for
    /// "closed Xm ago" display and by recently-closed persistence.
    #[ts(type = "number | null")]
    pub closed_at: Option<i64>,
    /// If Some, this session is being controlled via the sidecar Agent SDK.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlBinding>,
    /// Hook lifecycle events captured for the event log.
    /// Skipped in SSE serialization (too large); streamed via WS only.
    #[serde(skip_serializing)]
    pub hook_events: Vec<HookEvent>,
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

/// Binding from observation (LiveSession) → control (sidecar SDK session).
/// Present when the user has taken interactive control of this session.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ControlBinding {
    /// The sidecar's internal control ID (UUID).
    pub control_id: String,
    /// Unix timestamp when this binding was created.
    #[ts(type = "number")]
    pub bound_at: i64,
    /// Cancellation token to abort the WS relay task on unbind.
    /// Not serialized — runtime-only.
    #[serde(skip)]
    #[ts(skip)]
    pub cancel: tokio_util::sync::CancellationToken,
}

/// A per-session snapshot entry persisted to disk for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEntry {
    /// Bound PID of the Claude process.
    pub pid: u32,
    /// Session status as string: "working", "paused", "done".
    pub status: String,
    /// Last known agent state (from hooks).
    pub agent_state: AgentState,
    /// Unix timestamp of last activity.
    pub last_activity_at: i64,
    /// Persisted control_id so controlled sessions survive Rust server restart.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub control_id: Option<String>,
}

/// The on-disk snapshot format (v2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub version: u8,
    pub sessions: std::collections::HashMap<String, SnapshotEntry>,
}

/// Events broadcast over the SSE channel to connected Live Monitor clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A new session JSONL file was discovered on disk.
    SessionDiscovered { session: LiveSession },
    /// An existing session was updated (new lines appended to JSONL).
    SessionUpdated { session: LiveSession },
    /// A session's process exited — session moves to "recently closed" on the frontend.
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

/// Derive SessionStatus from AgentState. No heuristics — purely structural.
pub fn status_from_agent_state(agent_state: &AgentState) -> SessionStatus {
    match agent_state.state.as_str() {
        "session_ended" => SessionStatus::Done,
        _ => match agent_state.group {
            AgentStateGroup::Autonomous => SessionStatus::Working,
            AgentStateGroup::NeedsYou => SessionStatus::Paused,
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

    #[test]
    fn test_status_from_compacting() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "compacting".into(),
            label: "Auto-compacting context...".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_control_binding_serializes_to_camel_case() {
        let binding = ControlBinding {
            control_id: "abc-123".to_string(),
            bound_at: 1700000000,
            cancel: tokio_util::sync::CancellationToken::new(),
        };
        let json = serde_json::to_value(&binding).unwrap();
        assert_eq!(json["controlId"], "abc-123");
        assert_eq!(json["boundAt"], 1700000000);
    }

    #[test]
    fn test_snapshot_entry_with_control_id() {
        let entry = SnapshotEntry {
            pid: 12345,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1700000000,
            control_id: Some("ctrl-456".to_string()),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["controlId"], "ctrl-456");
    }

    #[test]
    fn test_snapshot_entry_without_control_id_omits_field() {
        let entry = SnapshotEntry {
            pid: 12345,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1700000000,
            control_id: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json.get("controlId").is_none());
    }

    #[test]
    fn test_snapshot_entry_backward_compat_no_control_id() {
        let json = r#"{"pid":12345,"status":"working","agentState":{"group":"autonomous","state":"acting","label":"Working"},"lastActivityAt":1700000000}"#;
        let entry: SnapshotEntry = serde_json::from_str(json).unwrap();
        assert!(entry.control_id.is_none());
        assert_eq!(entry.pid, 12345);
    }

    /// Minimal LiveSession for tests.
    fn minimal_live_session(id: &str) -> LiveSession {
        LiveSession {
            id: id.to_string(),
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            status: SessionStatus::Working,
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            pid: None,
            title: "Test session".into(),
            last_user_message: String::new(),
            last_user_file: None,
            current_activity: "Working".into(),
            turn_count: 5,
            started_at: Some(1000),
            last_activity_at: 1000,
            model: None,
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            progress_items: Vec::new(),
            tools_used: Vec::new(),
            last_cache_hit_at: None,
            compact_count: 0,
            slug: None,
            user_files: None,
            closed_at: None,
            control: None,
            hook_events: Vec::new(),
        }
    }

    #[test]
    fn test_session_closed_event_serializes_with_type_tag() {
        let mut session = minimal_live_session("abc-123");
        session.status = SessionStatus::Done;
        session.closed_at = Some(1_700_000_000);

        let event = SessionEvent::SessionClosed { session };
        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(
            json["type"], "session_closed",
            "serde tag must produce 'session_closed' (snake_case)"
        );
        assert!(
            json["session"].is_object(),
            "must embed the full session object under 'session' key"
        );
        assert_eq!(json["session"]["id"], "abc-123");
        assert_eq!(json["session"]["closedAt"], 1_700_000_000);
    }
}
