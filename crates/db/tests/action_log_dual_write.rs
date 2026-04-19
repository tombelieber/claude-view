//! CQRS Phase 5 PR 5.6a — action-log writer verification.
//!
//! Every archive / unarchive / classify mutation must land a matching
//! row in `session_action_log`. As of Phase D.2 the log is the SOLE
//! writer — the legacy `sessions.*` UPDATE is gone and `session_flags`
//! is populated exclusively by the Phase 5.3 fold task. Tests that
//! need to observe shadow effects drain the fold with
//! `run_fold_batch`.
//!
//! Tests use `Database::new_in_memory()` + raw `INSERT` seeds, matching
//! the style of `crates/db/tests/archive_sessions.rs`. Tuple-style
//! `query_as` is used throughout because the workspace sqlx profile
//! (root `Cargo.toml`) does not enable the `macros` feature, so
//! `#[derive(sqlx::FromRow)]` is unavailable in integration tests.

use claude_view_db::fold::run_fold_batch;
use claude_view_db::Database;
use std::sync::Arc;

async fn drain_fold(db: Arc<Database>) {
    loop {
        let summary = run_fold_batch(db.clone()).await.unwrap();
        if summary.rows_observed == 0 {
            break;
        }
    }
}

/// (seq, session_id, action, payload, actor, at)
type ActionRow = (i64, String, String, String, String, i64);

async fn seed_session(db: &Database, id: &str) {
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, is_sidechain)
         VALUES (?1, 'proj-1', ?2, 0)",
    )
    .bind(id)
    .bind(format!("/tmp/{id}.jsonl"))
    .execute(db.pool())
    .await
    .unwrap();
}

async fn fetch_actions_for(db: &Database, session_id: &str) -> Vec<ActionRow> {
    sqlx::query_as::<_, ActionRow>(
        "SELECT seq, session_id, action, payload, actor, at
         FROM session_action_log
         WHERE session_id = ?1
         ORDER BY seq ASC",
    )
    .bind(session_id)
    .fetch_all(db.pool())
    .await
    .unwrap()
}

async fn fetch_actions_by_action(db: &Database, action: &str) -> Vec<ActionRow> {
    sqlx::query_as::<_, ActionRow>(
        "SELECT seq, session_id, action, payload, actor, at
         FROM session_action_log
         WHERE action = ?1
         ORDER BY seq ASC",
    )
    .bind(action)
    .fetch_all(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn dual_write_archive_emits_action_log_row() {
    let db = Database::new_in_memory().await.unwrap();
    seed_session(&db, "s-arch-1").await;

    let before_ms = chrono::Utc::now().timestamp_millis();
    db.archive_session("s-arch-1").await.unwrap();

    let rows = fetch_actions_for(&db, "s-arch-1").await;
    assert_eq!(rows.len(), 1, "archive must emit exactly one log row");
    let (_, _, action, payload, actor, at) = &rows[0];
    assert_eq!(action, "archive");
    assert_eq!(actor, "user");
    assert_eq!(payload, "{}");
    assert!(*at >= before_ms, "at={at} must be >= before_ms={before_ms}");
}

#[tokio::test]
async fn dual_write_archive_noop_when_already_archived_leaves_log_empty() {
    // Second archive on the same session must NOT log a second row.
    // Post Phase D.2 the gate is `session_flags.archived_at`; the fold
    // must run between the two calls so the shadow reflects the first
    // archive by the time the second call checks.
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s-arch-noop").await;

    db.archive_session("s-arch-noop").await.unwrap();
    assert_eq!(fetch_actions_for(&db, "s-arch-noop").await.len(), 1);
    drain_fold(db.clone()).await;

    db.archive_session("s-arch-noop").await.unwrap();
    assert_eq!(
        fetch_actions_for(&db, "s-arch-noop").await.len(),
        1,
        "already-archived no-op must NOT emit a second log row"
    );
}

#[tokio::test]
async fn dual_write_unarchive_emits_action_log_row() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s-unarch-1").await;

    db.archive_session("s-unarch-1").await.unwrap();
    drain_fold(db.clone()).await;
    db.unarchive_session("s-unarch-1", "/tmp/new.jsonl")
        .await
        .unwrap();

    let rows = fetch_actions_for(&db, "s-unarch-1").await;
    assert_eq!(rows.len(), 2, "archive then unarchive = 2 log rows");
    assert_eq!(rows[0].2, "archive");
    assert_eq!(rows[1].2, "unarchive");
    assert!(
        rows[0].0 < rows[1].0,
        "seq must be monotonically increasing"
    );
}

#[tokio::test]
async fn dual_write_unarchive_noop_leaves_log_untouched() {
    let db = Database::new_in_memory().await.unwrap();
    seed_session(&db, "s-unarch-noop").await;

    let changed = db
        .unarchive_session("s-unarch-noop", "/tmp/x.jsonl")
        .await
        .unwrap();
    assert!(!changed, "unarchive on unarchived session returns false");

    let rows = fetch_actions_for(&db, "s-unarch-noop").await;
    assert!(rows.is_empty(), "no UPDATE, no log row");
}

#[tokio::test]
async fn dual_write_bulk_archive_logs_one_row_per_successful_update() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "bulk-a").await;
    seed_session(&db, "bulk-b").await;
    seed_session(&db, "bulk-c").await;
    // Pre-archive one so the bulk call sees it as already-archived; the
    // fold must run so `session_flags.archived_at` reflects it before
    // archive_sessions_bulk probes the shadow.
    db.archive_session("bulk-b").await.unwrap();
    assert_eq!(fetch_actions_by_action(&db, "archive").await.len(), 1);
    drain_fold(db.clone()).await;

    let ids = vec![
        "bulk-a".to_string(),
        "bulk-b".to_string(),
        "bulk-c".to_string(),
    ];
    let results = db.archive_sessions_bulk(&ids).await.unwrap();
    assert_eq!(results.len(), 2, "only 2 of 3 actually archived in bulk");

    // Pre-archive of bulk-b (1) + bulk archive of bulk-a + bulk-c (2) = 3.
    let all = fetch_actions_by_action(&db, "archive").await;
    assert_eq!(all.len(), 3);
    let ids_logged: Vec<&str> = all.iter().map(|r| r.1.as_str()).collect();
    assert!(ids_logged.contains(&"bulk-a"));
    assert!(ids_logged.contains(&"bulk-c"));
    // bulk-b appears exactly once (pre-archive), never twice.
    assert_eq!(
        ids_logged.iter().filter(|s| **s == "bulk-b").count(),
        1,
        "bulk-b must have exactly one archive log row"
    );
}

