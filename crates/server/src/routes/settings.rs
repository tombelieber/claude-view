//! App settings API routes.

use std::sync::Arc;

use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{error::ApiError, state::AppState};
use claude_view_db::AppSettings;

/// Allowed model values for Claude CLI.
const VALID_MODELS: &[&str] = &["haiku", "sonnet", "opus"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSettingsRequest {
    llm_model: Option<String>,
    llm_timeout_secs: Option<i64>,
}

/// GET /api/settings - Read current app settings.
async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AppSettings>, ApiError> {
    let settings = state
        .db
        .get_app_settings()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read settings: {e}")))?;
    Ok(Json(settings))
}

/// PUT /api/settings - Update app settings (partial).
async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateSettingsRequest>,
) -> Result<Json<AppSettings>, ApiError> {
    // Validate model if provided
    if let Some(ref m) = body.llm_model {
        if !VALID_MODELS.contains(&m.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "Invalid model '{}'. Valid options: {}",
                m,
                VALID_MODELS.join(", ")
            )));
        }
    }

    // Validate timeout if provided
    if let Some(t) = body.llm_timeout_secs {
        if t < 10 || t > 300 {
            return Err(ApiError::BadRequest(
                "Timeout must be between 10 and 300 seconds".to_string(),
            ));
        }
    }

    let settings = state
        .db
        .update_app_settings(body.llm_model.as_deref(), body.llm_timeout_secs)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update settings: {e}")))?;

    Ok(Json(settings))
}

/// Create the settings routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/settings", get(get_settings).put(update_settings))
}

/// Create an LLM provider from the user's persisted settings.
///
/// All LLM callsites MUST use this instead of hardcoding a model.
pub async fn create_llm_provider(
    db: &claude_view_db::Database,
) -> Result<claude_view_core::llm::ClaudeCliProvider, ApiError> {
    let settings = db.get_app_settings().await
        .map_err(|e| ApiError::Internal(format!("Failed to read LLM settings: {e}")))?;
    Ok(claude_view_core::llm::ClaudeCliProvider::new(&settings.llm_model)
        .with_timeout(settings.llm_timeout_secs.clamp(10, 300) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    async fn test_app() -> Router {
        let db = claude_view_db::Database::new_in_memory()
            .await
            .expect("in-memory DB");
        let state = AppState::new(db);
        Router::new()
            .nest("/api", router())
            .with_state(state)
    }

    #[tokio::test]
    async fn test_get_settings_returns_defaults() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let settings: AppSettings = serde_json::from_slice(&body).unwrap();
        assert_eq!(settings.llm_model, "haiku");
        assert_eq!(settings.llm_timeout_secs, 120);
    }

    #[tokio::test]
    async fn test_update_model() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"llmModel":"sonnet"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let settings: AppSettings = serde_json::from_slice(&body).unwrap();
        assert_eq!(settings.llm_model, "sonnet");
        assert_eq!(settings.llm_timeout_secs, 120);
    }

    #[tokio::test]
    async fn test_update_timeout() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"llmTimeoutSecs":60}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let settings: AppSettings = serde_json::from_slice(&body).unwrap();
        assert_eq!(settings.llm_model, "haiku");
        assert_eq!(settings.llm_timeout_secs, 60);
    }

    #[tokio::test]
    async fn test_update_both() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"llmModel":"opus","llmTimeoutSecs":180}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let settings: AppSettings = serde_json::from_slice(&body).unwrap();
        assert_eq!(settings.llm_model, "opus");
        assert_eq!(settings.llm_timeout_secs, 180);
    }

    #[tokio::test]
    async fn test_invalid_model_rejected() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"llmModel":"gpt-4"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_timeout_too_low_rejected() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"llmTimeoutSecs":5}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_timeout_too_high_rejected() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"llmTimeoutSecs":999}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_boundary_timeout_10_accepted() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"llmTimeoutSecs":10}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_boundary_timeout_300_accepted() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"llmTimeoutSecs":300}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }
}
