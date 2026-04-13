//! Tests for CLI sessions API.
//!
//! Uses MockTmux for all handler tests -- no real tmux needed.

use super::*;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use claude_view_db::Database;
use tmux::mock::MockTmux;
use tower::ServiceExt;
use types::{CliSessionStatus, CreateResponse};

// ============================================================================
// Helpers
// ============================================================================

async fn test_db() -> Database {
    Database::new_in_memory().await.expect("in-memory DB")
}

fn build_app(db: Database, mock_tmux: MockTmux) -> Router {
    let mut state = crate::state::AppState::new(db);
    {
        let s = std::sync::Arc::get_mut(&mut state).unwrap();
        s.tmux = std::sync::Arc::new(mock_tmux);
    }
    Router::new().nest("/api", router()).with_state(state)
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
// Handler tests -- create
// ============================================================================

#[tokio::test]
async fn test_create_session_success() {
    let mock = MockTmux::new();
    let app = build_app(test_db().await, mock);

    let payload = serde_json::json!({
        "projectDir": "/tmp",
        "args": ["--verbose"]
    });

    let (status, body) = do_request(
        app,
        Method::POST,
        "/api/cli-sessions",
        Some(&payload.to_string()),
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let resp: CreateResponse = serde_json::from_str(&body).unwrap();
    assert!(resp.session.id.starts_with("cv-"));
    assert_eq!(resp.session.status, CliSessionStatus::Running);
    assert_eq!(resp.session.project_dir, Some("/tmp".to_string()));
    assert_eq!(resp.session.args, vec!["--verbose"]);
    assert!(resp.session.created_at > 0);
}

#[tokio::test]
async fn test_create_session_minimal_request() {
    let mock = MockTmux::new();
    let app = build_app(test_db().await, mock);

    // Empty body -- project_dir and args should default.
    let (status, body) = do_request(app, Method::POST, "/api/cli-sessions", Some("{}")).await;

    assert_eq!(status, StatusCode::OK);

    let resp: CreateResponse = serde_json::from_str(&body).unwrap();
    assert!(resp.session.id.starts_with("cv-"));
    assert!(resp.session.project_dir.is_none());
    assert!(resp.session.args.is_empty());
}

#[tokio::test]
async fn test_create_session_tmux_unavailable() {
    let mock = MockTmux::unavailable();
    let app = build_app(test_db().await, mock);

    let (status, _body) = do_request(app, Method::POST, "/api/cli-sessions", Some("{}")).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// ============================================================================
// Handler tests -- kill
// ============================================================================

#[tokio::test]
async fn test_kill_session_success() {
    let db = test_db().await;
    let mut state = crate::state::AppState::new(db);
    let mock_tmux = MockTmux::new();
    // Pre-register a session in mock tmux.
    mock_tmux.new_session("cv-kill-me", None, &[]).unwrap();
    {
        let s = std::sync::Arc::get_mut(&mut state).unwrap();
        s.tmux = std::sync::Arc::new(mock_tmux);
    }

    // Register in tmux index.
    state.tmux_index.insert("cv-kill-me".to_string()).await;

    let app = Router::new()
        .nest("/api", router())
        .with_state(state.clone());

    let (status, body) =
        do_request(app, Method::DELETE, "/api/cli-sessions/cv-kill-me", None).await;

    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(resp["removed"], true);
    assert_eq!(resp["id"], "cv-kill-me");

    // Verify removed from tmux index.
    assert!(!state.tmux_index.contains("cv-kill-me").await);
}

#[tokio::test]
async fn test_kill_session_not_found() {
    let mock = MockTmux::new();
    let app = build_app(test_db().await, mock);

    let (status, _body) = do_request(
        app,
        Method::DELETE,
        "/api/cli-sessions/cv-nonexistent",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Mock tmux unit tests
// ============================================================================

#[test]
fn test_mock_tmux_new_and_kill() {
    let mock = MockTmux::new();
    assert!(mock.is_available());
    assert!(!mock.has_session("cv-test"));

    mock.new_session("cv-test", None, &[]).unwrap();
    assert!(mock.has_session("cv-test"));

    let sessions = mock.active_sessions();
    assert!(sessions.contains("cv-test"));

    mock.kill_session("cv-test").unwrap();
    assert!(!mock.has_session("cv-test"));
}

#[test]
fn test_mock_tmux_unavailable() {
    let mock = MockTmux::unavailable();
    assert!(!mock.is_available());

    let result = mock.new_session("cv-fail", None, &[]);
    assert!(result.is_err());
}

#[test]
fn test_mock_tmux_duplicate_session_errors() {
    let mock = MockTmux::new();
    mock.new_session("cv-dup", None, &[]).unwrap();

    let result = mock.new_session("cv-dup", None, &[]);
    assert!(result.is_err());
}

#[test]
fn test_mock_tmux_kill_missing_errors() {
    let mock = MockTmux::new();
    let result = mock.kill_session("cv-nope");
    assert!(result.is_err());
}

// ============================================================================
// Path validation tests
// ============================================================================

#[tokio::test]
async fn test_create_session_rejects_relative_project_dir() {
    let mock = MockTmux::new();
    let app = build_app(test_db().await, mock);

    let (status, resp) = do_request(
        app,
        Method::POST,
        "/api/cli-sessions",
        Some(r#"{"projectDir":"relative/path"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        resp.contains("absolute path"),
        "Expected absolute path error, got: {resp}"
    );
}

#[tokio::test]
async fn test_create_session_rejects_nonexistent_project_dir() {
    let mock = MockTmux::new();
    let app = build_app(test_db().await, mock);

    let (status, resp) = do_request(
        app,
        Method::POST,
        "/api/cli-sessions",
        Some(r#"{"projectDir":"/nonexistent/path/that/does/not/exist"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        resp.contains("not a directory") || resp.contains("does not exist"),
        "Expected directory error, got: {resp}"
    );
}
