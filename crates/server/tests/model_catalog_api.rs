//! Integration test: GET /api/models returns new catalog fields.

use axum::body::Body;
use claude_view_db::{Database, LiteLlmModelContext};
use tower::ServiceExt;

#[tokio::test]
async fn api_models_includes_catalog_fields() {
    let db = Database::new_in_memory().await.unwrap();

    // Populate via both sources
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Claude Opus 4.6".into()),
        Some("Most capable for complex work".into()),
    )])
    .await
    .unwrap();

    db.upsert_litellm_context(&[LiteLlmModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(1_000_000),
        max_output_tokens: Some(32_000),
    }])
    .await
    .unwrap();

    let app = claude_view_server::create_app(db);

    let req = axum::http::Request::builder()
        .uri("/api/models")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), 200);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let models: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    let opus = models
        .iter()
        .find(|m| m["id"] == "claude-opus-4-6")
        .unwrap();
    assert_eq!(opus["displayName"], "Claude Opus 4.6");
    assert_eq!(opus["description"], "Most capable for complex work");
    assert_eq!(opus["maxInputTokens"], 1_000_000);
    assert_eq!(opus["maxOutputTokens"], 32_000);
}

#[tokio::test]
async fn api_models_legacy_model_has_null_metadata() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert a legacy model with no catalog data
    sqlx::query(
        "INSERT OR REPLACE INTO models (id, provider, family, first_seen, last_seen) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("claude-3-opus-20240229")
    .bind("anthropic")
    .bind("opus")
    .bind(1700000000_i64)
    .bind(1700000000_i64)
    .execute(db.pool())
    .await
    .unwrap();

    let app = claude_view_server::create_app(db);

    let req = axum::http::Request::builder()
        .uri("/api/models")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), 200);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let models: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    let legacy = models
        .iter()
        .find(|m| m["id"] == "claude-3-opus-20240229")
        .unwrap();
    assert!(legacy["displayName"].is_null());
    assert!(legacy["maxInputTokens"].is_null());
    assert_eq!(legacy["provider"], "anthropic");
}
