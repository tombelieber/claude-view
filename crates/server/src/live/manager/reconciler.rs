//! Reconciliation loop and death consumer.
//!
//! PID liveness checks via `reap_session()`, process count refresh,
//! and event-driven death notification handling.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tracing::info;

use crate::live::process::detect_claude_processes;
use crate::live::state::SessionStatus;

use super::LiveSessionManager;

impl LiveSessionManager {
    /// Spawn the reconciliation loop.
    ///
    /// Two-phase design on a 10-second tick:
    ///
    /// **Phase 1 (every tick = 10s) -- lightweight liveness:**
    /// For each session with a bound PID, check `is_pid_alive(pid)`.
    /// Mark dead sessions as Done, remove from map, broadcast completion, save snapshot.
    ///
    /// **Phase 2 (every 3rd tick = 30s) -- process count + snapshot:**
    /// 1. Refresh process count via `detect_claude_processes` (display metric only).
    /// 2. Unconditional snapshot save (defense in depth).
    pub(super) fn spawn_reconciliation_loop(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            let mut tick_count: u64 = 0;

            loop {
                interval.tick().await;
                tick_count += 1;

                // =============================================================
                // Phase 1: Lightweight liveness check (every tick = 10s)
                // =============================================================
                // NOTE: Do NOT filter by `status != Done`. The hook-based
                // SessionEnd path sets status=Done via the coordinator BEFORE
                // the process exits. If we skip Done sessions here, and kqueue
                // misses the subsequent PID death, the session stays as a zombie.
                // reap_session() has its own is_pid_alive() guard — safe to call
                // on any session with a dead PID regardless of status.
                let dead_session_ids: Vec<String> = {
                    let sessions = manager.sessions.read().await;
                    sessions
                        .iter()
                        .filter(|(_, session)| {
                            session
                                .hook
                                .pid
                                .is_some_and(|pid| !crate::live::process::is_pid_alive(pid))
                        })
                        .map(|(id, _)| id.clone())
                        .collect()
                };

                if !dead_session_ids.is_empty() {
                    let count = manager.reap_sessions(&dead_session_ids).await;
                    if count > 0 {
                        info!(reaped = count, "Reconciliation: reaped dead sessions");
                    }
                }

                // =============================================================
                // Phase 1b: Stale control binding detection
                // =============================================================
                manager.reconcile_controlled_sessions().await;

                // Sweep expired pending mutations from coordinator buffer
                manager.coordinator.sweep_expired().await;

                // =============================================================
                // Phase 2: Process count + snapshot (every 3rd tick = 30s)
                // =============================================================
                if !tick_count.is_multiple_of(3) {
                    continue;
                }

                manager.refresh_process_data().await;
                manager.register_pids_with_death_watcher().await;

                // Phase 2b: Sessions dir crash detection (every 30s)
                // Check for session files whose PIDs are dead (crashed without cleanup).
                manager.detect_and_clean_crashed_sessions().await;

                // Unconditional snapshot save (defense in depth)
                manager.save_session_snapshot_from_state().await;
            }
        });
    }

    /// Detect crashed sessions from ~/.claude/sessions/ dir and backfill
    /// alive sessions that the watcher missed.
    ///
    /// Scans session files, checks PID liveness, removes stale files.
    /// Complements the existing PID liveness check in Phase 1 — this catches
    /// sessions that exist in the sessions dir but aren't tracked in our map yet.
    ///
    /// Also performs **birth backfill**: if a session file has an alive PID but
    /// is NOT in our live sessions map (watcher missed the Birth event due to
    /// partial JSON during file creation), insert it as a new session.
    async fn detect_and_clean_crashed_sessions(&self) {
        let dir_sessions =
            tokio::task::spawn_blocking(crate::live::sessions_watcher::scan_sessions_dir)
                .await
                .unwrap_or_default();

        // Collect session IDs we already know about (read lock, released quickly).
        let known_ids: std::collections::HashSet<String> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };

        for session in dir_sessions {
            if !crate::live::process::is_pid_alive(session.pid) {
                // PID dead but file still exists → crashed without cleanup
                if let Some(sessions_dir) = claude_view_core::session_files::claude_sessions_dir() {
                    let stale_path = sessions_dir.join(format!("{}.json", session.pid));
                    if stale_path.exists() {
                        tracing::debug!(
                            pid = session.pid,
                            session_id = %session.session_id,
                            "Cleaning crashed session file"
                        );
                        let _ = std::fs::remove_file(&stale_path);
                    }
                }
            } else if !known_ids.contains(&session.session_id) {
                // PID alive but NOT in our sessions map → watcher missed
                // the Birth event. Backfill by inserting a minimal session
                // so it becomes visible to SSE clients. The file watcher and
                // coordinator will enrich it with JSONL data on the next tick.
                tracing::info!(
                    pid = session.pid,
                    session_id = %session.session_id,
                    "Backfill: discovered alive session missed by watcher"
                );

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let mut live = crate::live::state::test_live_session(&session.session_id);
                live.hook.pid = Some(session.pid);
                live.started_at = Some(session.started_at / 1000); // ms → s
                live.hook.last_activity_at = now;
                live.session_kind = Some(session.kind);
                live.entrypoint = Some(session.entrypoint);
                live.status = SessionStatus::Working;

                let mut sessions = self.sessions.write().await;
                // Double-check under write lock to avoid TOCTOU race.
                if !sessions.contains_key(&session.session_id) {
                    sessions.insert(session.session_id.clone(), live.clone());
                    let _ = self
                        .tx
                        .send(super::super::state::SessionEvent::SessionDiscovered {
                            session: live,
                        });
                }
            }
        }
    }

    /// Reconcile controlled sessions with sidecar state.
    async fn reconcile_controlled_sessions(self: &Arc<Self>) {
        let controlled = self.controlled_session_ids().await;
        if controlled.is_empty() {
            return;
        }
        if let Some(ref sidecar) = self.sidecar {
            if !sidecar.is_running() {
                tracing::warn!(
                    "Sidecar not running, attempting restart for {} controlled sessions",
                    controlled.len()
                );
                match sidecar.ensure_running().await {
                    Ok(_) => {
                        let recovered = sidecar.recover_controlled_sessions(&controlled).await;
                        for (session_id, new_control_id) in &recovered {
                            let old_id = controlled
                                .iter()
                                .find(|(id, _)| id == session_id)
                                .map(|(_, cid)| cid.as_str());
                            self.bind_control(session_id, new_control_id.clone(), old_id)
                                .await;
                        }
                        let recovered_ids: std::collections::HashSet<&str> =
                            recovered.iter().map(|(id, _)| id.as_str()).collect();
                        for (session_id, old_control_id) in &controlled {
                            if !recovered_ids.contains(session_id.as_str()) {
                                self.unbind_control_if(session_id, old_control_id).await;
                            }
                        }
                        self.request_snapshot_save();
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to restart sidecar: {e}. Clearing all control bindings."
                        );
                        for (session_id, old_control_id) in &controlled {
                            self.unbind_control_if(session_id, old_control_id).await;
                        }
                        self.request_snapshot_save();
                    }
                }
            }
        }
    }

    /// Refresh process count from oracle (Phase 2 of reconciliation).
    ///
    /// Session source is derived from the JSONL `entrypoint` field via
    /// `apply_jsonl_metadata` — no process-based classification needed.
    async fn refresh_process_data(self: &Arc<Self>) {
        let oracle_snap = self.oracle_rx.borrow().clone();
        let total_count = match oracle_snap.claude_processes.as_ref() {
            Some(cp) => cp.count,
            None => tokio::task::spawn_blocking(detect_claude_processes)
                .await
                .map(|(_, count)| count)
                .unwrap_or(0),
        };
        self.process_count.store(total_count, Ordering::Relaxed);
    }

    /// Register alive PIDs with death watcher (idempotent).
    async fn register_pids_with_death_watcher(&self) {
        let sessions = self.sessions.read().await;
        for (id, session) in sessions.iter() {
            if session.status != SessionStatus::Done {
                if let Some(pid) = session.hook.pid {
                    self._death_watcher.watch(pid, id.clone()).await;
                }
            }
        }
    }

    /// Spawn the death notification consumer.
    ///
    /// Reads from the kqueue-based ProcessDeathWatcher and immediately reaps
    /// sessions when their PID exits.
    pub(super) fn spawn_death_consumer(
        self: &Arc<Self>,
        mut death_rx: tokio::sync::mpsc::Receiver<super::super::process_death::DeathNotification>,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some((pid, session_id)) = death_rx.recv().await {
                // Verify this session still maps to this PID before reaping.
                let should_reap = {
                    let sessions = manager.sessions.read().await;
                    matches!(
                        sessions.get(&session_id),
                        Some(session) if session.hook.pid == Some(pid)
                    )
                };

                if should_reap {
                    info!(
                        session_id = %session_id,
                        pid = pid,
                        "kqueue: PID death -> reaping session"
                    );
                    manager.reap_session(&session_id).await;
                }
            }
        });
    }
}
