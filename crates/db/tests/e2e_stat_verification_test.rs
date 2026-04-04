// End-to-end stat verification test.
//
// Writes JSONL fixture → parses with parse_bytes → inserts into DB via
// upsert_parsed_session → queries dashboard stats → asserts every number
// matches the fixture's ground truth.
//
// This is the critical test that proves: what the user sees = what actually happened.

use claude_view_db::indexer_parallel::{parse_bytes, ParsedSession, CURRENT_PARSE_VERSION};

/// Build a realistic multi-turn JSONL with known ground truth.
fn fixture_jsonl() -> Vec<u8> {
    let lines = vec![
        // Turn 1: user asks, assistant reads + edits
        r#"{"type":"user","message":{"content":"Fix the authentication bug","timestamp":"2026-04-01T10:00:00Z"}}"#,
        r#"{"type":"assistant","message":{"id":"msg_001","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/auth.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/auth.rs","old_string":"fn login()","new_string":"fn login(ctx: &Context)"}}],"usage":{"input_tokens":50000,"output_tokens":8000,"cache_read_input_tokens":20000,"cache_creation_input_tokens":5000},"model":"claude-opus-4-6"}}"#,
        // Turn 2: user asks for tests, assistant writes + runs bash
        r#"{"type":"user","message":{"content":"Now write tests for the fix","timestamp":"2026-04-01T10:05:00Z"}}"#,
        r##"{"type":"assistant","message":{"id":"msg_002","content":[{"type":"tool_use","name":"Write","input":{"file_path":"/tests/auth_test.rs","content":"#[test] fn test_login() {}"}},{"type":"tool_use","name":"Bash","input":{"command":"cargo test auth"}}],"usage":{"input_tokens":40000,"output_tokens":12000,"cache_read_input_tokens":30000,"cache_creation_input_tokens":3000},"model":"claude-opus-4-6"}}"##,
        // Turn 3: user confirms, assistant reads one more file
        r#"{"type":"user","message":{"content":"Check the integration test too","timestamp":"2026-04-01T10:08:00Z"}}"#,
        r#"{"type":"assistant","message":{"id":"msg_003","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/tests/integration.rs"}},{"type":"text","text":"All tests pass."}],"usage":{"input_tokens":30000,"output_tokens":5000,"cache_read_input_tokens":15000,"cache_creation_input_tokens":2000},"model":"claude-opus-4-6"}}"#,
    ];
    lines.join("\n").into_bytes()
}

// Ground truth for the fixture above:
const EXPECTED_TURN_COUNT: i32 = 3;
const EXPECTED_TOOL_READ: i32 = 2; // Read auth.rs + Read integration.rs
const EXPECTED_TOOL_EDIT: i32 = 1; // Edit auth.rs
const EXPECTED_TOOL_WRITE: i32 = 1; // Write auth_test.rs
const EXPECTED_TOOL_BASH: i32 = 1; // Bash cargo test
const EXPECTED_TOOL_TOTAL: i32 = 5; // 2+1+1+1
const EXPECTED_INPUT_TOKENS: i64 = 120_000; // 50k + 40k + 30k
const EXPECTED_OUTPUT_TOKENS: i64 = 25_000; // 8k + 12k + 5k
const EXPECTED_CACHE_READ: i64 = 65_000; // 20k + 30k + 15k
const EXPECTED_CACHE_CREATE: i64 = 10_000; // 5k + 3k + 2k

#[test]
fn parse_bytes_matches_ground_truth() {
    let data = fixture_jsonl();
    let result = parse_bytes(&data);

    assert_eq!(result.deep.turn_count as i32, EXPECTED_TURN_COUNT);
    assert_eq!(result.deep.tool_counts.read as i32, EXPECTED_TOOL_READ);
    assert_eq!(result.deep.tool_counts.edit as i32, EXPECTED_TOOL_EDIT);
    assert_eq!(result.deep.tool_counts.write as i32, EXPECTED_TOOL_WRITE);
    assert_eq!(result.deep.tool_counts.bash as i32, EXPECTED_TOOL_BASH);
    assert_eq!(
        result.deep.total_input_tokens as i64, EXPECTED_INPUT_TOKENS,
        "input tokens: expected {}, got {}",
        EXPECTED_INPUT_TOKENS, result.deep.total_input_tokens
    );
    assert_eq!(
        result.deep.total_output_tokens as i64, EXPECTED_OUTPUT_TOKENS,
        "output tokens: expected {}, got {}",
        EXPECTED_OUTPUT_TOKENS, result.deep.total_output_tokens
    );
    assert_eq!(
        result.deep.cache_read_tokens as i64, EXPECTED_CACHE_READ,
        "cache read tokens: expected {}, got {}",
        EXPECTED_CACHE_READ, result.deep.cache_read_tokens
    );
    assert_eq!(
        result.deep.cache_creation_tokens as i64, EXPECTED_CACHE_CREATE,
        "cache creation tokens: expected {}, got {}",
        EXPECTED_CACHE_CREATE, result.deep.cache_creation_tokens
    );
}

