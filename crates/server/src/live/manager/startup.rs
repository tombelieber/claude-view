//! Startup recovery: snapshot promotion, PID dedup, and closed session restoration.
//!
//! These methods run once during server startup to reconstruct in-memory state
//! from disk artifacts (PID snapshot, SQLite closed_at, JSONL files).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

use crate::live::process::is_pid_alive;
use crate::live::state::{
    status_from_agent_state, AgentState, AgentStateGroup, SessionEvent, SessionStatus,
};

use super::accumulator::{
    apply_jsonl_metadata, build_metadata_from_accumulator, build_recovered_session,
    derive_agent_state_from_jsonl,
};
use super::helpers::{
    extract_project_info, extract_session_id, load_session_snapshot, pid_snapshot_path,
};
use super::LiveSessionManager;

impl LiveSessionManager {
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

            if let Some(path) = initial_paths
                .iter()
                .find(|p| extract_session_id(p) == *session_id)
            {
                let file_path_str = path.to_string_lossy().to_string();
                let mut session = build_recovered_session(session_id, entry, &file_path_str);

                // Enrich with accumulator metrics if available
                let accumulators = self.accumulators.read().await;
                let cached_cwd = accumulators
                    .get(session_id)
                    .and_then(|a| a.resolved_cwd.as_deref());
                let (project, project_display_name, project_path, _) =
                    extract_project_info(path, cached_cwd);
                if let Some(acc) = accumulators.get(session_id) {
                    let metadata = build_metadata_from_accumulator(
                        acc,
                        entry.last_activity_at,
                        Some(entry.pid),
                    );
                    drop(accumulators);

                    apply_jsonl_metadata(
                        &mut session,
                        &metadata,
                        &file_path_str,
                        &project,
                        &project_display_name,
                        &project_path,
                    );
                    // Populate team data from TeamsStore
                    if let Some(ref tn) = session.jsonl.team_name.clone() {
                        if let Some(detail) = self.teams.get(tn) {
                            session.jsonl.team_members = detail.members;
                        }
                        session.jsonl.team_inbox_count = self
                            .teams
                            .inbox(tn)
                            .map(|msgs| msgs.len() as u32)
                            .unwrap_or(0);
                    } else {
                        session.jsonl.team_members = Vec::new();
                        session.jsonl.team_inbox_count = 0;
                    }
                } else {
                    drop(accumulators);
                }

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

                self.sessions
                    .write()
                    .await
                    .insert(session_id.clone(), session.clone());
                let _ = self.tx.send(SessionEvent::SessionDiscovered { session });
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
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            for dupe_id in &pid_dupes {
                if let Some(session) = sessions.get_mut(dupe_id) {
                    info!(
                        session_id = %dupe_id,
                        pid = ?session.hook.pid,
                        "Snapshot PID dedup: evicting stale entry"
                    );
                    session.status = SessionStatus::Done;
                    session.closed_at = Some(now);
                    session.hook.agent_state = AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "session_ended".into(),
                        label: "Evicted (PID collision)".into(),
                        context: None,
                    };
                }
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

    /// Restore recently-closed sessions from SQLite.
    pub(super) async fn restore_closed_sessions(
        self: &Arc<Self>,
        initial_paths: &[std::path::PathBuf],
    ) {
        let closed_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT id, closed_at FROM sessions WHERE closed_at IS NOT NULL AND dismissed_at IS NULL",
        )
        .fetch_all(self.db.pool())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to load recently-closed sessions from SQLite");
            Vec::new()
        });

        // Phase 1: Parse JSONL files for closed sessions (OUTSIDE sessions lock)
        for (session_id, _closed_at) in &closed_rows {
            if self.sessions.read().await.contains_key(session_id) {
                continue;
            }
            if let Some(path) = initial_paths
                .iter()
                .find(|p| extract_session_id(p) == *session_id)
            {
                self.process_jsonl_update(path).await;
            }
        }

        // Phase 2: Mark recovered sessions as closed (with sessions lock)
        {
            let mut sessions = self.sessions.write().await;
            let mut restored = 0u32;
            for (session_id, closed_at) in &closed_rows {
                if let Some(session) = sessions.get_mut(session_id) {
                    if session.closed_at.is_none() {
                        session.status = SessionStatus::Done;
                        session.closed_at = Some(*closed_at);
                        session.hook.agent_state = AgentState {
                            group: AgentStateGroup::NeedsYou,
                            state: "session_ended".into(),
                            label: "Session ended".into(),
                            context: None,
                        };
                        session.hook.hook_events.clear();
                        restored += 1;
                    }
                }
            }
            if restored > 0 {
                info!(
                    count = restored,
                    "Restored recently-closed sessions from SQLite"
                );
            }
        }
    }
}
