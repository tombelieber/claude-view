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
    db.upsert_invocable("tool::Read", Some("core"), "Read", "tool", "Read files from disk")
        .await
        .unwrap();

    let items = db.list_invocables_with_counts().await.unwrap();
    assert_eq!(items.len(), 1, "Should still be 1 invocable after upsert");
    assert_eq!(items[0].description, "Read files from disk");
}

#[tokio::test]
async fn test_batch_insert_invocations() {
    let db = Database::new_in_memory().await.unwrap();

    // Must insert invocables first (FK constraint)
    db.upsert_invocable("tool::Read", None, "Read", "tool", "")
        .await
        .unwrap();
    db.upsert_invocable("tool::Edit", None, "Edit", "tool", "")
        .await
        .unwrap();

    // Must insert sessions first (FK constraint on invocations.session_id)
    for sid in &["sess-1", "sess-2"] {
        db.insert_session_from_index(sid, "proj-a", "proj-a", "/tmp", &format!("/tmp/{}.jsonl", sid), "", None, 0, 1000, None, false, 0).await.unwrap();
    }

    let invocations = vec![
        ("file1.jsonl".to_string(), 100, "tool::Read".to_string(), "sess-1".to_string(), "proj-a".to_string(), 1000),
        ("file1.jsonl".to_string(), 200, "tool::Edit".to_string(), "sess-1".to_string(), "proj-a".to_string(), 1001),
        ("file2.jsonl".to_string(), 50, "tool::Read".to_string(), "sess-2".to_string(), "proj-a".to_string(), 2000),
    ];

    let inserted = db.batch_insert_invocations(&invocations).await.unwrap();
    assert_eq!(inserted, 3, "Should insert 3 rows");
}

#[tokio::test]
async fn test_batch_insert_invocations_ignores_duplicates() {
    let db = Database::new_in_memory().await.unwrap();

    db.upsert_invocable("tool::Read", None, "Read", "tool", "")
        .await
        .unwrap();

    // Must insert session first (FK constraint on invocations.session_id)
    db.insert_session_from_index("sess-1", "proj-a", "proj-a", "/tmp", "/tmp/f.jsonl", "", None, 0, 1000, None, false, 0).await.unwrap();

    let invocations = vec![
        ("file1.jsonl".to_string(), 100, "tool::Read".to_string(), "sess-1".to_string(), "proj-a".to_string(), 1000),
    ];

    let inserted = db.batch_insert_invocations(&invocations).await.unwrap();
    assert_eq!(inserted, 1);

    // Insert same (source_file, byte_offset) again — should be ignored
    let inserted2 = db.batch_insert_invocations(&invocations).await.unwrap();
    assert_eq!(inserted2, 0, "Duplicate should be ignored (INSERT OR IGNORE)");
}

#[tokio::test]
async fn test_list_invocables_with_counts() {
    let db = Database::new_in_memory().await.unwrap();

    db.upsert_invocable("tool::Read", None, "Read", "tool", "Read files")
        .await
        .unwrap();
    db.upsert_invocable("tool::Edit", None, "Edit", "tool", "Edit files")
        .await
        .unwrap();
    db.upsert_invocable("tool::Bash", None, "Bash", "tool", "Run commands")
        .await
        .unwrap();

    // Must insert sessions first (FK constraint on invocations.session_id)
    for sid in &["s1", "s2"] {
        db.insert_session_from_index(sid, "p", "p", "/tmp", &format!("/tmp/{}.jsonl", sid), "", None, 0, 1000, None, false, 0).await.unwrap();
    }

    // Add invocations: Read x3, Edit x1, Bash x0
    let invocations = vec![
        ("f1.jsonl".to_string(), 10, "tool::Read".to_string(), "s1".to_string(), "p".to_string(), 1000),
        ("f1.jsonl".to_string(), 20, "tool::Read".to_string(), "s1".to_string(), "p".to_string(), 2000),
        ("f2.jsonl".to_string(), 10, "tool::Read".to_string(), "s2".to_string(), "p".to_string(), 3000),
        ("f2.jsonl".to_string(), 20, "tool::Edit".to_string(), "s2".to_string(), "p".to_string(), 3001),
    ];
    db.batch_insert_invocations(&invocations).await.unwrap();

    let items = db.list_invocables_with_counts().await.unwrap();
    assert_eq!(items.len(), 3);

    // Ordered by invocation_count DESC, then name ASC
    assert_eq!(items[0].id, "tool::Read");
    assert_eq!(items[0].invocation_count, 3);
    assert_eq!(items[0].last_used_at, Some(3000));

    assert_eq!(items[1].id, "tool::Edit");
    assert_eq!(items[1].invocation_count, 1);

    assert_eq!(items[2].id, "tool::Bash");
    assert_eq!(items[2].invocation_count, 0);
    assert_eq!(items[2].last_used_at, None);
}

#[tokio::test]
async fn test_batch_upsert_invocables() {
    let db = Database::new_in_memory().await.unwrap();

    let batch = vec![
        ("tool::Read".to_string(), Some("core".to_string()), "Read".to_string(), "tool".to_string(), "Read files".to_string()),
        ("tool::Edit".to_string(), None, "Edit".to_string(), "tool".to_string(), "Edit files".to_string()),
        ("skill::commit".to_string(), Some("git".to_string()), "commit".to_string(), "skill".to_string(), "Git commit".to_string()),
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
