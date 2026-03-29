//! File watcher: initial scan, JSONL tail parsing, and file event processing.
//!
//! `spawn_file_watcher` performs the startup recovery sequence (eager process scan,
//! initial JSONL scan, snapshot promotion, closed session restoration) then enters
//! the main file event loop processing Modified/Removed/Rescan events.
//!
//! `process_jsonl_update` is the core JSONL processing logic for a single session file.

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use claude_view_core::pricing::TokenUsage;

use claude_view_db::indexer_parallel::{build_index_hints, scan_and_index_all};

use crate::live::mutation::types::{LifecycleEvent, SessionMutation};
use crate::live::process::count_claude_processes;
use crate::live::state::{
    append_capped_hook_event, HookEvent, SessionEvent, MAX_HOOK_EVENTS_PER_SESSION,
};
use crate::live::watcher::{initial_scan, start_watcher, FileEvent};

use super::accumulator::{
    apply_jsonl_metadata, build_metadata_from_accumulator, SessionAccumulator,
};
use super::helpers::{extract_project_info, extract_session_id};
use super::LiveSessionManager;

impl LiveSessionManager {
    /// Run a one-shot process count scan (display metric only).
    pub(super) async fn run_eager_process_scan(&self) {
        let oracle_snap = self.oracle_rx.borrow().clone();
        let total_count = match oracle_snap.claude_processes.as_ref() {
            Some(cp) => cp.count,
            None => tokio::task::spawn_blocking(count_claude_processes)
                .await
                .unwrap_or_default(),
        };
        self.process_count
            .store(total_count, std::sync::atomic::Ordering::Relaxed);
        info!("Process scan: {} Claude processes", total_count);
    }

