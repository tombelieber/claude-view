// crates/db/src/indexer_parallel/orchestrator/mod.rs
// Startup scan: parse (parallel) -> SQLite write (chunked) -> finalization.

mod discovery;
mod phase_finalize;
mod phase_parse;
mod phase_write;

use claude_view_core::Registry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::Database;

use super::types::IndexHints;

/// Startup scan: parse (parallel) -> SQLite write (chunked) -> finalization.
///
/// Phase 1: Parse all changed JSONL files in parallel (CPU-bound, zero DB writes).
/// Phase 2: Write sessions, turns, models, invocations in chunked transactions.
/// Phase 3: Finalize index-run bookkeeping.
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
#[tracing::instrument(skip_all)]
pub async fn scan_and_index_all<F, T, W>(
    claude_dir: &Path,
    db: &Database,
    hints: &HashMap<String, IndexHints>,
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
        false,
        on_file_done.clone(),
    )
    .await?;

    if indexed_sessions.is_empty() {
        return Ok((0, skipped_count));
    }

    tracing::info!(
        sessions = indexed_sessions.len(),
        "Phase 1 parse complete, starting Phase 2 SQLite write"
    );

    // Aggregate integrity counters + create index run record
    let integrity = phase_finalize::aggregate_integrity(&indexed_sessions);

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
        "Phase 2 SQLite write complete, starting finalization"
    );

    on_finalize_start();

    // Persist index run completion with aggregated integrity counters
    phase_finalize::finalize_index_run(
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
