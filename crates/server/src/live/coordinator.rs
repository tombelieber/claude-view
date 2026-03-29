//! SessionCoordinator — single entry point for all session state mutations.
//!
//! 4-phase pipeline:
//!   Phase 1: Buffer — park mutations for undiscovered sessions.
//!   Phase 2+3: Plan + Execute — plan side effects, apply mutation under write lock.
//!   Phase 3b: Execute side effects — IO after lock is dropped (RAII enforced).
//!   Phase 4: Broadcast — single `live_tx.send()` per mutation.
//!
//! Lock ordering (always respected):
//!   1. self.pending (tokio::sync::Mutex)
//!   2. ctx.sessions (tokio::sync::RwLock — write)
//!   3. ctx.transcript_to_session (tokio::sync::RwLock — write)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, RwLock};
use tracing::debug;

use crate::live::buffer::PendingMutations;
use crate::live::manager::{LiveSessionManager, LiveSessionMap, TranscriptMap};
use crate::live::mutation::apply_control::apply_control;
use crate::live::mutation::apply_lifecycle::apply_lifecycle;
use crate::live::mutation::apply_reconcile::apply_reconcile;
use crate::live::mutation::apply_statusline::apply_statusline;
use crate::live::mutation::types::{
    BroadcastAction, CommonPostMutation, LifecycleEvent, MutationResult, ReconcileData,
    SessionMutation, SideEffect,
};
use crate::live::state::{
    append_capped_hook_event, AgentState, AgentStateGroup, HookEvent, HookFields, JsonlFields,
    LiveSession, SessionEvent, SessionStatus, StatuslineFields, MAX_HOOK_EVENTS_PER_SESSION,
};

// ---------------------------------------------------------------------------
// MutationContext — borrows from AppState for the duration of one handle() call
// ---------------------------------------------------------------------------

/// Borrowed references to shared state needed by the mutation pipeline.
/// Created per-call from `AppState` fields — no ownership transfer.
pub struct MutationContext<'a> {
    pub sessions: &'a LiveSessionMap,
    pub live_tx: &'a broadcast::Sender<SessionEvent>,
    pub live_manager: Option<&'a Arc<LiveSessionManager>>,
    pub db: &'a claude_view_db::Database,
    pub transcript_to_session: &'a TranscriptMap,
    pub hook_event_channels: &'a Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>>,
}

// ---------------------------------------------------------------------------
// SessionCoordinator
// ---------------------------------------------------------------------------

/// Single entry point for all session state mutations.
///
/// Replaces 25+ scattered `live_tx.send()` calls with one deterministic
/// pipeline that enforces lock ordering and side-effect separation.
/// A buffered mutation with its associated hook event.
type BufferedMutation = (SessionMutation, Option<HookEvent>);

pub struct SessionCoordinator {
    pending: tokio::sync::Mutex<PendingMutations<BufferedMutation>>,
}

/// Default buffer TTL for pending mutations (2 minutes).
const PENDING_TTL: Duration = Duration::from_secs(120);

/// Maximum hook events kept per session (re-exported for clarity).
const MAX_EVENTS: usize = MAX_HOOK_EVENTS_PER_SESSION;

impl Default for SessionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionCoordinator {
    pub fn new() -> Self {
        Self {
            pending: tokio::sync::Mutex::new(PendingMutations::new(PENDING_TTL)),
        }
    }

    // -----------------------------------------------------------------------
    // Phase 1 → 4: handle()
    // -----------------------------------------------------------------------

