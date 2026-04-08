// crates/db/src/indexer_parallel/orchestrator/mod.rs
// 3-phase startup scan: parse (parallel) -> SQLite write (chunked) -> search index.

mod discovery;
mod phase_parse;
mod phase_search;
mod phase_write;

use claude_view_core::Registry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::Database;

use super::types::IndexHints;

/// 3-phase startup scan: parse (parallel) -> SQLite write (chunked) -> search index.
///
/// Phase 1: Parse all changed JSONL files in parallel (CPU-bound, zero DB writes).
/// Phase 2: Write sessions, turns, models, invocations in chunked transactions.
/// Phase 3: Write search index to Tantivy (after SQLite success).
///
/// Returns (indexed_count, skipped_count).
///
/// `on_file_done` fires for **every** `.jsonl` file (parsed or skipped) so
/// callers can drive a progress counter that reaches 100%.
///
/// `on_total_known` fires once with the actual `.jsonl` file count right
/// after the filesystem walk, before any parsing begins. This is the single
/// source of truth for "total sessions to process" -- callers should use it
/// to set their progress total instead of guessing from external sources.
pub async fn scan_and_index_all<F, T, W>(
    claude_dir: &Path,
    db: &Database,
    hints: &HashMap<String, IndexHints>,
    search_index: Option<Arc<claude_view_search::SearchIndex>>,
    registry: Option<Arc<Registry>>,
    on_file_done: F,
    on_total_known: T,
    on_finalize_start: W,
) -> Result<(usize, usize), String>
where
    F: Fn(&str) + Send + Sync + 'static,
    T: FnOnce(usize),
    W: FnOnce(),
{
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Ok((0, 0));
    }

    // When the search index was rebuilt (schema version mismatch), force re-parse
    // of ALL sessions so search_messages get regenerated and fed to Tantivy.
    let force_search_reindex = search_index
        .as_ref()
        .map(|idx| idx.needs_full_reindex)
        .unwrap_or(false);

    if force_search_reindex {
        tracing::info!(
            "Search index was rebuilt -- forcing full re-parse to repopulate search data"
        );
    }
    let source_docs_validation_enabled = search_index.is_some();

    // Discover all .jsonl files
    let files = discovery::discover_jsonl_files(&projects_dir)?;

    // Report actual file count -- single source of truth for progress total.
    on_total_known(files.len());

    // Pre-load all existing session staleness info from DB in one query.
    let existing_sessions = db
        .get_sessions_needing_deep_index()
        .await
        .map_err(|e| format!("Failed to load existing sessions: {}", e))?;
    let existing_map: HashMap<String, (Option<i64>, Option<i64>, i32, Option<String>)> =
        existing_sessions
            .into_iter()
            .map(
                |(id, _fp, stored_size, stored_mtime, _deep_at, pv, _proj, archived_at)| {
                    (id, (stored_size, stored_mtime, pv, archived_at))
                },
            )
            .collect();

    let on_file_done = Arc::new(on_file_done);

    // Phase 1: PARSE (parallel, CPU-bound, zero I/O writes)
    let (indexed_sessions, skipped_count) = phase_parse::run_phase_parse(
        files,
        hints,
        &existing_map,
        registry,
        force_search_reindex,
        source_docs_validation_enabled,
        on_file_done.clone(),
    )
    .await?;

    if indexed_sessions.is_empty() {
        return Ok((0, skipped_count));
    }

    let total_search_bytes: usize = indexed_sessions
        .iter()
        .map(|s| {
            s.search_messages
                .iter()
                .map(|m| m.content.len())
                .sum::<usize>()
        })
        .sum();
    tracing::info!(
        sessions = indexed_sessions.len(),
        search_bytes = total_search_bytes,
        "Phase 1 parse complete, starting Phase 2 SQLite write"
    );

    // Aggregate integrity counters + create index run record
    let integrity = phase_search::aggregate_integrity(&indexed_sessions);

    let index_run_start = std::time::Instant::now();
    let index_run_id = match db.create_index_run("full", None, Some(&integrity)).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to create index run");
            None
        }
    };

    // Phase 2: SQLITE WRITE (sequential, chunked, single writer)
    let seen_at = chrono::Utc::now().timestamp();
    let indexed_count =
        phase_write::run_phase_write(db, &indexed_sessions, seen_at, &on_file_done).await?;

    tracing::info!(
        indexed = indexed_count,
        "Phase 2 SQLite write complete, starting Phase 3 search index"
    );

    on_finalize_start();

    // Phase 3: SEARCH INDEX (sequential, after SQLite success)
    if let Some(ref search) = search_index {
        phase_search::run_phase_search(&indexed_sessions, search, force_search_reindex);
    }

    // Persist index run completion with aggregated integrity counters
    phase_search::finalize_index_run(
        db,
        &indexed_sessions,
        indexed_count,
        index_run_id,
        index_run_start,
        &integrity,
    )
    .await;

    Ok((indexed_count, skipped_count))
}
