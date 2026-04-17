//! Shared test helpers and basic endpoint tests for sessions module.

#![cfg(test)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use claude_view_core::session_catalog::CatalogRow;
use claude_view_core::{
    Message, PaginatedMessages, ParsedSession, SessionInfo, SessionMetadata, ToolCounts,
};
use claude_view_db::Database;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::state::AppState;

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

/// Harness for tests that exercise the JSONL-first `/api/sessions*` routes.
///
/// Owns the live `AppState` so tests can seed both the in-memory
/// `SessionCatalog` (authoritative file-path lookup) and the SQLite mirror
/// (archive / commit / skills / reedit enrichment). The pre-JSONL-first
/// test harness only wrote DB rows — those tests could not observe the new
/// pipeline and had to be `#[ignore]`d.
///
/// The inner `_tempdir` is kept alive for the lifetime of the fixture
/// because every catalog row points at a real JSONL file on disk.
pub(super) struct CatalogFixture {
    pub state: Arc<AppState>,
    /// Keeps all seeded JSONL files alive for the lifetime of the test.
    _tempdir: TempDir,
}

impl CatalogFixture {
    pub async fn new() -> Self {
        let db = test_db().await;
        let state = AppState::new(db);
        let tempdir = tempfile::tempdir().expect("tempdir");
        Self {
            state,
            _tempdir: tempdir,
        }
    }

    /// Build a `Router` bound to this fixture's `AppState`.
    ///
    /// Call after seeding — the router captures the state via `Arc` clone,
    /// so later mutations are still visible inside the handlers.
    pub fn app(&self) -> axum::Router {
        crate::routes::api_routes(self.state.clone())
    }

    /// Seed a session in both the catalog and DB.
    ///
    /// Writes a minimal valid JSONL file (a session summary, one user text,
    /// and one assistant text with usage) so `session_stats::extract_stats`
    /// returns sensible defaults. The DB insert uses `insert_session`, which
    /// covers every enrichment field the `/api/sessions` list/detail handlers
    /// read (`archived_at` is handled separately via `archive_session` —
    /// callers should chain `.archive()` after this).
    ///
    /// `session` is mutated so its `file_path` points at the real tempfile,
    /// and `modified_at` sets the JSONL mtime (driving `last_ts` in the
    /// catalog row → sort order).
    pub async fn seed(&self, mut session: SessionInfo, project_display: &str) -> SessionInfo {
        let project_id = session.project.clone();
        let jsonl_path = self._tempdir.path().join(format!("{}.jsonl", session.id));

        // Minimal valid JSONL: one user prompt + one assistant turn with usage.
        // Timestamps are anchored at `modified_at` and span `duration_seconds`
        // so:
        //   * `session_stats::extract_stats` reports the caller's duration
        //     (list handler reads it from JSONL, not DB)
        //   * `CatalogRow::sort_ts()` returns a stable value the caller can
        //     predict when writing time-range filter tests.
        let first_ts_epoch: i64 = session.modified_at;
        let last_ts_epoch = first_ts_epoch + session.duration_seconds as i64;
        let first_ts = chrono::DateTime::from_timestamp(first_ts_epoch, 0)
            .expect("first_ts")
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let last_ts = chrono::DateTime::from_timestamp(last_ts_epoch, 0)
            .expect("last_ts")
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        // Choose a primary model. `insert_session` does not persist
        // `primary_model`, so tests that filter by model rely on what's
        // written into the JSONL here.
        let primary_model = session
            .primary_model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4".to_string());

        // Emit one `Read` tool_use per `files_read_count` and one `Edit` per
        // `files_edited_count` so the JSONL-derived counts match the caller's
        // expectation. Split across synthetic assistant messages so each
        // `files_edited_count` bump requires a dedicated tool_use block.
        let mut tool_blocks: Vec<String> = Vec::new();
        for i in 0..session.files_read_count {
            tool_blocks.push(format!(
                r#"{{"type":"tool_use","id":"read_{i}","name":"Read","input":{{}}}}"#
            ));
        }
        for i in 0..session.files_edited_count {
            tool_blocks.push(format!(
                r#"{{"type":"tool_use","id":"edit_{i}","name":"Edit","input":{{}}}}"#
            ));
        }

        // Token counts come from JSONL; if the caller set totals, distribute
        // them across the single assistant message here.
        let input_tokens = session.total_input_tokens.unwrap_or(100);
        let output_tokens = session.total_output_tokens.unwrap_or(50);

        let assistant_content = if tool_blocks.is_empty() {
            r#"[{"type":"text","text":"Seed reply"}]"#.to_string()
        } else {
            format!("[{}]", tool_blocks.join(","))
        };

        // git_branch is serialized as `gitBranch` (camelCase) in JSONL to match
        // the real Claude Code transcript shape. `session_stats::extract_stats`
        // reads the first non-empty occurrence as the session branch.
        let git_branch_frag = match &session.git_branch {
            Some(b) if !b.is_empty() => format!(r#","gitBranch":"{}""#, b),
            _ => String::new(),
        };

        let jsonl_body = format!(
            "{user}\n{assistant}\n",
            user = format!(
                r#"{{"type":"user","timestamp":"{first_ts}"{branch},"message":{{"role":"user","content":[{{"type":"text","text":"Seed prompt"}}]}}}}"#,
                branch = git_branch_frag,
            ),
            assistant = format!(
                r#"{{"type":"assistant","timestamp":"{last_ts}"{branch},"message":{{"id":"msg_{id}","model":"{model}","role":"assistant","content":{content},"usage":{{"input_tokens":{input},"output_tokens":{output}}},"stop_reason":"end_turn"}}}}"#,
                branch = git_branch_frag,
                id = session.id,
                model = primary_model,
                content = assistant_content,
                input = input_tokens,
                output = output_tokens,
            ),
        );
        std::fs::write(&jsonl_path, jsonl_body).expect("write JSONL");

        session.file_path = jsonl_path.to_string_lossy().into_owned();

        // Persist DB row with the caller's desired enrichment fields.
        self.state
            .db
            .insert_session(&session, &project_id, project_display)
            .await
            .expect("insert_session");

        // Register the catalog row so the handler's authoritative path
        // resolver (`state.session_catalog.get`) sees this session.
        //
        // `mtime` is set to the caller's requested `modified_at` (NOT the
        // real filesystem mtime) because `build_session_info` copies
        // `row.mtime` into `SessionInfo.modified_at`, and the
        // `/api/sessions` list handler sorts descending by that field.
        // Tests rely on a deterministic ordering.
        let meta = std::fs::metadata(&jsonl_path).expect("jsonl metadata");
        let existing = self.state.session_catalog.list(
            &claude_view_core::session_catalog::Filter::default(),
            claude_view_core::session_catalog::Sort::LastTsDesc,
            usize::MAX,
        );
        let mut rows: Vec<CatalogRow> = existing.into_iter().collect();
        rows.retain(|r| r.id != session.id);
        rows.push(CatalogRow {
            id: session.id.clone(),
            file_path: jsonl_path,
            is_compressed: false,
            bytes: meta.len(),
            mtime: session.modified_at,
            project_id,
            first_ts: Some(first_ts_epoch),
            last_ts: Some(last_ts_epoch),
        });
        self.state.session_catalog.replace_all(rows);

        session
    }
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
        ownership: None,
        pending_interaction: None,
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
