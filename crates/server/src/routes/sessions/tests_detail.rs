//! Tests for GET /api/sessions/:id (detail), GET /api/branches, GET /api/sessions/activity.

#![cfg(test)]

use axum::http::StatusCode;

use super::tests_common::*;

// ========================================================================
// GET /api/sessions/:id tests
// ========================================================================

#[tokio::test]
async fn test_get_session_detail() {
    let db = test_db().await;

    let session = make_session("sess-123", "project-a", 1700000000);
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/sess-123").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["id"], "sess-123");
    assert!(json["commits"].is_array());
    assert!(json["derivedMetrics"].is_object());
    // Note: tokensPerPrompt requires turns table data which we don't insert in tests.
    // The tokens come from the turns aggregate, not from session.total_input_tokens.
    // Since we have files_edited_count=5 and reedited_files_count=2, reeditRate should be 0.4
    assert!(json["derivedMetrics"]["reeditRate"].is_number());
    assert_eq!(json["derivedMetrics"]["reeditRate"], 0.4);
}

#[tokio::test]
async fn test_get_session_detail_not_found() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/nonexistent").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["details"].as_str().unwrap().contains("nonexistent"));
}

// ========================================================================
// GET /api/branches tests
// ========================================================================

#[tokio::test]
async fn test_list_branches_empty() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/branches").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_branches_with_data() {
    let db = test_db().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.git_branch = Some("main".to_string());
    db.insert_session(&session1, "project-a", "Project A")
        .await
        .unwrap();

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.git_branch = Some("feature/auth".to_string());
    db.insert_session(&session2, "project-a", "Project A")
        .await
        .unwrap();

    let mut session3 = make_session("sess-3", "project-a", 1700000200);
    session3.git_branch = Some("main".to_string()); // Duplicate
    db.insert_session(&session3, "project-a", "Project A")
        .await
        .unwrap();

    let mut session4 = make_session("sess-4", "project-a", 1700000300);
    session4.git_branch = None; // No branch - should be excluded
    db.insert_session(&session4, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/branches").await;

    assert_eq!(status, StatusCode::OK);
    let branches: Vec<String> = serde_json::from_str(&body).unwrap();
    assert_eq!(branches.len(), 2); // Only "feature/auth" and "main"
    assert_eq!(branches, vec!["feature/auth", "main"]); // Alphabetically sorted
}

// ========================================================================
// GET /api/sessions/activity tests
// ========================================================================

#[tokio::test]
async fn test_session_activity() {
    let db = test_db().await;
    let session = make_session("sess-activity", "project-a", 1700000000);
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/activity").await;

    assert_eq!(status, StatusCode::OK);
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(resp["activity"].is_array());
    assert!(resp["bucket"].is_string());
    assert_eq!(resp["total"].as_u64().unwrap(), 1);
    let activity = resp["activity"].as_array().unwrap();
    assert!(!activity.is_empty());
    assert!(activity[0]["date"].is_string());
    assert!(activity[0]["count"].is_number());
}

// ========================================================================
// GET /api/sessions/:id/parsed tests
// ========================================================================

#[tokio::test]
async fn test_get_session_parsed_not_in_db() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/nonexistent/parsed").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_parsed_file_gone() {
    let db = test_db().await;
    let mut session = make_session("parsed-test", "proj", 1700000000);
    session.file_path = "/nonexistent/path.jsonl".to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/parsed-test/parsed").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_parsed_success() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("success-test.jsonl");
    std::fs::write(
        &session_file,
        r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-01T00:00:00Z"}"#,
    )
    .unwrap();

    let mut session = make_session("parsed-ok", "proj", 1700000000);
    session.file_path = session_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/parsed-ok/parsed").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let messages = json["messages"]
        .as_array()
        .expect("Response should contain messages array");
    assert!(
        !messages.is_empty(),
        "Fixture should produce at least one parsed message"
    );
}
