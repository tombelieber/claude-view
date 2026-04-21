//! Integration tests for Database model/stats query methods.

use claude_view_db::Database;

mod queries_shared;
use queries_shared::make_session;

#[tokio::test]
async fn test_get_stats_overview() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert a session so total_sessions > 0
    let s1 = make_session("sess-1", "project-a", 1000);
    db.insert_session(&s1, "project-a", "Project A")
        .await
        .unwrap();

    // Registry ids use the CQRS `builtin:*` convention so the reader's
    // key→id heuristic (see `queries/invocables.rs::key_to_invocable_id`)
    // attaches counts to the right registry rows.
    db.upsert_invocable("builtin:Read", None, "Read", "tool", "")
        .await
        .unwrap();
    db.upsert_invocable("builtin:Edit", None, "Edit", "tool", "")
        .await
        .unwrap();

    // Seed the session_stats row that backs the CQRS read path.
    sqlx::query(
        r#"INSERT INTO session_stats (
               session_id, source_content_hash, source_size,
               parser_version, stats_version, indexed_at,
               invocation_counts
           ) VALUES ('sess-1', X'01', 0, 1, 1, 0, '{"Read":2,"Edit":1}')"#,
    )
    .execute(db.pool())
    .await
    .unwrap();

    let stats = db.get_stats_overview().await.unwrap();
    assert_eq!(stats.total_sessions, 1);
    assert_eq!(stats.total_invocations, 3);
    assert_eq!(stats.unique_invocables_used, 2);
    assert!(stats.top_invocables.len() <= 10);
    assert_eq!(stats.top_invocables[0].id, "builtin:Read");
    assert_eq!(stats.top_invocables[0].invocation_count, 2);
}
