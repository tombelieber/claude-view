//! Startup recovery: snapshot promotion and PID dedup.
//!
//! These methods run once during server startup to reconstruct in-memory state
//! from disk artifacts (PID snapshot, JSONL files). OS process table is truth.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{info, warn};

use crate::live::process::is_pid_alive;
use crate::live::state::{
    status_from_agent_state, AgentState, AgentStateGroup, SessionEvent, SessionStatus,
};

use super::accumulator::{build_recovered_session, derive_agent_state_from_jsonl};
use super::helpers::{extract_session_id, load_session_snapshot, pid_snapshot_path};
use super::LiveSessionManager;

impl LiveSessionManager {
    /// Scan ~/.claude/sessions/ as the PRIMARY lifecycle source.
    ///
    /// This runs before JSONL scan and snapshot recovery. It provides:
    /// - Immediate list of alive sessions (no hooks needed)
    /// - kind (interactive/background) and entrypoint (cli/vscode/desktop)
    /// - Crash detection: session file exists but PID is dead
    ///
    /// The results are stored as a "pre-enrichment map" that later stages
    /// (promote_from_snapshot, coordinator) use to populate session_kind/entrypoint.
    pub(super) async fn scan_sessions_dir_at_startup(self: &Arc<Self>) {
        let sessions =
            tokio::task::spawn_blocking(crate::live::sessions_watcher::scan_sessions_dir)
                .await
                .unwrap_or_default();

        if sessions.is_empty() {
            info!("Sessions dir scan: no active session files found");
            return;
        }

        let (alive, crashed) = tokio::task::spawn_blocking(move || {
            crate::live::sessions_watcher::partition_by_liveness(sessions)
        })
        .await
        .unwrap_or_default();

        info!(
            alive = alive.len(),
            crashed = crashed.len(),
            "Sessions dir scan: primary lifecycle source"
        );

        // Clean up crashed session files (PID dead but file left behind)
        for crashed_session in &crashed {
            if let Some(sessions_dir) = claude_view_core::session_files::claude_sessions_dir() {
                let stale_path = sessions_dir.join(format!("{}.json", crashed_session.pid));
                if stale_path.exists() {
                    info!(
                        pid = crashed_session.pid,
                        session_id = %crashed_session.session_id,
                        "Cleaning stale session file (PID dead)"
                    );
                    let _ = std::fs::remove_file(&stale_path);
                }
            }
        }

        // For alive sessions, create live sessions immediately AND register
        // with the death watcher. Previously this only registered PIDs — the
        // actual session creation was deferred to promote_from_snapshot (which
        // requires a matching JSONL in the 24h window) or the 30s reconciliation
        // backfill. This caused the SSE initial burst to only contain snapshot
        // sessions (~2), with the rest appearing 30s later.
        for session in alive {
            let pid = session.pid;
            self.handle_session_birth(session, pid).await;
        }
    }

    /// Promote sessions from crash-recovery snapshot.
    pub(super) async fn promote_from_snapshot(
        self: &Arc<Self>,
        initial_paths: &[std::path::PathBuf],
    ) {
        let Some(snap_path) = pid_snapshot_path() else {
            return;
        };
        let snapshot = load_session_snapshot(&snap_path);
        if snapshot.sessions.is_empty() {
            return;
        }

        let mut promoted = 0u32;
        let mut dead = 0u32;
        let mut dead_ids: Vec<String> = Vec::new();
        let mut sessions_to_recover: Vec<(String, String)> = Vec::new();

        for (session_id, entry) in &snapshot.sessions {
            if self.sessions.read().await.contains_key(session_id) {
                continue;
            }
            if !is_pid_alive(entry.pid) {
                dead += 1;
                dead_ids.push(session_id.clone());
                continue;
            }

            // PID reuse guard placeholder: Task 4 will replace this with a
            // PID file check (~/.claude/sessions/{pid}.json) to detect PID
            // recycling. For now, liveness alone is sufficient since the
            // reconciliation loop catches stale sessions within 30s.

            if let Some(path) = initial_paths
                .iter()
                .find(|p| extract_session_id(p) == *session_id)
            {
                let file_path_str = path.to_string_lossy().to_string();
                let mut session = build_recovered_session(session_id, entry, &file_path_str);

                // Structural invariant: parse JSONL → enrich → then insert.
                // Same pattern as coordinator Phase 1b → apply_accumulator_to_session.
                self.process_jsonl_update(path).await;
                self.apply_accumulator_to_session(session_id, &mut session)
                    .await;

                // Override snapshot agent_state with JSONL ground truth
                if let Some(derived) = derive_agent_state_from_jsonl(path).await {
                    if derived.group != session.hook.agent_state.group
                        || derived.state != session.hook.agent_state.state
                    {
                        info!(
                            session_id = %session_id,
                            snapshot = %session.hook.agent_state.state,
                            derived = %derived.state,
                            "JSONL ground truth overrides snapshot agent_state"
                        );
                    }
                    session.status = status_from_agent_state(&derived);
                    session.hook.current_activity = derived.label.clone();
                    session.hook.agent_state = derived;
                } else {
                    session.hook.agent_state = AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Waiting for your next prompt".into(),
                        context: None,
                    };
                    session.status = SessionStatus::Paused;
                }

                // Enrich with kind/entrypoint from sessions/{pid}.json
                super::helpers::enrich_from_session_file(&mut session, entry.pid);

                self.sessions
                    .write()
                    .await
                    .insert(session_id.clone(), session.clone());
                let _ = self.tx.send(SessionEvent::SessionUpsert { session });
                promoted += 1;
                if let Some(ref ctrl_id) = entry.control_id {
                    sessions_to_recover.push((session_id.clone(), ctrl_id.clone()));
                }
            } else {
                warn!(
                    session_id = %session_id,
                    pid = entry.pid,
                    "Snapshot entry has alive PID but no matching JSONL file in 24h scan window -- skipping"
                );
            }
        }

