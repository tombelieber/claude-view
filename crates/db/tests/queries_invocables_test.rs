#![allow(deprecated)]
//! Integration tests for Database invocable/invocation query methods.

use claude_view_db::Database;

#[tokio::test]
async fn test_upsert_invocable() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert a new invocable
    db.upsert_invocable("tool::Read", Some("core"), "Read", "tool", "Read files")
        .await
        .unwrap();

    let items = db.list_invocables_with_counts().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "tool::Read");
    assert_eq!(items[0].plugin_name, Some("core".to_string()));
    assert_eq!(items[0].description, "Read files");

    // Upsert same id with a different description
    db.upsert_invocable(
        "tool::Read",
        Some("core"),
        "Read",
        "tool",
        "Read files from disk",
    )
    .await
    .unwrap();

    let items = db.list_invocables_with_counts().await.unwrap();
    assert_eq!(items.len(), 1, "Should still be 1 invocable after upsert");
    assert_eq!(items[0].description, "Read files from disk");
}

// CQRS Phase 6.4: `batch_insert_invocations` / `batch_insert_turns`
// tests retired along with the `invocations` + `turns` tables
// (migration 87). Per-invocable counts are now written to
// `session_stats.invocation_counts` by indexer_v2 — the rebuilt
// behaviour is covered by `test_list_invocables_with_counts` and
// `test_get_stats_overview`.

#[tokio::test]
async fn test_list_invocables_with_counts() {
    let db = Database::new_in_memory().await.unwrap();

    // Registry ids mirror what `classify_tool_use` emits for built-in tools
    // (`"builtin:<name>"`), which is how the reader's key→id heuristic
    // bridges `session_stats.invocation_counts` → `invocables`.
    db.upsert_invocable("builtin:Read", None, "Read", "tool", "Read files")
        .await
        .unwrap();
    db.upsert_invocable("builtin:Edit", None, "Edit", "tool", "Edit files")
        .await
        .unwrap();
    db.upsert_invocable("builtin:Bash", None, "Bash", "tool", "Run commands")
        .await
        .unwrap();

    // Seed matching sessions + session_stats rows. The reader joins
    // `valid_sessions` × `session_stats`, so both sides must exist.
    // Counts: Read x3 (s1=2, s2=1), Edit x1 (s2), Bash x0.
    for (sid, counts) in &[("s1", r#"{"Read":2}"#), ("s2", r#"{"Read":1,"Edit":1}"#)] {
        claude_view_db::test_support::SessionSeedBuilder::new(*sid)
            .project_id("p")
            .project_display_name("p")
            .project_path("/tmp")
            .file_path(format!("/tmp/{}.jsonl", sid))
            .modified_at(1000)
            .seed(&db)
            .await
            .unwrap();
        sqlx::query(
            r#"INSERT INTO session_stats (
                   session_id, source_content_hash, source_size,
                   parser_version, stats_version, indexed_at,
                   invocation_counts
               ) VALUES (?, X'01', 0, 1, 1, 0, ?)"#,
        )
        .bind(sid)
        .bind(counts)
        .execute(db.pool())
        .await
        .unwrap();
    }

    let items = db.list_invocables_with_counts().await.unwrap();
    assert_eq!(items.len(), 3);

    // Ordered by invocation_count DESC, then name ASC.
    assert_eq!(items[0].id, "builtin:Read");
    assert_eq!(items[0].invocation_count, 3);
    // `last_used_at` isn't tracked in the JSON column — readers return None.
    assert_eq!(items[0].last_used_at, None);

    assert_eq!(items[1].id, "builtin:Edit");
    assert_eq!(items[1].invocation_count, 1);

    assert_eq!(items[2].id, "builtin:Bash");
    assert_eq!(items[2].invocation_count, 0);
    assert_eq!(items[2].last_used_at, None);
}

#[tokio::test]
async fn test_batch_upsert_invocables() {
    let db = Database::new_in_memory().await.unwrap();

    let batch = vec![
        (
            "tool::Read".to_string(),
            Some("core".to_string()),
            "Read".to_string(),
            "tool".to_string(),
            "Read files".to_string(),
        ),
        (
            "tool::Edit".to_string(),
            None,
            "Edit".to_string(),
            "tool".to_string(),
            "Edit files".to_string(),
        ),
        (
            "skill::commit".to_string(),
            Some("git".to_string()),
            "commit".to_string(),
            "skill".to_string(),
            "Git commit".to_string(),
        ),
    ];

    let affected = db.batch_upsert_invocables(&batch).await.unwrap();
    assert_eq!(affected, 3);

    let items = db.list_invocables_with_counts().await.unwrap();
    assert_eq!(items.len(), 3, "All 3 invocables should be present");

    // Verify one of them
    let commit = items.iter().find(|i| i.id == "skill::commit").unwrap();
    assert_eq!(commit.plugin_name, Some("git".to_string()));
    assert_eq!(commit.kind, "skill");
}
