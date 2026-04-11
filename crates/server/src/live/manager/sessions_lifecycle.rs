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

                        // Optimistic existence check (no lock held between check
                        // and action). The coordinator's Phase 2 write-lock guard
                        // is the actual creation guard — this read-lock check is
                        // just a fast path to avoid coordinator overhead for the
                        // common case (session already exists from hook/JSONL).
                        let exists = {
                            let sessions = manager.sessions.read().await;
                            sessions.contains_key(&session.session_id)
                        };

                        if exists {
                            // Enrich existing — kind/entrypoint are display-only
                            // metadata. No SSE broadcast needed: the session was
                            // already broadcast when created, and these fields will
                            // be included in the next SessionUpdated event.
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
