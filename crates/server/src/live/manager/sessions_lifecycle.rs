//! Consumer for sessions directory lifecycle events.
//!
//! Receives Born/Exited/Crashed events from the sessions_watcher and
//! enriches LiveSession with kind/entrypoint data, or marks sessions dead.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::info;

use crate::live::sessions_watcher::SessionLifecycleEvent;
use crate::live::state::SessionStatus;

use super::LiveSessionManager;

impl LiveSessionManager {
    /// Spawn the sessions lifecycle event consumer.
    ///
    /// Processes Born/Exited/Crashed events from the sessions directory watcher.
    /// - Born: enriches existing sessions with kind/entrypoint, or records the
    ///   PID→session mapping for later matching.
    /// - Exited: triggers session reaping (same path as kqueue death).
    /// - Crashed: same as Exited but for sessions whose PID died without cleanup.
    pub(super) fn spawn_sessions_lifecycle_consumer(
        self: &Arc<Self>,
        mut rx: mpsc::Receiver<SessionLifecycleEvent>,
    ) {
        let manager = self.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    SessionLifecycleEvent::Born { pid, session } => {
                        info!(
                            pid = pid,
                            session_id = %session.session_id,
                            kind = %session.kind,
                            entrypoint = %session.entrypoint,
                            "sessions_watcher: new session born"
                        );

                        // 1. Check: is there an existing Spawning session from a
                        //    tmux POST? Match by: find the tmux session that owns
                        //    this PID, then find the Spawning LiveSession with
                        //    that tmux binding.
                        let spawning_key = {
                            let tmux_names = manager.tmux_index.list().await;
                            let mut found_name = None;
                            for name in &tmux_names {
                                if manager.tmux.pane_pid(name) == Some(pid) {
                                    found_name = Some(name.clone());
                                    break;
                                }
                            }
                            if let Some(ref name) = found_name {
                                let sessions = manager.sessions.read().await;
                                sessions.iter().find_map(|(key, s)| {
                                    if s.status == SessionStatus::Spawning
                                        && s.ownership
                                            .as_ref()
                                            .and_then(|o| o.tmux.as_ref())
                                            .is_some_and(|t| t.cli_session_id == *name)
                                    {
                                        Some(key.clone())
                                    } else {
                                        None
                                    }
                                })
                            } else {
                                None
                            }
                        };

                        if let Some(key) = spawning_key {
                            // ENRICH existing Spawning session (tmux-spawned).
                            // Acquire sessions write lock, mutate, broadcast, then
                            // drop before acquiring the secondary index lock to
                            // avoid nested lock deadlocks.
                            {
                                let mut sessions = manager.sessions.write().await;
                                if let Some(live) = sessions.get_mut(&key) {
                                    live.status = SessionStatus::Working;
                                    live.hook.pid = Some(pid);
                                    live.session_kind = Some(session.kind.clone());
                                    live.entrypoint = Some(session.entrypoint.clone());
                                    live.jsonl.source =
                                        Some(crate::live::process::entrypoint_to_source(
                                            &session.entrypoint,
                                        ));
                                    live.started_at = Some(session.started_at);
                                    live.hook.agent_state = crate::live::state::AgentState {
                                        group: crate::live::state::AgentStateGroup::Autonomous,
                                        state: "acting".into(),
                                        label: "Working".into(),
                                        context: None,
                                    };
                                    live.hook.last_activity_at = session.started_at;

                                    let _ = manager.tx.send(
                                        crate::live::state::SessionEvent::SessionUpsert {
                                            session: live.clone(),
                                        },
                                    );

                                    info!(
                                        key = %key,
                                        uuid = %session.session_id,
                                        "Enriched Spawning → Working (tmux-spawned)"
                                    );
                                }
                            }
                            // sessions write lock dropped here

                            // Update secondary index: Claude UUID → map key
                            manager
                                .claude_session_id_index
                                .write()
                                .await
                                .insert(session.session_id.clone(), key);

                            // Register PID with death watcher
                            manager._death_watcher.watch(pid, session.session_id).await;
                        } else {
                            // Check by Claude UUID (existing path for non-tmux sessions)
                            let exists = {
                                let sessions = manager.sessions.read().await;
                                sessions.contains_key(&session.session_id)
                            };

                            if exists {
                                // Enrich existing — kind/entrypoint are display-only
                                // metadata. No SSE broadcast needed: the session was
                                // already broadcast when created, and these fields
                                // will be included in the next SessionUpdated event.
                                let mut sessions = manager.sessions.write().await;
                                if let Some(live) = sessions.get_mut(&session.session_id) {
                                    live.session_kind = Some(session.kind);
                                    live.entrypoint = Some(session.entrypoint);
                                    if live.hook.pid.is_none() {
                                        live.hook.pid = Some(pid);
                                    }
                                }
                            } else {
                                // Route through coordinator — single creation path
                                manager.handle_session_birth(session, pid).await;
                            }
                        }
                    }

                    SessionLifecycleEvent::Exited { pid } => {
                        info!(pid = pid, "sessions_watcher: session process exited");

                        // Find the session with this PID and reap it
                        let session_id = {
                            let sessions = manager.sessions.read().await;
                            sessions
                                .iter()
                                .find(|(_, s)| {
                                    s.hook.pid == Some(pid) && s.status != SessionStatus::Done
                                })
                                .map(|(id, _)| id.clone())
                        };

                        if let Some(id) = session_id {
                            manager.reap_session(&id).await;
                        }
                    }

                    SessionLifecycleEvent::Crashed { pid, session_id } => {
                        info!(
                            pid = pid,
                            session_id = %session_id,
                            "sessions_watcher: crashed session detected"
                        );

                        // Same treatment as Exited — reap by session ID
                        let has_session = {
                            let sessions = manager.sessions.read().await;
                            sessions.contains_key(&session_id)
                        };

                        if has_session {
                            manager.reap_session(&session_id).await;
                        }
                    }
                }
            }
        });
    }
}
