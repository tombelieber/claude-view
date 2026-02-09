//! Sync endpoints for triggering git commit scanning and deep index rebuilds.

use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Serialize;
use tokio::sync::Mutex;
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::metrics::record_sync;
use crate::state::AppState;

/// Global mutex to prevent concurrent git syncs.
/// Uses a lazy static pattern via std::sync::OnceLock.
static GIT_SYNC_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

/// Global mutex to prevent concurrent deep index rebuilds.
static DEEP_INDEX_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

fn get_sync_mutex() -> &'static Mutex<()> {
    GIT_SYNC_MUTEX.get_or_init(|| Mutex::new(()))
}

fn get_deep_index_mutex() -> &'static Mutex<()> {
    DEEP_INDEX_MUTEX.get_or_init(|| Mutex::new(()))
}

/// Status value for accepted sync responses.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Accepted,
}

/// Response for successful sync initiation.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SyncAcceptedResponse {
    pub message: String,
    pub status: SyncStatus,
}

/// POST /api/sync/git - Trigger git commit scanning (A8.5).
///
/// Returns:
/// - 202 Accepted: Sync started (no sync was running)
/// - 409 Conflict: Sync already in progress
///
/// The sync runs in the background. Poll /api/status for completion.
pub async fn trigger_git_sync(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Response> {
    let mutex = get_sync_mutex();

    match mutex.try_lock() {
        Ok(guard) => {
            let db = state.db.clone();
            tokio::spawn(async move {
                // Hold the mutex guard for the entire duration of the sync.
                let _guard = guard;
                let start = Instant::now();

                tracing::info!("Git sync triggered via API");
                match vibe_recall_db::git_correlation::run_git_sync(&db).await {
                    Ok(result) => {
                        let duration = start.elapsed();
                        tracing::info!(
                            repos_scanned = result.repos_scanned,
                            commits_found = result.commits_found,
                            links_created = result.links_created,
                            errors = result.errors.len(),
                            duration_secs = duration.as_secs_f64(),
                            "Git sync complete"
                        );
                        // Record sync metrics
                        record_sync("git", duration, Some(result.commits_found as u64));
                    }
                    Err(e) => {
                        let duration = start.elapsed();
                        tracing::error!(
                            error = %e,
                            duration_secs = duration.as_secs_f64(),
                            "Git sync failed"
                        );
                        // Still record duration for failed syncs
                        record_sync("git", duration, None);
                    }
                }
            });

            let response = SyncAcceptedResponse {
                message: "Git sync initiated".to_string(),
                status: SyncStatus::Accepted,
            };

            Ok((StatusCode::ACCEPTED, Json(response)).into_response())
        }
        Err(_) => {
            Err(ApiError::Conflict(
                "Git sync already in progress. Please wait for it to complete.".to_string(),
            ))
        }
    }
}

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
pub async fn trigger_deep_index(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Response> {
    let mutex = get_deep_index_mutex();

    match mutex.try_lock() {
        Ok(guard) => {
            let db = state.db.clone();
            let indexing = state.indexing.clone();

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
                        tracing::info!(
                            sessions_marked = count,
                            "Marked sessions for re-indexing"
                        );
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

                // Step 2: Run deep indexing pass with progress wired to IndexingState
                let indexing_cb = indexing.clone();
                let result = vibe_recall_db::indexer_parallel::pass_2_deep_index(
                    &db,
                    None, // No registry needed for rebuild
                    move |indexed, total| {
                        indexing_cb.set_total(total);
                        indexing_cb.set_indexed(indexed);
                    },
                )
                .await;

                match result {
                    Ok(indexed_count) => {
                        let duration = start.elapsed();
                        tracing::info!(
                            sessions_indexed = indexed_count,
                            duration_secs = duration.as_secs_f64(),
                            "Deep index rebuild complete"
                        );
                        indexing.set_status(crate::indexing_state::IndexingStatus::Done);
                        // Record sync metrics
                        record_sync("deep", duration, Some(indexed_count as u64));
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
        Err(_) => {
            Err(ApiError::Conflict(
                "Deep index rebuild already in progress. Please wait for it to complete.".to_string(),
            ))
        }
    }
}

/// Create the sync routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sync/git", post(trigger_git_sync))
        .route("/sync/deep", post(trigger_deep_index))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_post(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn test_sync_git_accepted() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_post(app, "/api/sync/git").await;

        assert_eq!(status, StatusCode::ACCEPTED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "accepted");
        assert!(json["message"].as_str().unwrap().contains("initiated"));
    }

    #[tokio::test]
    async fn test_sync_deep_accepted() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_post(app, "/api/sync/deep").await;

        assert_eq!(status, StatusCode::ACCEPTED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "accepted");
        assert!(json["message"].as_str().unwrap().contains("Deep index"));
    }

    // Note: Testing the 409 Conflict case requires holding the mutex during the test,
    // which is tricky with the current design. In a real implementation, we would
    // have a more sophisticated sync state management that allows better testing.
}
