//! Status endpoint for index metadata and data freshness.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use vibe_recall_db::trends::IndexMetadata;

use crate::error::ApiResult;
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

/// Create the status routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/status", get(get_status))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
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
}
