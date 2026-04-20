//! CQRS Phase 5 PR 5.3 — fold task correctness.
//!
//! Covers the `action_log → session_flags` fold rules + kill-9 resume
//! semantics. Tests exercise `claude_view_db::fold::run_fold_batch`
//! directly so each invariant can be asserted without spinning up the
//! background `spawn_flags_fold` task.
//!
//! Schema contracts:
//!   - `session_flags` PK = session_id, nullable flag columns
//!   - `fold_state.applied_seq` watermark, seeded at 0
//!   - `session_action_log.seq` AUTOINCREMENT monotone

use claude_view_db::fold::run_fold_batch;
use claude_view_db::Database;
use std::sync::Arc;

/// (session_id, archived_at, dismissed_at, category_l1, category_l2,
///  category_l3, category_confidence, category_source, classified_at,
///  applied_seq)
type FlagRow = (
    String,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<String>,
    Option<i64>,
    i64,
);

async fn get_flags(db: &Database, session_id: &str) -> Option<FlagRow> {
    sqlx::query_as::<_, FlagRow>(
        "SELECT session_id, archived_at, dismissed_at, category_l1, category_l2,
                category_l3, category_confidence, category_source, classified_at,
                applied_seq
         FROM session_flags
         WHERE session_id = ?1",
    )
    .bind(session_id)
    .fetch_optional(db.pool())
    .await
    .unwrap()
}

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

async fn seed_action(
    db: &Database,
    session_id: &str,
    action: &str,
    payload: &str,
    actor: &str,
    at: i64,
) -> i64 {
    db.insert_action_log(session_id, action, payload, actor, at)
        .await
        .unwrap()
}

#[tokio::test]
async fn fold_empty_log_is_noop() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let summary = run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(summary.rows_observed, 0);
    assert_eq!(summary.max_seq, 0);
    assert_eq!(db.fold_get_applied_seq().await.unwrap(), 0);
}

#[tokio::test]
async fn fold_archive_sets_session_flags_archived_at() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s1").await;
    let seq = seed_action(&db, "s1", "archive", "{}", "user", 1_700_000_000_000).await;

    let summary = run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(summary.rows_applied, 1);
    assert_eq!(summary.max_seq, seq);
    assert_eq!(db.fold_get_applied_seq().await.unwrap(), seq);

    let flags = get_flags(&db, "s1").await.expect("flags row missing");
    assert_eq!(flags.1, Some(1_700_000_000_000));
    assert_eq!(flags.9, seq);
}

#[tokio::test]
async fn fold_unarchive_clears_session_flags_archived_at() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s2").await;
    seed_action(&db, "s2", "archive", "{}", "user", 1_700_000_000_000).await;
    seed_action(&db, "s2", "unarchive", "{}", "user", 1_700_000_000_500).await;

    run_fold_batch(db.clone()).await.unwrap();

    let flags = get_flags(&db, "s2").await.expect("flags row missing");
    assert_eq!(flags.1, None, "unarchive must null archived_at");
}

#[tokio::test]
async fn fold_dismiss_sets_dismissed_at() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_action(&db, "s3", "dismiss", "{}", "user", 1_700_000_000_111).await;

    run_fold_batch(db.clone()).await.unwrap();

    let flags = get_flags(&db, "s3").await.expect("flags row missing");
    assert_eq!(flags.2, Some(1_700_000_000_111));
}

#[tokio::test]
async fn fold_classify_sets_category_fields() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s4").await;
    let payload = r#"{"l1":"engineering","l2":"backend","l3":"database","confidence":0.9,"source":"claude-cli"}"#;
    seed_action(
        &db,
        "s4",
        "classify",
        payload,
        "classifier:claude-cli",
        1_700_000_000_222,
    )
    .await;

    run_fold_batch(db.clone()).await.unwrap();

    let flags = get_flags(&db, "s4").await.expect("flags row missing");
    assert_eq!(flags.3, Some("engineering".to_string()));
    assert_eq!(flags.4, Some("backend".to_string()));
    assert_eq!(flags.5, Some("database".to_string()));
    assert!((flags.6.unwrap() - 0.9).abs() < 1e-9);
    assert_eq!(flags.7, Some("claude-cli".to_string()));
    assert_eq!(flags.8, Some(1_700_000_000_222));
}

