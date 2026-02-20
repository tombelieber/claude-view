//! Integration tests for Database model/stats query methods.

use claude_view_db::Database;

mod queries_shared;
use queries_shared::make_session;

#[tokio::test]
async fn test_get_stats_overview() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert a session so total_sessions > 0
    let s1 = make_session("sess-1", "project-a", 1000);
    db.insert_session(&s1, "project-a", "Project A").await.unwrap();

    // Insert invocables
    db.upsert_invocable("tool::Read", None, "Read", "tool", "")
        .await
        .unwrap();
    db.upsert_invocable("tool::Edit", None, "Edit", "tool", "")
        .await
        .unwrap();

    // Insert invocations
    let invocations = vec![
        ("f1.jsonl".to_string(), 10, "tool::Read".to_string(), "sess-1".to_string(), "p".to_string(), 1000),
        ("f1.jsonl".to_string(), 20, "tool::Read".to_string(), "sess-1".to_string(), "p".to_string(), 1001),
        ("f1.jsonl".to_string(), 30, "tool::Edit".to_string(), "sess-1".to_string(), "p".to_string(), 1002),
    ];
    db.batch_insert_invocations(&invocations).await.unwrap();

    let stats = db.get_stats_overview().await.unwrap();
    assert_eq!(stats.total_sessions, 1);
    assert_eq!(stats.total_invocations, 3);
    assert_eq!(stats.unique_invocables_used, 2);
    assert!(stats.top_invocables.len() <= 10);
    assert_eq!(stats.top_invocables[0].id, "tool::Read");
    assert_eq!(stats.top_invocables[0].invocation_count, 2);
}
