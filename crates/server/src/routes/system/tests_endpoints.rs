//! Integration tests for system HTTP endpoints.

#[allow(deprecated)]
use super::*;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use claude_view_db::{Database, IndexRunIntegrityCounters};
use tower::ServiceExt;

async fn test_db() -> Database {
    Database::new_in_memory().await.expect("in-memory DB")
}

fn build_app(db: Database) -> axum::Router {
    crate::create_app(db)
}

async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn do_post_json(app: axum::Router, uri: &str, json_body: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(json_body.to_string()))
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

// ========================================================================
// GET /api/system tests
// ========================================================================

#[tokio::test]
async fn test_system_endpoint_empty_db() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/system").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    // Storage should exist with zeros
    assert!(json["storage"].is_object());
    assert_eq!(json["storage"]["jsonlBytes"], 0);
    assert_eq!(json["storage"]["dbBytes"], 0);

    // Performance should have null values
    assert!(json["performance"].is_object());
    assert!(json["performance"]["lastIndexDurationMs"].is_null());

    // Health should show 0 counts, healthy status
    assert!(json["health"].is_object());
    assert_eq!(json["health"]["sessionsCount"], 0);
    assert_eq!(json["health"]["commitsCount"], 0);
    assert_eq!(json["health"]["projectsCount"], 0);
    assert_eq!(json["health"]["status"], "healthy");

    // Integrity counters should exist and default to zero
    assert!(json["integrity"].is_object());
    assert!(json["integrity"]["counters"].is_object());
    assert_eq!(json["integrity"]["counters"]["unknownTopLevelTypeCount"], 0);
    assert_eq!(json["integrity"]["counters"]["unknownRequiredPathCount"], 0);
    assert_eq!(json["integrity"]["counters"]["imaginaryPathAccessCount"], 0);
    assert_eq!(json["integrity"]["counters"]["legacyFallbackPathCount"], 0);
    assert_eq!(
        json["integrity"]["counters"]["droppedLineInvalidJsonCount"],
        0
    );
    assert_eq!(json["integrity"]["counters"]["schemaMismatchCount"], 0);
    assert_eq!(json["integrity"]["counters"]["unknownSourceRoleCount"], 0);
    assert_eq!(
        json["integrity"]["counters"]["derivedSourceMessageDocCount"],
        0
    );
    assert_eq!(
        json["integrity"]["counters"]["sourceMessageNonSourceProvenanceCount"],
        0
    );

    // Index history should be empty
    assert!(json["indexHistory"].is_array());
    assert_eq!(json["indexHistory"].as_array().unwrap().len(), 0);

    // Classification should show zeros
    assert!(json["classification"].is_object());
    assert_eq!(json["classification"]["classifiedCount"], 0);
    assert_eq!(json["classification"]["unclassifiedCount"], 0);
    assert!(!json["classification"]["isRunning"].as_bool().unwrap());

    // Claude CLI should be present (may or may not be installed)
    assert!(json["claudeCli"].is_object());
}

#[tokio::test]
async fn test_system_endpoint_with_index_metadata() {
    let db = test_db().await;

    // Set some index metadata
    db.update_index_metadata_on_success(2800, 6712, 47)
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/system").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    // Performance should reflect the metadata
    assert_eq!(json["performance"]["lastIndexDurationMs"], 2800);
    assert!(json["performance"]["sessionsPerSec"].is_number());

    // Health should show healthy with a recent sync
    assert_eq!(json["health"]["status"], "healthy");
    assert!(json["health"]["lastSyncAt"].is_string());
}

#[tokio::test]
async fn test_system_endpoint_with_sessions() {
    let db = test_db().await;

    // Insert a session
    db.insert_session_from_index(
        "sess-1",
        "project-a",
        "Project A",
        "/tmp/project-a",
        "/tmp/sess1.jsonl",
        "Test session",
        None,
        5,
        chrono::Utc::now().timestamp(),
        None,
        false,
        1000,
    )
    .await
    .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/system").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(json["health"]["sessionsCount"], 1);
    assert_eq!(json["health"]["projectsCount"], 1);

    // Unclassified should be 1 since no classification done
    assert_eq!(json["classification"]["unclassifiedCount"], 1);
}

