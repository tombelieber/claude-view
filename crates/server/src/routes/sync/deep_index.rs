//! Deep index rebuild trigger handler.

use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use crate::error::{ApiError, ApiResult};
use crate::metrics::record_sync;
use crate::state::AppState;

use super::mutex::get_deep_index_mutex;
use super::types::{SyncAcceptedResponse, SyncStatus};

/// POST /api/sync/deep - Trigger a full deep index rebuild.
///
/// This endpoint:
/// 1. Marks all sessions for re-indexing (clears deep_indexed_at)
/// 2. Runs Pass 2 deep indexing on all sessions
///
/// Returns:
/// - 202 Accepted: Deep index rebuild started
/// - 409 Conflict: A rebuild is already in progress
///
/// The rebuild runs in the background. Poll /api/status or /api/indexing/progress for completion.
/// POST /api/sync/deep-index — Trigger a full deep index rebuild.
#[utoipa::path(post, path = "/api/sync/deep-index", tag = "sync",
    responses(
        (status = 202, description = "Deep index rebuild started", body = crate::routes::sync::SyncAcceptedResponse),
        (status = 409, description = "Deep index already running"),
    )
)]
pub async fn trigger_deep_index(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let mutex = get_deep_index_mutex();

    match mutex.try_lock() {
        Ok(guard) => {
            let db = state.db.clone();
            let indexing = state.indexing.clone();
            let search_holder = state.search_index.clone();
            let registry_holder = state.registry.clone();

            // Reset indexing state BEFORE spawning so SSE clients that
            // connect after receiving the 202 never see stale `Done` from
            // a previous run.
            indexing.set_indexed(0);
            indexing.set_total(0);
            indexing.set_status(crate::indexing_state::IndexingStatus::ReadingIndexes);

            tokio::spawn(async move {
                // Hold the mutex guard for the entire duration of the rebuild.
                let _guard = guard;
                let start = Instant::now();

                tracing::info!("Deep index rebuild triggered via API");

                // Step 1: Mark all sessions for re-indexing
                match db.mark_all_sessions_for_reindex().await {
                    Ok(count) => {
                        tracing::info!(sessions_marked = count, "Marked sessions for re-indexing");
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "Failed to mark sessions for re-indexing"
                        );
                        indexing.set_error(format!("Failed to mark sessions: {e}"));
                        return;
                    }
                }

                // Transition to deep indexing phase
                indexing.set_status(crate::indexing_state::IndexingStatus::DeepIndexing);

                // Step 2: Resolve claude dir + build hints, then run unified scan
                let claude_dir = match dirs::home_dir() {
                    Some(home) => home.join(".claude"),
                    None => {
                        indexing.set_error("Could not determine home directory".to_string());
                        return;
                    }
                };
                let hints = claude_view_db::indexer_parallel::build_index_hints(&claude_dir);

                let indexing_cb = indexing.clone();
                let indexing_total = indexing.clone();
                let indexing_finalize = indexing.clone();
                let search_for_scan = search_holder.read().unwrap().clone();
                let registry_for_scan = registry_holder
                    .read()
                    .unwrap()
                    .as_ref()
                    .map(|r| std::sync::Arc::new(r.clone()));
                let result = claude_view_db::indexer_parallel::scan_and_index_all(
                    &claude_dir,
                    &db,
                    &hints,
                    search_for_scan,
                    registry_for_scan,
                    move |_session_id| {
                        indexing_cb.increment_indexed();
                    },
                    move |file_count| {
                        indexing_total.set_total(file_count);
                        indexing_total.set_sessions_found(file_count);
                    },
                    move || {
                        indexing_finalize
                            .set_status(crate::indexing_state::IndexingStatus::Finalizing);
                    },
                )
                .await;

                match result {
                    Ok((indexed_count, _skipped)) => {
                        let duration = start.elapsed();
                        tracing::info!(
                            sessions_indexed = indexed_count,
                            duration_secs = duration.as_secs_f64(),
                            "Deep index rebuild complete"
                        );
                        indexing.set_status(crate::indexing_state::IndexingStatus::Done);
                        // Persist index metadata so Settings > Data Status shows real values
                        let duration_ms = duration.as_millis() as i64;
                        let project_count = db.get_project_count().await.unwrap_or(0);
                        if let Err(e) = db
                            .update_index_metadata_on_success(
                                duration_ms,
                                indexed_count as i64,
                                project_count,
                            )
                            .await
                        {
                            tracing::warn!(error = %e, "Failed to persist index metadata after rebuild");
                        }
                        // Record sync metrics
                        record_sync("deep", duration, Some(indexed_count as u64));
                        // Prune sessions whose JSONL files no longer exist on disk
                        if let Err(e) =
                            claude_view_db::indexer_parallel::prune_stale_sessions(&db).await
                        {
                            tracing::warn!(error = %e, "Failed to prune stale sessions after rebuild");
                        }
                    }
                    Err(e) => {
                        let duration = start.elapsed();
                        tracing::error!(
                            error = %e,
                            duration_secs = duration.as_secs_f64(),
                            "Deep index rebuild failed"
                        );
                        indexing.set_error(format!("Deep index failed: {e}"));
                        // Still record duration for failed rebuilds
                        record_sync("deep", duration, None);
                    }
                }
            });

            let response = SyncAcceptedResponse {
                message: "Deep index rebuild initiated".to_string(),
                status: SyncStatus::Accepted,
            };

            Ok((StatusCode::ACCEPTED, Json(response)).into_response())
        }
        Err(_) => Err(ApiError::Conflict(
            "Deep index rebuild already in progress. Please wait for it to complete.".to_string(),
        )),
    }
}