    /// Spawn the file watcher background task.
    ///
    /// 1. Performs an initial scan of `~/.claude/projects/` for recent JSONL files.
    /// 2. Starts a notify watcher for ongoing file changes.
    /// 3. Processes each Modified/Removed event by parsing new JSONL lines.
    pub(super) fn spawn_file_watcher(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            // --- Startup recovery sequence ---
            // 1. Eager process scan FIRST
            manager.run_eager_process_scan().await;

            // 2. Initial JSONL scan
            let projects_dir = match dirs::home_dir() {
                Some(home) => home.join(".claude").join("projects"),
                None => {
                    warn!("Could not determine home directory; skipping initial scan");
                    return;
                }
            };

            let initial_paths = {
                let dir = projects_dir.clone();
                tokio::task::spawn_blocking(move || initial_scan(&dir))
                    .await
                    .unwrap_or_default()
            };

            info!(
                "Initial scan found {} recent JSONL files",
                initial_paths.len()
            );

            // Warm up accumulators
            for path in &initial_paths {
                manager.process_jsonl_update(path).await;
            }

            // 3. Promote sessions from crash-recovery snapshot
            manager.promote_from_snapshot(&initial_paths).await;

            // 4. Load recently-closed sessions from SQLite
            manager.restore_closed_sessions(&initial_paths).await;

            // Start the file system watcher
            let (file_tx, mut file_rx) = mpsc::channel::<FileEvent>(512);
            let (_watcher, dropped_events) = match start_watcher(file_tx) {
                Ok((w, d)) => (w, d),
                Err(e) => {
                    error!("Failed to start file watcher: {}", e);
                    return;
                }
            };

            let mut last_catchup_count = 0u64;

            // Process file events forever
            while let Some(event) = file_rx.recv().await {
                // Check if drops occurred since last check
                let current_drops = dropped_events.load(std::sync::atomic::Ordering::Relaxed);
                if current_drops > last_catchup_count {
                    last_catchup_count = current_drops;
                    info!(
                        dropped_total = current_drops,
                        "Detected dropped watcher events -- triggering catch-up scan"
                    );
                    let catchup_paths = {
                        let dir = projects_dir.clone();
                        tokio::task::spawn_blocking(move || initial_scan(&dir))
                            .await
                            .unwrap_or_default()
                    };
                    for path in &catchup_paths {
                        manager.process_jsonl_update(path).await;
                    }
                }
                match event {
                    FileEvent::Modified(path) => {
                        let session_id = extract_session_id(&path);
                        manager.process_jsonl_update(&path).await;

                        let sessions = manager.sessions.read().await;
                        if let Some(session) = sessions.get(&session_id) {
                            let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                session: session.clone(),
                            });
                        }
                    }
                    FileEvent::Removed(path) => {
                        manager.handle_file_removed(&path).await;
                    }
                    FileEvent::Rescan => {
                        manager.handle_rescan().await;
                    }
                }
            }
        });
    }

    /// Handle a JSONL file removal event.
    async fn handle_file_removed(self: &Arc<Self>, path: &Path) {
        let session_id = extract_session_id(path);
        let (should_close, already_closed) = {
            let sessions = self.sessions.read().await;
            match sessions.get(&session_id) {
                Some(session) if session.closed_at.is_some() => (false, true),
                Some(_) => (true, false),
                None => (false, false),
            }
        };

        if already_closed {
            tracing::debug!(
                session_id = %session_id,
                "JSONL file removed for recently-closed session -- keeping in map"
            );
        } else if should_close {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let ctx = self.mutation_context(self);
            self.coordinator
                .handle(
                    &ctx,
                    &session_id,
                    SessionMutation::Lifecycle(LifecycleEvent::End { reason: None }),
                    None,
                    now,
                    None,
                    None,
                    None,
                )
                .await;
            let db = self.db.clone();
            let sid = session_id.clone();
            tokio::spawn(async move {
                let _ = sqlx::query(
                    "UPDATE sessions SET closed_at = ?1 WHERE id = ?2 AND closed_at IS NULL",
                )
                .bind(now)
                .bind(&sid)
                .execute(db.pool())
                .await;
            });
        }
    }

    /// Handle a rescan event (overflow detected by watcher).
    async fn handle_rescan(self: &Arc<Self>) {
        tracing::info!("Overflow detected -- triggering full reconciliation scan");
        let Some(home) = dirs::home_dir() else {
            tracing::warn!("HOME not set, skipping rescan");
            return;
        };
        let claude_dir = home.join(".claude");
        let hints = build_index_hints(&claude_dir);
        let search_for_rescan = self.search_index.read().unwrap().clone();
        let registry_for_rescan = self
            .registry
            .read()
            .unwrap()
            .as_ref()
            .map(|r| Arc::new(r.clone()));
        let (indexed, _) = scan_and_index_all(
            &claude_dir,
            &self.db,
            &hints,
            search_for_rescan,
            registry_for_rescan,
            |_| {},
            |_| {},
            || {},
        )
        .await
        .unwrap_or((0, 0));
        if indexed > 0 {
            tracing::info!(
                indexed,
                "Reconciliation scan complete -- resyncing live state"
            );
            let recent_paths = initial_scan(&claude_dir);
            for path in &recent_paths {
                self.process_jsonl_update(path).await;
            }
        }
    }

    // promote_from_snapshot, dedup_snapshot_pids, restore_closed_sessions
    // are in startup.rs

    /// Core JSONL processing logic for a single session file.
    ///
    /// 1. Extracts session ID and project info from the file path.
    /// 2. Calls `parse_tail` from the stored offset to read only new lines.
    /// 3. Accumulates token counts and user turn counts.
    /// 4. Derives session status, activity, and cost.
    /// 5. Updates the shared session map.
    pub(super) async fn process_jsonl_update(&self, path: &Path) {
        let session_id = extract_session_id(path);
        let cached_cwd = {
            let accumulators = self.accumulators.read().await;
            accumulators
                .get(&session_id)
                .and_then(|a| a.resolved_cwd.clone())
        };
        let (project, project_display_name, project_path, resolved_cwd) =
            extract_project_info(path, cached_cwd.as_deref());

        let current_offset = {
            let accumulators = self.accumulators.read().await;
            accumulators.get(&session_id).map(|a| a.offset).unwrap_or(0)
        };

        // Parse new lines from the JSONL file (blocking I/O)
        let finders = self.finders.clone();
        let path_owned = path.to_path_buf();
        let parse_result = tokio::task::spawn_blocking(move || {
            claude_view_core::live_parser::parse_tail(&path_owned, current_offset, &finders)
        })
        .await;

        let (new_lines, new_offset) = match parse_result {
            Ok(Ok((lines, offset))) => (lines, offset),
            Ok(Err(e)) => {
                tracing::debug!("Failed to parse tail for {}: {}", session_id, e);
                return;
            }
            Err(e) => {
                error!("spawn_blocking panicked for {}: {}", session_id, e);
                return;
            }
        };

        if new_lines.is_empty() && current_offset > 0 {
            return;
        }

        let last_activity_at = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64
            });

        // Update accumulator with new lines
        let mut accumulators = self.accumulators.write().await;
        let acc = accumulators
            .entry(session_id.clone())
            .or_insert_with(SessionAccumulator::new);

        acc.offset = new_offset;
        acc.file_path = Some(path.to_path_buf());
        acc.project_path = Some(project_path.clone());
        if acc.resolved_cwd.is_none() {
            acc.resolved_cwd = resolved_cwd;
        }

        // Detect file replacement: offset rollback
        if new_offset > 0 && new_offset < current_offset {
            tracing::info!(
                session_id = %session_id,
                old_offset = current_offset,
                new_offset = new_offset,
                "File replaced -- clearing task progress for clean re-accumulation"
            );
            acc.task_items.clear();
            acc.mcp_servers.clear();
            acc.skills.clear();
            acc.at_files.clear();
            acc.pasted_paths.clear();
            acc.tokens = TokenUsage::default();
            acc.tool_counts_edit = 0;
            acc.tool_counts_read = 0;
            acc.tool_counts_bash = 0;
            acc.tool_counts_write = 0;
            acc.compact_count = 0;
            acc.accumulated_cost = claude_view_core::pricing::CostBreakdown::default();
            acc.seen_api_calls.clear();
            acc.phase_labels.clear();
            acc.message_buf.clear();
            acc.message_buf_dirty = false;
            acc.message_buf_total = 0;
            acc.stabilizer.reset();
        }

        let mut channel_a_events: Vec<HookEvent> = Vec::new();

        for line in &new_lines {
            self.process_single_line(
                line,
                acc,
                last_activity_at,
                &session_id,
                &mut channel_a_events,
            );
        }

        // Build metadata from accumulator
        let metadata = build_metadata_from_accumulator(acc, last_activity_at, None);

        // Persist partial state to DB (fire-and-forget)
        let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
        if let Err(e) = self
            .db
            .update_session_from_tail(
                &session_id,
                acc.user_turn_count as i32 + acc.tokens.total_tokens.min(1) as i32,
                acc.user_turn_count as i32,
                last_activity_at,
                &acc.last_user_message,
                file_size,
                file_size,
                last_activity_at,
                acc.tokens.input_tokens as i64,
                acc.tokens.output_tokens as i64,
                acc.tokens.cache_read_tokens as i64,
                acc.tokens.cache_creation_tokens as i64,
                acc.tool_counts_edit as i32,
                acc.tool_counts_read as i32,
                acc.tool_counts_bash as i32,
                acc.tool_counts_write as i32,
            )
            .await
        {
            tracing::warn!(session_id = %session_id, error = %e, "Failed to update session from tail");
        }

        let file_path_str = path.to_str().unwrap_or("").to_string();

        // Drop accumulators lock before acquiring sessions lock
        drop(accumulators);

        // Self-dedup Channel A events
        if !channel_a_events.is_empty() {
            channel_a_events.sort_by(|a, b| {
                a.timestamp
                    .cmp(&b.timestamp)
                    .then(a.event_name.cmp(&b.event_name))
                    .then(a.tool_name.cmp(&b.tool_name))
                    .then(a.source.cmp(&b.source))
            });
            channel_a_events.dedup_by(|a, b| {
                a.event_name == b.event_name
                    && a.timestamp == b.timestamp
                    && a.tool_name == b.tool_name
                    && a.source == b.source
            });
        }

        // Update the shared session map
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            if session.closed_at.is_some() {
                return;
            }
            apply_jsonl_metadata(
                session,
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

            // Apply Channel A events to LiveSession
            if !channel_a_events.is_empty() {
                for event in channel_a_events {
                    append_capped_hook_event(
                        &mut session.hook.hook_events,
                        event,
                        MAX_HOOK_EVENTS_PER_SESSION,
                    );
                }
            }
        }
    }

    // process_single_line is in line_processor.rs
}
