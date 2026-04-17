//! Tests for the per-turn breakdown endpoint and scanner.

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_core::{SessionInfo, ToolCounts};
    use claude_view_db::Database;
    use tower::ServiceExt;

    use crate::routes::turns::scanner::{scan_turns, truncate_preview};
    use crate::routes::turns::types::TurnInfo;

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

    fn make_session(id: &str, file_path: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: "test-project".to_string(),
            project_path: "/home/user/test-project".to_string(),
            display_name: "test-project".to_string(),
            git_root: None,
            file_path: file_path.to_string(),
            modified_at: 1700000000,
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
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
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
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        }
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_turns_not_found() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/nonexistent/turns").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_turns_file_gone() {
        let db = test_db().await;
        let session = make_session("turns-gone", "/nonexistent/path.jsonl");
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-gone/turns").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_turns_empty_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("empty.jsonl");
        std::fs::write(&session_file, "").unwrap();

        let session = make_session("turns-empty", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-empty/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(turns.is_empty(), "Empty session should return empty array");
    }

    #[tokio::test]
    async fn test_get_turns_valid_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("multi_turn.jsonl");

        // Two-turn session with turn_duration system messages
        let jsonl = r#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"Read and fix auth.rs"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":"2026-01-28T10:01:00Z","message":{"role":"assistant","content":[{"type":"text","text":"I'll read the file first."}]}}
{"type":"user","uuid":"u2","parentUuid":"a1","timestamp":"2026-01-28T10:02:00Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"fn authenticate() { todo!() }"}]}}
{"type":"assistant","uuid":"a2","parentUuid":"u2","timestamp":"2026-01-28T10:03:00Z","message":{"role":"assistant","content":[{"type":"text","text":"I've fixed it."}]}}
{"type":"system","uuid":"s1","timestamp":"2026-01-28T10:03:05Z","subtype":"turn_duration","durationMs":180000,"isMeta":true}
{"type":"user","uuid":"u3","timestamp":"2026-01-28T10:05:00Z","message":{"role":"user","content":"Now run the tests please"}}
{"type":"assistant","uuid":"a3","parentUuid":"u3","timestamp":"2026-01-28T10:07:00Z","message":{"role":"assistant","content":[{"type":"text","text":"All tests pass!"}]}}
{"type":"system","uuid":"s2","timestamp":"2026-01-28T10:07:05Z","subtype":"turn_duration","durationMs":120000,"isMeta":true}
"#;
        std::fs::write(&session_file, jsonl).unwrap();

        let session = make_session("turns-ok", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-ok/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();

        assert_eq!(turns.len(), 2, "Should detect 2 turns");

        // Turn 1
        assert_eq!(turns[0]["index"], 1);
        assert!(turns[0]["startedAt"].is_number());
        assert!(turns[0]["wallClockSeconds"].is_number());
        assert_eq!(turns[0]["ccDurationMs"], 180000);
        assert_eq!(turns[0]["promptPreview"], "Read and fix auth.rs");

        // Turn 2
        assert_eq!(turns[1]["index"], 2);
        assert_eq!(turns[1]["ccDurationMs"], 120000);
        assert_eq!(turns[1]["promptPreview"], "Now run the tests please");

        // Wall clock for turn 1: from 10:00:00 to 10:05:00 (next turn) = 300 seconds
        // But actually turn 1 ends at the last message before turn 2 starts.
        // The turn_duration system message at 10:03:05 is the last message before turn 2 at 10:05:00.
        let wall1 = turns[0]["wallClockSeconds"].as_i64().unwrap();
        // Turn 1: started 10:00:00, last timestamp before turn 2 is 10:03:05 = 185 seconds
        assert_eq!(
            wall1, 185,
            "Wall clock should be from turn start to last message before next turn"
        );

        // Wall clock for turn 2: from 10:05:00 to 10:07:05 = 125 seconds
        let wall2 = turns[1]["wallClockSeconds"].as_i64().unwrap();
        assert_eq!(wall2, 125, "Wall clock for last turn should extend to EOF");
    }

    #[tokio::test]
    async fn test_get_turns_response_shape() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("shape.jsonl");

        let jsonl = r#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"Hello world"}}
{"type":"assistant","uuid":"a1","timestamp":"2026-01-28T10:00:30Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
"#;
        std::fs::write(&session_file, jsonl).unwrap();

        let session = make_session("turns-shape", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-shape/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();

        assert_eq!(turns.len(), 1);
        let turn = &turns[0];

        // Verify all expected fields are present
        assert!(turn.get("index").is_some(), "Must have index");
        assert!(turn.get("startedAt").is_some(), "Must have startedAt");
        assert!(
            turn.get("wallClockSeconds").is_some(),
            "Must have wallClockSeconds"
        );
        // ccDurationMs is optional — should be absent (not null) when no turn_duration message
        assert!(
            turn.get("ccDurationMs").is_none(),
            "ccDurationMs should be absent when not available"
        );
        assert!(
            turn.get("promptPreview").is_some(),
            "Must have promptPreview"
        );
    }

    #[tokio::test]
    async fn test_get_turns_prompt_preview_truncation() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("truncate.jsonl");

        let long_prompt = "a".repeat(100); // 100 chars, should be truncated to 60 + "..."
        let jsonl = format!(
            r#"{{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{{"role":"user","content":"{}"}}}}"#,
            long_prompt,
        );
        // Add a trailing newline
        let jsonl = format!("{}\n", jsonl);
        std::fs::write(&session_file, jsonl).unwrap();

        let session = make_session("turns-trunc", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-trunc/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();

        assert_eq!(turns.len(), 1);
        let preview = turns[0]["promptPreview"].as_str().unwrap();
        // 60 chars + "..." = 63 total
        assert_eq!(preview.len(), 63, "Preview should be 60 chars + '...'");
        assert!(
            preview.ends_with("..."),
            "Truncated preview should end with '...'"
        );
    }

    // ========================================================================
    // Unit Tests for scan_turns
    // ========================================================================

    #[test]
    fn test_scan_turns_empty() {
        let turns = scan_turns(b"");
        assert!(turns.is_empty());
    }

    #[test]
    fn test_scan_turns_skips_system_prefixes() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"message":{"role":"user","content":"Hello world"}}
{"type":"assistant","uuid":"a1","timestamp":1700000060,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
{"type":"user","uuid":"u2","timestamp":1700000100,"message":{"role":"user","content":"<system-reminder>Context refresh</system-reminder>"}}
{"type":"assistant","uuid":"a2","timestamp":1700000120,"message":{"role":"assistant","content":[{"type":"text","text":"Ok."}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(
            turns.len(),
            1,
            "System-prefix user messages should not start a new turn"
        );
        assert_eq!(turns[0].prompt_preview, "Hello world");
        // Wall clock: 1700000000 -> 1700000120 = 120s
        assert_eq!(turns[0].wall_clock_seconds, 120);
    }

    #[test]
    fn test_scan_turns_skips_tool_result_continuations() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"message":{"role":"user","content":"Fix the bug"}}
{"type":"assistant","uuid":"a1","timestamp":1700000030,"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu1","name":"Read","input":{}}]}}
{"type":"user","uuid":"u2","timestamp":1700000035,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"file contents"}]}}
{"type":"assistant","uuid":"a2","timestamp":1700000060,"message":{"role":"assistant","content":[{"type":"text","text":"Fixed!"}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(
            turns.len(),
            1,
            "Tool result continuation should not start a new turn"
        );
        assert_eq!(turns[0].prompt_preview, "Fix the bug");
    }

    #[test]
    fn test_scan_turns_with_turn_duration() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"message":{"role":"user","content":"Hello"}}
{"type":"assistant","uuid":"a1","timestamp":1700000030,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
{"type":"system","uuid":"s1","timestamp":1700000035,"subtype":"turn_duration","durationMs":30000,"isMeta":true}
"#;
        let turns = scan_turns(data);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].cc_duration_ms, Some(30000));
        assert_eq!(turns[0].wall_clock_seconds, 35); // 1700000000 -> 1700000035
    }

    #[test]
    fn test_scan_turns_iso_timestamps() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"Hello"}}
{"type":"assistant","uuid":"a1","timestamp":"2026-01-28T10:07:00Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].wall_clock_seconds, 420); // 7 minutes
    }

    #[test]
    fn test_scan_turns_skips_meta_user_messages() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"isMeta":true,"message":{"role":"user","content":"System init"}}
{"type":"user","uuid":"u2","timestamp":1700000010,"message":{"role":"user","content":"Real prompt"}}
{"type":"assistant","uuid":"a1","timestamp":1700000060,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(
            turns.len(),
            1,
            "isMeta user message should not start a turn"
        );
        assert_eq!(turns[0].prompt_preview, "Real prompt");
    }

    #[test]
    fn test_truncate_preview_short() {
        assert_eq!(truncate_preview("short", 60), "short");
    }

    #[test]
    fn test_truncate_preview_exact() {
        let s = "a".repeat(60);
        assert_eq!(truncate_preview(&s, 60), s);
    }

    #[test]
    fn test_truncate_preview_long() {
        let s = "a".repeat(100);
        let result = truncate_preview(&s, 60);
        assert_eq!(result.len(), 63); // 60 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_trims_whitespace() {
        assert_eq!(truncate_preview("  hello  ", 60), "hello");
    }
}