#[tokio::test]
async fn test_system_endpoint_with_index_runs() {
    let db = test_db().await;

    // Create an index run
    let run_id = db.create_index_run("full", Some(0), None).await.unwrap();
    let counters = IndexRunIntegrityCounters {
        unknown_top_level_type_count: 1,
        unknown_required_path_count: 2,
        imaginary_path_access_count: 3,
        legacy_fallback_path_count: 4,
        dropped_line_invalid_json_count: 5,
        schema_mismatch_count: 6,
        unknown_source_role_count: 7,
        derived_source_message_doc_count: 8,
        source_message_non_source_provenance_count: 9,
    };
    db.complete_index_run(run_id, Some(100), 2500, Some(5.2), Some(&counters))
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/system").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    let history = json["indexHistory"].as_array().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["type"], "full");
    assert_eq!(history[0]["status"], "completed");
    assert_eq!(history[0]["sessionsCount"], 100);
    assert_eq!(history[0]["durationMs"], 2500);

    assert_eq!(json["integrity"]["counters"]["unknownTopLevelTypeCount"], 1);
    assert_eq!(json["integrity"]["counters"]["unknownRequiredPathCount"], 2);
    assert_eq!(json["integrity"]["counters"]["imaginaryPathAccessCount"], 3);
    assert_eq!(json["integrity"]["counters"]["legacyFallbackPathCount"], 4);
    assert_eq!(
        json["integrity"]["counters"]["droppedLineInvalidJsonCount"],
        5
    );
    assert_eq!(json["integrity"]["counters"]["schemaMismatchCount"], 6);
    assert_eq!(json["integrity"]["counters"]["unknownSourceRoleCount"], 7);
    assert_eq!(
        json["integrity"]["counters"]["derivedSourceMessageDocCount"],
        8
    );
    assert_eq!(
        json["integrity"]["counters"]["sourceMessageNonSourceProvenanceCount"],
        9
    );
}

// ========================================================================
// POST /api/system/reindex tests
// ========================================================================

#[tokio::test]
async fn test_reindex_endpoint() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_post(app, "/api/system/reindex").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "started");
    assert!(json["message"].as_str().unwrap().contains("re-index"));
}

// ========================================================================
// POST /api/system/clear-cache tests
// ========================================================================

#[tokio::test]
async fn test_clear_cache_endpoint() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_post(app, "/api/system/clear-cache").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "success");
    assert!(json["clearedBytes"].is_number());
}

// ========================================================================
// POST /api/system/git-resync tests
// ========================================================================

#[tokio::test]
async fn test_git_resync_endpoint() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_post(app, "/api/system/git-resync").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "not_implemented");
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("not yet available"));
}

// ========================================================================
// POST /api/system/reset tests
// ========================================================================

#[tokio::test]
async fn test_reset_requires_confirmation() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, _body) = do_post_json(app, "/api/system/reset", r#"{"confirm": "wrong"}"#).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_reset_with_correct_confirmation() {
    let db = test_db().await;

    // Insert some data first
    db.insert_session_from_index(
        "sess-1",
        "project-a",
        "Project A",
        "/tmp/project-a",
        "/tmp/sess1.jsonl",
        "Test session",
        None,
        5,
        chrono::Utc::now().timestamp(),
        None,
        false,
        1000,
    )
    .await
    .unwrap();

    // Verify data exists
    let health = db.get_health_stats().await.unwrap();
    assert_eq!(health.sessions_count, 1);

    let app = build_app(db.clone());
    let (status, body) =
        do_post_json(app, "/api/system/reset", r#"{"confirm": "RESET_ALL_DATA"}"#).await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "success");

    // Verify data is cleared
    let health = db.get_health_stats().await.unwrap();
    assert_eq!(health.sessions_count, 0);
    assert_eq!(health.commits_count, 0);
}

#[tokio::test]
async fn test_reset_without_body_fails() {
    let db = test_db().await;
    let app = build_app(db);

    // POST without a JSON body should fail
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/system/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should get an error status (400, 415, or 422 depending on framework)
    assert!(
        response.status().is_client_error(),
        "Expected 4xx client error, got {}",
        response.status()
    );
}