    /// Apply a single mutation to a session, returning the outcome.
    ///
    /// This is the ONLY public entry point for session state changes.
    /// All side effects (DB writes, accumulator cleanup) are executed
    /// after the write lock is dropped.
    /// `cwd`: optional working directory from the hook payload. When present
    /// and the session doesn't exist, the coordinator upserts (creates a shell
    /// session) instead of buffering. This handles server restarts where the
    /// original `SessionStart` was consumed by the previous process.
    #[allow(clippy::too_many_arguments)]
    pub async fn handle(
        &self,
        ctx: &MutationContext<'_>,
        session_id: &str,
        mutation: SessionMutation,
        pid: Option<u32>,
        now: i64,
        hook_event: Option<HookEvent>,
        cwd: Option<&str>,
    ) -> MutationResult {
        // ── Phase 1: Buffer or upsert ───────────────────────────────────
        let session_exists = {
            let sessions = ctx.sessions.read().await;
            sessions.contains_key(session_id)
        };

        if !session_exists {
            let has_valid_cwd = cwd.is_some_and(|c| !c.trim().is_empty());
            if mutation.can_create_session() || has_valid_cwd {
                // Create session, then drain buffered mutations below
            } else {
                // Buffer mutation + hook event together, return early
                let mut pending = self.pending.lock().await;
                pending.push(session_id, (mutation, hook_event));
                return MutationResult::Buffered;
            }
        }

        // ── Phase 2+3: Plan + Execute under write lock ───────────────────
        let (snapshot, broadcast_action, side_effects) = {
            let mut sessions = ctx.sessions.write().await;

            // Create session if needed (Start, Reconcile, or upsert via cwd)
            if !sessions.contains_key(session_id) {
                let new_session = match &mutation {
                    SessionMutation::Lifecycle(LifecycleEvent::Start {
                        cwd: start_cwd,
                        model,
                        pid: start_pid,
                        ..
                    }) => create_session_from_start(session_id, start_cwd, model, start_pid, now),
                    SessionMutation::Reconcile(data) => create_session_shell(session_id, data, now),
                    // Upsert: session is actively sending hooks → Autonomous
                    _ => {
                        let mut s = create_session_from_start(
                            session_id,
                            &cwd.map(|c| c.to_string()),
                            &None,
                            &pid,
                            now,
                        );
                        // Override default NeedsYou/unknown — session is clearly active
                        s.hook.agent_state = AgentState {
                            group: AgentStateGroup::Autonomous,
                            state: "acting".into(),
                            label: "Working".into(),
                            context: None,
                        };
                        s.status = SessionStatus::Working;
                        s
                    }
                };
                sessions.insert(session_id.to_string(), new_session);

                // Drain buffered mutations — apply them inline before the
                // primary mutation (they arrived earlier chronologically).
                let buffered = {
                    let mut pending = self.pending.lock().await;
                    pending.drain(session_id)
                };
                for (buffered_mutation, buffered_hook_event) in buffered {
                    if let Some(session) = sessions.get_mut(session_id) {
                        apply_mutation_to_session(session, &buffered_mutation, now);
                        // Replay buffered hook event with the session's actual group
                        if let Some(mut event) = buffered_hook_event {
                            let actual_group = match session.hook.agent_state.group {
                                crate::live::state::AgentStateGroup::NeedsYou => "needs_you",
                                crate::live::state::AgentStateGroup::Autonomous => "autonomous",
                            };
                            event.group = actual_group.to_string();
                            append_capped_hook_event(
                                &mut session.hook.hook_events,
                                event,
                                MAX_EVENTS,
                            );
                        }
                    }
                }
            }

            let session = match sessions.get_mut(session_id) {
                Some(s) => s,
                None => return MutationResult::SessionNotFound,
            };

            // Plan side effects BEFORE mutation (capture data mutation will clear)
            let side_effects = plan_side_effects(session_id, session, &mutation, now);

            // Dispatch to the appropriate apply function
            let status_change = apply_mutation_to_session(session, &mutation, now);

            // Apply status change if returned
            if let Some(new_status) = status_change {
                session.status = new_status;
            }
            // Set closed_at when transitioning to Done
            if session.status == SessionStatus::Done && session.closed_at.is_none() {
                session.closed_at = Some(now);
            }

            // Common post-mutation bookkeeping
            let post = common_post_mutation(&mutation, pid, now);
            if let Some(bind_pid) = post.bind_pid {
                if session.hook.pid.is_none() {
                    session.hook.pid = Some(bind_pid);
                }
            }
            if let Some(activity_at) = post.update_activity_at {
                session.hook.last_activity_at = activity_at;
            }

            // Append hook event inside lock.
            // Rewrite the group field to match the session's actual agent_state
            // group AFTER mutation. For sub-entity events (SubagentStop, etc.)
            // the mutation doesn't change agent_state, so the session's existing
            // group is preserved. For state-changing events, the mutation updates
            // agent_state first, so we record the new group.
            if let Some(mut event) = hook_event {
                let actual_group = match session.hook.agent_state.group {
                    crate::live::state::AgentStateGroup::NeedsYou => "needs_you",
                    crate::live::state::AgentStateGroup::Autonomous => "autonomous",
                };
                event.group = actual_group.to_string();
                append_capped_hook_event(&mut session.hook.hook_events, event, MAX_EVENTS);
                // Forward to WS channel if listeners exist
                forward_hook_event_to_ws(
                    ctx.hook_event_channels,
                    session_id,
                    session.hook.hook_events.last().cloned(),
                )
                .await;
            }

            // Determine broadcast action
            let broadcast_action = if !session_exists && session.status != SessionStatus::Done {
                BroadcastAction::Created
            } else if session.status == SessionStatus::Done {
                BroadcastAction::Closed
            } else {
                BroadcastAction::Updated
            };

            // Clone snapshot, drop write lock (RAII)
            let snapshot = session.clone();
            (snapshot, broadcast_action, side_effects)
        };

        // ── Phase 3b: Side effects (no lock held) ───────────────────────
        for effect in &side_effects {
            execute_side_effect(ctx, effect).await;
        }

        // ── Phase 4: Broadcast ──────────────────────────────────────────
        match broadcast_action {
            BroadcastAction::Created => {
                let _ = ctx.live_tx.send(SessionEvent::SessionDiscovered {
                    session: snapshot.clone(),
                });
                MutationResult::Created(snapshot)
            }
            BroadcastAction::Updated => {
                let _ = ctx.live_tx.send(SessionEvent::SessionUpdated {
                    session: snapshot.clone(),
                });
                MutationResult::Updated(snapshot)
            }
            BroadcastAction::Closed => {
                let _ = ctx.live_tx.send(SessionEvent::SessionClosed {
                    session: snapshot.clone(),
                });
                MutationResult::Closed(snapshot)
            }
            BroadcastAction::Removed => {
                let _ = ctx.live_tx.send(SessionEvent::SessionCompleted {
                    session_id: session_id.to_string(),
                });
                MutationResult::Removed(session_id.to_string())
            }
            BroadcastAction::None => MutationResult::Updated(snapshot),
        }
    }