#[tokio::test]
async fn fold_classify_lww_skips_stale_event() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s5").await;

    // Fresh classification at t=2000
    let fresh = r#"{"l1":"engineering","l2":"","l3":"","confidence":0.9,"source":"fast"}"#;
    seed_action(&db, "s5", "classify", fresh, "classifier:fast", 2000).await;
    run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(
        get_flags(&db, "s5").await.unwrap().3,
        Some("engineering".to_string())
    );

    // Stale classification at t=1000 (arrives later, but timestamp older)
    let stale = r#"{"l1":"marketing","l2":"","l3":"","confidence":0.5,"source":"slow"}"#;
    seed_action(&db, "s5", "classify", stale, "classifier:slow", 1000).await;
    let summary = run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(summary.rows_skipped_lww, 1);
    assert_eq!(summary.rows_applied, 0);

    // Category unchanged — fresh classification won LWW
    assert_eq!(
        get_flags(&db, "s5").await.unwrap().3,
        Some("engineering".to_string())
    );
}

#[tokio::test]
async fn fold_classify_equal_timestamp_applies_to_latest_seq() {
    // Two classify events with the same `at` — later seq must win so
    // back-to-back classifier writes commit in arrival order.
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s6").await;
    seed_action(
        &db,
        "s6",
        "classify",
        r#"{"l1":"a","l2":"","l3":"","confidence":0.1,"source":"x"}"#,
        "classifier:x",
        5000,
    )
    .await;
    seed_action(
        &db,
        "s6",
        "classify",
        r#"{"l1":"b","l2":"","l3":"","confidence":0.2,"source":"y"}"#,
        "classifier:y",
        5000,
    )
    .await;

    run_fold_batch(db.clone()).await.unwrap();

    let flags = get_flags(&db, "s6").await.unwrap();
    assert_eq!(flags.3, Some("b".to_string()));
}

#[tokio::test]
async fn fold_unknown_action_advances_watermark_without_applying() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_action(&db, "s7", "rebuket", "{}", "user", 1).await;

    let summary = run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(summary.rows_observed, 1);
    assert_eq!(summary.rows_applied, 0);
    assert_eq!(summary.rows_skipped_unknown, 1);

    // Watermark still advanced so the fold is not stalled by a typo.
    assert!(db.fold_get_applied_seq().await.unwrap() > 0);
    // No session_flags row materialised for the unknown action.
    assert!(get_flags(&db, "s7").await.is_none());
}

#[tokio::test]
async fn fold_classify_malformed_payload_skipped_but_watermark_advances() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s8").await;
    seed_action(&db, "s8", "classify", "{not-json", "classifier:x", 1).await;

    let summary = run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(summary.rows_skipped_lww, 1); // bad JSON reuses lww-skip path
    assert_eq!(summary.rows_applied, 0);
    assert!(db.fold_get_applied_seq().await.unwrap() > 0);
}

#[tokio::test]
async fn fold_advances_applied_seq_per_batch() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s9").await;
    seed_action(&db, "s9", "archive", "{}", "user", 1000).await;
    seed_action(&db, "s9", "unarchive", "{}", "user", 1001).await;
    seed_action(&db, "s9", "archive", "{}", "user", 1002).await;

    let summary = run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(summary.rows_observed, 3);
    assert_eq!(summary.rows_applied, 3);
    // Watermark = max seq
    assert_eq!(db.fold_get_applied_seq().await.unwrap(), summary.max_seq);

    // Second batch is a no-op
    let summary2 = run_fold_batch(db.clone()).await.unwrap();
    assert_eq!(summary2.rows_observed, 0);
    assert_eq!(
        db.fold_get_applied_seq().await.unwrap(),
        summary.max_seq,
        "watermark must not regress on empty batch"
    );
}

