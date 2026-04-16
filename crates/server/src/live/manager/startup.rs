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

/// Parse a PID session file and check if the sessionId matches.
/// Extracted for testability -- the I/O wrapper is `is_pid_still_claude`.
fn pid_file_matches_session(file_content: &str, expected_session_id: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(file_content)
        .ok()
        .and_then(|v| {
            v.get("sessionId")?
                .as_str()
                .map(|s| s == expected_session_id)
        })
        .unwrap_or(false)
}

/// PID reuse guard: verify a PID still belongs to the expected Claude session.
///
/// Checks `~/.claude/sessions/{pid}.json` -- the canonical session lifecycle file.
/// Returns `true` if the file exists and its `sessionId` matches. Returns `false`
/// if the file is missing (session ended cleanly), unreadable, or has a different
/// `sessionId` (PID was recycled for a different Claude session).
fn is_pid_still_claude(pid: u32, expected_session_id: &str) -> bool {
    let Some(sessions_dir) = claude_view_core::session_files::claude_sessions_dir() else {
        return false;
    };
    let path = sessions_dir.join(format!("{pid}.json"));
    match std::fs::read_to_string(&path) {
        Ok(data) => pid_file_matches_session(&data, expected_session_id),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            tracing::warn!(pid, path = %path.display(), error = %e, "PID file unreadable — treating as gone");
            false
        }
    }
}

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

    /// Reconcile tmux ownership at startup — runtime-derived discovery.
    ///
    /// Runs two passes:
    ///
    /// **Pass A — tmux_index hydration for cv-*-spawned sessions**. Lists
    /// tmux sessions whose name starts with `cv-` and re-registers them in
    /// `tmux_index`. This is needed because `scan_sessions_dir_at_startup`
    /// creates sessions without going through the POST handler that
    /// normally populates the index, and the index gates the DELETE
    /// endpoint, health checks, and the `MAX_CLI_SESSIONS` limit.
    /// **User-spawned sessions are intentionally NOT added to the index** —
    /// they are observed, not owned, so claude-view must not kill them or
    /// count them against its own session budget.
    ///
    /// **Pass B — ownership discovery for every live session**. For every
    /// session with a PID, probes the process's environment for
    /// `TMUX_PANE`, resolves the pane to a tmux session name, and sets
    /// `ownership.tmux` on the live session. This covers both cv-*-spawned
    /// and user-spawned tmux sessions uniformly, regardless of process-tree
    /// depth (the env var is inherited transitively through any shell).
    ///
    /// Replaces the old direct `pane_pid == claude.pid` matching, which
    /// only worked for sessions where claude was the direct child of a
    /// tmux pane — breaking the common `tmux → shell → claude` case.
    pub(super) async fn reconcile_tmux_ownership(self: &Arc<Self>) {
        // --- Pass A: re-hydrate tmux_index for cv-* sessions ---
        let tmux_names = self.tmux.list_sessions();
        let cv_names: Vec<String> = tmux_names
            .into_iter()
            .filter(|n| n.starts_with("cv-"))
            .collect();
        for name in &cv_names {
            self.tmux_index.insert(name.clone()).await;
        }

        // --- Pass B: env-probe based ownership discovery ---
        // Snapshot (session_id, pid) pairs under a read lock, then drop it
        // before running subprocess probes — we never want to hold the
        // sessions lock across a blocking `ps eww` or `tmux display-message`.
        let session_pids: Vec<(String, u32)> = {
            let sessions = self.sessions.read().await;
            sessions
                .iter()
                .filter_map(|(id, s)| s.hook.pid.map(|pid| (id.clone(), pid)))
                .collect()
        };

        let mut enriched = 0u32;
        for (session_id, pid) in session_pids {
            if self.try_bind_tmux_env_for_pid(pid, &session_id).await {
                enriched += 1;
            }
        }

        if enriched > 0 {
            info!(
                enriched,
                cv_indexed = cv_names.len(),
                "Tmux ownership reconciliation complete"
            );
        }
    }

    /// Probe a PID's environment for a tmux pane binding, resolve to a tmux
    /// session name, and write `ownership.tmux` into the live session map.
    ///
    /// Returns `true` iff a binding was written (or was already correct —
    /// the call is idempotent, so repeated invocations short-circuit).
    /// Returns `false` if the process isn't in tmux, the pane can't be
    /// resolved, the session isn't in the map, or tmux is unavailable.
    ///
    /// Called from:
    /// - `reconcile_tmux_ownership` (Pass B) for startup catch-up.
    /// - `handle_session_birth` for every runtime-born session.
    ///
    /// The two paths double-cover sessions born during startup
    /// (`scan_sessions_dir_at_startup` → `handle_session_birth` → helper,
    /// then later → `reconcile_tmux_ownership` → helper). That's fine:
    /// the second call short-circuits on the idempotent check.
    pub(crate) async fn try_bind_tmux_env_for_pid(
        self: &Arc<Self>,
        pid: u32,
        session_id: &str,
    ) -> bool {
        // Step 1: read TMUX env vars from the process. Fast path — no lock.
        let Some(env) = crate::live::env_probe::read_tmux_env(pid) else {
            return false;
        };

        // Step 2: resolve pane_id to a tmux session name. Shells out to
        // `tmux display-message`. Also no lock held — this can block.
        let Some(tmux_name) = self.tmux.pane_to_session_name(&env.pane_id) else {
            tracing::debug!(
                session_id,
                pid,
                pane_id = %env.pane_id,
                "env probe found TMUX_PANE but tmux display-message failed to resolve"
            );
            return false;
        };

        // Step 3: write ownership under the sessions write lock. Idempotent
        // check short-circuits if ownership is already set to the same
        // tmux name (common when startup Pass B re-runs over sessions that
        // handle_session_birth already covered).
        let mut sessions = self.sessions.write().await;
        let Some(session) = sessions.get_mut(session_id) else {
            return false;
        };

        if let Some(existing) = session
            .ownership
            .as_ref()
            .and_then(|o| o.tmux.as_ref())
            .filter(|t| t.cli_session_id == tmux_name)
        {
            tracing::trace!(
                session_id,
                tmux = %existing.cli_session_id,
                "tmux ownership already bound — short-circuit"
            );
            return true;
        }

        let mut ownership = session.ownership.clone().unwrap_or_default();
        ownership.tmux = Some(claude_view_types::TmuxBinding {
            cli_session_id: tmux_name.clone(),
        });
        session.ownership = Some(ownership);

        let snapshot = session.clone();
        drop(sessions);

        let _ = self
            .tx
            .send(SessionEvent::SessionUpsert { session: snapshot });

        tracing::info!(
            session_id,
            tmux = %tmux_name,
            pid,
            pane_id = %env.pane_id,
            "Bound tmux ownership via env probe"
        );

        true
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

            // PID reuse guard: verify this PID still belongs to this session.
            // After a crash, the OS may have recycled the PID for an unrelated process.
            if !is_pid_still_claude(entry.pid, session_id) {
                info!(
                    session_id = %session_id,
                    pid = entry.pid,
                    "PID file mismatch or missing — PID may have been recycled, discarding"
                );
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

        // Recover controlled sessions via sidecar (one-shot at boot only).
        // Runtime recovery is lazy, per-session, via
        // `ensure_session_control_alive` — never autonomous.
        if !sessions_to_recover.is_empty() {
            if let Some(ref sidecar) = self.sidecar {
                match sidecar.ensure_running("boot").await {
                    Ok(_) => {
                        let gen = sidecar.generation();
                        let recovered = sidecar
                            .recover_controlled_sessions(&sessions_to_recover)
                            .await;
                        for (sid, new_ctrl_id) in &recovered {
                            self.bind_control(sid, new_ctrl_id.clone(), gen, None).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_file_matches_session_exact_match() {
        assert!(pid_file_matches_session(
            r#"{"sessionId":"abc-123","pid":12345}"#,
            "abc-123"
        ));
    }

    #[test]
    fn test_pid_file_matches_session_wrong_id() {
        assert!(!pid_file_matches_session(
            r#"{"sessionId":"different","pid":12345}"#,
            "abc-123"
        ));
    }

    #[test]
    fn test_pid_file_matches_session_invalid_json() {
        assert!(!pid_file_matches_session("not json", "abc-123"));
    }

    #[test]
    fn test_pid_file_matches_session_missing_field() {
        assert!(!pid_file_matches_session(r#"{"pid":12345}"#, "abc-123"));
    }

    #[test]
    fn test_pid_file_matches_session_null_session_id() {
        assert!(!pid_file_matches_session(
            r#"{"sessionId":null}"#,
            "abc-123"
        ));
    }

    #[test]
    fn test_pid_file_matches_session_numeric_session_id() {
        assert!(!pid_file_matches_session(r#"{"sessionId":42}"#, "abc-123"));
    }
}
