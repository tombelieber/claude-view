//! Trends endpoint for week-over-week metrics.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use claude_view_db::trends::WeekTrends;

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/trends - Get week-over-week trend metrics.
///
/// Returns trends for:
/// - Session count
/// - Total tokens
/// - Avg tokens per prompt
/// - Total files edited
/// - Avg re-edit rate
/// - Commit link count
pub async fn get_trends(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<WeekTrends>> {
    let trends = state.db.get_week_trends().await?;
    Ok(Json(trends))
}

/// Create the trends routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/trends", get(get_trends))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use claude_view_db::Database;

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
    async fn test_trends_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/trends").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All metrics should be present with 0/0 values
        assert!(json["sessionCount"].is_object());
        assert!(json["totalTokens"].is_object());
        assert!(json["avgTokensPerPrompt"].is_object());
        assert!(json["totalFilesEdited"].is_object());
        assert!(json["avgReeditRate"].is_object());
        assert!(json["commitLinkCount"].is_object());

        // Verify structure of a metric
        assert_eq!(json["sessionCount"]["current"], 0);
        assert_eq!(json["sessionCount"]["previous"], 0);
        assert_eq!(json["sessionCount"]["delta"], 0);
    }
}
