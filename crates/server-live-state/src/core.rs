//! Core session types: LiveSession, SessionStatus, ControlBinding, snapshots.

/// Maximum number of recently-closed sessions kept in the ring buffer.
pub const CLOSED_RING_CAPACITY: usize = 100;

use claude_view_types::{PendingInteractionMeta, SessionOwnership};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::agent::AgentState;
use super::hook_fields::HookFields;
use super::jsonl_fields::JsonlFields;
use super::statusline_fields::StatuslineFields;

/// The current status of a live Claude Code session.
///
/// 3-state model: Working (actively streaming/tool use), Paused (waiting for
/// input, task complete, or idle), Done (session over).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../packages/shared/src/types/generated/")
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

/// Binding from observation (LiveSession) -> control (sidecar SDK session).
/// Present when the user has taken interactive control of this session.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../packages/shared/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ControlBinding {
    /// The sidecar's internal control ID (UUID).
    pub control_id: String,
    /// Unix timestamp when this binding was created.
    #[ts(type = "number")]
    pub bound_at: i64,
    /// Cancellation token to abort the WS relay task on unbind.
    /// Not serialized -- runtime-only.
    #[serde(skip)]
    #[ts(skip)]
    pub cancel: tokio_util::sync::CancellationToken,
}

/// A live session snapshot broadcast to connected SSE clients.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../packages/shared/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    /// Session UUID (filename without .jsonl extension).
    pub id: String,
    /// Current derived session status.
    pub status: SessionStatus,
    /// Unix timestamp when the session started, if known.
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    /// Unix timestamp when this session's process exited (None = still running).
    /// Set by reconciliation loop or SessionEnd hook. Used by frontend for
    /// "closed Xm ago" display and by recently-closed persistence.
    #[ts(type = "number | null")]
    pub closed_at: Option<i64>,
    /// If Some, this session is being controlled via the sidecar Agent SDK.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlBinding>,
    // -- Cross-source fields (written by both statusline and JSONL watcher) --
    /// The primary model used in this session.
    pub model: Option<String>,
    /// Display name from statusline (e.g. "Opus", "Sonnet"). Source of truth for live sessions.
    /// Cross-source field -- NOT moved into any sub-struct.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_display_name: Option<String>,
    /// Monotonic timestamp when `model` was last set. Writers only overwrite
    /// when their timestamp > this value, preventing stale statusline updates
    /// from clobbering newer hook values. NOT serialized.
    #[serde(skip)]
    #[ts(skip)]
    pub model_set_at: i64,
    /// Current context window fill: total input tokens from the last assistant turn.
    #[ts(type = "number")]
    pub context_window_tokens: u64,
    // -- Sub-structs (flattened for zero wire format change) --
    /// All statusline-derived fields (32 fields). Flattened into the JSON
    /// output for zero wire format change.
    #[serde(flatten)]
    #[ts(flatten)]
    pub statusline: StatuslineFields,
    /// All hook-sourced fields (agent_state, pid, title, turn_count, etc.).
    /// Flattened into JSON for zero wire format change.
    #[serde(flatten)]
    #[ts(flatten)]
    pub hook: HookFields,
    /// All JSONL-watcher-sourced fields (22 fields). Flattened into JSON
    /// for zero wire format change.
    #[serde(flatten)]
    #[ts(flatten)]
    pub jsonl: JsonlFields,

    // -- Session file fields (from ~/.claude/sessions/{pid}.json) --
    /// Session kind: "interactive" or "background" (subagent).
    /// Populated from sessions/{pid}.json, NOT from hooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_kind: Option<String>,
    /// Entrypoint: "cli", "claude-vscode", "claude-desktop", etc.
    /// Populated from sessions/{pid}.json, NOT from hooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,

    // -- Ownership + interaction fields --
    /// Session ownership as independent facts (tmux binding, SDK binding).
    /// Stored directly in the session record by `write_ownership()`,
    /// `bind_control`, and `unbind_control`. SSE/REST reads directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<SessionOwnership>,
    /// Lightweight pending interaction metadata for SessionCard display.
    /// Set by SetPendingInteraction mutation, cleared by ClearPendingInteraction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_interaction: Option<PendingInteractionMeta>,
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

