use claude_view_db::fold::run_fold_batch;
use claude_view_db::Database;
use std::sync::Arc;

/// CQRS Phase 5 PR 5.6a (D.2+D.3): archive / unarchive are action-log-only.
/// Each test calls the Database methods, then drives the fold so
/// `session_flags` reflects the latest state, then asserts visibility via
/// `valid_sessions` (post-migration-85 view that filters on
/// `session_flags.archived_at`).

async fn drain_fold(db: Arc<Database>) {
    loop {
        let summary = run_fold_batch(db.clone()).await.unwrap();
        if summary.rows_observed == 0 {
            break;
        }
    }
}

#[tokio::test]
async fn test_archive_session() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, is_sidechain) VALUES ('test-1', 'proj-1', '/tmp/test.jsonl', 0)",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Archive it
    let result = db.archive_session("test-1").await.unwrap();
    assert_eq!(result, Some("/tmp/test.jsonl".to_string()));

    // Drain the action log → session_flags.
    drain_fold(db.clone()).await;

    // Verify session_flags.archived_at is set
    let archived: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT archived_at FROM session_flags WHERE session_id = 'test-1'")
            .fetch_optional(db.pool())
            .await
            .unwrap();
    assert!(
        archived.and_then(|r| r.0).is_some(),
        "session_flags.archived_at must be set after fold"
    );

    // Verify session no longer appears in valid_sessions (post migration 85
    // rewires the view to JOIN session_flags).
    let in_view: Option<(String,)> =
        sqlx::query_as("SELECT id FROM valid_sessions WHERE id = 'test-1'")
            .fetch_optional(db.pool())
            .await
            .unwrap();
    assert!(in_view.is_none());

    // Archive again should return None (already archived via shadow).
    let result2 = db.archive_session("test-1").await.unwrap();
    assert_eq!(result2, None);
}

#[tokio::test]
async fn test_unarchive_session() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, is_sidechain) VALUES ('test-2', 'proj-2', '/tmp/test2.jsonl', 0)",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Archive then fold so shadow is populated.
    db.archive_session("test-2").await.unwrap();
    drain_fold(db.clone()).await;

    // Now unarchive + fold.
    let result = db
        .unarchive_session("test-2", "/tmp/restored.jsonl")
        .await
        .unwrap();
    assert!(result);
    drain_fold(db.clone()).await;

    // Verify session_flags.archived_at is NULL and file_path updated
    let flag: (Option<i64>,) =
        sqlx::query_as("SELECT archived_at FROM session_flags WHERE session_id = 'test-2'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(
        flag.0.is_none(),
        "session_flags.archived_at must be cleared"
    );
    let path: (String,) = sqlx::query_as("SELECT file_path FROM sessions WHERE id = 'test-2'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(path.0, "/tmp/restored.jsonl");

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
    let db = Arc::new(Database::new_in_memory().await.unwrap());
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

    drain_fold(db.clone()).await;

    // Verify 3 archived, 2 still visible
    let visible: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM valid_sessions WHERE id LIKE 'bulk-%'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(visible.0, 2);
}
