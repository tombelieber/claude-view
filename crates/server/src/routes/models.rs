//! Model tracking endpoints.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use claude_view_db::ModelWithStats;

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/models - List all known models with usage counts.
///
/// Merges two sources:
/// 1. User's session history (from DB `models` table) — models actually used
/// 2. Pricing map keys (from LiteLLM + hardcoded defaults) — all known Claude models
///
/// Models from the pricing map that the user hasn't used appear with zero usage stats.
pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ModelWithStats>>> {
    let mut models = state.db.get_all_models().await?;
    let existing_ids: HashSet<String> = models.iter().map(|m| m.id.clone()).collect();

    // Merge pricing map keys that aren't in the DB yet
    let extras: Vec<ModelWithStats> = {
        let pricing = state.pricing.read().expect("pricing lock poisoned");
        pricing
            .keys()
            .filter(|id| !existing_ids.contains(id.as_str()))
            .map(|model_id| {
                let (provider, family) = claude_view_core::parse_model_id(model_id);
                ModelWithStats {
                    id: model_id.clone(),
                    provider: Some(provider.to_string()),
                    family: Some(family.to_string()),
                    display_name: None,
                    description: None,
                    max_input_tokens: None,
                    max_output_tokens: None,
                    first_seen: None,
                    last_seen: None,
                    total_turns: 0,
                    total_sessions: 0,
                }
            })
            .collect()
    };
    models.extend(extras);

    // Sort: used models first (by total_turns desc), then unused alphabetically
    models.sort_by(|a, b| {
        b.total_turns
            .cmp(&a.total_turns)
            .then_with(|| a.id.cmp(&b.id))
    });

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
    use claude_view_db::Database;
    use tower::ServiceExt;

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
    async fn test_models_endpoint_includes_pricing_models() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = get(app, "/api/models").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let items = json.as_array().expect("response should be an array");
        // Even with empty DB, pricing defaults should populate the list
        assert!(
            !items.is_empty(),
            "Should include models from pricing defaults"
        );
        // Verify claude-opus-4-6 is present (from hardcoded defaults)
        let ids: Vec<&str> = items
            .iter()
            .filter_map(|i| i.get("id").and_then(|v| v.as_str()))
            .collect();
        assert!(
            ids.contains(&"claude-opus-4-6"),
            "Should contain claude-opus-4-6 from defaults"
        );
    }
}
