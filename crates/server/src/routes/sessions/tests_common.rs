//! Shared test helpers and basic endpoint tests for sessions module.

#![cfg(test)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use claude_view_core::{
    Message, PaginatedMessages, ParsedSession, SessionInfo, SessionMetadata, ToolCounts,
};
use claude_view_db::Database;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

use super::types::DerivedMetrics;

pub(super) async fn test_db() -> Database {
    Database::new_in_memory().await.expect("in-memory DB")
}

pub(super) fn build_app(db: Database) -> axum::Router {
    crate::create_app(db)
}

pub(super) async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
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

pub(super) fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
    SessionInfo {
        id: id.to_string(),
        project: project.to_string(),
        project_path: format!("/home/user/{}", project),
        display_name: project.to_string(),
        git_root: None,
        file_path: format!("/path/{}.jsonl", id),
        modified_at,
        size_bytes: 2048,
        preview: "Test".to_string(),
        last_message: "Last msg".to_string(),
        files_touched: vec![],
        skills_used: vec![],
        tool_counts: ToolCounts::default(),
        message_count: 10,
        turn_count: 5,
        summary: None,
        git_branch: None,
        is_sidechain: false,
        deep_indexed: true,
        total_input_tokens: Some(10000),
        total_output_tokens: Some(5000),
        total_cache_read_tokens: None,
        total_cache_creation_tokens: None,
        turn_count_api: Some(10),
        primary_model: Some("claude-sonnet-4".to_string()),
        user_prompt_count: 10,
        api_call_count: 20,
        tool_call_count: 50,
        files_read: vec!["a.rs".to_string()],
        files_edited: vec!["b.rs".to_string()],
        files_read_count: 20,
        files_edited_count: 5,
        reedited_files_count: 2,
        duration_seconds: 600,
        commit_count: 0,
        thinking_block_count: 0,
        turn_duration_avg_ms: None,
        turn_duration_max_ms: None,
        api_error_count: 0,
        compaction_count: 0,
        agent_spawn_count: 0,
        bash_progress_count: 0,
        hook_progress_count: 0,
        mcp_progress_count: 0,

        parse_version: 0,
        lines_added: 0,
        lines_removed: 0,
        loc_source: 0,
        category_l1: None,
        category_l2: None,
        category_l3: None,
        category_confidence: None,
        category_source: None,
        classified_at: None,
        prompt_word_count: None,
        correction_count: 0,
        same_file_edit_count: 0,
        total_task_time_seconds: None,
        longest_task_seconds: None,
        longest_task_preview: None,
        first_message_at: None,
        total_cost_usd: None,
        slug: None,
        entrypoint: None,
    }
}

/// Helper: create a LiveSession with a given file_path (no DB insertion).
pub(super) fn make_live_session(id: &str, file_path: &str) -> crate::live::state::LiveSession {
    use crate::live::state::{
        AgentState, AgentStateGroup, HookFields, JsonlFields, LiveSession, SessionStatus,
    };

    LiveSession {
        id: id.to_string(),
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
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            pid: None,
            title: "Test session".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 0,
            last_activity_at: 1000,
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
            file_path: file_path.to_string(),
            project: "test-project".to_string(),
            project_display_name: "test-project".to_string(),
            project_path: "/tmp/test".to_string(),
            ..JsonlFields::default()
        },
        session_kind: None,
        entrypoint: None,
    }
}

// ========================================================================
// Basic tests
// ========================================================================

#[test]
fn test_parsed_session_serialization() {
    let session = ParsedSession {
        messages: vec![
            Message::user("Hello Claude!"),
            Message::assistant("Hello! How can I help?"),
        ],
        metadata: SessionMetadata {
            total_messages: 2,
            tool_call_count: 0,
        },
    };

    let json = serde_json::to_string(&session).unwrap();
    assert!(json.contains("\"role\":\"user\""));
    assert!(json.contains("\"role\":\"assistant\""));
    assert!(json.contains("\"totalMessages\":2"));
}

#[test]
fn test_session_path_construction() {
    let project_dir = "Users-user-dev-myproject";
    let session_id = "abc123-def456";

    let base = PathBuf::from("/Users/user/.claude/projects");
    let session_path = base
        .join(project_dir)
        .join(session_id)
        .with_extension("jsonl");

    assert_eq!(
        session_path.to_string_lossy(),
        "/Users/user/.claude/projects/Users-user-dev-myproject/abc123-def456.jsonl"
    );
}

#[test]
fn test_derived_metrics_calculation() {
    let session = make_session("test", "project", 1700000000);
    let metrics = DerivedMetrics::from(&session);

    // (10000 + 5000) / 10 = 1500.0
    assert_eq!(metrics.tokens_per_prompt, Some(1500.0));
    // 2 / 5 = 0.4
    assert_eq!(metrics.reedit_rate, Some(0.4));
    // 50 / 20 = 2.5
    assert_eq!(metrics.tool_density, Some(2.5));
    // 5 / (600 / 60) = 0.5
    assert_eq!(metrics.edit_velocity, Some(0.5));
    // 20 / 5 = 4.0
    assert_eq!(metrics.read_to_edit_ratio, Some(4.0));
}

#[test]
fn test_paginated_messages_serialization() {
    let result = PaginatedMessages {
        messages: vec![Message::user("Hello"), Message::assistant("Hi")],
        total: 100,
        offset: 0,
        limit: 2,
        has_more: true,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"total\":100"));
    assert!(json.contains("\"hasMore\":true"));
}