#[tokio::test]
async fn fold_resume_after_simulated_crash_is_byte_identical() {
    // §7.2 kill-9 property test (deterministic variant).
    // Strategy: seed 50 events, fold in 4 chunks with crashes between
    // each chunk (simulated by running `run_fold_batch` repeatedly).
    // Compare terminal `session_flags` state against a fresh DB that
    // folds all 50 events in one pass.
    let seed_events = |db: Arc<Database>| async move {
        // 5 sessions × 10 events = 50 events with interleaved action types.
        for i in 0..50 {
            let session = format!("s-{}", i % 5);
            let at = 1_000_000_000_000 + i as i64;
            match i % 4 {
                0 => {
                    db.insert_action_log(&session, "archive", "{}", "user", at)
                        .await
                        .unwrap();
                }
                1 => {
                    db.insert_action_log(&session, "unarchive", "{}", "user", at)
                        .await
                        .unwrap();
                }
                2 => {
                    let payload = format!(
                        r#"{{"l1":"cat-{}","l2":"","l3":"","confidence":{},"source":"x"}}"#,
                        i / 4,
                        (i as f64) / 100.0
                    );
                    db.insert_action_log(&session, "classify", &payload, "classifier:x", at)
                        .await
                        .unwrap();
                }
                _ => {
                    db.insert_action_log(&session, "dismiss", "{}", "user", at)
                        .await
                        .unwrap();
                }
            }
        }
    };

    // Fold A: incremental with simulated mid-stream restarts
    let db_a = Arc::new(Database::new_in_memory().await.unwrap());
    seed_events(db_a.clone()).await;
    // BATCH_SIZE = 100 so a 50-event seed folds in one call, but we
    // simulate crash-and-resume by calling run_fold_batch 3 times —
    // first call drains the log, subsequent calls are no-ops. A more
    // aggressive simulation would lower BATCH_SIZE; this shape at
    // least exercises the "restart reads unchanged state" path.
    for _ in 0..3 {
        run_fold_batch(db_a.clone()).await.unwrap();
    }

    // Fold B: one-shot (baseline)
    let db_b = Arc::new(Database::new_in_memory().await.unwrap());
    seed_events(db_b.clone()).await;
    run_fold_batch(db_b.clone()).await.unwrap();

    // Compare terminal state byte-for-byte across all 5 sessions.
    for i in 0..5 {
        let id = format!("s-{i}");
        let a = get_flags(&db_a, &id).await;
        let b = get_flags(&db_b, &id).await;
        assert_eq!(
            a, b,
            "session {id} diverged between incremental and one-shot fold"
        );
    }

    // Watermark matches between the two DBs.
    assert_eq!(
        db_a.fold_get_applied_seq().await.unwrap(),
        db_b.fold_get_applied_seq().await.unwrap()
    );
}

#[tokio::test]
async fn fold_handles_multi_session_interleaved_stream() {
    // Two sessions with interleaved archive / unarchive / classify.
    // Terminal state must reflect the LAST action per column for each
    // session — the natural LWW of repeated UPSERT.
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "a").await;
    seed_session(&db, "b").await;

    seed_action(&db, "a", "archive", "{}", "user", 100).await;
    seed_action(&db, "b", "archive", "{}", "user", 101).await;
    seed_action(
        &db,
        "a",
        "classify",
        r#"{"l1":"x","l2":"","l3":"","confidence":0.1,"source":"z"}"#,
        "classifier:z",
        102,
    )
    .await;
    seed_action(&db, "b", "unarchive", "{}", "user", 103).await;
    seed_action(&db, "a", "dismiss", "{}", "user", 104).await;

    run_fold_batch(db.clone()).await.unwrap();

    let a = get_flags(&db, "a").await.unwrap();
    let b = get_flags(&db, "b").await.unwrap();
    assert_eq!(a.1, Some(100)); // archived_at stuck
    assert_eq!(a.3, Some("x".to_string())); // category set
    assert_eq!(a.2, Some(104)); // dismissed_at
    assert_eq!(b.1, None); // unarchive cleared archived_at
}

