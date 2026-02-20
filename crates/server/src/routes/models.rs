//! Model tracking endpoints.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use claude_view_db::ModelWithStats;

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/models - List all observed models with usage counts.
pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ModelWithStats>>> {
    let models = state.db.get_all_models().await?;
    Ok(Json(models))
}

/// Create the models routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/models", get(list_models))
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

    async fn get(app: axum::Router, uri: &str) -> (StatusCode, String) {
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
    async fn test_models_endpoint_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = get(app, "/api/models").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let items = json.as_array().expect("response should be an array");
        assert_eq!(items.len(), 0, "Empty DB should return empty array");
    }
}
