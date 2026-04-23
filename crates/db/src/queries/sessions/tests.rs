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

    // =========================================================================
    // CQRS Phase 7.h.2: full-row session_stats writer from ParsedSession.
    // =========================================================================

    #[tokio::test]
    async fn session_stats_from_parsed_round_trips_every_column() {
        use super::super::execute_upsert_session_stats_from_parsed;

        let db = Database::new_in_memory().await.unwrap();
        let mut session = make_parsed_session("sess-7h2-a", 42);
        session.summary = Some("SOTA design".to_string());
        session.git_branch = Some("main".to_string());
        session.summary_text = Some("Summary line".to_string());
        session.work_type = Some("deep_work".to_string());
        session.slug = Some("my-session-slug".to_string());

        execute_upsert_session_stats_from_parsed(db.pool(), &session)
            .await
            .unwrap();

        // Verify all 42 Phase 7.h columns landed on session_stats.
        let row: (
            String,         // project_display_name
            String,         // project_path
            Option<String>, // summary
            i64,            // message_count
            i64,            // size_bytes
            String,         // files_touched
            i64,            // tool_counts_edit
            i64,            // tool_counts_bash
            i64,            // parse_version
            Option<i64>,    // turn_duration_avg_ms
            Option<i64>,    // total_task_time_seconds
            Option<f64>,    // total_cost_usd
            Option<String>, // slug
            Option<String>, // entrypoint
        ) = sqlx::query_as(
            r#"SELECT project_display_name, project_path, summary, message_count,
                     size_bytes, files_touched,
                     tool_counts_edit, tool_counts_bash, parse_version,
                     turn_duration_avg_ms, total_task_time_seconds,
                     total_cost_usd, slug, entrypoint
              FROM session_stats WHERE session_id = ?"#,
        )
        .bind("sess-7h2-a")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, "Test Project");
        assert_eq!(row.1, "/test/project");
        assert_eq!(row.2, Some("SOTA design".to_string()));
        assert_eq!(row.3, 42);
        assert_eq!(row.4, 1024);
        assert_eq!(row.5, "[]");
        assert_eq!(row.6, 1);
        assert_eq!(row.7, 3);
        assert!(row.8 >= 1, "parse_version must be > 0 after a real parse");
        assert_eq!(row.9, Some(5000));
        assert_eq!(row.10, Some(0));
        assert_eq!(row.11, Some(0.05));
        assert_eq!(row.12, Some("my-session-slug".to_string()));
        assert_eq!(row.13, Some("cli".to_string()));
    }

    #[tokio::test]
    async fn session_stats_from_parsed_seeds_header_columns_on_insert() {
        use super::super::execute_upsert_session_stats_from_parsed;

        let db = Database::new_in_memory().await.unwrap();
        let session = make_parsed_session("sess-7h2-b", 10);

        execute_upsert_session_stats_from_parsed(db.pool(), &session)
            .await
            .unwrap();

        // Verify the session_stats header defaults are sensible so the StatsDelta
        // writer can ON CONFLICT UPDATE them later without a NOT NULL collision.
        let (hash, source_size, parser_version, stats_version, bash_count, mtime): (
            Vec<u8>,
            i64,
            i64,
            i64,
            i64,
            Option<i64>,
        ) = sqlx::query_as(
            r#"SELECT source_content_hash, source_size, parser_version, stats_version,
                     bash_count, source_mtime
              FROM session_stats WHERE session_id = ?"#,
        )
        .bind("sess-7h2-b")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(hash, Vec::<u8>::new(), "X'' must seed an empty BLOB");
        assert_eq!(source_size, 1024);
        assert!(parser_version >= 1);
        assert_eq!(stats_version, 4);
        assert_eq!(bash_count, 3); // mirrors tool_counts_bash
        assert_eq!(mtime, Some(1700000000));
    }

    #[tokio::test]
    async fn session_stats_from_parsed_preserves_statsdelta_header_on_conflict() {
        use super::super::execute_upsert_session_stats_from_parsed;

        let db = Database::new_in_memory().await.unwrap();

        // Pretend the StatsDelta writer arrived first and recorded authoritative
        // header + incremental stats (line_count, per_model_tokens_json,
        // invocation_counts, cache_creation_5m/1hr).
        sqlx::query(
            r#"INSERT INTO session_stats (
                session_id, source_content_hash, source_size, source_inode, source_mid_hash,
                parser_version, stats_version, indexed_at,
                total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens,
                cache_creation_5m_tokens, cache_creation_1hr_tokens,
                line_count,
                per_model_tokens_json, invocation_counts,
                project_id, file_path, is_compressed, source_mtime
            ) VALUES (?, X'DEADBEEF', 4096, 42, X'AABB', 1, 4, 1,
                      100, 200, 30, 40,
                      5, 7,
                      999,
                      '{"claude-opus":{"in":100}}', '{"Read":2}',
                      'test-project', '/test/session.jsonl', 0, 1700000001)"#,
        )
        .bind("sess-7h2-c")
        .execute(db.pool())
        .await
        .unwrap();

        // Now write from ParsedSession — should NOT touch the StatsDelta-owned
        // fields (source_content_hash, source_inode, source_mid_hash,
        // line_count, cache_creation_5m/1hr, per_model_tokens_json,
        // invocation_counts).
        let session = make_parsed_session("sess-7h2-c", 42);
        execute_upsert_session_stats_from_parsed(db.pool(), &session)
            .await
            .unwrap();

        let (hash, inode, mid, line_count, five_m, one_hr, pm_json, inv): (
            Vec<u8>,
            Option<i64>,
            Option<Vec<u8>>,
            i64,
            i64,
            i64,
            String,
            String,
        ) = sqlx::query_as(
            r#"SELECT source_content_hash, source_inode, source_mid_hash,
                     line_count, cache_creation_5m_tokens, cache_creation_1hr_tokens,
                     per_model_tokens_json, invocation_counts
              FROM session_stats WHERE session_id = ?"#,
        )
        .bind("sess-7h2-c")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(hash, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(inode, Some(42));
        assert_eq!(mid, Some(vec![0xAA, 0xBB]));
        assert_eq!(line_count, 999);
        assert_eq!(five_m, 5);
        assert_eq!(one_hr, 7);
        assert_eq!(pm_json, r#"{"claude-opus":{"in":100}}"#);
        assert_eq!(inv, r#"{"Read":2}"#);

        // But the Phase 7.h fields MUST have been written.
        let project_display_name: (String,) =
            sqlx::query_as("SELECT project_display_name FROM session_stats WHERE session_id = ?")
                .bind("sess-7h2-c")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(project_display_name.0, "Test Project");
    }
}
