//! Integration tests for /api/devices and /api/pairing/qr proxies.
//!
//! Uses wiremock to mock the Supabase URL. Serial because env vars are
//! process-global.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use claude_view_db::Database;
use claude_view_server::create_app;
use serde_json::json;
use serial_test::serial;
use tempfile::tempdir;
use tower::ServiceExt;
use wiremock::matchers::{method, path as wpath};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn build_app_with_session(
    dir: &tempfile::TempDir,
    supabase_url: &str,
    user_id: &str,
) -> axum::Router {
    std::env::set_var("CLAUDE_VIEW_DATA_DIR", dir.path());
    std::env::set_var("SUPABASE_URL", supabase_url);
    std::env::set_var("SUPABASE_PUBLISHABLE_KEY", "sb_publishable_test");

    // Seed auth-session.json so /api/auth/status reports authed, but tests
    // also prime the AppState holder via POST /api/auth/session below so
    // readers see it without a reload.
    let sess = json!({
        "user_id": user_id,
        "email": "test@example.com",
        "access_token": "at_test",
        "refresh_token": "rft_test",
        "expires_at_unix": 9999999999u64,
    });
    std::fs::write(dir.path().join("auth-session.json"), sess.to_string()).unwrap();

    let db = Database::new_in_memory().await.unwrap();
    let app = create_app(db);

    // Push session into AppState via POST /api/auth/session (create_app uses
    // create_app_with_static which goes through the builder — not
    // create_app_full — so the on-disk session isn't auto-loaded).
    let post = json!({
        "access_token": "at_test",
        "refresh_token": "rft_test",
        "expires_in": 9999999999u64,
        "user_id": user_id,
        "email": "test@example.com"
    });
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/session")
                .header("content-type", "application/json")
                .body(Body::from(post.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    app
}

#[tokio::test]
#[serial]
async fn list_devices_proxies_and_returns_rows() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wpath("/rest/v1/devices"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "device_id": "mac-0000000000000001",
                "user_id": "u-1",
                "platform": "mac",
                "display_name": "Test Mac",
                "created_at": "2026-04-17T00:00:00Z",
                "last_seen_at": "2026-04-17T00:00:00Z",
                "revoked_at": null,
                "revoked_reason": null
            }
        ])))
        .mount(&mock)
        .await;

    let dir = tempdir().unwrap();
    let app = build_app_with_session(&dir, &mock.uri(), "u-1").await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/devices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let devices: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["device_id"], "mac-0000000000000001");
}

#[tokio::test]
#[serial]
async fn list_devices_unauthenticated_returns_401() {
    let dir = tempdir().unwrap();
    std::env::set_var("CLAUDE_VIEW_DATA_DIR", dir.path());
    std::env::set_var("SUPABASE_URL", "http://localhost:1");
    std::env::set_var("SUPABASE_PUBLISHABLE_KEY", "x");
    let db = Database::new_in_memory().await.unwrap();
    let app = create_app(db);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/devices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn revoke_device_calls_edge_function() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wpath("/functions/v1/devices-revoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_id": "ios-abcdef0123456789",
            "device": {
                "device_id": "ios-abcdef0123456789",
                "user_id": "u-1",
                "platform": "ios",
                "display_name": "Phone",
                "created_at": "2026-04-17T00:00:00Z",
                "last_seen_at": "2026-04-17T00:00:00Z",
                "revoked_at": "2026-04-17T01:00:00Z",
                "revoked_reason": "user_action"
            }
        })))
        .mount(&mock)
        .await;

    let dir = tempdir().unwrap();
    let app = build_app_with_session(&dir, &mock.uri(), "u-1").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/devices/ios-abcdef0123456789")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn terminate_others_returns_count() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wpath("/functions/v1/devices-terminate-others"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "revoked_count": 3
        })))
        .mount(&mock)
        .await;

    let dir = tempdir().unwrap();
    let app = build_app_with_session(&dir, &mock.uri(), "u-1").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/devices/terminate-others")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["revoked_count"], 3);
}

#[tokio::test]
#[serial]
async fn pairing_qr_proxies_to_pair_offer_edge_fn() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wpath("/functions/v1/pair-offer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "token": "tok_aaabbbccc",
            "relay_ws_url": "wss://claude-view-relay.fly.dev/ws",
            "expires_at": "2026-04-17T00:05:00Z"
        })))
        .mount(&mock)
        .await;

    let dir = tempdir().unwrap();
    let app = build_app_with_session(&dir, &mock.uri(), "u-1").await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/pairing/qr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let qr: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(qr["t"], "tok_aaabbbccc");
    assert!(qr["url"].as_str().unwrap().contains("tok_aaabbbccc"));
}

#[tokio::test]
#[serial]
async fn pairing_qr_requires_signed_in_user() {
    let dir = tempdir().unwrap();
    std::env::set_var("CLAUDE_VIEW_DATA_DIR", dir.path());
    std::env::set_var("SUPABASE_URL", "http://localhost:1");
    std::env::set_var("SUPABASE_PUBLISHABLE_KEY", "x");
    let db = Database::new_in_memory().await.unwrap();
    let app = create_app(db);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/pairing/qr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
