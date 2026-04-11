//! Tests for hook events and block merge (Channel A + B).

#![cfg(test)]

use axum::http::StatusCode;
use std::sync::Arc;

use super::tests_common::*;

// ========================================================================
// GET /api/sessions/:id/hook-events tests
// ========================================================================

#[tokio::test]
async fn test_get_hook_events_empty() {
    let db = test_db().await;
    let app = build_app(db);

    let (status, body) = do_get(app, "/api/sessions/nonexistent/hook-events").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["hookEvents"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_hook_events_with_data() {
    let db = test_db().await;

    // Insert session first (FK reference)
    let session = make_session("hook-test", "project-a", 1700000000);
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    // Insert hook events
    let events = vec![
        claude_view_db::HookEventRow {
            timestamp: 1000,
            event_name: "SessionStart".into(),
            tool_name: None,
            label: "Waiting for first prompt".into(),
            group_name: "needs_you".into(),
            context: None,
            source: "hook".into(),
        },
        claude_view_db::HookEventRow {
            timestamp: 1001,
            event_name: "PreToolUse".into(),
            tool_name: Some("Bash".into()),
            label: "Running: git status".into(),
            group_name: "autonomous".into(),
            context: Some(r#"{"command":"git status"}"#.into()),
            source: "hook".into(),
        },
    ];
    claude_view_db::hook_events_queries::insert_hook_events(&db, "hook-test", &events)
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/hook-test/hook-events").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let hook_events = json["hookEvents"].as_array().unwrap();
    assert_eq!(hook_events.len(), 2);

    // Verify camelCase serialization
    assert_eq!(hook_events[0]["eventName"], "SessionStart");
    assert_eq!(hook_events[0]["group"], "needs_you");
    assert!(hook_events[0]["toolName"].is_null());

    assert_eq!(hook_events[1]["eventName"], "PreToolUse");
    assert_eq!(hook_events[1]["toolName"], "Bash");
    assert_eq!(hook_events[1]["label"], "Running: git status");
    assert!(hook_events[1]["context"]
        .as_str()
        .unwrap()
        .contains("git status"));
}

#[tokio::test]
async fn test_get_hook_events_from_live_session() {
    use crate::live::state::{
        AgentState, AgentStateGroup, HookEvent, HookFields, JsonlFields, LiveSession, SessionStatus,
    };

    let db = test_db().await;

    let state = crate::state::AppState::builder(db)
        .with_indexing(Arc::new(crate::indexing_state::IndexingState::new()))
        .build();

    // Insert a live session with hook events into the live_sessions map
    let mut session = LiveSession {
        id: "live-hook-test".to_string(),
        status: SessionStatus::Working,
        started_at: Some(1000),
        closed_at: None,
        control: None,
        model: Some("claude-sonnet-4-5-20250929".to_string()),
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 200000,
        statusline: crate::live::state::StatuslineFields::default(),
        hook: HookFields {
            agent_state: AgentState {
                state: "working".to_string(),
                group: AgentStateGroup::Autonomous,
                label: "Running".to_string(),
                context: None,
            },
            pid: Some(12345),
            title: String::new(),
            last_user_message: String::new(),
            current_activity: String::new(),
            turn_count: 1,
            last_activity_at: 1001,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
            hook_events: Vec::new(),
        },
        jsonl: JsonlFields {
            project: "test-project".to_string(),
            project_display_name: "test-project".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: String::new(),
            ..JsonlFields::default()
        },
        session_kind: None,
        entrypoint: None,
        ownership: None,
        pending_interaction: None,
    };
    session.hook.hook_events.push(HookEvent {
        timestamp: 1000,
        event_name: "SessionStart".to_string(),
        tool_name: None,
        label: "Waiting for prompt".to_string(),
        group: "needs_you".to_string(),
        context: None,
        source: "hook".to_string(),
    });
    session.hook.hook_events.push(HookEvent {
        timestamp: 1001,
        event_name: "PreToolUse".to_string(),
        tool_name: Some("Read".to_string()),
        label: "Reading file".to_string(),
        group: "autonomous".to_string(),
        context: Some(r#"{"file_path":"/foo/bar.rs"}"#.to_string()),
        source: "hook".to_string(),
    });

    state
        .live_sessions
        .write()
        .await
        .insert("live-hook-test".to_string(), session);

    // Build app from the state (already Arc<AppState>)
    let app = crate::api_routes(state);

    // Should return hook events from live session (not SQLite)
    let (status, body) = do_get(app, "/api/sessions/live-hook-test/hook-events").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let hook_events = json["hookEvents"].as_array().unwrap();
    assert_eq!(hook_events.len(), 2);
    assert_eq!(hook_events[0]["eventName"], "SessionStart");
    assert_eq!(hook_events[0]["group"], "needs_you");
    assert_eq!(hook_events[1]["eventName"], "PreToolUse");
    assert_eq!(hook_events[1]["toolName"], "Read");
    assert_eq!(hook_events[1]["label"], "Reading file");
}

// ========================================================================
// Hook block merge into ?format=block tests
// ========================================================================

#[tokio::test]
async fn get_messages_block_format_includes_db_hook_events() {
    use claude_view_db::hook_events_queries;

    let tmp = tempfile::TempDir::new().unwrap();
    let db = test_db().await;

    // Create a minimal JSONL session file on disk
    let jsonl_path = tmp.path().join("block-hook-test.jsonl");
    std::fs::write(
        &jsonl_path,
        r#"{"type":"human","message":{"role":"user","content":"hello"},"timestamp":"2026-01-01T00:00:00Z"}
"#,
    )
    .unwrap();

    // Insert a session row so resolve_session_file_path() can find the JSONL
    let mut session = make_session("block-hook-test", "project-a", 1735689600);
    session.file_path = jsonl_path.to_string_lossy().to_string();
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    // Insert hook events into DB for this session
    let events = vec![claude_view_db::HookEventRow {
        timestamp: 1735689600,
        event_name: "PreToolUse".into(),
        tool_name: Some("Bash".into()),
        label: "Running: git status".into(),
        group_name: "autonomous".into(),
        context: None,
        source: "hook".into(),
    }];
    hook_events_queries::insert_hook_events(&db, "block-hook-test", &events)
        .await
        .unwrap();

    // Request blocks via the API
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/block-hook-test/messages?format=block").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let blocks = json["blocks"].as_array().unwrap();

    // Should contain the merged hook event as a ProgressBlock(Hook)
    let hook_blocks: Vec<_> = blocks
        .iter()
        .filter(|b| b["type"] == "progress" && b["variant"] == "hook")
        .collect();
    assert_eq!(hook_blocks.len(), 1);
    assert_eq!(hook_blocks[0]["data"]["hookEvent"], "PreToolUse");
    assert_eq!(hook_blocks[0]["data"]["hookName"], "Bash");
    assert_eq!(
        hook_blocks[0]["data"]["statusMessage"],
        "Running: git status"
    );
}

#[tokio::test]
async fn get_messages_block_format_merges_both_channels() {
    use claude_view_db::hook_events_queries;

    let tmp = tempfile::TempDir::new().unwrap();
    let db = test_db().await;

    // Create JSONL with a Channel A hook_progress block
    let jsonl_path = tmp.path().join("both-channels-test.jsonl");
    std::fs::write(
        &jsonl_path,
        r#"{"type":"human","message":{"role":"user","content":"hello"},"timestamp":"2026-01-01T00:00:00Z"}
{"type":"progress","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"Read","command":"","statusMessage":"Reading file"},"timestamp":"2026-01-01T00:00:01Z"}
"#,
    )
    .unwrap();

    let mut session = make_session("both-channels-test", "project-a", 1735689600);
    session.file_path = jsonl_path.to_string_lossy().to_string();
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    // Insert DB hook events (Channel B) — BOTH channels should render
    let events = vec![claude_view_db::HookEventRow {
        timestamp: 1735689601,
        event_name: "PreToolUse".into(),
        tool_name: Some("Bash".into()),
        label: "Running: ls".into(),
        group_name: "autonomous".into(),
        context: None,
        source: "hook".into(),
    }];
    hook_events_queries::insert_hook_events(&db, "both-channels-test", &events)
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(
        app,
        "/api/sessions/both-channels-test/messages?format=block",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let blocks = json["blocks"].as_array().unwrap();

    // Channel A (from JSONL) and Channel B (from DB) both present
    let hook_blocks: Vec<_> = blocks
        .iter()
        .filter(|b| b["type"] == "progress" && b["variant"] == "hook")
        .collect();
    assert!(
        hook_blocks.len() >= 2,
        "Both Channel A and Channel B hook blocks should coexist, got {}",
        hook_blocks.len()
    );

    // DB hook events have "hook-db-" prefix
    let db_hook_blocks: Vec<_> = blocks
        .iter()
        .filter(|b| {
            b["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("hook-db-"))
        })
        .collect();
    assert_eq!(
        db_hook_blocks.len(),
        1,
        "Channel B hook block must be present alongside Channel A"
    );
}
