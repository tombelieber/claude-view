//! Basic HTTP integration tests for the stateless relay.
//!
//! The full WS message-routing scenarios live in `stateless_ws_test.rs`.
//! This file only exercises the `/health` endpoint (and any future HTTP
//! routes the relay grows). The pre-Phase-1 `/pair`, `/pair/claim`, and
//! auth-flow tests were deleted when pairing.rs was deleted.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use claude_view_relay::device_cache::DeviceCache;
use claude_view_relay::rate_limit::RateLimiter;
use claude_view_relay::state::RelayState;
use claude_view_relay::supabase::{MockSupabaseClient, SupabaseClient};
use dashmap::DashMap;
use tower::ServiceExt;

fn test_state() -> RelayState {
    let mock = Arc::new(MockSupabaseClient::default());
    let device_cache = Arc::new(DeviceCache::new(
        mock as Arc<dyn SupabaseClient>,
        Duration::from_secs(60),
    ));

    RelayState {
        connections: Arc::new(DashMap::new()),
        supabase_auth: None,
        http: reqwest::Client::new(),
        device_cache,
        onesignal_app_id: None,
        onesignal_rest_api_key: None,
        onesignal_http: None,
        posthog_api_key: None,
        posthog_http: None,
        ws_rate_limiter: Arc::new(RateLimiter::new(100.0, 100.0)),
        push_rate_limiter: Arc::new(RateLimiter::new(100.0, 100.0)),
    }
}

async fn request(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);

    let body = if let Some(json) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(serde_json::to_string(&json).unwrap())
    } else {
        Body::empty()
    };

    let response = app.oneshot(builder.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn health_check() {
    let state = test_state();
    let app = claude_view_relay::app(state);
    let (status, body) = request(app, "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn push_tokens_endpoint_accepts_valid_request() {
    // OneSignal not configured — handler returns 200 with ok=true (no-op).
    let state = test_state();
    let app = claude_view_relay::app(state);
    let (status, body) = request(
        app,
        "POST",
        "/push-tokens",
        Some(serde_json::json!({
            "device_id": "phone-push-001",
            "onesignal_player_id": "player-abc-123",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["ok"], true);
}

#[tokio::test]
async fn push_tokens_empty_device_id_returns_400() {
    let state = test_state();
    let app = claude_view_relay::app(state);
    let (status, _) = request(
        app,
        "POST",
        "/push-tokens",
        Some(serde_json::json!({
            "device_id": "",
            "onesignal_player_id": "player-abc-123",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
