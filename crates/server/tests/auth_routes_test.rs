//! Integration tests for POST /api/auth/session, DELETE /api/auth/session,
//! GET /api/auth/status.
//!
//! Uses tower::ServiceExt::oneshot against a freshly-built Axum app so we
//! exercise the real router, the real state, and the real file writes —
//! all against a temp CLAUDE_VIEW_DATA_DIR. Marked #[serial] because
//! CLAUDE_VIEW_DATA_DIR is a process-global env var.

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

async fn build_app_in(dir: &tempfile::TempDir) -> axum::Router {
    std::env::set_var("CLAUDE_VIEW_DATA_DIR", dir.path());
    let db = Database::new_in_memory().await.unwrap();
    create_app(db)
}

fn jwt_for(user_id: &str, expires_in: u64) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + expires_in;
    let payload = URL_SAFE_NO_PAD.encode(format!(
        r#"{{"sub":"{user_id}","iss":"https://iebjyftoadahqptmfcio.supabase.co/auth/v1","aud":"authenticated","exp":{exp}}}"#
    ));
    let sig = URL_SAFE_NO_PAD.encode("fake-sig");
    format!("{header}.{payload}.{sig}")
}

#[tokio::test]
#[serial]
async fn post_session_persists_and_status_reflects() {
    let dir = tempdir().unwrap();
    let app = build_app_in(&dir).await;

    let body = json!({
        "access_token": jwt_for("u-abc", 3600),
        "refresh_token": "rft_abc",
        "expires_in": 3600,
        "user_id": "u-abc",
        "email": "abc@example.com"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/session")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["authenticated"], true);
    assert_eq!(json["user_id"], "u-abc");
    assert_eq!(json["email"], "abc@example.com");
    assert!(dir.path().join("auth-session.json").exists());
}

#[tokio::test]
#[serial]
async fn delete_session_clears_state_and_file() {
    let dir = tempdir().unwrap();
    let app = build_app_in(&dir).await;

    let body = json!({
        "access_token": jwt_for("u-del", 3600),
        "refresh_token": "rft_del",
        "expires_in": 3600,
        "user_id": "u-del"
    });
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/session")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/auth/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(!dir.path().join("auth-session.json").exists());

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["authenticated"], false);
}

#[tokio::test]
#[serial]
async fn status_without_session_returns_authenticated_false() {
    let dir = tempdir().unwrap();
    let app = build_app_in(&dir).await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["authenticated"], false);
}

#[tokio::test]
#[serial]
async fn post_session_rejects_malformed_body() {
    let dir = tempdir().unwrap();
    let app = build_app_in(&dir).await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/session")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
