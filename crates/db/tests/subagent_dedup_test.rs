// Subagent non-duplication test.
//
// Verifies that subagent JSONL files are merged into their parent session
// (tokens, tools, turns) and do NOT appear as separate rows in valid_sessions.
// This proves the monitor dashboard won't double-count subagent work.

use claude_view_db::indexer_parallel::parse_bytes;

/// Helper: create a minimal JSONL with the given number of tool calls and token usage.
fn make_session_jsonl(
    user_msg: &str,
    tools: &[(&str, &str)],       // (tool_name, file_path)
    input_tokens: u64,
    output_tokens: u64,
) -> Vec<u8> {
    let mut lines = Vec::new();

    // User message
    lines.push(format!(
        r#"{{"type":"user","message":{{"content":"{}"}}}}"#,
        user_msg
    ));

    // Assistant message with tool calls + usage
    let tool_blocks: Vec<String> = tools
        .iter()
        .map(|(name, path)| {
            format!(
                r#"{{"type":"tool_use","name":"{}","input":{{"file_path":"{}"}}}}"#,
                name, path
            )
        })
        .collect();

    let content_json = tool_blocks.join(",");
    lines.push(format!(
        r#"{{"type":"assistant","message":{{"id":"msg_test","content":[{}],"usage":{{"input_tokens":{},"output_tokens":{}}}}}}}"#,
        content_json, input_tokens, output_tokens
    ));

    lines.join("\n").into_bytes()
}

#[test]
fn subagent_tokens_merged_into_parent() {
    let parent_data = make_session_jsonl(
        "Fix the bug",
        &[("Read", "/src/main.rs"), ("Edit", "/src/main.rs")],
        1000,
        500,
    );
    let sub1_data = make_session_jsonl(
        "Subagent 1: research",
        &[("Read", "/src/lib.rs"), ("Bash", "cargo test")],
        2000,
        800,
    );
    let sub2_data = make_session_jsonl(
        "Subagent 2: write tests",
        &[("Write", "/tests/test.rs"), ("Read", "/src/lib.rs")],
        3000,
        1200,
    );

    let mut parent = parse_bytes(&parent_data);
    let sub1 = parse_bytes(&sub1_data);
    let sub2 = parse_bytes(&sub2_data);

    // Record parent-only values before merge
    let parent_input_tokens = parent.deep.total_input_tokens as i64;
    let parent_output_tokens = parent.deep.total_output_tokens as i64;
    let parent_tool_read = parent.deep.tool_counts.read as i64;
    let parent_tool_edit = parent.deep.tool_counts.edit as i64;

    // Merge subagents (same logic as merge_subagent_parse_result)
    parent.deep.total_input_tokens += sub1.deep.total_input_tokens;
    parent.deep.total_output_tokens += sub1.deep.total_output_tokens;
    parent.deep.cache_read_tokens += sub1.deep.cache_read_tokens;
    parent.deep.cache_creation_tokens += sub1.deep.cache_creation_tokens;
    parent.deep.tool_counts.edit += sub1.deep.tool_counts.edit;
    parent.deep.tool_counts.read += sub1.deep.tool_counts.read;
    parent.deep.tool_counts.bash += sub1.deep.tool_counts.bash;
    parent.deep.tool_counts.write += sub1.deep.tool_counts.write;
    parent.deep.tool_call_count += sub1.deep.tool_call_count;

    parent.deep.total_input_tokens += sub2.deep.total_input_tokens;
    parent.deep.total_output_tokens += sub2.deep.total_output_tokens;
    parent.deep.cache_read_tokens += sub2.deep.cache_read_tokens;
    parent.deep.cache_creation_tokens += sub2.deep.cache_creation_tokens;
    parent.deep.tool_counts.edit += sub2.deep.tool_counts.edit;
    parent.deep.tool_counts.read += sub2.deep.tool_counts.read;
    parent.deep.tool_counts.bash += sub2.deep.tool_counts.bash;
    parent.deep.tool_counts.write += sub2.deep.tool_counts.write;
    parent.deep.tool_call_count += sub2.deep.tool_call_count;

    // Verify: merged tokens = parent + sub1 + sub2
    assert_eq!(
        parent.deep.total_input_tokens as i64,
        parent_input_tokens + 2000 + 3000,
        "input tokens must be parent + all subagents"
    );
    assert_eq!(
        parent.deep.total_output_tokens as i64,
        parent_output_tokens + 800 + 1200,
        "output tokens must be parent + all subagents"
    );

    // Verify: merged tool counts = parent + sub1 + sub2
    assert_eq!(
        parent.deep.tool_counts.read as i64,
        parent_tool_read + 1 + 1,
        "Read count must include subagent reads"
    );
    assert_eq!(
        parent.deep.tool_counts.edit as i64,
        parent_tool_edit + 0 + 0,
        "Edit count must only include parent edits"
    );
    assert_eq!(parent.deep.tool_counts.bash, 1, "Bash from sub1");
    assert_eq!(parent.deep.tool_counts.write, 1, "Write from sub2");

    // Verify: total tool_call_count = sum of all
    assert_eq!(
        parent.deep.tool_call_count as i64,
        2 + 2 + 2,
        "total tool calls must be parent(2) + sub1(2) + sub2(2)"
    );
}

