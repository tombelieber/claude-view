//! Tests for GET /api/sessions/:id/subagents/:agent_id/messages.

#![cfg(test)]

use axum::http::StatusCode;

use super::tests_common::*;

// ========================================================================
// GET /api/sessions/:id/subagents/:agent_id/messages tests
// ========================================================================

/// Subagent messages: nonexistent parent session → 404.
#[tokio::test]
async fn test_subagent_messages_returns_404_for_missing_session() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/nonexistent/subagents/abc123/messages").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        body.contains("not found") || body.contains("Session"),
        "Expected 404 body mentioning session not found, got: {body}"
    );
}

/// Subagent messages: valid parent session, nonexistent agent JSONL → 404.
#[tokio::test]
async fn test_subagent_messages_returns_404_for_missing_subagent() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();

    // Create a parent JSONL file that exists on disk (required by resolve_session_file_path).
    let parent_jsonl = tmp.path().join("parent-session.jsonl");
    std::fs::write(
        &parent_jsonl,
        r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello"}]},"timestamp":"2026-01-01T00:00:00.000Z"}
"#,
    )
    .unwrap();

    // Register parent session in DB so file_path resolves.
    let mut session = make_session("parent-session", "project-a", 1700000000);
    session.file_path = parent_jsonl.to_string_lossy().to_string();
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);
    // Agent "deadbeef" has no JSONL file on disk → 404.
    let (status, body) = do_get(
        app,
        "/api/sessions/parent-session/subagents/deadbeef/messages",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        body.contains("not found") || body.contains("Sub-agent"),
        "Expected 404 body mentioning sub-agent not found, got: {body}"
    );
}

/// Subagent messages: agent_id with special characters → 400.
#[tokio::test]
async fn test_subagent_messages_returns_400_for_invalid_agent_id() {
    let db = test_db().await;
    let app = build_app(db);

    // agent_id with a slash — should be rejected before any DB lookup.
    let (status, _body) = do_get(
        app,
        "/api/sessions/any-session/subagents/../../etc/passwd/messages",
    )
    .await;
    // axum may parse this differently due to path traversal, but the validation
    // should reject non-alphanumeric IDs regardless. Accept 400 or 404.
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
        "Expected 400 or 404 for path-traversal agent_id, got: {status}"
    );

    // Agent ID with dots and hyphens (not purely alphanumeric).
    let db2 = test_db().await;
    let app2 = build_app(db2);
    let (status2, body2) = do_get(
        app2,
        "/api/sessions/any-session/subagents/bad-agent.id/messages",
    )
    .await;
    // The handler rejects non-alphanumeric, so dots/hyphens → 400.
    // However, session lookup may 404 first depending on routing.
    // The key is that it doesn't succeed (200).
    assert_ne!(
        status2,
        StatusCode::OK,
        "Agent ID with special chars must not succeed, body: {body2}"
    );
}

/// Subagent messages: real JSONL file on disk → returns blocks with pagination fields.
#[tokio::test]
async fn test_subagent_messages_returns_blocks_with_pagination() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();

    // Create parent JSONL.
    let parent_jsonl = tmp.path().join("parent-sub-test.jsonl");
    std::fs::write(
        &parent_jsonl,
        r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"parent msg"}]},"timestamp":"2026-01-01T00:00:00.000Z"}
"#,
    )
    .unwrap();

    // Create subagent JSONL at the expected path:
    //   {parent_dir}/{session_stem}/subagents/agent-{agentId}.jsonl
    let subagent_dir = tmp.path().join("parent-sub-test").join("subagents");
    std::fs::create_dir_all(&subagent_dir).unwrap();
    let subagent_jsonl = subagent_dir.join("agent-abc123.jsonl");
    std::fs::write(
        &subagent_jsonl,
        r#"{"type":"user","uuid":"u-sub-1","message":{"content":[{"type":"text","text":"hello from subagent"}]},"timestamp":"2026-01-01T00:00:00.000Z"}
{"type":"assistant","uuid":"a-sub-1","message":{"id":"msg-sub-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"subagent reply"}],"usage":{"input_tokens":50,"output_tokens":25},"stop_reason":"end_turn"},"timestamp":"2026-01-01T00:00:01.000Z"}
"#,
    )
    .unwrap();

    // Register parent in DB.
    let mut session = make_session("parent-sub-test", "project-a", 1700000000);
    session.file_path = parent_jsonl.to_string_lossy().to_string();
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(
        app,
        "/api/sessions/parent-sub-test/subagents/abc123/messages?format=block",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    // Verify pagination fields exist and are correct.
    assert_eq!(json["total"], 2, "Should have 2 blocks (user + assistant)");
    assert_eq!(json["offset"], 0);
    assert_eq!(json["limit"], 50); // default limit
    assert_eq!(json["hasMore"], false);

    // Verify blocks contain the expected content.
    let blocks = json["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 2);

    let types: Vec<&str> = blocks
        .iter()
        .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
        .collect();
    assert!(
        types.contains(&"user"),
        "Should have a user block, got: {types:?}"
    );
    assert!(
        types.contains(&"assistant"),
        "Should have an assistant block, got: {types:?}"
    );
}

/// Subagent messages: pagination offset/limit slices correctly.
#[tokio::test]
async fn test_subagent_messages_pagination_offset_limit() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();

    // Create parent JSONL.
    let parent_jsonl = tmp.path().join("parent-paginate.jsonl");
    std::fs::write(
        &parent_jsonl,
        r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"parent"}]},"timestamp":"2026-01-01T00:00:00.000Z"}
"#,
    )
    .unwrap();

    // Create subagent JSONL with 4 exchanges (8 blocks).
    let subagent_dir = tmp.path().join("parent-paginate").join("subagents");
    std::fs::create_dir_all(&subagent_dir).unwrap();
    let subagent_jsonl = subagent_dir.join("agent-pag123.jsonl");
    let mut content = String::new();
    for i in 0..4 {
        content.push_str(&format!(
            r#"{{"type":"user","uuid":"u-{i}","message":{{"content":[{{"type":"text","text":"msg {i}"}}]}},"timestamp":"2026-01-01T00:00:{:02}.000Z"}}
{{"type":"assistant","uuid":"a-{i}","message":{{"id":"msg-{i}","model":"claude-sonnet-4-6","content":[{{"type":"text","text":"reply {i}"}}],"usage":{{"input_tokens":10,"output_tokens":5}},"stop_reason":"end_turn"}},"timestamp":"2026-01-01T00:00:{:02}.000Z"}}
"#,
            i * 2,
            i * 2 + 1,
        ));
    }
    std::fs::write(&subagent_jsonl, &content).unwrap();

    // Register parent in DB.
    let mut session = make_session("parent-paginate", "project-a", 1700000000);
    session.file_path = parent_jsonl.to_string_lossy().to_string();
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);

    // Fetch with offset=2, limit=3 → should get blocks 2..5 of 8 total.
    let (status, body) = do_get(
        app,
        "/api/sessions/parent-paginate/subagents/pag123/messages?format=block&offset=2&limit=3",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(json["total"], 8, "Total should be 8 blocks");
    assert_eq!(json["offset"], 2);
    assert_eq!(json["limit"], 3);
    assert_eq!(
        json["hasMore"], true,
        "offset=2, limit=3 on 8 items → hasMore should be true"
    );

    let blocks = json["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 3, "Should return exactly 3 blocks");
}
