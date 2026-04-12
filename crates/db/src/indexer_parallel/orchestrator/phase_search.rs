// crates/db/src/indexer_parallel/orchestrator/phase_search.rs
// Phase 3: Tantivy search index writes + index run bookkeeping.

use std::sync::Arc;

use crate::Database;

use super::super::types::IndexedSession;

/// Write session search documents to the Tantivy search index.
///
/// Called after Phase 2 SQLite writes succeed. Commits the index and
/// reloads the reader on success.
pub(crate) fn run_phase_search(
    indexed_sessions: &[IndexedSession],
    search_index: &Arc<claude_view_search::SearchIndex>,
    force_search_reindex: bool,
) {
    let mut search_errors = 0u32;
    let mut sessions_indexed = 0u32;

    for session in indexed_sessions {
        if session.search_messages.is_empty() {
            continue;
        }

        let docs: Vec<claude_view_search::SearchDocument> = session
            .search_messages
            .iter()
            .enumerate()
            .map(|(i, msg)| claude_view_search::SearchDocument {
                session_id: session.parsed.id.clone(),
                project: session.project_for_search.clone(),
                branch: session.parsed.git_branch.clone().unwrap_or_default(),
                model: session.parsed.primary_model.clone().unwrap_or_default(),
                role: msg.role.clone(),
                content: msg.content.clone(),
                turn_number: (i + 1) as u64,
                timestamp: msg.timestamp.unwrap_or(0),
                skills: serde_json::from_str(&session.parsed.skills_used).unwrap_or_default(),
            })
            .collect();

        if let Err(e) = search_index.index_session(&session.parsed.id, &docs) {
            tracing::warn!(session_id = %session.parsed.id, error = %e, "Failed to index session for search");
            search_errors += 1;
        }
        sessions_indexed += 1;
    }

    if sessions_indexed > 0 {
        if let Err(e) = search_index.commit() {
            tracing::warn!(error = %e, "Failed to commit search index");
        } else {
            if let Err(e) = search_index.reader.reload() {
                tracing::warn!(error = %e, "Failed to reload search reader after commit");
            }
            if search_errors > 0 {
                tracing::info!(
                    indexed = sessions_indexed,
                    errors = search_errors,
                    "Search index write complete (with errors)"
                );
            }
            if force_search_reindex {
                search_index.mark_schema_synced();
            }

            // Release the 50 MB bulk writer heap now that indexing is done.
            // Subsequent live updates will lazily create a smaller 5 MB writer.
            if let Err(e) = search_index.release_writer() {
                tracing::warn!(error = %e, "Failed to release search index writer");
            }
        }
    }
}

/// Aggregate per-session `ParseDiagnostics` into `IndexRunIntegrityCounters`.
pub(crate) fn aggregate_integrity(
    indexed_sessions: &[IndexedSession],
) -> crate::IndexRunIntegrityCounters {
    let mut integrity = crate::IndexRunIntegrityCounters::default();
    for session in indexed_sessions {
        let d = &session.diagnostics;
        integrity.unknown_top_level_type_count += d.lines_unknown_type as i64;
        integrity.dropped_line_invalid_json_count += d.json_parse_failures as i64;
        integrity.unknown_source_role_count += d.unknown_source_role_count as i64;
        integrity.derived_source_message_doc_count += d.derived_source_message_doc_count as i64;
        integrity.source_message_non_source_provenance_count +=
            d.source_message_non_source_provenance_count as i64;
    }
    integrity
}

/// Persist the index run completion record with timing and integrity data.
pub(crate) async fn finalize_index_run(
    db: &Database,
    indexed_sessions: &[IndexedSession],
    indexed_count: usize,
    index_run_id: Option<i64>,
    index_run_start: std::time::Instant,
    integrity: &crate::IndexRunIntegrityCounters,
) {
    if let Some(run_id) = index_run_id {
        let index_run_duration_ms = index_run_start.elapsed().as_millis() as i64;
        let total_bytes: u64 = indexed_sessions
            .iter()
            .map(|s| s.diagnostics.bytes_total)
            .sum();
        let throughput = if index_run_duration_ms > 0 {
            Some(total_bytes as f64 / (1024.0 * 1024.0) / (index_run_duration_ms as f64 / 1000.0))
        } else {
            None
        };
        if let Err(e) = db
            .complete_index_run(
                run_id,
                Some(indexed_count as i64),
                index_run_duration_ms,
                throughput,
                Some(integrity),
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to complete index run");
        }
    }
}
