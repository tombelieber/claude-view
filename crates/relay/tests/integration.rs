use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

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

    let response = app
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn health_check() {
    let state = claude_view_relay::state::RelayState::new();
    let app = claude_view_relay::app(state);

    let (status, body) = request(app, "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn pair_creates_offer() {
    let state = claude_view_relay::state::RelayState::new();
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
    let state = claude_view_relay::state::RelayState::new();

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
    let state = claude_view_relay::state::RelayState::new();

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
        })),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
}

#[tokio::test]
async fn claim_nonexistent_token_returns_404() {
    let state = claude_view_relay::state::RelayState::new();
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
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
