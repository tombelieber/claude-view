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

#[tokio::test]
async fn api_models_sdk_supported_flag_in_response() {
    let db = Database::new_in_memory().await.unwrap();

    // SDK upsert: marks opus as supported
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Claude Opus 4.6".into()),
        None,
    )])
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

    // SDK-upserted model must have sdkSupported: true
    let opus = models
        .iter()
        .find(|m| m["id"] == "claude-opus-4-6")
        .unwrap();
    assert_eq!(
        opus["sdkSupported"], true,
        "SDK-upserted model must be sdkSupported"
    );

    // Pricing-only models (from pricing map merge) must have sdkSupported: false
    let non_sdk: Vec<_> = models
        .iter()
        .filter(|m| m["id"] != "claude-opus-4-6" && m["sdkSupported"] == false)
        .collect();
    assert!(
        !non_sdk.is_empty(),
        "pricing-only models must have sdkSupported: false"
    );
}

#[tokio::test]
async fn api_models_sdk_supported_reset_on_second_upsert() {
    let db = Database::new_in_memory().await.unwrap();

    // First: both opus and sonnet are SDK-supported
    db.upsert_sdk_models(&[
        (
            "claude-opus-4-6".into(),
            "anthropic".into(),
            "opus".into(),
            Some("Claude Opus 4.6".into()),
            None,
        ),
        (
            "claude-sonnet-4-6".into(),
            "anthropic".into(),
            "sonnet".into(),
            Some("Claude Sonnet 4.6".into()),
            None,
        ),
    ])
    .await
    .unwrap();

    // Second: SDK now only reports sonnet (opus removed)
    db.upsert_sdk_models(&[(
        "claude-sonnet-4-6".into(),
        "anthropic".into(),
        "sonnet".into(),
        Some("Claude Sonnet 4.6".into()),
        None,
    )])
    .await
    .unwrap();

    let app = claude_view_server::create_app(db);

    let req = axum::http::Request::builder()
        .uri("/api/models")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let models: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    let opus = models
        .iter()
        .find(|m| m["id"] == "claude-opus-4-6")
        .unwrap();
    assert_eq!(
        opus["sdkSupported"], false,
        "opus must lose sdkSupported when SDK stops reporting it"
    );

    let sonnet = models
        .iter()
        .find(|m| m["id"] == "claude-sonnet-4-6")
        .unwrap();
    assert_eq!(
        sonnet["sdkSupported"], true,
        "sonnet must keep sdkSupported"
    );
}

/// Regression: SDK upsert with NULL display_name must produce null in API response.
/// The frontend falls back to formatModelName(id) → "Claude Opus 4.6".
/// Previously COALESCE preserved stale alias names like "Default (recommended)".
#[tokio::test]
async fn api_models_null_display_name_when_sdk_passes_none() {
    let db = Database::new_in_memory().await.unwrap();

    // SDK upsert with None for display_name
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        None, // intentionally NULL
        Some("Most capable for complex work".into()),
    )])
    .await
    .unwrap();

    let app = claude_view_server::create_app(db);
    let req = axum::http::Request::builder()
        .uri("/api/models")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let models: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    let opus = models
        .iter()
        .find(|m| m["id"] == "claude-opus-4-6")
        .unwrap();
    assert!(
        opus["displayName"].is_null(),
        "displayName must be null so frontend uses formatModelName(). Got: {}",
        opus["displayName"]
    );
    assert_eq!(opus["description"], "Most capable for complex work");
    assert_eq!(opus["sdkSupported"], true);
}
