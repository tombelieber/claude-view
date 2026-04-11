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
use store::CliSessionStore;
use tmux::mock::MockTmux;
use tower::ServiceExt;
use types::{CliSessionStatus, CreateResponse, ListResponse};

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
// Store tests
// ============================================================================

#[tokio::test]
async fn test_store_insert_and_get() {
    let store = CliSessionStore::new();
    store
        .insert(types::CliSession {
            id: "cv-aaa".to_string(),
            created_at: 1000,
            status: CliSessionStatus::Running,
            project_dir: Some("/tmp/proj".to_string()),
            args: vec!["--flag".to_string()],
            claude_session_id: None,
        })
        .await;

    let session = store.get("cv-aaa").await.unwrap();
    assert_eq!(session.id, "cv-aaa");
    assert_eq!(session.created_at, 1000);
    assert_eq!(session.status, CliSessionStatus::Running);
    assert_eq!(session.project_dir, Some("/tmp/proj".to_string()));
    assert_eq!(session.args, vec!["--flag"]);
}

#[tokio::test]
async fn test_store_get_missing_returns_none() {
    let store = CliSessionStore::new();
    assert!(store.get("nonexistent").await.is_none());
}

#[tokio::test]
async fn test_store_remove() {
    let store = CliSessionStore::new();
    store
        .insert(types::CliSession {
            id: "cv-bbb".to_string(),
            created_at: 2000,
            status: CliSessionStatus::Running,
            project_dir: None,
            args: vec![],
            claude_session_id: None,
        })
        .await;

    let removed = store.remove("cv-bbb").await;
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, "cv-bbb");

    // Should be gone now.
    assert!(store.get("cv-bbb").await.is_none());
}

#[tokio::test]
async fn test_store_remove_missing_returns_none() {
    let store = CliSessionStore::new();
    assert!(store.remove("nope").await.is_none());
}

#[tokio::test]
async fn test_store_list_sorted_newest_first() {
    let store = CliSessionStore::new();
    for (id, ts) in [("cv-old", 100u64), ("cv-mid", 500), ("cv-new", 900)] {
        store
            .insert(types::CliSession {
                id: id.to_string(),
                created_at: ts,
                status: CliSessionStatus::Running,
                project_dir: None,
                args: vec![],
            })
            .await;
    }

    let list = store.list().await;
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].id, "cv-new");
    assert_eq!(list[1].id, "cv-mid");
    assert_eq!(list[2].id, "cv-old");
}

#[tokio::test]
async fn test_store_update_status() {
    let store = CliSessionStore::new();
    store
        .insert(types::CliSession {
            id: "cv-ccc".to_string(),
            created_at: 3000,
            status: CliSessionStatus::Running,
            project_dir: None,
            args: vec![],
            claude_session_id: None,
        })
        .await;

    let updated = store
        .update_status("cv-ccc", CliSessionStatus::Exited)
        .await;
    assert!(updated);

    let session = store.get("cv-ccc").await.unwrap();
    assert_eq!(session.status, CliSessionStatus::Exited);
}

#[tokio::test]
async fn test_store_update_status_missing_returns_false() {
    let store = CliSessionStore::new();
    let updated = store.update_status("nope", CliSessionStatus::Exited).await;
    assert!(!updated);
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
// Handler tests -- list
// ============================================================================

#[tokio::test]
async fn test_list_sessions_empty() {
    let mock = MockTmux::new();
    let app = build_app(test_db().await, mock);

    let (status, body) = do_request(app, Method::GET, "/api/cli-sessions", None).await;

    assert_eq!(status, StatusCode::OK);

    let resp: ListResponse = serde_json::from_str(&body).unwrap();
    assert!(resp.sessions.is_empty());
}

#[tokio::test]
async fn test_list_sessions_marks_dead_as_exited() {
    let mock = MockTmux::new();
    let db = test_db().await;
    let mut state = crate::state::AppState::new(db);
    {
        let s = std::sync::Arc::get_mut(&mut state).unwrap();
        s.tmux = std::sync::Arc::new(mock);
    }

    // Insert a session that is NOT in mock tmux's tracking.
    // This simulates a tmux session that died externally.
    state
        .cli_sessions
        .insert(types::CliSession {
            id: "cv-ghost".to_string(),
            created_at: 1000,
            status: CliSessionStatus::Running,
            project_dir: None,
            args: vec![],
            claude_session_id: None,
        })
        .await;

    let app = Router::new().nest("/api", router()).with_state(state);

    let (status, body) = do_request(app, Method::GET, "/api/cli-sessions", None).await;

    assert_eq!(status, StatusCode::OK);

    let resp: ListResponse = serde_json::from_str(&body).unwrap();
    assert_eq!(resp.sessions.len(), 1);
    assert_eq!(resp.sessions[0].id, "cv-ghost");
    assert_eq!(resp.sessions[0].status, CliSessionStatus::Exited);
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

    // Also insert into the store.
    state
        .cli_sessions
        .insert(types::CliSession {
            id: "cv-kill-me".to_string(),
            created_at: 1000,
            status: CliSessionStatus::Running,
            project_dir: None,
            args: vec![],
            claude_session_id: None,
        })
        .await;

    let app = Router::new()
        .nest("/api", router())
        .with_state(state.clone());

    let (status, body) =
        do_request(app, Method::DELETE, "/api/cli-sessions/cv-kill-me", None).await;

    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(resp["removed"], true);
    assert_eq!(resp["id"], "cv-kill-me");

    // Verify removed from store.
    assert!(state.cli_sessions.get("cv-kill-me").await.is_none());
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
