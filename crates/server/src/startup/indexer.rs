//! Indexer task spawner — registry build, scan_and_index_all, backup ingest,
//! prompt-history indexing, and the periodic re-scan loop.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Lifecycle, ordering, and
//! telemetry events are unchanged from the pre-split runtime.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use claude_view_db::indexer_parallel::{build_index_hints, scan_and_index_all};
use claude_view_db::Database;

use crate::record_sync;
use crate::startup::background::{run_git_sync_logged, run_snapshot_generation};
use crate::telemetry::TelemetryClient;
use crate::{
    IndexingState, IndexingStatus, PromptIndexHolder, PromptStatsHolder, PromptTemplatesHolder,
    SearchIndexHolder,
};

/// All the shared state the indexer task needs. Bundled into a struct so
/// the spawn signature stays readable.
pub struct IndexerDeps {
    pub db: Database,
    pub claude_dir: PathBuf,
    pub indexing: Arc<IndexingState>,
    pub registry_holder: Arc<RwLock<Option<claude_view_core::registry::Registry>>>,
    pub search_holder: SearchIndexHolder,
    pub prompt_index_holder: PromptIndexHolder,
    pub prompt_stats_holder: PromptStatsHolder,
    pub prompt_templates_holder: PromptTemplatesHolder,
    pub telemetry: Option<TelemetryClient>,
}

