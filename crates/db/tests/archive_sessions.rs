use claude_view_db::Database;

#[tokio::test]
async fn test_archive_session() {
    let db = Database::new_in_memory().await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, is_sidechain) VALUES ('test-1', 'proj-1', '/tmp/test.jsonl', 0)",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Archive it
    let result = db.archive_session("test-1").await.unwrap();
    assert_eq!(result, Some("/tmp/test.jsonl".to_string()));

    // Verify archived_at is set
    let archived: Option<(String,)> =
        sqlx::query_as("SELECT archived_at FROM sessions WHERE id = 'test-1'")
            .fetch_optional(db.pool())
            .await
            .unwrap();
    assert!(archived.is_some());

    // Verify session no longer appears in valid_sessions
    let in_view: Option<(String,)> =
        sqlx::query_as("SELECT id FROM valid_sessions WHERE id = 'test-1'")
            .fetch_optional(db.pool())
            .await
            .unwrap();
    assert!(in_view.is_none());

    // Archive again should return None (already archived)
    let result2 = db.archive_session("test-1").await.unwrap();
    assert_eq!(result2, None);
}

#[tokio::test]
async fn test_unarchive_session() {
    let db = Database::new_in_memory().await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, is_sidechain) VALUES ('test-2', 'proj-2', '/tmp/test2.jsonl', 0)",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Archive then unarchive
    db.archive_session("test-2").await.unwrap();
    let result = db
        .unarchive_session("test-2", "/tmp/restored.jsonl")
        .await
        .unwrap();
    assert!(result);

    // Verify archived_at is NULL and file_path updated
    let row: (Option<String>, String) =
        sqlx::query_as("SELECT archived_at, file_path FROM sessions WHERE id = 'test-2'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(row.0.is_none());
    assert_eq!(row.1, "/tmp/restored.jsonl");

    // Verify session reappears in valid_sessions
    let in_view: Option<(String,)> =
        sqlx::query_as("SELECT id FROM valid_sessions WHERE id = 'test-2'")
            .fetch_optional(db.pool())
            .await
            .unwrap();
    assert!(in_view.is_some());
}

#[tokio::test]
async fn test_bulk_archive() {
    let db = Database::new_in_memory().await.unwrap();
    for i in 1..=5 {
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, is_sidechain) VALUES (?1, 'proj-bulk', ?2, 0)",
        )
        .bind(format!("bulk-{i}"))
        .bind(format!("/tmp/bulk-{i}.jsonl"))
        .execute(db.pool())
        .await
        .unwrap();
    }

    let ids: Vec<String> = (1..=3).map(|i| format!("bulk-{i}")).collect();
    let results = db.archive_sessions_bulk(&ids).await.unwrap();
    assert_eq!(results.len(), 3);

    // Verify 3 archived, 2 still visible
    let visible: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM valid_sessions WHERE id LIKE 'bulk-%'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(visible.0, 2);
}
