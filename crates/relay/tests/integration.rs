use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use claude_view_relay::rate_limit::RateLimiter;
use tower::ServiceExt;

fn test_state() -> claude_view_relay::state::RelayState {
    claude_view_relay::state::RelayState::new(
        None,
        Arc::new(RateLimiter::new(100.0, 100.0)),
        Arc::new(RateLimiter::new(100.0, 100.0)),
        Arc::new(RateLimiter::new(100.0, 100.0)),
    )
}

/// Helper to make a request to the app.
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
async fn pair_creates_offer() {
    let state = test_state();
    let app = claude_view_relay::app(state.clone());

    use base64::{engine::general_purpose::STANDARD, Engine};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let pubkey_bytes = signing_key.verifying_key().to_bytes();

    let (status, _) = request(
        app,
        "POST",
        "/pair",
        Some(serde_json::json!({
            "device_id": "mac-test-001",
            "pubkey": STANDARD.encode(pubkey_bytes),
            "one_time_token": "test-token-123",
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(state.pairing_offers.contains_key("test-token-123"));
    assert!(state.devices.contains_key("mac-test-001"));
}

#[tokio::test]
async fn claim_consumes_token() {
    let state = test_state();

    use base64::{engine::general_purpose::STANDARD, Engine};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    // Mac creates offer
    let mac_key = SigningKey::generate(&mut OsRng);
    let app = claude_view_relay::app(state.clone());
    let (status, _) = request(
        app,
        "POST",
        "/pair",
        Some(serde_json::json!({
            "device_id": "mac-test-001",
            "pubkey": STANDARD.encode(mac_key.verifying_key().to_bytes()),
            "one_time_token": "claim-token-123",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Phone claims
    let phone_key = SigningKey::generate(&mut OsRng);
    let app = claude_view_relay::app(state.clone());
    let (status, _) = request(
        app,
        "POST",
        "/pair/claim",
        Some(serde_json::json!({
            "one_time_token": "claim-token-123",
            "device_id": "phone-test-001",
            "pubkey": STANDARD.encode(phone_key.verifying_key().to_bytes()),
            "pubkey_encrypted_blob": "encrypted-x25519-pubkey-placeholder",
            "x25519_pubkey": "dGVzdC14MjU1MTktcHVia2V5LXBsYWNlaG9sZGVy",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Token consumed
    assert!(!state.pairing_offers.contains_key("claim-token-123"));
    // Devices are paired
    assert!(state
        .devices
        .get("mac-test-001")
        .unwrap()
        .paired_devices
        .contains("phone-test-001"));
    assert!(state
        .devices
        .get("phone-test-001")
        .unwrap()
        .paired_devices
        .contains("mac-test-001"));
}

#[tokio::test]
async fn claim_expired_token_returns_gone() {
    let state = test_state();

    // Insert an expired offer directly
    state.pairing_offers.insert(
        "expired-token".into(),
        claude_view_relay::state::PairingOffer {
            device_id: "mac-old".into(),
            pubkey: vec![0u8; 32],
            created_at: std::time::Instant::now() - std::time::Duration::from_secs(600),
        },
    );

    let app = claude_view_relay::app(state);

    use base64::{engine::general_purpose::STANDARD, Engine};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    let phone_key = SigningKey::generate(&mut OsRng);

    let (status, _) = request(
        app,
        "POST",
        "/pair/claim",
        Some(serde_json::json!({
            "one_time_token": "expired-token",
            "device_id": "phone-late",
            "pubkey": STANDARD.encode(phone_key.verifying_key().to_bytes()),
            "pubkey_encrypted_blob": "doesnt-matter",
            "x25519_pubkey": "dGVzdC14MjU1MTktcHVia2V5LXBsYWNlaG9sZGVy",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
}

#[tokio::test]
async fn claim_nonexistent_token_returns_404() {
    let state = test_state();
    let app = claude_view_relay::app(state);

    use base64::{engine::general_purpose::STANDARD, Engine};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    let phone_key = SigningKey::generate(&mut OsRng);

    let (status, _) = request(
        app,
        "POST",
        "/pair/claim",
        Some(serde_json::json!({
            "one_time_token": "nonexistent",
            "device_id": "phone-lost",
            "pubkey": STANDARD.encode(phone_key.verifying_key().to_bytes()),
            "pubkey_encrypted_blob": "doesnt-matter",
            "x25519_pubkey": "dGVzdC14MjU1MTktcHVia2V5LXBsYWNlaG9sZGVy",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn push_token_registers_ok() {
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
async fn push_token_empty_device_id_returns_400() {
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

#[tokio::test]
async fn push_token_rate_limit_returns_429() {
    // burst=1 means only one token is available; rate=0 means no refill.
    // First request consumes the single token (200), second finds none (429).
    let state = claude_view_relay::state::RelayState::new(
        None,
        Arc::new(RateLimiter::new(100.0, 100.0)),
        Arc::new(RateLimiter::new(100.0, 100.0)),
        Arc::new(RateLimiter::new(0.0, 1.0)),
    );

    let body = serde_json::json!({
        "device_id": "phone-ratelimit-001",
        "onesignal_player_id": "player-xyz-999",
    });

    let (status_first, _) = request(
        claude_view_relay::app(state.clone()),
        "POST",
        "/push-tokens",
        Some(body.clone()),
    )
    .await;
    assert_eq!(status_first, StatusCode::OK);

    let (status_second, _) = request(
        claude_view_relay::app(state.clone()),
        "POST",
        "/push-tokens",
        Some(body),
    )
    .await;
    assert_eq!(status_second, StatusCode::TOO_MANY_REQUESTS);
}
