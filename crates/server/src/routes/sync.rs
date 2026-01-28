//! Git sync endpoint for triggering git commit scanning.

use std::sync::Arc;

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
use crate::state::AppState;

/// Global mutex to prevent concurrent git syncs.
/// Uses a lazy static pattern via std::sync::OnceLock.
static GIT_SYNC_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

fn get_sync_mutex() -> &'static Mutex<()> {
    GIT_SYNC_MUTEX.get_or_init(|| Mutex::new(()))
}

/// Response for successful sync initiation.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SyncAcceptedResponse {
    pub message: String,
    pub status: String,
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

                tracing::info!("Git sync triggered via API");
                match vibe_recall_db::git_correlation::run_git_sync(&db).await {
                    Ok(result) => {
                        tracing::info!(
                            "Git sync complete: {} repos, {} commits, {} links, {} errors",
                            result.repos_scanned,
                            result.commits_found,
                            result.links_created,
                            result.errors.len(),
                        );
                    }
                    Err(e) => {
                        tracing::error!("Git sync failed: {}", e);
                    }
                }
            });

            let response = SyncAcceptedResponse {
                message: "Git sync initiated".to_string(),
                status: "accepted".to_string(),
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

/// Create the sync routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/sync/git", post(trigger_git_sync))
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

    // Note: Testing the 409 Conflict case requires holding the mutex during the test,
    // which is tricky with the current design. In a real implementation, we would
    // have a more sophisticated sync state management that allows better testing.
}
