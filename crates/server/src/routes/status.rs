//! Status endpoint for index metadata and data freshness.
//! Settings endpoints for user-configurable options.

use std::sync::Arc;

use axum::{extract::State, routing::{get, put}, Json, Router};
use serde::Deserialize;
use vibe_recall_db::trends::IndexMetadata;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// GET /api/status - Get index metadata and data freshness info.
///
/// Returns:
/// - last_indexed_at: When indexing last completed
/// - last_index_duration_ms: How long the last index took
/// - sessions_indexed: Number of sessions in last index
/// - projects_indexed: Number of projects in last index
/// - last_git_sync_at: When git sync last completed
/// - commits_found: Commits found in last git sync
/// - links_created: Session-commit links created in last git sync
/// - updated_at: When metadata was last updated
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<IndexMetadata>> {
    let metadata = state.db.get_index_metadata().await?;
    Ok(Json(metadata))
}

/// Request body for updating the git sync interval.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGitSyncIntervalRequest {
    /// Interval in seconds. Must be between 10 and 3600.
    pub interval_secs: u64,
}

/// PUT /api/settings/git-sync-interval - Update the git sync interval.
///
/// Body: { "intervalSecs": 60 }
/// Returns the updated IndexMetadata.
pub async fn update_git_sync_interval(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateGitSyncIntervalRequest>,
) -> ApiResult<Json<IndexMetadata>> {
    // Validate bounds: 10s to 3600s (1 hour)
    if body.interval_secs < 10 || body.interval_secs > 3600 {
        return Err(ApiError::BadRequest(
            "intervalSecs must be between 10 and 3600".to_string(),
        ));
    }

    state.db.set_git_sync_interval(body.interval_secs).await?;
    let metadata = state.db.get_index_metadata().await?;
    Ok(Json(metadata))
}

/// Create the status routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(get_status))
        .route("/settings/git-sync-interval", put(update_git_sync_interval))
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

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn do_put_json(app: axum::Router, uri: &str, json_body: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(json_body.to_string()))
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
    async fn test_status_default() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/status").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Default values
        assert!(json["lastIndexedAt"].is_null());
        assert!(json["lastIndexDurationMs"].is_null());
        assert_eq!(json["sessionsIndexed"], 0);
        assert_eq!(json["projectsIndexed"], 0);
        assert!(json["lastGitSyncAt"].is_null());
        assert_eq!(json["commitsFound"], 0);
        assert_eq!(json["linksCreated"], 0);
        assert!(json["updatedAt"].is_number());
        assert_eq!(json["gitSyncIntervalSecs"], 60); // default
    }

    #[tokio::test]
    async fn test_status_after_index_update() {
        let db = test_db().await;

        // Update index metadata
        db.update_index_metadata_on_success(1500, 100, 5)
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/status").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert!(json["lastIndexedAt"].is_number());
        assert_eq!(json["lastIndexDurationMs"], 1500);
        assert_eq!(json["sessionsIndexed"], 100);
        assert_eq!(json["projectsIndexed"], 5);
    }

    #[tokio::test]
    async fn test_update_git_sync_interval() {
        let db = test_db().await;
        let app = build_app(db);

        let (status, body) = do_put_json(
            app,
            "/api/settings/git-sync-interval",
            r#"{"intervalSecs": 120}"#,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["gitSyncIntervalSecs"], 120);
    }

    #[tokio::test]
    async fn test_update_git_sync_interval_too_low() {
        let db = test_db().await;
        let app = build_app(db);

        let (status, _body) = do_put_json(
            app,
            "/api/settings/git-sync-interval",
            r#"{"intervalSecs": 5}"#,
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_update_git_sync_interval_too_high() {
        let db = test_db().await;
        let app = build_app(db);

        let (status, _body) = do_put_json(
            app,
            "/api/settings/git-sync-interval",
            r#"{"intervalSecs": 7200}"#,
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
