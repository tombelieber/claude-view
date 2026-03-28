//! Typed mutations for the session coordinator pipeline.
//!
//! One enum variant per upstream source (statusline, hooks, JSONL reconcile,
//! sidecar control). Exhaustive match in the coordinator ensures every source
//! is handled — no implicit fallthrough.

use std::path::PathBuf;

use crate::live::state::{AgentState, ControlBinding, HookEvent, LiveSession};
use crate::routes::statusline::StatuslinePayload;

// ---------------------------------------------------------------------------
// SessionMutation — the single entry point for all session state changes
// ---------------------------------------------------------------------------

/// Typed mutation — one per upstream source. Exhaustive match required.
pub enum SessionMutation {
    /// Statusline JSON forwarded from Claude Code wrapper script.
    Statusline(Box<StatuslinePayload>),
    /// Hook-driven lifecycle events (start, prompt, state change, end).
    Lifecycle(LifecycleEvent),
    /// JSONL reconciliation data (project, model, tokens, cost, phase).
    Reconcile(ReconcileData),
    /// Sidecar control binding / unbinding.
    Control(ControlAction),
}

impl SessionMutation {
    /// Only Start (with valid cwd) and Reconcile may create a brand-new session.
    ///
    /// Start events without a cwd are buffered — the session cannot be created
    /// without a project path (matches the old hooks.rs `has_valid_cwd` guard).
    pub fn can_create_session(&self) -> bool {
        match self {
            Self::Lifecycle(LifecycleEvent::Start { cwd, .. }) => {
                cwd.as_ref().is_some_and(|v| !v.trim().is_empty())
            }
            Self::Reconcile(_) => true,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// LifecycleEvent — hook-driven session lifecycle
// ---------------------------------------------------------------------------

/// Hook-driven lifecycle event for a session.
pub enum LifecycleEvent {
    /// Session started (first hook or process spawn).
    Start {
        cwd: Option<String>,
        model: Option<String>,
        source: Option<String>,
        pid: Option<u32>,
        transcript_path: Option<String>,
    },
    /// User submitted a new prompt turn.
    Prompt { text: String, pid: Option<u32> },
    /// Agent state transition (e.g. PreToolUse → PostToolUse).
    StateChange {
        agent_state: AgentState,
        event_name: String,
        pid: Option<u32>,
    },
    /// Session ended (process exited).
    End,
    /// Sub-entity lifecycle (subagent, task, teammate).
    SubEntity(SubEntityEvent),
}

/// Sub-entity lifecycle events within a parent session.
pub enum SubEntityEvent {
    SubagentComplete {
        agent_type: String,
        agent_id: Option<String>,
    },
    TaskComplete {
        task_id: String,
    },
    TeammateIdle,
}

// ---------------------------------------------------------------------------
// ControlAction — sidecar SDK binding
// ---------------------------------------------------------------------------

/// Sidecar control binding actions.
pub enum ControlAction {
    /// Bind a sidecar SDK session to a live session.
    Bind(ControlBinding),
    /// Unbind by control_id.
    Unbind(String),
}

// ---------------------------------------------------------------------------
// ReconcileData — JSONL watcher reconciliation
// ---------------------------------------------------------------------------

/// Data from JSONL reconciliation — partial updates merged into the session.
pub struct ReconcileData {
    pub project: Option<String>,
    pub project_display_name: Option<String>,
    pub project_path: Option<String>,
    pub model: Option<String>,
    pub model_display_name: Option<String>,
    pub tokens: Option<claude_view_core::pricing::TokenUsage>,
    pub context_window_tokens: Option<u64>,
    pub cost: Option<claude_view_core::pricing::CostBreakdown>,
    pub turn_count: Option<u32>,
    pub edit_count: Option<u32>,
    pub phase: Option<claude_view_core::phase::PhaseHistory>,
}

// ---------------------------------------------------------------------------
// SideEffect — deferred IO produced by pure mutation functions
// ---------------------------------------------------------------------------

/// Deferred side effect — produced by mutation logic, executed by the
/// coordinator after the pure mutation phase completes.
pub enum SideEffect {
    PersistHookEvents {
        session_id: String,
        events: Vec<HookEvent>,
    },
    RemoveAccumulator {
        session_id: String,
    },
    CleanTranscriptDedup {
        path: PathBuf,
    },
    SavePidBinding {
        session_id: String,
        pid: u32,
    },
    EvictSession {
        session_id: String,
        reason: String,
    },
    CreateAccumulator {
        session_id: String,
    },
    CleanHookEventChannel {
        session_id: String,
    },
    PersistClosedAt {
        session_id: String,
        closed_at: i64,
    },
}

// ---------------------------------------------------------------------------
// BroadcastAction / MutationResult — output of the mutation pipeline
// ---------------------------------------------------------------------------

/// What SSE broadcast (if any) should follow a mutation.
pub enum BroadcastAction {
    Created,
    Updated,
    Closed,
    Removed,
    None,
}

/// The outcome of applying a mutation to a session.
pub enum MutationResult {
    /// New session was created.
    Created(LiveSession),
    /// Existing session was updated.
    Updated(LiveSession),
    /// Session was closed (process exited).
    Closed(LiveSession),
    /// Session was removed from the store.
    Removed(String),
    /// Mutation was buffered (no session yet).
    Buffered,
    /// Target session not found and mutation cannot create one.
    SessionNotFound,
}

// ---------------------------------------------------------------------------
// CommonPostMutation — shared post-mutation bookkeeping
// ---------------------------------------------------------------------------

/// Shared bookkeeping fields extracted by mutation functions.
pub struct CommonPostMutation {
    /// If set, bind this PID to the session after mutation.
    pub bind_pid: Option<u32>,
    /// If set, update the session's last-activity timestamp.
    pub update_activity_at: Option<i64>,
}