        // PID dedup pass
        self.dedup_snapshot_pids(&mut sessions_to_recover).await;

        // Clean accumulators for dead snapshot PIDs
        if !dead_ids.is_empty() {
            let mut accumulators = self.accumulators.write().await;
            for id in &dead_ids {
                accumulators.remove(id);
            }
            info!(
                cleaned = dead_ids.len(),
                "Cleaned accumulators for dead snapshot PIDs"
            );
        }

        // Recover controlled sessions via sidecar
        if !sessions_to_recover.is_empty() {
            if let Some(ref sidecar) = self.sidecar {
                match sidecar.ensure_running().await {
                    Ok(_) => {
                        let recovered = sidecar
                            .recover_controlled_sessions(&sessions_to_recover)
                            .await;
                        for (sid, new_ctrl_id) in &recovered {
                            self.bind_control(sid, new_ctrl_id.clone(), None).await;
                        }
                        info!(
                            "Recovered {}/{} controlled sessions after restart",
                            recovered.len(),
                            sessions_to_recover.len()
                        );
                    }
                    Err(e) => {
                        warn!("Sidecar unavailable for recovery: {e}. Control bindings cleared.");
                    }
                }
            }
        }

        if promoted > 0 || dead > 0 {
            info!(
                promoted,
                dead,
                total = snapshot.sessions.len(),
                "Startup recovery: promoted sessions from crash snapshot"
            );
        }

        // Always re-save: prunes dead entries
        self.save_session_snapshot_from_state().await;
    }

    /// PID dedup pass: if two snapshot entries share the same PID, keep the more recent one.
    async fn dedup_snapshot_pids(&self, sessions_to_recover: &mut Vec<(String, String)>) {
        let mut sessions = self.sessions.write().await;
        let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
        let mut pid_dupes: Vec<String> = Vec::new();

        for (id, session) in sessions.iter() {
            if session.status == SessionStatus::Done {
                continue;
            }
            if let Some(pid) = session.hook.pid {
                if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                    let new_wins = session.hook.last_activity_at > *existing_ts
                        || (session.hook.last_activity_at == *existing_ts && *id > *existing_id);
                    if new_wins {
                        pid_dupes.push(existing_id.clone());
                        pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
                    } else {
                        pid_dupes.push(id.clone());
                    }
                } else {
                    pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
                }
            }
        }

        if !pid_dupes.is_empty() {
            for dupe_id in &pid_dupes {
                if let Some(session) = sessions.get(dupe_id) {
                    info!(
                        session_id = %dupe_id,
                        pid = ?session.hook.pid,
                        "Snapshot PID dedup: evicting stale entry"
                    );
                }
                sessions.remove(dupe_id);
            }
            let dupe_set: std::collections::HashSet<&str> =
                pid_dupes.iter().map(|s| s.as_str()).collect();
            sessions_to_recover.retain(|(id, _)| !dupe_set.contains(id.as_str()));
            info!(
                evicted = pid_dupes.len(),
                "Snapshot recovery PID dedup complete"
            );
        }
    }
}