#[tokio::test]
async fn valid_sessions_view_excludes_sidechains() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();

    // Insert a primary session
    sqlx::query(
        r#"INSERT INTO sessions (
            id, project_id, file_path, preview, project_path,
            total_input_tokens, total_output_tokens,
            tool_counts_read, tool_counts_edit, tool_counts_bash, tool_counts_write,
            tool_call_count, last_message_at, size_bytes, last_message,
            files_touched, skills_used, files_read, files_edited
        ) VALUES (
            'parent-session', 'proj', '/tmp/parent.jsonl', 'test', '/tmp',
            6000, 2500, 4, 1, 1, 1, 6, ?1, 1024, '',
            '[]', '[]', '[]', '[]'
        )"#,
    )
    .bind(now)
    .execute(db.pool())
    .await
    .unwrap();

    // Insert a sidechain session (should be excluded from valid_sessions)
    sqlx::query(
        r#"INSERT INTO sessions (
            id, project_id, file_path, preview, project_path,
            is_sidechain,
            total_input_tokens, total_output_tokens,
            tool_counts_read, tool_counts_edit, tool_counts_bash, tool_counts_write,
            tool_call_count, last_message_at, size_bytes, last_message,
            files_touched, skills_used, files_read, files_edited
        ) VALUES (
            'sidechain-session', 'proj', '/tmp/sidechain.jsonl', 'test', '/tmp',
            1,
            1000, 500, 1, 0, 0, 0, 1, ?1, 512, '',
            '[]', '[]', '[]', '[]'
        )"#,
    )
    .bind(now)
    .execute(db.pool())
    .await
    .unwrap();

    // Query valid_sessions — must return only the primary session
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM valid_sessions")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 1, "valid_sessions must exclude sidechain sessions");

    // Verify token totals from valid_sessions match parent only (no double-counting)
    let (total_input, total_output): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(total_input_tokens), 0), COALESCE(SUM(total_output_tokens), 0) FROM valid_sessions",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        total_input, 6000,
        "token sum must only include primary session"
    );
    assert_eq!(
        total_output, 2500,
        "token sum must only include primary session"
    );

    // Verify tool totals from valid_sessions
    let (tool_total,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(tool_call_count), 0) FROM valid_sessions",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        tool_total, 6,
        "tool count must only include primary session"
    );
}

#[tokio::test]
async fn archived_sessions_excluded_from_valid_sessions() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();

    // Insert active session
    sqlx::query(
        r#"INSERT INTO sessions (
            id, project_id, file_path, preview, project_path,
            total_input_tokens, last_message_at, size_bytes, last_message,
            files_touched, skills_used, files_read, files_edited
        ) VALUES (
            'active-session', 'proj', '/tmp/active.jsonl', 'test', '/tmp',
            5000, ?1, 1024, '', '[]', '[]', '[]', '[]'
        )"#,
    )
    .bind(now)
    .execute(db.pool())
    .await
    .unwrap();

    // Insert archived session
    sqlx::query(
        r#"INSERT INTO sessions (
            id, project_id, file_path, preview, project_path,
            total_input_tokens, last_message_at, size_bytes, last_message,
            archived_at, files_touched, skills_used, files_read, files_edited
        ) VALUES (
            'archived-session', 'proj', '/tmp/archived.jsonl', 'test', '/tmp',
            3000, ?1, 512, '', '2026-01-01T00:00:00Z',
            '[]', '[]', '[]', '[]'
        )"#,
    )
    .bind(now)
    .execute(db.pool())
    .await
    .unwrap();

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM valid_sessions")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 1, "valid_sessions must exclude archived sessions");

    let (total_input,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(total_input_tokens), 0) FROM valid_sessions",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        total_input, 5000,
        "token sum must exclude archived session"
    );
}
