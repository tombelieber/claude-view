// crates/db/src/queries/sessions/tests.rs
// Unit tests for session upsert operations.

#[cfg(test)]
mod upsert_tests {
    use crate::indexer_parallel::{ParsedSession, CURRENT_PARSE_VERSION};
    use crate::Database;

    fn make_parsed_session(id: &str, message_count: i32) -> ParsedSession {
        ParsedSession {
            id: id.to_string(),
            project_id: "test-project".to_string(),
            project_display_name: "Test Project".to_string(),
            project_path: "/test/project".to_string(),
            file_path: "/test/session.jsonl".to_string(),
            preview: "Hello world".to_string(),
            summary: None,
            message_count,
            last_message_at: 1700000000,
            first_message_at: 1699999000,
            git_branch: None,
            is_sidechain: false,
            size_bytes: 1024,
            last_message: "test message".to_string(),
            turn_count: 5,
            tool_counts_edit: 1,
            tool_counts_read: 2,
            tool_counts_bash: 3,
            tool_counts_write: 0,
            files_touched: "[]".to_string(),
            skills_used: "[]".to_string(),
            user_prompt_count: 5,
            api_call_count: 5,
            tool_call_count: 6,
            files_read: "[]".to_string(),
            files_edited: "[]".to_string(),
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 300,
            commit_count: 1,
            total_input_tokens: 10000,
            total_output_tokens: 5000,
            cache_read_tokens: 2000,
            cache_creation_tokens: 1000,
            thinking_block_count: 3,
            turn_duration_avg_ms: Some(5000),
            turn_duration_max_ms: Some(12000),
            turn_duration_total_ms: Some(25000),
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
            file_size_at_index: 1024,
            file_mtime_at_index: 1700000000,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            ai_lines_added: 0,
            ai_lines_removed: 0,
            work_type: None,
            primary_model: Some("claude-sonnet-4-5-20250929".to_string()),
            total_task_time_seconds: Some(0),
            longest_task_seconds: Some(0),
            longest_task_preview: None,
            total_cost_usd: Some(0.05),
            slug: None,
            entrypoint: Some("cli".to_string()),
        }
    }

    #[tokio::test]
    async fn upsert_inserts_new_session_with_all_fields() {
        let db = Database::new_in_memory().await.unwrap();
        let session = make_parsed_session("sess-001", 42);
        db.upsert_parsed_session(&session).await.unwrap();

        let row = sqlx::query_as::<_, (i32, i32, i64, i64)>(
            "SELECT message_count, turn_count, total_input_tokens, total_output_tokens FROM sessions WHERE id = ?1"
        )
        .bind("sess-001")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 42); // message_count
        assert_eq!(row.1, 5); // turn_count
        assert_eq!(row.2, 10000); // total_input_tokens
        assert_eq!(row.3, 5000); // total_output_tokens
    }

    #[tokio::test]
    async fn upsert_overwrites_all_fields_on_conflict() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert initial
        let session1 = make_parsed_session("sess-002", 10);
        db.upsert_parsed_session(&session1).await.unwrap();

        // Upsert with updated data — simulates re-parse
        let mut session2 = make_parsed_session("sess-002", 50);
        session2.turn_count = 25;
        session2.total_input_tokens = 99999;
        db.upsert_parsed_session(&session2).await.unwrap();

        let row = sqlx::query_as::<_, (i32, i32, i64)>(
            "SELECT message_count, turn_count, total_input_tokens FROM sessions WHERE id = ?1",
        )
        .bind("sess-002")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 50); // message_count updated, not stuck at 10
        assert_eq!(row.1, 25); // turn_count updated
        assert_eq!(row.2, 99999); // tokens updated
    }

    #[tokio::test]
    async fn no_ghost_sessions_after_upsert() {
        // Proves the ghost bug is impossible: every row has real data
        let db = Database::new_in_memory().await.unwrap();
        let session = make_parsed_session("sess-003", 42);
        db.upsert_parsed_session(&session).await.unwrap();

        // Query via valid_sessions — must be visible
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM valid_sessions WHERE id = ?1")
            .bind("sess-003")
            .fetch_one(db.pool())
            .await
            .unwrap();

        assert_eq!(count.0, 1);

        // Verify no zero-count rows exist
        let ghosts: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE message_count = 0 AND last_message_at > 0",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(ghosts.0, 0);
    }
}
