//! Dashboard statistics endpoint.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use vibe_recall_core::DashboardStats;

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/stats/dashboard - Pre-computed dashboard statistics.
pub async fn dashboard_stats(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<DashboardStats>> {
    let stats = state.db.get_dashboard_stats().await?;
    Ok(Json(stats))
}

/// Create the stats routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/stats/dashboard", get(dashboard_stats))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_core::{SessionInfo, ToolCounts};
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
    async fn test_dashboard_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 0);
        assert_eq!(json["totalProjects"], 0);
        assert!(json["heatmap"].is_array());
        assert!(json["topSkills"].is_array());
        assert!(json["topProjects"].is_array());
        assert!(json["toolTotals"].is_object());
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            file_path: "/path/sess-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec!["/commit".to_string()],
            tool_counts: ToolCounts { edit: 5, read: 10, bash: 3, write: 2 },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
        };
        db.insert_session(&session, "project-a", "Project A").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);
        assert_eq!(json["totalProjects"], 1);
        assert!(!json["heatmap"].as_array().unwrap().is_empty());
    }
}
