//! End-to-end integration test for the Phase 4 rollup pipeline.
//!
//! Exercises the full Stage C chain:
//!
//! ```text
//!  seed session_stats rows
//!          │
//!          ▼
//!  full_rebuild_from_session_stats
//!          │
//!          ▼
//!  UPSERTs into 15 rollup tables
//!          │
//!          ▼
//!  sum_global_stats_in_range
//!          │
//!          ▼
//!  assert totals match the seeded sum
//! ```
//!
//! A mismatch here means one of the Stage C links is broken — either
//! the delta math, the bucket alignment, the rollup UPSERT, or the
//! read-side SUM aggregation. The test intentionally keeps all four
//! in the same scope so a failure points at "the integration", not
//! "which component".

use chrono::{NaiveDate, TimeZone, Utc};
use claude_view_db::stage_c::full_rebuild_from_session_stats;
use claude_view_db::Database;
use claude_view_stats_rollup::{sum_global_stats_in_range, Bucket};
use sqlx::SqlitePool;

/// Boilerplate: wraps `Database::new_in_memory()` so the integration
/// test applies the canonical migration sequence via the same code
/// path production uses. Returns the raw `SqlitePool` because the
/// Stage C functions take a pool directly.
async fn setup_full_db() -> (Database, SqlitePool) {
    let db = Database::new_in_memory()
        .await
        .expect("in-memory DB should open + migrate cleanly");
    let pool = db.pool().clone();
    (db, pool)
}

#[tokio::test]
async fn stage_c_rollup_e2e_two_sessions_one_day() {
    let (_db, pool) = setup_full_db().await;

    // Two sessions on the same day → one row in daily_global_stats
    // with sums = 300 tokens + 2 sessions.
    // 2026-04-19 10:00 UTC — INTEGER unix seconds per migration 64.
    let apr19_unix = NaiveDate::from_ymd_opt(2026, 4, 19)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    for (sid, tokens) in [("s-a", 100), ("s-b", 200)] {
        sqlx::query(
            r"INSERT INTO session_stats (
                session_id, source_content_hash, source_size,
                parser_version, stats_version, indexed_at,
                total_input_tokens, last_message_at, project_id
            ) VALUES (?, X'01', 0, 1, 1, 0, ?, ?, 'p-e2e')",
        )
        .bind(sid)
        .bind(tokens as i64)
        .bind(apr19_unix)
        .execute(&pool)
        .await
        .unwrap();
    }

    let summary = full_rebuild_from_session_stats(&pool).await.unwrap();
    assert_eq!(summary.rows_observed, 2);
    assert_eq!(summary.rows_applied, 2);

    // Daily: 2026-04-19 midnight UTC ± 1 day range.
    let apr19_midnight = NaiveDate::from_ymd_opt(2026, 4, 19)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    let sum =
        sum_global_stats_in_range(&pool, Bucket::Daily, apr19_midnight, apr19_midnight + 86400)
            .await
            .unwrap();

    assert_eq!(sum.session_count, 2, "both sessions counted once");
    assert_eq!(sum.total_tokens, 300, "input tokens summed");
}

#[tokio::test]
async fn stage_c_rollup_e2e_monthly_aggregates_across_days() {
    let (_db, pool) = setup_full_db().await;

    // Three sessions spread across April 2026 — unix seconds.
    let seeds: [(&str, i64, i64); 3] = [
        (
            "s-early",
            Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0)
                .unwrap()
                .timestamp(),
            100,
        ),
        (
            "s-mid",
            Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0)
                .unwrap()
                .timestamp(),
            200,
        ),
        (
            "s-late",
            Utc.with_ymd_and_hms(2026, 4, 29, 23, 59, 0)
                .unwrap()
                .timestamp(),
            300,
        ),
    ];
    for (sid, ts, tokens) in seeds {
        sqlx::query(
            r"INSERT INTO session_stats (
                session_id, source_content_hash, source_size,
                parser_version, stats_version, indexed_at,
                total_input_tokens, last_message_at, project_id
            ) VALUES (?, X'01', 0, 1, 1, 0, ?, ?, 'p-month')",
        )
        .bind(sid)
        .bind(tokens)
        .bind(ts)
        .execute(&pool)
        .await
        .unwrap();
    }

    full_rebuild_from_session_stats(&pool).await.unwrap();

    // Monthly bucket for April: 2026-04-01 00:00 UTC.
    let april_start = Utc
        .with_ymd_and_hms(2026, 4, 1, 0, 0, 0)
        .unwrap()
        .timestamp();
    let may_start = Utc
        .with_ymd_and_hms(2026, 5, 1, 0, 0, 0)
        .unwrap()
        .timestamp();

    let sum = sum_global_stats_in_range(&pool, Bucket::Monthly, april_start, may_start)
        .await
        .unwrap();
    assert_eq!(sum.session_count, 3);
    assert_eq!(sum.total_tokens, 600);
}

#[tokio::test]
async fn stage_c_rollup_idempotent_under_repeat_rebuild() {
    let (_db, pool) = setup_full_db().await;
    let apr19_14_unix = Utc
        .with_ymd_and_hms(2026, 4, 19, 14, 0, 0)
        .unwrap()
        .timestamp();
    sqlx::query(
        r"INSERT INTO session_stats (
            session_id, source_content_hash, source_size,
            parser_version, stats_version, indexed_at,
            total_input_tokens, last_message_at, project_id
        ) VALUES ('s-idem', X'01', 0, 1, 1, 0, 500, ?, 'p-idem')",
    )
    .bind(apr19_14_unix)
    .execute(&pool)
    .await
    .unwrap();

    // Run rebuild twice; end state must match single-run state.
    for _ in 0..2 {
        full_rebuild_from_session_stats(&pool).await.unwrap();
    }

    let apr19_midnight = Utc
        .with_ymd_and_hms(2026, 4, 19, 0, 0, 0)
        .unwrap()
        .timestamp();
    let sum =
        sum_global_stats_in_range(&pool, Bucket::Daily, apr19_midnight, apr19_midnight + 86400)
            .await
            .unwrap();
    assert_eq!(sum.session_count, 1, "re-rebuild must not double-count");
    assert_eq!(sum.total_tokens, 500);
}