    /// Sweep expired entries from the pending buffer.
    /// Called periodically by the manager's cleanup task.
    pub async fn sweep_expired(&self) {
        let mut pending = self.pending.lock().await;
        pending.sweep_expired();
    }
}

// ---------------------------------------------------------------------------
// Session creation helpers
// ---------------------------------------------------------------------------

/// Create a new `LiveSession` from a `Start` lifecycle event.
fn create_session_from_start(
    session_id: &str,
    cwd: &Option<String>,
    model: &Option<String>,
    pid: &Option<u32>,
    now: i64,
) -> LiveSession {
    let hook = HookFields {
        last_activity_at: now,
        pid: *pid,
        ..HookFields::default()
    };

    LiveSession {
        id: session_id.to_string(),
        status: SessionStatus::Working,
        started_at: Some(now),
        closed_at: None,
        control: None,
        model: model.clone(),
        model_display_name: None,
        model_set_at: now,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook,
        jsonl: JsonlFields {
            project_path: cwd.clone().unwrap_or_default(),
            ..JsonlFields::default()
        },
    }
}

/// Create a minimal `LiveSession` shell for watcher discovery (Reconcile).
/// JSONL data fills in project/tokens/cost; hooks will follow.
fn create_session_shell(session_id: &str, data: &ReconcileData, now: i64) -> LiveSession {
    let hook = HookFields {
        last_activity_at: now,
        ..HookFields::default()
    };

    let mut jsonl = JsonlFields::default();
    if let Some(ref p) = data.project {
        jsonl.project = p.clone();
    }
    if let Some(ref p) = data.project_display_name {
        jsonl.project_display_name = p.clone();
    }
    if let Some(ref p) = data.project_path {
        jsonl.project_path = p.clone();
    }

    LiveSession {
        id: session_id.to_string(),
        status: SessionStatus::Working,
        started_at: Some(now),
        closed_at: None,
        control: None,
        model: data.model.clone(),
        model_display_name: data.model_display_name.clone(),
        model_set_at: now,
        context_window_tokens: data.context_window_tokens.unwrap_or(0),
        statusline: StatuslineFields::default(),
        hook,
        jsonl,
    }
}

