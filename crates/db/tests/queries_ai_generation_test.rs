//! Integration tests for Database AI generation stats query methods.

use vibe_recall_db::Database;

#[tokio::test]
async fn test_get_ai_generation_stats() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert 2 sessions with different primary_model values
    db.insert_session_from_index(
        "ai-gen-1",
        "proj-ai",
        "Project AI",
        "/tmp/proj-ai",
        "/tmp/ai1.jsonl",
        "Preview 1",
        None,
        10,
        1000,
        None,
        false,
        2000,
    )
    .await
    .unwrap();

    db.insert_session_from_index(
        "ai-gen-2",
        "proj-ai2",
        "Project AI 2",
        "/tmp/proj-ai2",
        "/tmp/ai2.jsonl",
        "Preview 2",
        None,
        5,
        2000,
        None,
        false,
        1000,
    )
    .await
    .unwrap();

    // Update deep fields with token data and primary_model
    db.update_session_deep_fields(
        "ai-gen-1",
        "Last msg",
        3,
        5, 10, 3, 2,
        "[]", "[]",
        5, 8, 15,
        "[]", "[]",
        0, 4, 0,
        120, 1,
        Some(500),
        3000, 2000, 0, 0, // total_input, total_output, cache_read, cache_creation
        0,
        None, None, None,
        0, 0, 0, 0,
        0, 0, 0, 0,
        None,
        1,
        2000,
        1706200000,
        0, 0, 0, // lines_added, lines_removed, loc_source
        0, 0,    // ai_lines_added, ai_lines_removed
        None,    // work_type
        None,    // git_branch
        Some("claude-opus-4-5-20251101"),
        None, // last_message_at
        None, // first_user_prompt
    )
    .await
    .unwrap();

    db.update_session_deep_fields(
        "ai-gen-2",
        "Last msg 2",
        2,
        3, 5, 1, 1,
        "[]", "[]",
        3, 5, 10,
        "[]", "[]",
        0, 2, 0,
        60, 0,
        Some(1500),
        1000, 500, 0, 0,
        0,
        None, None, None,
        0, 0, 0, 0,
        0, 0, 0, 0,
        None,
        1,
        1000,
        1706200000,
        0, 0, 0, // lines_added, lines_removed, loc_source
        0, 0,    // ai_lines_added, ai_lines_removed
        None,    // work_type
        None,    // git_branch
        Some("claude-sonnet-4-20250514"),
        None, // last_message_at
        None, // first_user_prompt
    )
    .await
    .unwrap();

    // Test all-time (no range filter)
    let stats = db.get_ai_generation_stats(None, None, None, None).await.unwrap();

    // files_created = sum of files_edited_count: 4 + 2 = 6
    assert_eq!(stats.files_created, 6, "Sum of files_edited_count");
    // Total tokens from sessions table
    assert_eq!(stats.total_input_tokens, 4000, "3000 + 1000");
    assert_eq!(stats.total_output_tokens, 2500, "2000 + 500");
    // lines not tracked yet
    assert_eq!(stats.lines_added, 0);
    assert_eq!(stats.lines_removed, 0);

    // 2 model entries
    assert_eq!(stats.tokens_by_model.len(), 2, "Should have 2 model entries");
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
