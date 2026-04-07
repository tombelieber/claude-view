//! Tests for sync endpoints.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use claude_view_db::Database;
use tower::ServiceExt;

async fn test_db() -> Database {
    Database::new_in_memory().await.expect("in-memory DB")
}

fn build_app(db: Database) -> axum::Router {
    crate::create_app(db)
}

async fn do_post(app: axum::Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn test_sync_git_accepted() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_post(app, "/api/sync/git").await;

    assert_eq!(status, StatusCode::ACCEPTED);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "accepted");
    assert!(json["message"].as_str().unwrap().contains("initiated"));
}

#[tokio::test]
async fn test_sync_deep_accepted() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_post(app, "/api/sync/deep").await;

    assert_eq!(status, StatusCode::ACCEPTED);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "accepted");
    assert!(json["message"].as_str().unwrap().contains("Deep index"));
}

// Note: Testing the 409 Conflict case requires holding the mutex during the test,
// which is tricky with the current design. In a real implementation, we would
// have a more sophisticated sync state management that allows better testing.

// ========================================================================
// SSE Git Sync Progress Tests
// ========================================================================

use crate::create_app_with_git_sync;
use crate::git_sync_state::{GitSyncPhase, GitSyncState};
use std::sync::Arc;

#[tokio::test]
async fn test_sse_done_emits_done_event() {
    let db = test_db().await;
    let state = Arc::new(GitSyncState::new());
    state.set_phase(GitSyncPhase::Done);
    state.set_repos_scanned(3);
    state.set_total_repos(3);
    state.add_commits_found(42);
    state.add_links_created(7);

    let app = create_app_with_git_sync(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sync/git/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        body_str.contains("event: done"),
        "Expected 'event: done' in body: {}",
        body_str
    );
    assert!(
        body_str.contains("\"reposScanned\":3"),
        "Expected reposScanned=3 in body: {}",
        body_str
    );
    assert!(
        body_str.contains("\"commitsFound\":42"),
        "Expected commitsFound=42 in body: {}",
        body_str
    );
    assert!(
        body_str.contains("\"linksCreated\":7"),
        "Expected linksCreated=7 in body: {}",
        body_str
    );
}

#[tokio::test]
async fn test_sse_error_emits_error_event() {
    let db = test_db().await;
    let state = Arc::new(GitSyncState::new());
    state.set_error("disk full".to_string());

    let app = create_app_with_git_sync(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sync/git/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        body_str.contains("event: error"),
        "Expected 'event: error' in body: {}",
        body_str
    );
    assert!(
        body_str.contains("disk full"),
        "Expected 'disk full' in body: {}",
        body_str
    );
}

#[tokio::test]
async fn test_sse_content_type() {
    let db = test_db().await;
    let state = Arc::new(GitSyncState::new());
    // Set to Done so the stream terminates quickly
    state.set_phase(GitSyncPhase::Done);

    let app = create_app_with_git_sync(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sync/git/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("text/event-stream"),
        "Expected text/event-stream, got: {}",
        content_type
    );
}
