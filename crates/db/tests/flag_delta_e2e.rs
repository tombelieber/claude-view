//! CQRS Phase 4.9 end-to-end: action_log → fold → session_flags
//! → stage_c_outbox → drainer → rollup tables.
//!
//! Wires the full flag-fold→rollup loop without the background task.
//! Each test seeds sessions + stats + actions, runs one fold batch and
//! one drain batch synchronously, and asserts the rollup table reflects
//! the compensating delta.

use claude_view_db::fold::run_fold_batch;
use claude_view_db::stage_c::run_drain_batch;
use claude_view_db::Database;
use std::sync::Arc;

async fn seed_session_and_stats(db: &Database, sid: &str) {
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, is_sidechain)
         VALUES (?1, 'p1', ?2, 0)",
    )
    .bind(sid)
    .bind(format!("/tmp/{sid}.jsonl"))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_stats (session_id, source_content_hash, source_size,
            parser_version, stats_version, indexed_at,
            total_input_tokens, total_output_tokens, user_prompt_count,
            duration_seconds, last_message_at, project_id)
         VALUES (?1, X'00', 0, 1, 1, 0,
                 100, 200, 5, 60, 1700000000000, 'p1')",
    )
    .bind(sid)
    .execute(db.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn classify_round_trip_populates_daily_category_stats() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session_and_stats(&db, "e2e-1").await;

    db.insert_action_log(
        "e2e-1",
        "classify",
        r#"{"l1":"engineering","l2":"","l3":"","confidence":0.9,"source":"x"}"#,
        "classifier:x",
        1_700_000_000_000,
    )
    .await
    .unwrap();

    // Fold action_log → session_flags + stage_c_outbox
    run_fold_batch(db.clone()).await.unwrap();
    // Drain stage_c_outbox → rollup UPDATEs
    run_drain_batch(db.clone()).await.unwrap();

    let (session_count, total_tokens): (i64, i64) = sqlx::query_as(
        "SELECT session_count, total_tokens FROM daily_category_stats
         WHERE category_l1 = 'engineering'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(session_count, 1);
    assert_eq!(total_tokens, 300);

    let (pending,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM stage_c_outbox WHERE applied_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(pending, 0, "outbox must be fully drained");

    let (applied,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM stage_c_outbox WHERE applied_at IS NOT NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(applied, 1, "classify emits exactly one outbox row");
}

#[tokio::test]
async fn archive_then_unarchive_leaves_rollups_clean() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session_and_stats(&db, "e2e-2").await;

    // Seed rollup state: classify first so category dimension is populated,
    // then archive (subtract), then unarchive (re-add) — net zero.
    db.insert_action_log(
        "e2e-2",
        "classify",
        r#"{"l1":"marketing","l2":"","l3":"","confidence":0.9,"source":"x"}"#,
        "classifier:x",
        1_700_000_000_000,
    )
    .await
    .unwrap();
    db.insert_action_log("e2e-2", "archive", "{}", "user", 1_700_000_000_001)
        .await
        .unwrap();
    db.insert_action_log("e2e-2", "unarchive", "{}", "user", 1_700_000_000_002)
        .await
        .unwrap();

    run_fold_batch(db.clone()).await.unwrap();
    run_drain_batch(db.clone()).await.unwrap();

    // Classify added +1/+300 to marketing, archive subtracted -1/-300 globally
    // (and from marketing), unarchive added +1/+300 back. Category ends at
    // +1/+300, global ends at net 0 — which is the correct steady state
    // for a currently-unarchived session that contributed its stats once.
    let (marketing_count, marketing_tokens): (i64, i64) = sqlx::query_as(
        "SELECT session_count, total_tokens FROM daily_category_stats
         WHERE category_l1 = 'marketing'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(marketing_count, 1);
    assert_eq!(marketing_tokens, 300);

    let (pending,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM stage_c_outbox WHERE applied_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(pending, 0, "outbox must be fully drained");
}

#[tokio::test]
async fn dismiss_emits_noop_outbox_row() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session_and_stats(&db, "e2e-3").await;

    db.insert_action_log("e2e-3", "dismiss", "{}", "user", 1_700_000_000_000)
        .await
        .unwrap();

    run_fold_batch(db.clone()).await.unwrap();
    run_drain_batch(db.clone()).await.unwrap();

    // Dismiss is audit-only — no rollup dimension, so rollup tables stay empty.
    let (rollup_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM daily_global_stats")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        rollup_rows, 0,
        "dismiss does not touch any rollup dimension"
    );

    // But the outbox row was still applied (marked applied_at NOT NULL).
    let (applied,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM stage_c_outbox WHERE applied_at IS NOT NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(applied, 1, "dismiss enqueues outbox, drainer marks applied");
}