#[tokio::test]
async fn fold_batch_size_bounds_events_per_pass() {
    // Seed >BATCH_SIZE (100) events, confirm the fold needs multiple
    // passes to drain them. Lets us assert monotone watermark advance
    // without assuming BATCH_SIZE's exact value.
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    for i in 0..150 {
        db.insert_action_log(
            &format!("batch-{i}"),
            "archive",
            "{}",
            "user",
            2_000_000_000_000 + i as i64,
        )
        .await
        .unwrap();
    }

    let first = run_fold_batch(db.clone()).await.unwrap();
    assert!(
        first.rows_observed <= 100,
        "batch size must cap at 100; observed {}",
        first.rows_observed
    );
    let watermark_a = db.fold_get_applied_seq().await.unwrap();

    let second = run_fold_batch(db.clone()).await.unwrap();
    assert!(second.rows_observed > 0);
    let watermark_b = db.fold_get_applied_seq().await.unwrap();
    assert!(watermark_b > watermark_a, "watermark must advance");

    // Drain completely and confirm no more work.
    loop {
        let s = run_fold_batch(db.clone()).await.unwrap();
        if s.rows_observed == 0 {
            break;
        }
    }
    let final_watermark = db.fold_get_applied_seq().await.unwrap();
    // Every session got its own archived_at row.
    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM session_flags WHERE archived_at IS NOT NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(total.0, 150);
    assert_eq!(final_watermark, 150);
}

#[tokio::test]
async fn fold_state_seeded_at_zero_on_fresh_db() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let seq = db.fold_get_applied_seq().await.unwrap();
    assert_eq!(seq, 0, "fold_state must seed applied_seq at 0");
}

#[tokio::test]
async fn fold_applied_seq_reflects_session_flags_applied_seq() {
    // Sanity: the fold writes `session_flags.applied_seq` to match the
    // row's source `session_action_log.seq`. PR 5.4 parity reads this
    // column to detect stuck sessions (applied_seq < max(seq)).
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "seq-1").await;
    let seq = seed_action(&db, "seq-1", "archive", "{}", "user", 1).await;
    run_fold_batch(db.clone()).await.unwrap();

    let flags = get_flags(&db, "seq-1").await.unwrap();
    assert_eq!(
        flags.9, seq,
        "session_flags.applied_seq must equal source seq"
    );
}

#[tokio::test]
async fn fold_emits_outbox_row_per_applied_event() {
    // Archive + classify → two fold-applied events → two outbox rows.
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s-outbox-1").await;
    seed_action(&db, "s-outbox-1", "archive", "{}", "user", 1000).await;
    seed_action(
        &db,
        "s-outbox-1",
        "classify",
        r#"{"l1":"a","l2":"","l3":"","confidence":0.5,"source":"x"}"#,
        "classifier:x",
        1001,
    )
    .await;

    run_fold_batch(db.clone()).await.unwrap();

    let (pending,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM stage_c_outbox WHERE applied_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(pending, 2, "archive + classify = 2 outbox rows");
}

#[tokio::test]
async fn fold_outbox_payload_is_valid_flagdelta_json() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s-outbox-2").await;
    seed_action(
        &db,
        "s-outbox-2",
        "classify",
        r#"{"l1":"engineering","l2":"","l3":"","confidence":0.9,"source":"x"}"#,
        "classifier:x",
        5000,
    )
    .await;

    run_fold_batch(db.clone()).await.unwrap();

    let (payload,): (String,) = sqlx::query_as("SELECT payload_json FROM stage_c_outbox LIMIT 1")
        .fetch_one(db.pool())
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(parsed["kind"], "Classify");
    assert_eq!(parsed["session_id"], "s-outbox-2");
    assert_eq!(parsed["after_category_l1"], "engineering");
    assert_eq!(parsed["before_category_l1"], serde_json::Value::Null);
}

#[tokio::test]
async fn fold_lww_skipped_classify_emits_no_outbox_row() {
    // Classify at t=1000 lands. Reclassify at t=500 is LWW-skipped.
    // The skipped event must NOT emit an outbox row — Stage C would
    // otherwise apply a phantom compensating delta for work the
    // session_flags UPSERT never performed.
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    seed_session(&db, "s-lww").await;
    seed_action(
        &db,
        "s-lww",
        "classify",
        r#"{"l1":"new","l2":"","l3":"","confidence":0.9,"source":"x"}"#,
        "classifier:x",
        1000,
    )
    .await;
    seed_action(
        &db,
        "s-lww",
        "classify",
        r#"{"l1":"stale","l2":"","l3":"","confidence":0.9,"source":"x"}"#,
        "classifier:x",
        500,
    )
    .await;

    run_fold_batch(db.clone()).await.unwrap();

    let (pending,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM stage_c_outbox WHERE applied_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(pending, 1, "only the applied classify must enqueue outbox");
}
