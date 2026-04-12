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

use crate::live::buffer::PendingMutations;
use crate::live::mutation::types::{
    BroadcastAction, LifecycleEvent, MutationResult, SessionMutation,
};
use crate::live::state::{append_capped_hook_event, HookEvent, SessionEvent, SessionStatus};

use super::dispatch::{apply_mutation_to_session, common_post_mutation};
use super::execution::{execute_side_effect, forward_hook_event_to_ws};
use super::planning::plan_side_effects;
use super::session_factory::{
    create_session_from_birth, create_session_from_start, create_session_shell,
};
use super::types::{BufferedMutation, MutationContext, MAX_EVENTS, PENDING_TTL};

// ---------------------------------------------------------------------------
// SessionCoordinator
// ---------------------------------------------------------------------------

/// Single entry point for all session state mutations.
///
/// Replaces 25+ scattered `live_tx.send()` calls with one deterministic
/// pipeline that enforces lock ordering and side-effect separation.
pub struct SessionCoordinator {
    pending: tokio::sync::Mutex<PendingMutations<BufferedMutation>>,
}

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
    // Phase 1 -> 4: handle()
    // -----------------------------------------------------------------------

    /// Apply a single mutation to a session, returning the outcome.
    ///
    /// This is the ONLY public entry point for session state changes.
    /// All side effects (DB writes, accumulator cleanup) are executed
    /// after the write lock is dropped.
    ///
    /// `cwd` + `transcript_path`: optional fields from the hook payload.
    /// When cwd is present and the session doesn't exist, the coordinator
    /// upserts a shell session. transcript_path links it to the JSONL file
    /// so the reconciler can backfill model/tokens/cost/title.
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
        transcript_path: Option<&str>,
    ) -> MutationResult {
        // -- Phase 1: Buffer or upsert --
        let session_exists = {
            let sessions = ctx.sessions.read().await;
            sessions.contains_key(session_id)
        };

        if !session_exists {
            if mutation.can_create_session() {
                // Create session, then drain buffered mutations below
            } else {
                // Buffer mutation + hook event together, return early
                let mut pending = self.pending.lock().await;
                pending.push(session_id, (mutation, hook_event));
                return MutationResult::Buffered;
            }
        }

        // -- Phase 1b: Pre-enrich from JSONL (BEFORE write lock) --
        // Structural invariant: every session MUST have its JSONL parsed
        // before it enters the map. We do this outside the write lock because
        // process_jsonl_update does blocking IO. When the session already
        // exists in the map, this is a no-op (fast path).
        //
        // When transcript_path is None (e.g. upsert via cwd without
        // transcript_path in payload), fall back to the accumulator's
        // cached file_path — the file watcher may have already seen it.
        if !session_exists {
            if let Some(mgr) = ctx.live_manager {
                if let Some(tp) = transcript_path {
                    mgr.process_jsonl_update(std::path::Path::new(tp)).await;
                } else {
                    // Fallback: check if the watcher already discovered this
                    // session's JSONL (accumulator has file_path from initial_scan)
                    let fallback_path = mgr.accumulator_file_path(session_id).await;
                    if let Some(ref fp) = fallback_path {
                        mgr.process_jsonl_update(fp).await;
                    }
                }
            }
        }

        // -- Phase 2+3: Plan + Execute under write lock --
        let (snapshot, broadcast_action, side_effects) = {
            let mut sessions = ctx.sessions.write().await;

            // Create session if needed (Birth, Reconcile, or Start fallback)
            if !sessions.contains_key(session_id) {
                let new_session = match &mutation {
                    SessionMutation::Birth(active_session) => {
                        create_session_from_birth(active_session, now)
                    }
                    SessionMutation::Lifecycle(LifecycleEvent::Start {
                        cwd: start_cwd,
                        model,
                        pid: start_pid,
                        transcript_path: start_tp,
                        ..
                    }) => {
                        let mut s =
                            create_session_from_start(session_id, start_cwd, model, start_pid, now);
                        if let Some(tp) = start_tp {
                            s.jsonl.file_path = tp.clone();
                        }
                        s
                    }
                    SessionMutation::Reconcile(data) => create_session_shell(session_id, data, now),
                    // Upsert: shell session — state set by the mutation that follows
                    _ => {
                        let mut s = create_session_from_start(
                            session_id,
                            &cwd.map(|c| c.to_string()),
                            &None,
                            &pid,
                            now,
                        );
                        if let Some(tp) = transcript_path {
                            s.jsonl.file_path = tp.to_string();
                        }
                        s
                    }
                };

                // Apply pre-enriched JSONL data from accumulator (populated in Phase 1b).
                // This ensures the session enters the map with title/tokens/cost
                // already filled — no empty card is ever broadcast.
                let mut new_session = new_session;
                if let Some(mgr) = ctx.live_manager {
                    mgr.apply_accumulator_to_session(session_id, &mut new_session)
                        .await;
                }

                // Enrich with kind/entrypoint from sessions/{pid}.json
                if let Some(pid) = new_session.hook.pid {
                    crate::live::manager::helpers::enrich_from_session_file(&mut new_session, pid);
                }

                // Compute ownership on session creation so the first broadcast
                // carries tmux/SDK bindings. Without this, sessions discovered
                // after server restart (reconcile) or sessions whose 6s polling
                // window expired would have ownership=null permanently.
                new_session.ownership = Some(
                    crate::live::ownership::compute_ownership(&new_session, ctx.cli_sessions).await,
                );

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

        // -- Phase 3b: Side effects (no lock held) --
        for effect in &side_effects {
            execute_side_effect(ctx, effect).await;
        }

        // -- Phase 4: Broadcast --
        // Ownership is a stored field in the session record — computed on
        // session creation (Phase 2+3) and updated by write_ownership /
        // bind_control / unbind_control. The snapshot goes out with
        // whatever ownership is currently stored.
        let snapshot = snapshot;

        match broadcast_action {
            BroadcastAction::Created | BroadcastAction::Updated => {
                let _ = ctx.live_tx.send(SessionEvent::SessionUpsert {
                    session: snapshot.clone(),
                });
                if matches!(broadcast_action, BroadcastAction::Created) {
                    MutationResult::Created(snapshot)
                } else {
                    MutationResult::Updated(snapshot)
                }
            }
            BroadcastAction::Closed => {
                let _ = ctx.live_tx.send(SessionEvent::SessionRemove {
                    session_id: session_id.to_string(),
                    session: snapshot.clone(),
                });
                MutationResult::Closed(snapshot)
            }
            BroadcastAction::Removed => {
                // Removed = dismissed from recently closed.
                // SessionRemove carries the last snapshot for the ring buffer.
                let _ = ctx.live_tx.send(SessionEvent::SessionRemove {
                    session_id: session_id.to_string(),
                    session: snapshot.clone(),
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
