use axum::body::Body;
use axum::http::{Request, StatusCode};
use claude_view_db::Database;
use tower::ServiceExt;

fn test_app_with_telemetry_path(db: Database, config_path: std::path::PathBuf) -> axum::Router {
    claude_view_server::create_app_with_telemetry_path(db, config_path)
}

#[tokio::test]
async fn consent_enable_returns_enabled_status() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = Database::new_in_memory().await.unwrap();
    let app = test_app_with_telemetry_path(db, dir.path().join("telemetry.json"));
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"enabled":true}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "enabled");
}

#[tokio::test]
async fn consent_disable_returns_disabled_status() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = Database::new_in_memory().await.unwrap();
    let app = test_app_with_telemetry_path(db, dir.path().join("telemetry.json"));
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"enabled":false}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "disabled");
}

#[tokio::test]
async fn consent_invalid_body_returns_422() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = Database::new_in_memory().await.unwrap();
    let app = test_app_with_telemetry_path(db, dir.path().join("telemetry.json"));
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"bad":"field"}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
