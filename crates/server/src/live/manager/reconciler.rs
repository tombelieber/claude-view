//! Reconciliation loop, cleanup task, and death consumer.
//!
//! PID liveness checks, stale session cleanup, process count refresh,
//! and event-driven death notification handling.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing::info;

use crate::live::mutation::types::{LifecycleEvent, SessionMutation};
use crate::live::process::detect_claude_processes;
use crate::live::state::{AgentState, AgentStateGroup, SessionEvent, SessionStatus};

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
                let mut dead_sessions: Vec<String> = Vec::new();
                let mut ghost_sessions: Vec<String> = Vec::new();
                {
                    let sessions = manager.sessions.read().await;
                    for (session_id, session) in sessions.iter() {
                        if session.status == SessionStatus::Done {
                            continue;
                        }
                        if let Some(pid) = session.hook.pid {
                            if !crate::live::process::is_pid_alive(pid) {
                                let is_ghost = session.jsonl.file_path.is_empty()
                                    && session.hook.turn_count == 0;
                                if is_ghost {
                                    info!(
                                        session_id = %session_id,
                                        pid = pid,
                                        "Ghost session (no JSONL, zero turns) -- auto-completing"
                                    );
                                    ghost_sessions.push(session_id.clone());
                                } else {
                                    info!(
                                        session_id = %session_id,
                                        pid = pid,
                                        "Bound PID is dead -- marking session ended"
                                    );
                                    dead_sessions.push(session_id.clone());
                                }
                            }
                        }
                    }
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // Route dead sessions through coordinator
                if !dead_sessions.is_empty() {
                    let ctx = manager.mutation_context(&manager);
                    for session_id in &dead_sessions {
                        manager
                            .coordinator
                            .handle(
                                &ctx,
                                session_id,
                                SessionMutation::Lifecycle(LifecycleEvent::End { reason: None }),
                                None,
                                now,
                                None,
                                None,
                            )
                            .await;
                    }
                }

                // Ghost sessions: manual removal + SessionCompleted
                if !ghost_sessions.is_empty() {
                    {
                        let mut sessions = manager.sessions.write().await;
                        for session_id in &ghost_sessions {
                            if let Some(session) = sessions.get_mut(session_id) {
                                session.hook.agent_state = AgentState {
                                    group: AgentStateGroup::NeedsYou,
                                    state: "session_ended".into(),
                                    label: "Session ended".into(),
                                    context: None,
                                };
                                session.status = SessionStatus::Done;
                                session.closed_at = Some(now);
                            }
                            sessions.remove(session_id);
                        }
                    }
                    for session_id in &ghost_sessions {
                        let _ = manager.tx.send(SessionEvent::SessionCompleted {
                            session_id: session_id.clone(),
                        });
                    }
                    let mut accumulators = manager.accumulators.write().await;
                    for session_id in &ghost_sessions {
                        accumulators.remove(session_id);
                    }
                }

                // Save session snapshot if any sessions changed
                if !dead_sessions.is_empty() || !ghost_sessions.is_empty() {
                    manager.save_session_snapshot_from_state().await;
                }

                // Persist closed_at to SQLite
                let all_closed: Vec<String> = dead_sessions
                    .iter()
                    .chain(ghost_sessions.iter())
                    .cloned()
                    .collect();
                if !all_closed.is_empty() {
                    let db = manager.db.clone();
                    tokio::spawn(async move {
                        let mut tx = match db.pool().begin().await {
                            Ok(tx) => tx,
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to begin transaction for closed_at persistence");
                                return;
                            }
                        };
                        for session_id in all_closed {
                            let _ = sqlx::query(
                                "UPDATE sessions SET closed_at = ?1 WHERE id = ?2 AND closed_at IS NULL",
                            )
                            .bind(now)
                            .bind(&session_id)
                            .execute(&mut *tx)
                            .await;
                        }
                        let _ = tx.commit().await;
                    });
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

                // Unconditional snapshot save (defense in depth)
                manager.save_session_snapshot_from_state().await;
            }
        });
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

    /// Refresh process data from oracle (Phase 2 of reconciliation).
    async fn refresh_process_data(self: &Arc<Self>) {
        let oracle_snap = self.oracle_rx.borrow().clone();
        let (processes, total_count) = match oracle_snap.claude_processes.as_ref() {
            Some(cp) => (cp.processes.clone(), cp.count),
            None => tokio::task::spawn_blocking(detect_claude_processes)
                .await
                .unwrap_or_default(),
        };
        self.process_count.store(total_count, Ordering::Relaxed);

        // Classify source for all live sessions
        let sdk_source = crate::live::process::SessionSourceInfo {
            category: crate::live::process::SessionSource::AgentSdk,
            label: None,
        };
        let backfilled: Vec<crate::live::state::LiveSession> = {
            let mut sessions = self.sessions.write().await;
            let mut updated = Vec::new();
            for session in sessions.values_mut() {
                if session.status == SessionStatus::Done {
                    continue;
                }
                if session.control.is_some() {
                    if session.jsonl.source.as_ref() != Some(&sdk_source) {
                        session.jsonl.source = Some(sdk_source.clone());
                        updated.push(session.clone());
                    }
                    continue;
                }
                if let Some(pid) = session.hook.pid {
                    if let Some(cp) = processes.get(&pid) {
                        let new_source = Some(cp.source.clone());
                        if session.jsonl.source != new_source {
                            session.jsonl.source = new_source;
                            updated.push(session.clone());
                        }
                    }
                }
            }
            updated
        };
        for session in backfilled {
            let _ = self.tx.send(SessionEvent::SessionUpdated { session });
        }
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

    /// Spawn the periodic housekeeping task.
    ///
    /// Every 60 seconds: removes orphaned accumulators.
    pub(super) fn spawn_cleanup_task(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;

                let sessions = manager.sessions.read().await;
                let mut accumulators = manager.accumulators.write().await;
                let orphan_ids: Vec<String> = accumulators
                    .keys()
                    .filter(|id| !sessions.contains_key(*id))
                    .cloned()
                    .collect();
                for id in &orphan_ids {
                    accumulators.remove(id);
                }
                if !orphan_ids.is_empty() {
                    info!("Cleaned up {} orphaned accumulators", orphan_ids.len());
                }
            }
        });
    }

    /// Spawn the death notification consumer.
    ///
    /// Reads from the kqueue-based ProcessDeathWatcher and immediately marks
    /// sessions as Done when their PID exits.
    pub(super) fn spawn_death_consumer(
        self: &Arc<Self>,
        mut death_rx: tokio::sync::mpsc::Receiver<super::super::process_death::DeathNotification>,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some((pid, session_id)) = death_rx.recv().await {
                let (should_act, is_ghost) = {
                    let sessions = manager.sessions.read().await;
                    match sessions.get(&session_id) {
                        Some(session)
                            if session.status != SessionStatus::Done
                                && session.hook.pid == Some(pid) =>
                        {
                            let ghost =
                                session.jsonl.file_path.is_empty() && session.hook.turn_count == 0;
                            (true, ghost)
                        }
                        _ => (false, false),
                    }
                };

                if !should_act {
                    continue;
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                info!(
                    session_id = %session_id,
                    pid = pid,
                    ghost = is_ghost,
                    "kqueue: PID death -> marking session ended"
                );

                if is_ghost {
                    let mut sessions = manager.sessions.write().await;
                    if let Some(session) = sessions.get_mut(&session_id) {
                        session.hook.agent_state = AgentState {
                            group: AgentStateGroup::NeedsYou,
                            state: "session_ended".into(),
                            label: "Session ended".into(),
                            context: None,
                        };
                        session.status = SessionStatus::Done;
                        session.closed_at = Some(now);
                    }
                    let sid = session_id.clone();
                    sessions.remove(&sid);
                    drop(sessions);
                    let _ = manager
                        .tx
                        .send(SessionEvent::SessionCompleted { session_id: sid });
                } else {
                    let ctx = manager.mutation_context(&manager);
                    manager
                        .coordinator
                        .handle(
                            &ctx,
                            &session_id,
                            SessionMutation::Lifecycle(LifecycleEvent::End { reason: None }),
                            None,
                            now,
                            None,
                            None,
                        )
                        .await;
                }
                manager.request_snapshot_save();
            }
        });
    }
}
