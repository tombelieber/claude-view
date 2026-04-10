//! Integration tests for the webhooks CRUD API.

use super::*;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use claude_view_db::Database;
use tempfile::TempDir;
use tower::ServiceExt;

// ============================================================================
// Test helpers
// ============================================================================

async fn test_db() -> Database {
    Database::new_in_memory().await.expect("in-memory DB")
}

async fn build_test_app(tmp: &TempDir) -> (Router, std::sync::Arc<crate::state::AppState>) {
    let db = test_db().await;
    let mut state = crate::state::AppState::builder(db).build();
    {
        let s = std::sync::Arc::get_mut(&mut state).unwrap();
        s.webhook_config_path = tmp.path().join("notifications.json");
        s.webhook_secrets_path = tmp.path().join("webhook-secrets.json");
    }
    let app = Router::new()
        .nest("/api", router())
        .with_state(state.clone());
    (app, state)
}

async fn do_request(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = if let Some(json) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(json.to_string())
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

// ============================================================================
// Test cases
// ============================================================================

/// 1. List empty → 200, empty array
#[tokio::test]
async fn test_list_empty_returns_200() {
    let tmp = TempDir::new().unwrap();
    let (app, _state) = build_test_app(&tmp).await;

    let (status, body) = do_request(app, Method::GET, "/api/webhooks", None).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["webhooks"].as_array().unwrap().len(), 0);
}

/// 2. Create webhook → 200, config + whsec_ signing secret
#[tokio::test]
async fn test_create_webhook_returns_signing_secret() {
    let tmp = TempDir::new().unwrap();
    let (app, _state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "My Webhook",
        "url": "https://example.com/hook",
        "format": "raw",
        "events": ["session.started"]
    });
    let (status, body) = do_request(
        app,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let secret = json["signingSecret"].as_str().expect("signingSecret field");
    assert!(
        secret.starts_with("whsec_"),
        "signing secret must start with whsec_, got: {secret}"
    );
    let wh = &json["webhook"];
    assert_eq!(wh["name"].as_str().unwrap(), "My Webhook");
    assert_eq!(wh["url"].as_str().unwrap(), "https://example.com/hook");
    assert!(wh["id"].as_str().unwrap().starts_with("wh_"));
    assert_eq!(wh["enabled"].as_bool().unwrap(), true);
}

/// 3. Create persists to disk
#[tokio::test]
async fn test_create_persists_to_disk() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("notifications.json");
    let secrets_path = tmp.path().join("webhook-secrets.json");
    let (app, _state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "Persist Test",
        "url": "https://example.com/hook",
        "format": "raw",
        "events": ["session.ended"]
    });
    let (status, _body) = do_request(
        app,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert!(config_path.exists(), "notifications.json should exist");
    assert!(secrets_path.exists(), "webhook-secrets.json should exist");

    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(config["webhooks"].as_array().unwrap().len(), 1);

    let secrets: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&secrets_path).unwrap()).unwrap();
    assert_eq!(secrets["secrets"].as_object().unwrap().len(), 1);
}

/// 4. Get webhook by ID → 200
#[tokio::test]
async fn test_get_webhook_by_id() {
    let tmp = TempDir::new().unwrap();
    let (app_create, state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "Get Test",
        "url": "https://example.com/hook",
        "format": "raw",
        "events": ["session.started"]
    });
    let (status, body) = do_request(
        app_create,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["webhook"]["id"].as_str().unwrap().to_string();

    let app_get = Router::new()
        .nest("/api", router())
        .with_state(state.clone());
    let (status, body) =
        do_request(app_get, Method::GET, &format!("/api/webhooks/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["id"].as_str().unwrap(), id);
    assert_eq!(json["name"].as_str().unwrap(), "Get Test");
}

/// 5. Get nonexistent → 404
#[tokio::test]
async fn test_get_nonexistent_webhook_returns_404() {
    let tmp = TempDir::new().unwrap();
    let (app, _state) = build_test_app(&tmp).await;

    let (status, _body) = do_request(app, Method::GET, "/api/webhooks/wh_doesnotexist", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// 6. Update webhook name → 200
#[tokio::test]
async fn test_update_webhook_name() {
    let tmp = TempDir::new().unwrap();
    let (app_create, state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "Before Update",
        "url": "https://example.com/hook",
        "format": "raw",
        "events": ["session.started"]
    });
    let (status, body) = do_request(
        app_create,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["webhook"]["id"].as_str().unwrap().to_string();

    let app_update = Router::new()
        .nest("/api", router())
        .with_state(state.clone());
    let update = serde_json::json!({"name": "After Update"});
    let (status, body) = do_request(
        app_update,
        Method::PUT,
        &format!("/api/webhooks/{id}"),
        Some(&update.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["name"].as_str().unwrap(), "After Update");
    // URL should be unchanged
    assert_eq!(json["url"].as_str().unwrap(), "https://example.com/hook");
}

/// 7. Delete webhook → 200, removes from config + secrets
#[tokio::test]
async fn test_delete_webhook_removes_from_config_and_secrets() {
    let tmp = TempDir::new().unwrap();
    let (app_create, state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "Delete Me",
        "url": "https://example.com/hook",
        "format": "raw",
        "events": ["session.started"]
    });
    let (status, body) = do_request(
        app_create,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = created["webhook"]["id"].as_str().unwrap().to_string();

    let app_del = Router::new()
        .nest("/api", router())
        .with_state(state.clone());
    let (status, body) = do_request(
        app_del,
        Method::DELETE,
        &format!("/api/webhooks/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["deleted"].as_bool().unwrap(), true);
    assert_eq!(json["id"].as_str().unwrap(), id);

    // Verify removed from both files
    let config_path = tmp.path().join("notifications.json");
    let secrets_path = tmp.path().join("webhook-secrets.json");
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(config["webhooks"].as_array().unwrap().len(), 0);
    let secrets: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&secrets_path).unwrap()).unwrap();
    assert_eq!(secrets["secrets"].as_object().unwrap().len(), 0);
}

/// 8. Delete nonexistent → 404
#[tokio::test]
async fn test_delete_nonexistent_webhook_returns_404() {
    let tmp = TempDir::new().unwrap();
    let (app, _state) = build_test_app(&tmp).await;

    let (status, _body) =
        do_request(app, Method::DELETE, "/api/webhooks/wh_doesnotexist", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// 9. Create with http:// URL → 400
#[tokio::test]
async fn test_create_with_http_url_returns_400() {
    let tmp = TempDir::new().unwrap();
    let (app, _state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "Bad URL",
        "url": "http://example.com/hook",
        "format": "raw",
        "events": ["session.started"]
    });
    let (status, _body) = do_request(
        app,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 10. Create with empty name → 400
#[tokio::test]
async fn test_create_with_empty_name_returns_400() {
    let tmp = TempDir::new().unwrap();
    let (app, _state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "",
        "url": "https://example.com/hook",
        "format": "raw",
        "events": ["session.started"]
    });
    let (status, _body) = do_request(
        app,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 11. Create with empty events → 400
#[tokio::test]
async fn test_create_with_empty_events_returns_400() {
    let tmp = TempDir::new().unwrap();
    let (app, _state) = build_test_app(&tmp).await;

    let payload = serde_json::json!({
        "name": "No Events",
        "url": "https://example.com/hook",
        "format": "raw",
        "events": []
    });
    let (status, _body) = do_request(
        app,
        Method::POST,
        "/api/webhooks",
        Some(&payload.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
