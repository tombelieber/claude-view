//! CQRS Phase 5.5 gate — deterministic parity replaces the 48h soak.
//!
//! Wipes `session_flags` + `fold_state` + `stage_c_outbox`, runs the
//! fold + outbox drainer until queues are empty, then runs the parity
//! sweep over ALL sessions. Asserts `total_diverged == 0`.
//!
//! Pass `CLAUDE_VIEW_PARITY_DB_PATH=<path>` to run against the real
//! local DB instead of the in-memory fixture. Local M5 Max with an
//! 8,500-session corpus completes in ~5-10 s.

use claude_view_db::fold::{run_fold_batch, run_parity_sweep};
use claude_view_db::stage_c::run_drain_batch;
use claude_view_db::Database;
use std::sync::Arc;

/// Drain fold + outbox to completion so the parity sweep observes
/// steady state. A fresh fixture with N actions completes in one pass
/// each; the loop is load-bearing for larger fixtures where
/// `FOLD_BATCH_SIZE` / `BATCH_SIZE` bound per-call work.
async fn drain_all(db: Arc<Database>) {
    loop {
        let s = run_fold_batch(db.clone()).await.unwrap();
        if s.rows_observed == 0 {
            break;
        }
    }
    loop {
        let s = run_drain_batch(db.clone()).await.unwrap();
        if s.rows_observed == 0 {
            break;
        }
    }
}

#[tokio::test]
async fn deterministic_parity_on_full_corpus() {
    let db = if let Ok(path) = std::env::var("CLAUDE_VIEW_PARITY_DB_PATH") {
        Arc::new(Database::new(std::path::Path::new(&path)).await.unwrap())
    } else {
        let d = Arc::new(Database::new_in_memory().await.unwrap());
        seed_parity_fixture(&d).await;
        d
    };

    // Wipe shadow state. Parity must hold after a full rebuild —
    // anything else would imply the fold is non-idempotent.
    sqlx::query("DELETE FROM session_flags")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE fold_state SET applied_seq = 0 WHERE id = 0")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM stage_c_outbox")
        .execute(db.pool())
        .await
        .unwrap();

    drain_all(db.clone()).await;

    let summary = run_parity_sweep(&db, i64::MAX).await.unwrap();

    assert_eq!(
        summary.total_diverged, 0,
        "parity gate FAILED — {} sessions diverged. Per-field: {:?}",
        summary.total_diverged, summary.per_field_counts
    );
    println!(
        "deterministic parity PASSED: sampled={} diverged={} missing_shadow={}",
        summary.total_sampled, summary.total_diverged, summary.total_missing_shadow
    );
}

/// 100 sessions × (archive + classify). Post-migration-85 the legacy
/// archive/category columns no longer exist, so the fixture only seeds
/// the action log. The fold task then populates `session_flags`; the
/// parity sweep compares the resulting shadow state to itself (the
/// legacy side returns `None` — see `parity::load_legacy`). The test
/// therefore exercises that the fold is internally consistent and
/// idempotent across kill-9 style wipes of `session_flags`.
async fn seed_parity_fixture(db: &Database) {
    let ms: i64 = 1_767_225_600_000; // 2026-01-01T00:00:00Z
    for i in 0..100 {
        let sid = format!("pf-{i}");
        sqlx::query(
            "INSERT INTO session_stats (session_id, source_content_hash, source_size,
                 parser_version, stats_version, indexed_at, project_id, file_path, is_sidechain)
             VALUES (?1, X'00', 0, 1, 4, 0, 'p', ?2, 0)",
        )
        .bind(&sid)
        .bind(format!("/tmp/{sid}.jsonl"))
        .execute(db.pool())
        .await
        .unwrap();
        db.insert_action_log(&sid, "archive", "{}", "user", ms)
            .await
            .unwrap();
        db.insert_action_log(
            &sid,
            "classify",
            r#"{"l1":"engineering","l2":"","l3":"","confidence":0.9,"source":"x"}"#,
            "classifier:x",
            ms,
        )
        .await
        .unwrap();
    }
}