// ---------------------------------------------------------------------------
// Mutation dispatch (pure — no IO)
// ---------------------------------------------------------------------------

/// Dispatch a mutation to the appropriate apply function, returning any
/// status change. Handles cross-source fields (model) inline.
fn apply_mutation_to_session(
    session: &mut LiveSession,
    mutation: &SessionMutation,
    now: i64,
) -> Option<SessionStatus> {
    match mutation {
        SessionMutation::Statusline(ref payload) => {
            apply_statusline(&mut session.statusline, payload);
            // Cross-source: context_window_tokens (derived from current_usage)
            if let Some(ref cw) = payload.context_window {
                if let Some(ref usage) = cw.current_usage {
                    let fill = usage.input_tokens.unwrap_or(0)
                        + usage.cache_creation_input_tokens.unwrap_or(0)
                        + usage.cache_read_input_tokens.unwrap_or(0);
                    if fill > 0 {
                        session.context_window_tokens = fill;
                    }
                }
            }
            // Cross-source: model — timestamp-guarded, empty-string rejected
            if let Some(ref m) = payload.model {
                if now >= session.model_set_at {
                    if let Some(ref id) = m.id {
                        if !id.is_empty() {
                            session.model = Some(id.clone());
                            session.model_set_at = now;
                        }
                    }
                    if let Some(ref dn) = m.display_name {
                        if !dn.is_empty() {
                            session.model_display_name = Some(dn.clone());
                        }
                    }
                }
            }
            None
        }
        SessionMutation::Lifecycle(event) => apply_lifecycle(&mut session.hook, event, now),
        SessionMutation::Reconcile(data) => {
            apply_reconcile(&mut session.jsonl, data);
            // Cross-source: model (only if newer than current)
            if let Some(ref m) = data.model {
                if now >= session.model_set_at {
                    session.model = Some(m.clone());
                    session.model_set_at = now;
                }
            }
            if let Some(ref md) = data.model_display_name {
                session.model_display_name = Some(md.clone());
            }
            // Cross-source: context_window_tokens
            if let Some(cwt) = data.context_window_tokens {
                session.context_window_tokens = cwt;
            }
            // Cross-source: turn_count (JSONL may have it)
            if let Some(tc) = data.turn_count {
                if tc > session.hook.turn_count {
                    session.hook.turn_count = tc;
                }
            }
            None
        }
        SessionMutation::Control(action) => {
            apply_control(&mut session.control, action);
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Side-effect planning (pure — captures data before mutation clears it)
// ---------------------------------------------------------------------------

/// Plan side effects that need to happen after the mutation.
/// Called BEFORE the mutation so we can capture data that may be cleared.
fn plan_side_effects(
    session_id: &str,
    session: &LiveSession,
    mutation: &SessionMutation,
    now: i64,
) -> Vec<SideEffect> {
    let mut effects = Vec::new();

    match mutation {
        SessionMutation::Lifecycle(LifecycleEvent::End { .. }) => {
            // Capture hook events before End clears them
            if !session.hook.hook_events.is_empty() {
                effects.push(SideEffect::PersistHookEvents {
                    session_id: session_id.to_string(),
                    events: session.hook.hook_events.clone(),
                });
            }
            effects.push(SideEffect::RemoveAccumulator {
                session_id: session_id.to_string(),
            });
            effects.push(SideEffect::CleanHookEventChannel {
                session_id: session_id.to_string(),
            });
            effects.push(SideEffect::PersistClosedAt {
                session_id: session_id.to_string(),
                closed_at: now,
            });
        }
        SessionMutation::Lifecycle(LifecycleEvent::Start { .. }) => {
            effects.push(SideEffect::CreateAccumulator {
                session_id: session_id.to_string(),
            });
        }
        _ => {}
    }

    effects
}

// ---------------------------------------------------------------------------
// Side-effect execution (async IO — no lock held)
// ---------------------------------------------------------------------------

/// Execute a single side effect. Called after the write lock is dropped.
async fn execute_side_effect(ctx: &MutationContext<'_>, effect: &SideEffect) {
    match effect {
        SideEffect::RemoveAccumulator { session_id } => {
            if let Some(mgr) = ctx.live_manager {
                mgr.remove_accumulator(session_id).await;
            }
        }
        SideEffect::CreateAccumulator { session_id } => {
            if let Some(mgr) = ctx.live_manager {
                mgr.create_accumulator_for_hook(session_id).await;
            }
        }
        SideEffect::CleanHookEventChannel { session_id } => {
            let mut channels = ctx.hook_event_channels.write().await;
            channels.remove(session_id.as_str());
        }
        SideEffect::PersistClosedAt {
            session_id,
            closed_at,
        } => {
            debug!(
                session_id,
                closed_at, "closed_at set in-memory; snapshot writer persists to disk"
            );
            // closed_at is set on the LiveSession in-memory and persisted
            // by the snapshot writer. No direct DB call needed here.
        }
        SideEffect::CleanTranscriptDedup { path } => {
            let mut map = ctx.transcript_to_session.write().await;
            map.remove(path);
        }
        SideEffect::PersistHookEvents { session_id, events } => {
            debug!(
                session_id,
                count = events.len(),
                "Would persist hook events (not yet wired to DB)"
            );
            // Future: batch-insert hook events to DB for historical queries.
        }
        SideEffect::SavePidBinding { session_id, pid } => {
            debug!(
                session_id,
                pid, "Would save PID binding (handled by snapshot writer)"
            );
        }
        SideEffect::EvictSession { session_id, reason } => {
            debug!(session_id, reason, "Would evict session");
            // Future: remove from session map + broadcast Removed
        }
    }
}

// ---------------------------------------------------------------------------
// Common post-mutation bookkeeping
// ---------------------------------------------------------------------------

/// Extract common post-mutation fields from the mutation.
fn common_post_mutation(
    mutation: &SessionMutation,
    caller_pid: Option<u32>,
    now: i64,
) -> CommonPostMutation {
    // Extract PID from lifecycle events, falling back to caller-provided PID.
    // `mutation` is borrowed, so `pid` fields are `&Option<u32>`.
    let bind_pid = match mutation {
        SessionMutation::Lifecycle(LifecycleEvent::Start { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Prompt { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::StateChange { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Stop { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::StopFailure { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Compacted { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::CwdChanged { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Observability { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::SubagentStarted { pid, .. }) => {
            (*pid).or(caller_pid)
        }
        _ => caller_pid,
    };

    CommonPostMutation {
        bind_pid,
        update_activity_at: Some(now),
    }
}

// ---------------------------------------------------------------------------
// Hook event WS forwarding
// ---------------------------------------------------------------------------

/// Forward a hook event to the per-session WS broadcast channel, if any
/// listeners are subscribed. Non-blocking — drops event if no listeners.
async fn forward_hook_event_to_ws(
    channels: &Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>>,
    session_id: &str,
    event: Option<HookEvent>,
) {
    if let Some(event) = event {
        let channels = channels.read().await;
        if let Some(tx) = channels.get(session_id) {
            let _ = tx.send(event);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::mutation::types::{LifecycleEvent, ReconcileData, SessionMutation};
    use crate::live::state::HookEvent;

    #[test]
    fn create_session_sets_basic_fields() {
        let session = create_session_from_start(
            "test-session-123",
            &Some("/home/user/project".to_string()),
            &Some("claude-sonnet-4-20250514".to_string()),
            &Some(12345),
            1700000000,
        );

        assert_eq!(session.id, "test-session-123");
        assert_eq!(session.status, SessionStatus::Working);
        assert_eq!(session.started_at, Some(1700000000));
        assert!(session.closed_at.is_none());
        assert_eq!(session.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(session.hook.pid, Some(12345));
        assert_eq!(session.hook.last_activity_at, 1700000000);
        assert_eq!(session.jsonl.project_path, "/home/user/project");
    }

    #[test]
    fn create_session_shell_from_reconcile() {
        let data = ReconcileData {
            project: Some("my-project".into()),
            project_display_name: Some("My Project".into()),
            project_path: Some("/home/user/my-project".into()),
            model: Some("claude-sonnet-4-20250514".into()),
            model_display_name: Some("Sonnet".into()),
            tokens: None,
            context_window_tokens: Some(200_000),
            cost: None,
            turn_count: None,
            edit_count: None,
            phase: None,
        };

        let session = create_session_shell("reconcile-123", &data, 1700000000);

        assert_eq!(session.id, "reconcile-123");
        assert_eq!(session.jsonl.project, "my-project");
        assert_eq!(session.jsonl.project_display_name, "My Project");
        assert_eq!(session.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(session.context_window_tokens, 200_000);
    }

    #[test]
    fn plan_side_effects_for_end_captures_events() {
        let mut session = create_session_from_start("end-test", &None, &None, &None, 1700000000);

        // Add some hook events
        session.hook.hook_events.push(HookEvent {
            timestamp: 1700000001,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "Reading file".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook".into(),
        });
        session.hook.hook_events.push(HookEvent {
            timestamp: 1700000002,
            event_name: "PostToolUse".into(),
            tool_name: Some("Read".into()),
            label: "Read complete".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook".into(),
        });

        let mutation = SessionMutation::Lifecycle(LifecycleEvent::End { reason: None });
        let effects = plan_side_effects("end-test", &session, &mutation, 1700000003);

        // Should have: PersistHookEvents, RemoveAccumulator,
        // CleanHookEventChannel, PersistClosedAt
        assert_eq!(effects.len(), 4);

        // Verify PersistHookEvents captured the events
        let persist = effects
            .iter()
            .find(|e| matches!(e, SideEffect::PersistHookEvents { .. }));
        assert!(persist.is_some(), "Expected PersistHookEvents side effect");
        if let Some(SideEffect::PersistHookEvents { events, .. }) = persist {
            assert_eq!(events.len(), 2);
        }

        // Verify RemoveAccumulator is planned
        let remove = effects
            .iter()
            .any(|e| matches!(e, SideEffect::RemoveAccumulator { .. }));
        assert!(remove, "Expected RemoveAccumulator side effect");

        // Verify CleanHookEventChannel is planned
        let clean = effects
            .iter()
            .any(|e| matches!(e, SideEffect::CleanHookEventChannel { .. }));
        assert!(clean, "Expected CleanHookEventChannel side effect");
    }

    #[test]
    fn plan_side_effects_for_start_creates_accumulator() {
        let session =
            create_session_from_start("start-test", &Some("/tmp".into()), &None, &None, 1700000000);

        let mutation = SessionMutation::Lifecycle(LifecycleEvent::Start {
            cwd: Some("/tmp".into()),
            model: None,
            source: None,
            pid: None,
            transcript_path: None,
        });
        let effects = plan_side_effects("start-test", &session, &mutation, 1700000000);

        let has_create = effects
            .iter()
            .any(|e| matches!(e, SideEffect::CreateAccumulator { .. }));
        assert!(has_create, "Expected CreateAccumulator side effect");
    }

    #[test]
    fn apply_statusline_mutation_updates_model() {
        let mut session = create_session_from_start("model-test", &None, &None, &None, 1700000000);

        assert!(session.model.is_none());

        let payload = crate::routes::statusline::StatuslinePayload {
            session_id: "model-test".into(),
            model: Some(crate::routes::statusline::StatuslineModel {
                id: Some("claude-sonnet-4-20250514".into()),
                display_name: Some("Sonnet".into()),
            }),
            cwd: None,
            workspace: None,
            cost: None,
            context_window: None,
            exceeds_200k_tokens: None,
            transcript_path: None,
            version: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: Default::default(),
        };

        let mutation = SessionMutation::Statusline(Box::new(payload));
        apply_mutation_to_session(&mut session, &mutation, 1700000001);

        assert_eq!(session.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(session.model_display_name.as_deref(), Some("Sonnet"));
    }

    #[test]
    fn common_post_mutation_extracts_pid() {
        let mutation = SessionMutation::Lifecycle(LifecycleEvent::Prompt {
            text: "hello".into(),
            pid: Some(999),
        });
        let post = common_post_mutation(&mutation, None, 1700000000);
        assert_eq!(post.bind_pid, Some(999));
        assert_eq!(post.update_activity_at, Some(1700000000));
    }

    #[test]
    fn common_post_mutation_falls_back_to_caller_pid() {
        let mutation =
            SessionMutation::Statusline(Box::new(crate::routes::statusline::StatuslinePayload {
                session_id: "test".into(),
                model: None,
                cwd: None,
                workspace: None,
                cost: None,
                context_window: None,
                exceeds_200k_tokens: None,
                transcript_path: None,
                version: None,
                output_style: None,
                vim: None,
                agent: None,
                worktree: None,
                rate_limits: None,
                extra: Default::default(),
            }));
        let post = common_post_mutation(&mutation, Some(555), 1700000000);
        assert_eq!(post.bind_pid, Some(555));
    }

    #[tokio::test]
    async fn buffer_phase_stores_and_drains() {
        let coordinator = SessionCoordinator::new();
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));
        let (live_tx, _rx) = broadcast::channel(16);
        let db = claude_view_db::Database::new_in_memory()
            .await
            .expect("in-memory DB");
        let transcript_to_session: TranscriptMap = Arc::new(RwLock::new(HashMap::new()));
        let hook_event_channels: Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &transcript_to_session,
            hook_event_channels: &hook_event_channels,
        };

        // Send a statusline mutation for a session that doesn't exist yet
        let payload = crate::routes::statusline::StatuslinePayload {
            session_id: "buffered-session".into(),
            model: Some(crate::routes::statusline::StatuslineModel {
                id: Some("claude-sonnet-4-20250514".into()),
                display_name: Some("Sonnet".into()),
            }),
            cwd: None,
            workspace: None,
            cost: None,
            context_window: None,
            exceeds_200k_tokens: None,
            transcript_path: None,
            version: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: Default::default(),
        };

        let result = coordinator
            .handle(
                &ctx,
                "buffered-session",
                SessionMutation::Statusline(Box::new(payload)),
                None,
                1700000000,
                None,
                None, // no cwd → buffer
            )
            .await;

        assert!(
            matches!(result, MutationResult::Buffered),
            "Expected Buffered, got different result"
        );

        // Now send a Start event which can create the session
        let result = coordinator
            .handle(
                &ctx,
                "buffered-session",
                SessionMutation::Lifecycle(LifecycleEvent::Start {
                    cwd: Some("/tmp".into()),
                    model: None,
                    source: None,
                    pid: Some(111),
                    transcript_path: None,
                }),
                Some(111),
                1700000001,
                None,
                None, // Start carries cwd internally
            )
            .await;

        assert!(
            matches!(result, MutationResult::Created(_)),
            "Expected Created after Start"
        );

        // Verify the buffered statusline was drained and applied
        let sessions = ctx.sessions.read().await;
        let session = sessions.get("buffered-session").unwrap();
        assert_eq!(
            session.model.as_deref(),
            Some("claude-sonnet-4-20250514"),
            "Buffered statusline model should have been applied"
        );
    }
}
