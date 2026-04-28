//! File watcher: initial scan, JSONL tail parsing, and file event processing.
//!
//! `spawn_file_watcher` performs the startup recovery sequence (eager process scan,
//! initial JSONL scan, snapshot promotion, closed session restoration) then enters
//! the main file event loop processing Modified/Removed/Rescan events.
//!
//! `process_jsonl_update` is the core JSONL processing logic for a single session file.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use claude_view_core::pricing::TokenUsage;

use claude_view_db::indexer_parallel::{build_index_hints, scan_and_index_all};
use claude_view_db::indexer_v2::{build_delta_from_file, DeltaSource, StatsDelta};

use crate::live::mutation::types::{LifecycleEvent, SessionMutation};
use crate::live::process::count_claude_processes;
use crate::live::state::{
    append_capped_hook_event, HookEvent, SessionEvent, SessionStatus, MAX_HOOK_EVENTS_PER_SESSION,
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
        let total_count = tokio::task::spawn_blocking(count_claude_processes)
            .await
            .unwrap_or_default();
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

            // 1b. Sessions dir scan — primary lifecycle source (hook-free)
            // Scan ~/.claude/sessions/ for alive sessions BEFORE JSONL/snapshot.
            // This gives us immediate knowledge of who is alive + kind/entrypoint.
            manager.scan_sessions_dir_at_startup().await;

            // 1c. Tmux ownership reconciliation — match existing tmux panes
            // to live sessions by PID and set ownership.tmux. Required because
            // scan_sessions_dir creates sessions via handle_session_birth which
            // bypasses the Born handler's tmux matching. Without this, all tmux
            // sessions lose ownership after server restart.
            manager.reconcile_tmux_ownership().await;

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

                        // Resolve map key via secondary index: tmux sessions
                        // are keyed by tmux name, not Claude UUID.
                        let map_key = manager
                            .claude_session_id_index
                            .read()
                            .await
                            .get(&session_id)
                            .cloned()
                            .unwrap_or_else(|| session_id.clone());

                        let sessions = manager.sessions.read().await;
                        if let Some(session) = sessions.get(&map_key) {
                            let _ = manager.tx.send(SessionEvent::SessionUpsert {
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
        // Resolve map key via secondary index (tmux sessions keyed by tmux name).
        let map_key = self
            .claude_session_id_index
            .read()
            .await
            .get(&session_id)
            .cloned()
            .unwrap_or_else(|| session_id.clone());
        let should_close = {
            let sessions = self.sessions.read().await;
            matches!(
                sessions.get(&map_key),
                Some(session) if session.status != SessionStatus::Done
            )
        };

        if should_close {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let ctx = self.mutation_context(self);
            self.coordinator
                .handle(
                    &ctx,
                    &map_key,
                    SessionMutation::Lifecycle(LifecycleEvent::End {
                        reason: Some("File removed".into()),
                    }),
                    None,
                    now,
                    None,
                    None,
                    None,
                )
                .await;
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

    // promote_from_snapshot, dedup_snapshot_pids are in startup.rs

    /// Core JSONL processing logic for a single session file.
    ///
    /// 1. Extracts session ID and project info from the file path.
    /// 2. Calls `parse_tail` from the stored offset to read only new lines.
    /// 3. Accumulates token counts and user turn counts.
    /// 4. Derives session status, activity, and cost.
    /// 5. Updates the shared session map.
    pub(crate) async fn process_jsonl_update(&self, path: &Path) {
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
        let mut saw_team_delete = false;

        for line in &new_lines {
            if line.tool_names.iter().any(|n| n == "TeamDelete") {
                saw_team_delete = true;
            }
            self.process_single_line(
                line,
                acc,
                last_activity_at,
                &session_id,
                &mut channel_a_events,
            );
        }

        // Snapshot team data before TeamDelete cleanup fires
        if saw_team_delete {
            if let Some(ref team_name) = acc.team_name {
                match crate::teams::snapshot_team(
                    team_name,
                    &session_id,
                    &self.claude_dir,
                    &self.claude_view_dir,
                ) {
                    Ok(()) => tracing::info!(
                        session_id = %session_id,
                        team = %team_name,
                        "Snapshotted team before TeamDelete",
                    ),
                    Err(e) => tracing::warn!(
                        session_id = %session_id,
                        team = %team_name,
                        error = %e,
                        "Failed to snapshot team before TeamDelete",
                    ),
                }
            }
        }

        // Build metadata from accumulator (feeds the in-memory live UI
        // layer; `session_stats` is populated separately via the delta
        // channel below).
        let metadata = build_metadata_from_accumulator(acc, last_activity_at, None);

        // Phase 2.5 — publish the parsed stats to the shared writer
        // channel instead of writing to the legacy `sessions` table.
        // The producer path is:
        //
        //   parse_tail → accumulator update → spawn(build_delta_from_file + try_send)
        //
        // Design constraints enforced here (SOTA §10 / design §4.2):
        //   - Non-blocking: the hot tail loop keeps no lock on the
        //     delta publish; the parse happens on a blocking thread
        //     off-reactor (see `build_delta_from_file`).
        //   - `try_send` only. On `TrySendError::Full` we bump the
        //     `stage_c_producer_drop_total{producer="live_tail"}`
        //     counter; the fsnotify shadow-indexer path (500 ms
        //     debounce) covers the drop.
        //   - No direct write to `sessions` — indexer_v2 owns
        //     `session_stats` exclusively.
        let delta_path = path.to_path_buf();
        let delta_session_id = session_id.clone();
        let delta_tx = self.stats_delta_tx.clone();
        let delta_seq = self.stats_delta_seq.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            publish_live_tail_delta(delta_path, delta_session_id, delta_tx, delta_seq).await;
        });

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

        // Resolve map key via secondary index: tmux sessions are keyed by
        // tmux name (e.g. "cv-abc"), not Claude UUID. The secondary index
        // maps UUID → map key, populated by the Born handler.
        let map_key = self
            .claude_session_id_index
            .read()
            .await
            .get(&session_id)
            .cloned()
            .unwrap_or_else(|| session_id.clone());

        // Update the shared session map
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&map_key) {
            if session.status == SessionStatus::Done {
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

/// Phase 2.5 — parse a live JSONL file into a [`StatsDelta`] and
/// `try_send` it to the shared writer channel.
///
/// Runs off the hot tail loop in its own `tokio::spawn` task so the
/// file I/O + parse never blocks `process_jsonl_update`. Dropping the
/// delta on a full channel is acceptable: the fsnotify shadow indexer
/// re-indexes this session within `DEBOUNCE_MS` anyway, so
/// `session_stats` converges without retry logic here.
///
/// Three error paths, each with the minimum observability:
///   - `build_delta_from_file` failed (e.g. file was rotated out from
///     under us, parse rejected new content) — debug-log and return.
///     Transient conditions resolve on the next fsnotify event.
///   - Channel full — bump `stage_c_producer_drop_total{producer="live_tail"}`.
///     Surfaces overflow in `/metrics` without flooding logs.
///   - Channel closed — log at error. Indicates the consumer task
///     exited, which should never happen outside shutdown.
async fn publish_live_tail_delta(
    path: PathBuf,
    session_id: String,
    tx: mpsc::Sender<StatsDelta>,
    seq: u64,
) {
    use tokio::sync::mpsc::error::TrySendError;

    let delta =
        match build_delta_from_file(path, session_id.clone(), DeltaSource::LiveTail, seq).await {
            Ok(d) => d,
            Err(e) => {
                tracing::debug!(
                    session_id = %session_id,
                    error = %e,
                    "indexer_v2 live-tail: build_delta_from_file failed (transient)"
                );
                return;
            }
        };

    match tx.try_send(delta) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            metrics::counter!(
                "stage_c_producer_drop_total",
                "producer" => DeltaSource::LiveTail.metric_label(),
            )
            .increment(1);
        }
        Err(TrySendError::Closed(_)) => {
            tracing::error!(
                session_id = %session_id,
                "stats_delta channel closed — indexer_v2 consumer exited"
            );
        }
    }
}