/// Spawn the background indexer task.
///
/// ORDERING: must be called BEFORE [`spawn_search_rebuild_if_pending`]. The
/// rebuild polls `registry_holder`, which is only populated inside this
/// task's closure after Pass 1 completes.
pub fn spawn_indexer_task(deps: IndexerDeps) {
    let IndexerDeps {
        db: idx_db,
        claude_dir,
        indexing: idx_state,
        registry_holder: idx_registry,
        search_holder: idx_search,
        prompt_index_holder: idx_prompt_index,
        prompt_stats_holder: idx_prompt_stats,
        prompt_templates_holder: idx_prompt_templates,
        telemetry: idx_telemetry,
    } = deps;

    tokio::spawn(async move {
        idx_state.set_status(IndexingStatus::ReadingIndexes);
        let index_start = Instant::now();

        // 1. Build hints from sessions-index.json (no DB writes, sync function)
        let hints = build_index_hints(&claude_dir);
        let hint_count = hints.len();
        idx_state.set_sessions_found(hint_count);
        // Count unique projects from hints for the "ready" SSE event
        let unique_projects: std::collections::HashSet<&str> = hints
            .values()
            .filter_map(|h| h.project_display_name.as_deref())
            .collect();
        idx_state.set_projects_found(unique_projects.len());

        // 2. Build registry
        let registry = claude_view_core::build_registry(&claude_dir).await;

        // 2b. Seed invocables into DB so invocations can reference them (FK constraint)
        let invocable_tuples: Vec<(String, Option<String>, String, String, String)> = registry
            .all_invocables()
            .map(|info| {
                (
                    info.id.clone(),
                    info.plugin_name.clone(),
                    info.name.clone(),
                    info.kind.to_string(),
                    info.description.clone(),
                )
            })
            .collect();
        if !invocable_tuples.is_empty() {
            if let Err(e) = idx_db.batch_upsert_invocables(&invocable_tuples).await {
                tracing::warn!(error = %e, "Failed to seed invocables");
            }
        }

        // 2c. Auto-reindex: compare combined fingerprint with stored hash.
        //
        // Combined = registry state (sessions/plugins/skills) + computation
        // versions that affect stored aggregates. Bumping any version tag
        // mismatches the stored hash and triggers a full reindex, keeping
        // DB aggregates (e.g. sessions.total_cost_usd) in sync with the
        // latest pricing/extraction logic. See pricing::PRICING_VERSION.
        //
        // Format: "{registry_fp}:pv{N}" — self-describing, trivially
        // extensible (add `:ev{M}` etc. for future version tags).
        let registry_fp = registry.fingerprint();
        let new_hash = format!(
            "{}:pv{}",
            registry_fp,
            claude_view_core::pricing::PRICING_VERSION
        );
        match idx_db.get_registry_hash().await {
            Ok(Some(stored)) if stored == new_hash => {
                tracing::debug!("Registry unchanged (hash={new_hash}), skipping full re-index");
            }
            Ok(stored) => {
                let reason = if stored.is_none() {
                    "first run"
                } else {
                    "registry changed"
                };
                tracing::info!(
                    "Registry hash mismatch ({reason}), marking all sessions for re-index"
                );
                match idx_db.mark_all_sessions_for_reindex().await {
                    Ok(n) => tracing::info!("Marked {n} sessions for re-index"),
                    Err(e) => tracing::warn!("Failed to mark sessions for re-index: {e}"),
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read registry hash: {e}, skipping auto-reindex check");
            }
        }

        // Store registry in shared holder for API routes and keep an Arc for indexing
        let registry_arc = Arc::new(registry);
        *idx_registry.write().unwrap() = Some((*registry_arc).clone());

        // Extract search index Arc from holder (clone Arc, don't hold lock during scan)
        let search_for_scan = idx_search.read().unwrap().clone();

        // 3. Single-pass scan: parse + upsert for each changed file
        idx_state.set_status(IndexingStatus::DeepIndexing);
        let state_for_progress = idx_state.clone();
        let state_for_total = idx_state.clone();
        let state_for_finalize = idx_state.clone();
        match scan_and_index_all(
            &claude_dir,
            &idx_db,
            &hints,
            search_for_scan,
            Some(registry_arc.clone()),
            move |_session_id| {
                state_for_progress.increment_indexed();
            },
            move |file_count| {
                state_for_total.set_total(file_count);
                // Filesystem .jsonl count is source of truth for session count.
                // sessions-index.json hints only cover ~24% of project dirs.
                state_for_total.set_sessions_found(file_count);
            },
            move || {
                state_for_finalize.set_status(IndexingStatus::Finalizing);
            },
        )
        .await
        {
            Ok((indexed, skipped)) => {
                tracing::info!(
                    indexed,
                    skipped,
                    elapsed_ms = index_start.elapsed().as_millis() as u64,
                    "Startup scan complete"
                );

                // Ingest backup sessions from ~/.claude-backup (optimistic, best-effort).
                // Runs after primary scan so dedup check against DB is accurate.
                {
                    let search_for_backup = idx_search.read().unwrap().clone();
                    let (backup_imported, backup_skipped) =
                        claude_view_db::indexer_parallel::ingest_backup_sessions(
                            &idx_db,
                            search_for_backup,
                        )
                        .await;
                    if backup_imported > 0 {
                        tracing::info!(
                            backup_imported,
                            backup_skipped,
                            "Backup sessions imported from ~/.claude-backup"
                        );
                    }
                }

                // Signal Done immediately — search index is ready.
                // Post-scan cleanup below is housekeeping, not indexing.
                idx_state.set_status(IndexingStatus::Done);

                // Persist index metadata so Settings > Data Status shows real values.
                let duration_ms = index_start.elapsed().as_millis() as i64;
                let sessions = idx_db.get_session_count().await.unwrap_or(0);
                let projects = idx_db.get_project_count().await.unwrap_or(0);
                if let Err(e) = idx_db
                    .update_index_metadata_on_success(duration_ms, sessions, projects)
                    .await
                {
                    tracing::warn!(error = %e, "Failed to persist index metadata");
                }

                // Fire telemetry events for first_index_completed and sessions_milestone
                if let Some(ref client) = idx_telemetry {
                    let telemetry_config_path =
                        claude_view_core::telemetry_config::telemetry_config_path();
                    let mut config = claude_view_core::telemetry_config::read_telemetry_config(
                        &telemetry_config_path,
                    );

                    // first_index_completed — fires exactly once per install
                    if !config.first_index_completed {
                        config.first_index_completed = true;
                        let _ = claude_view_core::telemetry_config::write_telemetry_config(
                            &telemetry_config_path,
                            &config,
                        );
                        client.track(
                            "first_index_completed",
                            serde_json::json!({
                                "session_count": sessions,
                                "version": env!("CARGO_PKG_VERSION"),
                            }),
                        );
                    }

                    // sessions_milestone — fires each time a new milestone is crossed
                    if let Some(milestone) = claude_view_core::telemetry_config::check_milestone(
                        sessions as u64,
                        config.last_milestone.unwrap_or(0),
                    ) {
                        client.track(
                            "sessions_milestone",
                            serde_json::json!({
                                "milestone": milestone,
                                "session_count": sessions,
                            }),
                        );
                        config.last_milestone = Some(milestone);
                        let _ = claude_view_core::telemetry_config::write_telemetry_config(
                            &telemetry_config_path,
                            &config,
                        );
                    }
                }

                // 4. Post-scan cleanup
                // Prune DB rows for JSONL files that no longer exist on disk.
                match claude_view_db::indexer_parallel::prune_stale_sessions(&idx_db).await {
                    Ok(n) if n > 0 => tracing::info!("Pruned {} stale sessions from DB", n),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Failed to prune stale sessions: {}", e),
                }
                // Persist registry fingerprint so next startup can detect changes.
                if let Err(e) = idx_db.set_registry_hash(&new_hash).await {
                    tracing::warn!("Failed to persist registry hash: {e}");
                }

                // 5. Post-index tasks
                run_git_sync_logged(&idx_db, "initial").await;
                run_snapshot_generation(&idx_db, "initial").await;

                // 6. Prompt History Indexing
                index_prompt_history(&idx_prompt_index, &idx_prompt_stats, &idx_prompt_templates)
                    .await;

                // 7. Periodic sync loop: re-scan changed sessions, git-sync, snapshots.
                // No more two-pass polling — the watcher handles incremental updates.
                // This loop handles periodic git-sync and snapshot generation only,
                // plus a lightweight re-scan for any files the watcher might have missed.
                loop {
                    let interval_secs = idx_db.get_git_sync_interval().await.unwrap_or(120);
                    let sync_interval = Duration::from_secs(interval_secs);
                    tokio::time::sleep(sync_interval).await;

                    // Lightweight re-scan: picks up any files the watcher missed
                    let hints = build_index_hints(&claude_dir);
                    let rescan_start = Instant::now();
                    let search_rescan = idx_search.read().unwrap().clone();
                    match scan_and_index_all(
                        &claude_dir,
                        &idx_db,
                        &hints,
                        search_rescan,
                        Some(registry_arc.clone()),
                        |_| {},
                        |_| {},
                        || {},
                    )
                    .await
                    {
                        Ok((indexed, _)) => {
                            if indexed > 0 {
                                tracing::info!(indexed, "Periodic re-scan indexed new sessions");
                                record_sync(
                                    "periodic-rescan",
                                    rescan_start.elapsed(),
                                    Some(indexed as u64),
                                );
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "Periodic re-scan failed (non-fatal)"),
                    }

                    run_git_sync_logged(&idx_db, "periodic").await;
                    run_snapshot_generation(&idx_db, "periodic").await;
                }
            }
            Err(e) => {
                idx_state.set_error(e);
            }
        }
    });
}

/// Build the Tantivy prompt-history index from `~/.claude/history.jsonl`.
/// No-op when the history file does not exist.
async fn index_prompt_history(
    prompt_index_holder: &PromptIndexHolder,
    prompt_stats_holder: &PromptStatsHolder,
    prompt_templates_holder: &PromptTemplatesHolder,
) {
    let ph_start = std::time::Instant::now();
    let history_path = dirs::home_dir()
        .expect("home dir")
        .join(".claude")
        .join("history.jsonl");

    if !history_path.exists() {
        tracing::info!("~/.claude/history.jsonl not found, skipping prompt indexing");
        return;
    }

    let entries = match claude_view_core::prompt_history::parse_history(&history_path).await {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse prompt history");
            return;
        }
    };

    tracing::info!(count = entries.len(), "parsed prompt history");

    // Compute stats
    let stats = claude_view_core::prompt_history::compute_stats(&entries);
    *prompt_stats_holder.write().unwrap() = Some(stats);

    // Compute templates
    let prompt_strs: Vec<&str> = entries.iter().map(|e| e.display.as_str()).collect();
    let templates = claude_view_core::prompt_templates::detect_templates(&prompt_strs, 3);
    *prompt_templates_holder.write().unwrap() = Some(templates);

    // Build Tantivy index
    let index_path = claude_view_core::paths::prompt_index_dir();
    let index = match claude_view_search::prompt_index::PromptSearchIndex::open(&index_path) {
        Ok(index) => index,
        Err(e) => {
            tracing::error!(error = %e, "failed to open prompt index");
            return;
        }
    };

    let documents: Vec<claude_view_search::prompt_index::PromptDocument> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| claude_view_search::prompt_index::PromptDocument {
            prompt_id: format!("{}-{}", e.timestamp_ms, i),
            display: e.display.clone(),
            paste_text: e.paste_text(),
            project: e.project_display_name().to_string(),
            session_id: e.session_id.clone(),
            branch: String::new(),
            model: String::new(),
            git_root: e.project.clone(),
            intent: claude_view_core::prompt_history::classify_intent(&e.display).to_string(),
            complexity: claude_view_core::prompt_history::complexity_bucket(&e.display).to_string(),
            timestamp: e.timestamp_secs(),
            has_paste: e.pasted_contents.as_ref().is_some_and(|p| !p.is_empty()),
        })
        .collect();

    if let Err(e) = index.index_prompts(&documents) {
        tracing::error!(error = %e, "failed to index prompts");
        return;
    }
    if let Err(e) = index.commit() {
        tracing::error!(error = %e, "failed to commit prompt index");
        return;
    }
    index.mark_schema_synced();
    index.release_writer().ok();
    *prompt_index_holder.write().unwrap() = Some(Arc::new(index));
    tracing::info!(
        count = documents.len(),
        elapsed_ms = ph_start.elapsed().as_millis() as u64,
        "prompt history indexed"
    );
}

/// Schedule a background search-index rebuild when a schema bump is pending.
///
/// ORDERING: must be called AFTER [`spawn_indexer_task`]. The rebuild task
/// polls `registry_holder`, which is only populated inside the indexer's
/// closure — calling this first would hang the rebuild for
/// `REGISTRY_WAIT_TIMEOUT` (60s) before aborting.
pub fn spawn_search_rebuild_if_pending(
    pending: Option<claude_view_search::migration::PendingMigration>,
    deps: &IndexerDeps,
) {
    let Some(plan) = pending else {
        return;
    };
    tracing::info!(
        target_version = plan.target_version,
        fallback_path = ?plan.old_version_path,
        "scheduling background search index rebuild (blue-green migration)"
    );
    let rebuild_hints = build_index_hints(&deps.claude_dir);
    crate::search_migration::spawn_background_rebuild(
        plan,
        deps.search_holder.clone(),
        deps.claude_dir.clone(),
        deps.db.clone(),
        rebuild_hints,
        deps.registry_holder.clone(),
    );
}
