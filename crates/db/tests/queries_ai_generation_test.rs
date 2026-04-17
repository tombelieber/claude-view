#![allow(deprecated)]
//! Integration tests for Database AI generation stats query methods.

use claude_view_db::Database;
use sqlx::Executor;

#[tokio::test]
async fn test_get_ai_generation_stats() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert + deep-index ai-gen-1 via single UPSERT
    claude_view_db::test_support::SessionSeedBuilder::new("ai-gen-1")
        .project_id("proj-ai")
        .project_display_name("Project AI")
        .project_path("/tmp/proj-ai")
        .file_path("/tmp/ai1.jsonl")
        .preview("Preview 1")
        .message_count(10)
        .modified_at(1000)
        .size_bytes(2000)
        .turn_count(3)
        .total_input_tokens(3000)
        .total_output_tokens(2000)
        .total_cost_usd(0.0)
        .primary_model("claude-opus-4-5-20251101")
        .with_parsed(|s| {
            s.last_message = "Last msg".to_string();
            s.tool_counts_edit = 5;
            s.tool_counts_read = 10;
            s.tool_counts_bash = 3;
            s.tool_counts_write = 2;
            s.user_prompt_count = 5;
            s.api_call_count = 8;
            s.tool_call_count = 15;
            s.files_edited_count = 4;
            s.duration_seconds = 120;
            s.commit_count = 1;
            s.first_message_at = 500;
            s.parse_version = 1;
            s.file_size_at_index = 2000;
            s.file_mtime_at_index = 1706200000;
        })
        .seed(&db)
        .await
        .unwrap();

    // Insert + deep-index ai-gen-2 via single UPSERT
    claude_view_db::test_support::SessionSeedBuilder::new("ai-gen-2")
        .project_id("proj-ai2")
        .project_display_name("Project AI 2")
        .project_path("/tmp/proj-ai2")
        .file_path("/tmp/ai2.jsonl")
        .preview("Preview 2")
        .message_count(5)
        .modified_at(2000)
        .size_bytes(1000)
        .turn_count(2)
        .total_input_tokens(1000)
        .total_output_tokens(500)
        .total_cost_usd(0.0)
        .primary_model("claude-sonnet-4-20250514")
        .with_parsed(|s| {
            s.last_message = "Last msg 2".to_string();
            s.tool_counts_edit = 3;
            s.tool_counts_read = 5;
            s.tool_counts_bash = 1;
            s.tool_counts_write = 1;
            s.user_prompt_count = 3;
            s.api_call_count = 5;
            s.tool_call_count = 10;
            s.files_edited_count = 2;
            s.duration_seconds = 60;
            s.first_message_at = 1500;
            s.parse_version = 1;
            s.file_size_at_index = 1000;
            s.file_mtime_at_index = 1706200000;
        })
        .seed(&db)
        .await
        .unwrap();

    // Ground-truth model usage comes from turns.model_id.
    db.pool()
        .execute(sqlx::query(
            r#"
                INSERT OR IGNORE INTO models (id, provider, family, first_seen, last_seen)
                VALUES ('claude-opus-4-5-20251101', 'anthropic', 'opus', 0, 0)
                "#,
        ))
        .await
        .unwrap();
    db.pool()
        .execute(sqlx::query(
            r#"
                INSERT OR IGNORE INTO models (id, provider, family, first_seen, last_seen)
                VALUES ('claude-sonnet-4-20250514', 'anthropic', 'sonnet', 0, 0)
                "#,
        ))
        .await
        .unwrap();

    db.pool()
        .execute(
            sqlx::query(
                r#"
                INSERT INTO turns (
                    session_id, uuid, seq, model_id, input_tokens, output_tokens,
                    cache_read_tokens, cache_creation_tokens, timestamp
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind("ai-gen-1")
            .bind("turn-ai-gen-1")
            .bind(1)
            .bind("claude-opus-4-5-20251101")
            .bind(3000)
            .bind(2000)
            .bind(0)
            .bind(0)
            .bind(1000),
        )
        .await
        .unwrap();

    db.pool()
        .execute(
            sqlx::query(
                r#"
                INSERT INTO turns (
                    session_id, uuid, seq, model_id, input_tokens, output_tokens,
                    cache_read_tokens, cache_creation_tokens, timestamp
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind("ai-gen-2")
            .bind("turn-ai-gen-2")
            .bind(1)
            .bind("claude-sonnet-4-20250514")
            .bind(1000)
            .bind(500)
            .bind(0)
            .bind(0)
            .bind(2000),
        )
        .await
        .unwrap();

    // Test all-time (no range filter)
    let stats = db
        .get_ai_generation_stats(None, None, None, None)
        .await
        .unwrap();

    // files_created = sum of files_edited_count: 4 + 2 = 6
    assert_eq!(stats.files_created, 6, "Sum of files_edited_count");
    // Total tokens from sessions table
    assert_eq!(stats.total_input_tokens, 4000, "3000 + 1000");
    assert_eq!(stats.total_output_tokens, 2500, "2000 + 500");
    // lines not tracked yet
    assert_eq!(stats.lines_added, 0);
    assert_eq!(stats.lines_removed, 0);

    // 2 model entries
    assert_eq!(
        stats.tokens_by_model.len(),
        2,
        "Should have 2 model entries"
    );
    let opus = stats
        .tokens_by_model
        .iter()
        .find(|m| m.model == "claude-opus-4-5-20251101")
        .unwrap();
    assert_eq!(opus.input_tokens, 3000);
    assert_eq!(opus.output_tokens, 2000);

    let sonnet = stats
        .tokens_by_model
        .iter()
        .find(|m| m.model == "claude-sonnet-4-20250514")
        .unwrap();
    assert_eq!(sonnet.input_tokens, 1000);
    assert_eq!(sonnet.output_tokens, 500);

    // Project breakdown (2 projects)
    assert_eq!(
        stats.tokens_by_project.len(),
        2,
        "Should have 2 project entries"
    );

    // Test with time range: only ai-gen-1 has last_message_at = 1000
    let ranged = db
        .get_ai_generation_stats(Some(900), Some(1100), None, None)
        .await
        .unwrap();
    assert_eq!(ranged.files_created, 4, "Only ai-gen-1 within range");
    assert_eq!(ranged.total_input_tokens, 3000);
    assert_eq!(ranged.total_output_tokens, 2000);
    assert_eq!(ranged.tokens_by_model.len(), 1);
}