#[tokio::test]
async fn dual_write_bulk_unarchive_logs_one_row_per_successful_update() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "bulk-u-a").await;
    seed_session(&db, "bulk-u-b").await;

    db.archive_session("bulk-u-a").await.unwrap();
    db.archive_session("bulk-u-b").await.unwrap();
    // Fold must run so unarchive_sessions_bulk sees the archived state.
    drain_fold(db.clone()).await;

    // Third ID never existed — must not produce a log row.
    let paths = vec![
        ("bulk-u-a".to_string(), "/tmp/ua.jsonl".to_string()),
        ("bulk-u-b".to_string(), "/tmp/ub.jsonl".to_string()),
        ("ghost".to_string(), "/tmp/g.jsonl".to_string()),
    ];
    let count = db.unarchive_sessions_bulk(&paths).await.unwrap();
    assert_eq!(count, 2);

    let unarch = fetch_actions_by_action(&db, "unarchive").await;
    assert_eq!(unarch.len(), 2);
    let ids: Vec<&str> = unarch.iter().map(|r| r.1.as_str()).collect();
    assert!(ids.contains(&"bulk-u-a"));
    assert!(ids.contains(&"bulk-u-b"));
    assert!(!ids.contains(&"ghost"), "ghost must NOT produce a log row");
}

#[tokio::test]
async fn dual_write_classify_emits_classify_row_with_json_payload() {
    let db = Database::new_in_memory().await.unwrap();
    seed_session(&db, "s-cls-1").await;

    let updates = vec![(
        "s-cls-1".to_string(),
        "engineering".to_string(),
        "backend".to_string(),
        "database".to_string(),
        0.92_f64,
        "claude-cli".to_string(),
    )];
    db.batch_update_session_classifications(&updates)
        .await
        .unwrap();

    let rows = fetch_actions_for(&db, "s-cls-1").await;
    assert_eq!(rows.len(), 1);
    let (_, _, action, payload, actor, _) = &rows[0];
    assert_eq!(action, "classify");
    assert_eq!(actor, "classifier:claude-cli");

    let parsed: serde_json::Value = serde_json::from_str(payload).unwrap();
    assert_eq!(parsed["l1"], "engineering");
    assert_eq!(parsed["l2"], "backend");
    assert_eq!(parsed["l3"], "database");
    assert_eq!(parsed["source"], "claude-cli");
    assert!((parsed["confidence"].as_f64().unwrap() - 0.92).abs() < 1e-9);
}

#[tokio::test]
async fn dual_write_classify_batch_preserves_order_and_shape() {
    let db = Database::new_in_memory().await.unwrap();
    seed_session(&db, "batch-1").await;
    seed_session(&db, "batch-2").await;
    seed_session(&db, "batch-3").await;

    let updates = vec![
        (
            "batch-1".to_string(),
            "a".to_string(),
            "a1".to_string(),
            "a1a".to_string(),
            0.1,
            "claude-cli".to_string(),
        ),
        (
            "batch-2".to_string(),
            "b".to_string(),
            "b1".to_string(),
            "b1b".to_string(),
            0.5,
            "human".to_string(),
        ),
        (
            "batch-3".to_string(),
            "c".to_string(),
            "c1".to_string(),
            "c1c".to_string(),
            0.9,
            "claude-cli".to_string(),
        ),
    ];
    db.batch_update_session_classifications(&updates)
        .await
        .unwrap();

    let all = fetch_actions_by_action(&db, "classify").await;
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].1, "batch-1");
    assert_eq!(all[0].4, "classifier:claude-cli");
    assert_eq!(all[1].1, "batch-2");
    assert_eq!(all[1].4, "classifier:human");
    assert_eq!(all[2].1, "batch-3");

    // seq strictly increasing — §7.1 fold watermark contract.
    for w in all.windows(2) {
        assert!(w[0].0 < w[1].0, "seq must strictly increase in a batch");
    }
}

#[tokio::test]
async fn standalone_dismiss_insert_assigns_seq_and_persists_fields() {
    // Dismiss has no accompanying column write today (the ring buffer
    // is in-memory only). The standalone `Database::insert_action_log`
    // path is the one the live/actions.rs dismiss handler calls.
    let db = Database::new_in_memory().await.unwrap();

    let seq = db
        .insert_action_log("s-dismiss-1", "dismiss", "{}", "user", 1_700_000_000_000)
        .await
        .unwrap();
    assert!(seq >= 1, "seq must be assigned from AUTOINCREMENT");

    let rows = fetch_actions_for(&db, "s-dismiss-1").await;
    assert_eq!(rows.len(), 1);
    let (_, _, action, _, actor, at) = &rows[0];
    assert_eq!(action, "dismiss");
    assert_eq!(actor, "user");
    assert_eq!(*at, 1_700_000_000_000);
}
