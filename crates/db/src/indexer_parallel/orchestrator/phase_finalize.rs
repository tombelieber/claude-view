// crates/db/src/indexer_parallel/orchestrator/phase_finalize.rs
// Index run bookkeeping for the startup scan.

use crate::Database;

use super::super::types::IndexedSession;

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
#[tracing::instrument(skip_all)]
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