/// Minimal LiveSession factory for cross-module/cross-crate tests.
pub fn test_live_session(id: &str) -> LiveSession {
    use super::agent::{AgentState, AgentStateGroup};
    use claude_view_core::phase::PhaseHistory;
    use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};

    LiveSession {
        id: id.to_string(),
        status: SessionStatus::Working,
        started_at: Some(1000),
        closed_at: None,
        control: None,
        model: None,
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook: HookFields {
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            pid: None,
            title: "Test session".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 5,
            last_activity_at: 1000,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
            hook_events: Vec::new(),
        },
        jsonl: JsonlFields {
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            tokens: TokenUsage::default(),
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            last_turn_task_seconds: None,
            last_cache_hit_at: None,
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            tools_used: Vec::new(),
            slug: None,
            user_files: None,
            source: None,
            phase: PhaseHistory::default(),
            ai_title: None,
        },
        session_kind: None,
        entrypoint: None,
        ownership: None,
        pending_interaction: None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::agent::{AgentState, AgentStateGroup};
    use super::super::field_types::{FileSourceKind, VerifiedFile};
    use super::*;
    use claude_view_types::InteractionVariant;

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

    #[test]
    fn verified_file_in_live_session_serializes_correctly() {
        let mut session = test_live_session("test-1");
        session.jsonl.user_files = Some(vec![
            VerifiedFile {
                path: "/Users/dev/src/auth.rs".into(),
                kind: FileSourceKind::Mention,
                display_name: "src/auth.rs".into(),
            },
            VerifiedFile {
                path: "/Users/dev/src/main.rs".into(),
                kind: FileSourceKind::Ide,
                display_name: "src/main.rs".into(),
            },
        ]);
        let json = serde_json::to_value(&session).unwrap();
        let files = json["userFiles"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["kind"], "mention");
        assert_eq!(files[0]["displayName"], "src/auth.rs");
        assert_eq!(files[1]["kind"], "ide");
    }

    #[test]
    fn test_session_remove_event_serializes_with_type_tag() {
        use super::super::event::SessionEvent;

        let mut session = test_live_session("abc-123");
        session.status = SessionStatus::Done;
        session.closed_at = Some(1_700_000_000);

        let event = SessionEvent::SessionRemove {
            session_id: "abc-123".to_string(),
            session,
        };
        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(
            json["type"], "session_remove",
            "serde tag must produce 'session_remove' (snake_case)"
        );
        assert_eq!(
            json["sessionId"], "abc-123",
            "session_id must serialize as camelCase 'sessionId'"
        );
        assert!(
            json["session"].is_object(),
            "must embed the full session object under 'session' key"
        );
        assert_eq!(json["session"]["id"], "abc-123");
        assert_eq!(json["session"]["closedAt"], 1_700_000_000);
    }

    // ================================================================
    // Ownership + PendingInteraction serialization tests (Task 3 TDD)
    // ================================================================

    #[test]
    fn ownership_none_omitted_from_json() {
        let session = test_live_session("s1");
        assert!(session.ownership.is_none());
        let json = serde_json::to_value(&session).unwrap();
        assert!(
            json.get("ownership").is_none(),
            "ownership=None must be omitted via skip_serializing_if"
        );
    }

    #[test]
    fn ownership_some_no_bindings_included_in_json() {
        let mut session = test_live_session("s2");
        session.ownership = Some(SessionOwnership {
            source: Some("terminal".into()),
            ..Default::default()
        });
        let json = serde_json::to_value(&session).unwrap();
        let ownership = json
            .get("ownership")
            .expect("ownership=Some must be present");
        assert!(ownership.get("tier").is_none());
        assert_eq!(ownership["source"], "terminal");
    }

    #[test]
    fn ownership_some_sdk_included_in_json() {
        let mut session = test_live_session("s3");
        session.ownership = Some(SessionOwnership {
            sdk: Some(claude_view_types::SdkBinding {
                control_id: "ctl-42".into(),
            }),
            entrypoint: Some("cli".into()),
            ..Default::default()
        });
        let json = serde_json::to_value(&session).unwrap();
        let ownership = json.get("ownership").expect("ownership must be present");
        assert!(ownership.get("tier").is_none());
        assert_eq!(ownership["sdk"]["controlId"], "ctl-42");
        assert_eq!(ownership["entrypoint"], "cli");
    }

    #[test]
    fn pending_interaction_none_omitted_from_json() {
        let session = test_live_session("s4");
        assert!(session.pending_interaction.is_none());
        let json = serde_json::to_value(&session).unwrap();
        assert!(
            json.get("pendingInteraction").is_none(),
            "pendingInteraction=None must be omitted via skip_serializing_if"
        );
    }

    #[test]
    fn pending_interaction_some_included_in_json() {
        let mut session = test_live_session("s5");
        session.pending_interaction = Some(PendingInteractionMeta {
            variant: InteractionVariant::Permission,
            request_id: "req-001".into(),
            preview: "Allow file write?".into(),
        });
        let json = serde_json::to_value(&session).unwrap();
        let pi = json
            .get("pendingInteraction")
            .expect("pendingInteraction=Some must be present");
        assert_eq!(pi["variant"], "permission");
        assert_eq!(pi["requestId"], "req-001");
        assert_eq!(pi["preview"], "Allow file write?");
    }

    #[test]
    fn test_live_session_factory_returns_none_for_ownership_and_interaction() {
        let session = test_live_session("factory-check");
        assert!(
            session.ownership.is_none(),
            "test_live_session must initialize ownership as None"
        );
        assert!(
            session.pending_interaction.is_none(),
            "test_live_session must initialize pending_interaction as None"
        );
    }
}
