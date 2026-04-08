//! Tests for GET /api/sessions/:id/messages (legacy + block format, live fallbacks).

#![cfg(test)]

use axum::http::StatusCode;
use std::sync::Arc;

use super::tests_common::*;

// ========================================================================
// GET /api/sessions/:id/messages tests
// ========================================================================

#[tokio::test]
async fn test_get_session_messages_by_id_not_in_db() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/nonexistent/messages?limit=10&offset=0").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_messages_by_id_file_gone() {
    let db = test_db().await;
    let mut session = make_session("msg-test", "proj", 1700000000);
    session.file_path = "/nonexistent/path.jsonl".to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/msg-test/messages?limit=10&offset=0").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_messages_by_id_success() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("msg-success.jsonl");
    std::fs::write(
        &session_file,
        r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-01T00:00:00Z"}"#,
    )
    .unwrap();

    let mut session = make_session("msg-ok", "proj", 1700000000);
    session.file_path = session_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/msg-ok/messages?limit=10&offset=0").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let messages = json["messages"]
        .as_array()
        .expect("Response should contain messages array");
    assert!(
        !messages.is_empty(),
        "Fixture should produce at least one parsed message"
    );
    assert!(
        json["total"].as_u64().unwrap() > 0,
        "Total should reflect the fixture message count"
    );
}

// ========================================================================
// GET /api/sessions/:id/messages?format=block tests
// ========================================================================

#[tokio::test]
async fn test_format_block_returns_paginated_blocks() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("block-test.jsonl");
    std::fs::write(
        &session_file,
        r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello"}]},"timestamp":"2026-03-21T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Hi there!"}],"usage":{"input_tokens":100,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-21T01:00:01.000Z"}
"#,
    )
    .unwrap();

    let mut session = make_session("block-test", "proj", 1700000000);
    session.file_path = session_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/block-test/messages?format=block").await;
    assert_eq!(status, StatusCode::OK);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        json.get("blocks").is_some(),
        "Response should have 'blocks' key"
    );
    let blocks = json["blocks"].as_array().unwrap();
    assert!(!blocks.is_empty(), "blocks should not be empty");
    assert!(
        blocks[0].get("type").is_some(),
        "Block should have 'type' discriminator"
    );
}

#[tokio::test]
async fn test_format_block_empty_session() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("block-empty.jsonl");
    std::fs::write(&session_file, "").unwrap();

    let mut session = make_session("block-empty", "proj", 1700000000);
    session.file_path = session_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/block-empty/messages?format=block").await;
    assert_eq!(status, StatusCode::OK);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let blocks = json["blocks"].as_array().unwrap();
    assert!(blocks.is_empty());
    assert_eq!(json["total"], 0);
    assert_eq!(json["hasMore"], false);
}

#[tokio::test]
async fn test_format_block_e2e_block_structure() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("e2e-block.jsonl");
    // Write a multi-line JSONL fixture with user + assistant + tool + boundary
    std::fs::write(&session_file, r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"List files"}]},"timestamp":"2026-03-21T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Sure!"},{"type":"tool_use","id":"tu-1","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":500,"output_tokens":100},"stop_reason":"tool_use"},"timestamp":"2026-03-21T01:00:01.000Z"}
{"type":"user","uuid":"u-2","message":{"content":[{"type":"tool_result","tool_use_id":"tu-1","content":"file1\nfile2","is_error":false}]},"timestamp":"2026-03-21T01:00:02.000Z"}
{"type":"assistant","uuid":"a-2","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Here are the files."}],"usage":{"input_tokens":600,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-21T01:00:03.000Z"}
{"type":"system","uuid":"s-1","durationMs":3000,"timestamp":"2026-03-21T01:00:04.000Z"}
{"type":"system","uuid":"s-2","stopReason":"end_turn","hookInfos":[],"hookErrors":[],"hookCount":0,"timestamp":"2026-03-21T01:00:05.000Z"}
"#).unwrap();

    let mut session = make_session("e2e-block", "proj", 1700000000);
    session.file_path = session_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/e2e-block/messages?format=block").await;
    assert_eq!(status, StatusCode::OK);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let blocks = json["blocks"].as_array().unwrap();

    // Verify block types present
    let types: Vec<&str> = blocks
        .iter()
        .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
        .collect();
    assert!(types.contains(&"user"), "Should have user block");
    assert!(types.contains(&"assistant"), "Should have assistant block");
    assert!(
        types.contains(&"turn_boundary"),
        "Should have turn_boundary block"
    );

    // Verify block count matches expected
    assert_eq!(
        json["total"].as_u64().unwrap() as usize,
        blocks.len(),
        "total should match actual block count for small sessions"
    );

    // Verify hasMore is false for small session
    assert_eq!(json["hasMore"], false);
}

