//! Consumer for sessions directory lifecycle events.
//!
//! Receives Born/Exited/Crashed events from the sessions_watcher and
//! enriches LiveSession with kind/entrypoint data, or marks sessions dead.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::info;

use crate::live::sessions_watcher::SessionLifecycleEvent;

use super::LiveSessionManager;

impl LiveSessionManager {
    /// Spawn the sessions lifecycle event consumer.
    ///
    /// Processes Born/Exited/Crashed events from the sessions directory watcher.
    /// - Born: creates the session (if new) or enriches existing, then annotates
    ///   tmux ownership by matching the PID to tmux panes.
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

                        let session_id = session.session_id.clone();

                        // Check if UUID already exists as a primary key.
                        let exists = {
                            let sessions = manager.sessions.read().await;
                            sessions.contains_key(&session_id)
                        };

                        if exists {
                            // Enrich existing — kind/entrypoint are display-only
                            // metadata. No SSE broadcast needed: the session was
                            // already broadcast when created, and these fields
                            // will be included in the next SessionUpdated event.
                            let mut sessions = manager.sessions.write().await;
                            if let Some(live) = sessions.get_mut(&session_id) {
                                live.session_kind = Some(session.kind);
                                live.entrypoint = Some(session.entrypoint);
                                if live.hook.pid.is_none() {
                                    live.hook.pid = Some(pid);
                                }
                            }
                        } else {
                            // Also check secondary index — UUID might already
                            // be linked to an existing entry (e.g. from a prior
                            // Born that was processed as a Modify event).
                            let already_linked = {
                                let idx = manager.claude_session_id_index.read().await;
                                idx.contains_key(&session_id)
                            };

                            if !already_linked {
                                // Route through coordinator — single creation path.
                                manager.handle_session_birth(session, pid).await;

                                // Annotate tmux ownership: check if this PID
                                // matches any tmux pane and set ownership.tmux.
                                manager.annotate_tmux_ownership(&session_id, pid).await;
                            }
                        }

                        // Register PID with death watcher.
                        manager._death_watcher.watch(pid, session_id).await;
                    }

                    SessionLifecycleEvent::Exited { pid } => {
                        info!(pid = pid, "sessions_watcher: session process exited");

                        // Find the session with this PID and reap it
                        let session_id = {
                            let sessions = manager.sessions.read().await;
                            sessions
                                .iter()
                                .find(|(_, s)| {
                                    s.hook.pid == Some(pid)
                                        && s.status != crate::live::state::SessionStatus::Done
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

    /// Set tmux ownership on a session by matching PID → tmux pane.
    ///
    /// Called after handle_session_birth() creates a UUID-keyed session.
    /// Checks all registered tmux sessions to see if this PID belongs
    /// to one of their panes, and if so, sets ownership.tmux + broadcasts.
    async fn annotate_tmux_ownership(self: &Arc<Self>, session_id: &str, pid: u32) {
        let tmux_names = self.tmux_index.list().await;
        let mut matched_name = None;

        for name in &tmux_names {
            if self.tmux.pane_pid(name) == Some(pid) {
                matched_name = Some(name.clone());
                break;
            }
        }

        let Some(tmux_name) = matched_name else {
            return;
        };

        // Set tmux ownership on the session and broadcast.
        let mut sessions = self.sessions.write().await;
        if let Some(live) = sessions.get_mut(session_id) {
            let ownership = live.ownership.get_or_insert_with(Default::default);
            ownership.tmux = Some(claude_view_types::TmuxBinding {
                cli_session_id: tmux_name.clone(),
            });

            let _ = self
                .tx
                .send(crate::live::state::SessionEvent::SessionUpsert {
                    session: live.clone(),
                });

            info!(
                session_id = %session_id,
                tmux_name = %tmux_name,
                pid = pid,
                "Annotated tmux ownership on Born session"
            );
        }
    }
}