#[tokio::test]
async fn parsed_session_roundtrips_through_db() {
    let data = fixture_jsonl();
    let result = parse_bytes(&data);
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();

    // Build ParsedSession from parse result
    let session = ParsedSession {
        id: "e2e-test-session".into(),
        project_id: "test-project".into(),
        project_display_name: "Test Project".into(),
        project_path: "/tmp/test".into(),
        file_path: "/tmp/test/session.jsonl".into(),
        preview: "Fix the authentication bug".into(),
        summary: None,
        message_count: 6,
        last_message_at: now,
        first_message_at: now - 480,
        git_branch: Some("fix/auth".into()),
        is_sidechain: false,
        size_bytes: data.len() as i64,
        last_message: "Check the integration test too".into(),
        turn_count: result.deep.turn_count as i32,
        tool_counts_edit: result.deep.tool_counts.edit as i32,
        tool_counts_read: result.deep.tool_counts.read as i32,
        tool_counts_bash: result.deep.tool_counts.bash as i32,
        tool_counts_write: result.deep.tool_counts.write as i32,
        files_touched: "[]".into(),
        skills_used: "[]".into(),
        user_prompt_count: result.deep.user_prompt_count as i32,
        api_call_count: 3,
        tool_call_count: result.deep.tool_call_count as i32,
        files_read: "[]".into(),
        files_edited: "[]".into(),
        files_read_count: 0,
        files_edited_count: 0,
        reedited_files_count: 0,
        duration_seconds: 480,
        commit_count: 0,
        total_input_tokens: result.deep.total_input_tokens as i64,
        total_output_tokens: result.deep.total_output_tokens as i64,
        cache_read_tokens: result.deep.cache_read_tokens as i64,
        cache_creation_tokens: result.deep.cache_creation_tokens as i64,
        thinking_block_count: 0,
        turn_duration_avg_ms: None,
        turn_duration_max_ms: None,
        turn_duration_total_ms: None,
        api_error_count: 0,
        api_retry_count: 0,
        compaction_count: 0,
        hook_blocked_count: 0,
        agent_spawn_count: 0,
        bash_progress_count: 0,
        hook_progress_count: 0,
        mcp_progress_count: 0,
        summary_text: None,
        parse_version: CURRENT_PARSE_VERSION,
        file_size_at_index: data.len() as i64,
        file_mtime_at_index: now,
        lines_added: 0,
        lines_removed: 0,
        loc_source: 0,
        ai_lines_added: 0,
        ai_lines_removed: 0,
        work_type: None,
        primary_model: Some("claude-opus-4-6".into()),
        total_task_time_seconds: None,
        longest_task_seconds: None,
        longest_task_preview: None,
        total_cost_usd: None,
        slug: None,
        entrypoint: None,
    };

    db.upsert_parsed_session(&session).await.unwrap();

    // Now query back from valid_sessions and verify every stat
    let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
            total_input_tokens, total_output_tokens,
            cache_read_tokens, cache_creation_tokens,
            tool_counts_read, tool_counts_edit,
            tool_counts_bash, tool_counts_write,
            tool_call_count, turn_count
        FROM valid_sessions WHERE id = ?1"#,
    )
    .bind("e2e-test-session")
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(row.0, EXPECTED_INPUT_TOKENS, "DB input tokens");
    assert_eq!(row.1, EXPECTED_OUTPUT_TOKENS, "DB output tokens");
    assert_eq!(row.2, EXPECTED_CACHE_READ, "DB cache read tokens");
    assert_eq!(row.3, EXPECTED_CACHE_CREATE, "DB cache creation tokens");
    assert_eq!(row.4, EXPECTED_TOOL_READ as i64, "DB tool read count");
    assert_eq!(row.5, EXPECTED_TOOL_EDIT as i64, "DB tool edit count");
    assert_eq!(row.6, EXPECTED_TOOL_BASH as i64, "DB tool bash count");
    assert_eq!(row.7, EXPECTED_TOOL_WRITE as i64, "DB tool write count");
    assert_eq!(row.8, EXPECTED_TOOL_TOTAL as i64, "DB total tool calls");
    assert_eq!(row.9, EXPECTED_TURN_COUNT as i64, "DB turn count");

    // Verify dashboard aggregation includes this session
    let (session_count, total_input, total_tool_calls): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
            COUNT(*),
            COALESCE(SUM(total_input_tokens), 0),
            COALESCE(SUM(tool_call_count), 0)
        FROM valid_sessions"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(session_count, 1, "exactly one session in valid_sessions");
    assert_eq!(
        total_input, EXPECTED_INPUT_TOKENS,
        "aggregated input tokens"
    );
    assert_eq!(
        total_tool_calls, EXPECTED_TOOL_TOTAL as i64,
        "aggregated tool calls"
    );
}