// ========================================================================
// Live session file_path fallback tests
// ========================================================================

/// Regression: GET /api/sessions/:id/messages?format=block must return blocks
/// for live sessions not yet indexed in the DB (file_path fallback).
#[tokio::test]
async fn test_messages_block_format_falls_back_to_live_session() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("live-only.jsonl");
    std::fs::write(
        &session_file,
        r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello from VS Code"}]},"timestamp":"2026-03-23T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Hi!"}],"usage":{"input_tokens":100,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-23T01:00:01.000Z"}
"#,
    )
    .unwrap();

    // NOT inserted into DB — simulates un-indexed live session
    let state = crate::state::AppState::builder(db)
        .with_indexing(Arc::new(crate::indexing_state::IndexingState::new()))
        .build();

    let live = make_live_session("live-only", session_file.to_str().unwrap());
    state
        .live_sessions
        .write()
        .await
        .insert("live-only".to_string(), live);

    let app = crate::api_routes(state);
    let (status, body) = do_get(app, "/api/sessions/live-only/messages?format=block").await;
    assert_eq!(status, StatusCode::OK);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let blocks = json["blocks"].as_array().unwrap();
    // Must have both user AND assistant blocks
    let types: Vec<&str> = blocks
        .iter()
        .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
        .collect();
    assert!(
        types.contains(&"user"),
        "Live session fallback must return user blocks, got: {types:?}"
    );
    assert!(
        types.contains(&"assistant"),
        "Live session fallback must return assistant blocks, got: {types:?}"
    );
    assert_eq!(json["total"], 2);
}

/// Regression: GET /api/sessions/:id/rich must work for live sessions
/// not yet indexed in the DB (file_path fallback).
#[tokio::test]
async fn test_rich_endpoint_falls_back_to_live_session() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("live-rich.jsonl");
    std::fs::write(
        &session_file,
        r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello"}]},"timestamp":"2026-03-23T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Hi!"}],"usage":{"input_tokens":100,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-23T01:00:01.000Z"}
"#,
    )
    .unwrap();

    let state = crate::state::AppState::builder(db)
        .with_indexing(Arc::new(crate::indexing_state::IndexingState::new()))
        .build();

    let live = make_live_session("live-rich", session_file.to_str().unwrap());
    state
        .live_sessions
        .write()
        .await
        .insert("live-rich".to_string(), live);

    let app = crate::api_routes(state);
    let (status, _body) = do_get(app, "/api/sessions/live-rich/rich").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "rich endpoint should succeed via live fallback"
    );
}

/// When session is in neither DB nor live store, should return 404.
#[tokio::test]
async fn test_messages_returns_404_when_not_in_db_or_live() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/nonexistent/messages?format=block").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("not found") || body.contains("Session"));
}

/// DB path takes precedence over live session (ensures no conflict).
#[tokio::test]
async fn test_db_path_takes_precedence_over_live_session() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();

    // DB file has one message
    let db_file = tmp.path().join("db-priority.jsonl");
    std::fs::write(
        &db_file,
        r#"{"type":"user","uuid":"u-db","message":{"content":[{"type":"text","text":"from DB"}]},"timestamp":"2026-03-23T01:00:00.000Z"}
"#,
    )
    .unwrap();

    let mut session = make_session("db-priority", "proj", 1700000000);
    session.file_path = db_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    // Live session points to a DIFFERENT file with different content
    let live_file = tmp.path().join("live-priority.jsonl");
    std::fs::write(
        &live_file,
        r#"{"type":"user","uuid":"u-live","message":{"content":[{"type":"text","text":"from live"}]},"timestamp":"2026-03-23T02:00:00.000Z"}
"#,
    )
    .unwrap();

    let state = crate::state::AppState::builder(db)
        .with_indexing(Arc::new(crate::indexing_state::IndexingState::new()))
        .build();
    let live = make_live_session("db-priority", live_file.to_str().unwrap());
    state
        .live_sessions
        .write()
        .await
        .insert("db-priority".to_string(), live);

    let app = crate::api_routes(state);
    let (status, body) = do_get(app, "/api/sessions/db-priority/messages?format=block").await;
    assert_eq!(status, StatusCode::OK);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let blocks = json["blocks"].as_array().unwrap();
    // Should use DB file ("from DB"), not live file ("from live")
    if let Some(user_block) = blocks.iter().find(|b| b["type"] == "user") {
        assert_eq!(
            user_block["text"].as_str().unwrap(),
            "from DB",
            "DB path should take precedence over live session"
        );
    } else {
        panic!("Expected a user block from DB file");
    }
}
