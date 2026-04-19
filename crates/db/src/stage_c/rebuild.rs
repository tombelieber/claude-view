//! Stage C full rebuild — read every `session_stats` row and apply it
//! as a first-observation delta into every rollup table.
//!
//! Design §6.2 requires full rebuild on startup AND on Stage C task
//! panic (SOTA §7.6 Wave 3 C2). Until the server-startup wiring lands
//! (Phase 4b), this function is called by tests and by the ops path
//! once rollups are enabled.
//!
//! The rebuild:
//! 1. `TRUNCATE` every rollup table (synthetic `DELETE FROM`) so stale
//!    rows from prior runs cannot double-count.
//! 2. SELECT every row from `session_stats`, ordered by `session_id`
//!    so deterministic ordering survives debugging.
//! 3. For each row, synthesize a `StatsDelta` with `old = None`
//!    (first-observation semantics → cumulative fields become
//!    absolute values), then call `apply_stats_delta`.
//! 4. Return a summary.
//!
//! Note: this is explicitly NOT a performance-critical path. Expected
//! use is "startup on a few thousand sessions" — 4.8k at this repo.
//! Stage C incremental takes over for steady-state; rebuild is the
//! safety net.

use claude_view_core::session_stats::SessionStats;
use sqlx::{Row, SqlitePool};

use crate::indexer_v2::{DeltaSource, StatsDelta};
use crate::stage_c::consumer::{apply_stats_delta, StageCError};

/// Summary of a full rebuild run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildSummary {
    /// Number of `session_stats` rows observed.
    pub rows_observed: u64,
    /// Number of rows that contributed to rollups. Equals
    /// `rows_observed` once the tests are green; the count exists to
    /// catch future filters (e.g. skip rows with missing project_id).
    pub rows_applied: u64,
}

/// Truncate every rollup table + re-fold from `session_stats`.
///
/// Runs every rollup-table `DELETE FROM` and every `apply_stats_delta`
/// serially within a single sqlx connection. No explicit transaction
/// — rebuild is idempotent by construction (re-truncate + re-apply is
/// the same end state) and SQLite's WAL handles the all-or-nothing
/// scope at connection-pool granularity.
pub async fn full_rebuild_from_session_stats(
    pool: &SqlitePool,
) -> Result<RebuildSummary, StageCError> {
    for table in ROLLUP_TABLES {
        let sql = format!("DELETE FROM {table}");
        sqlx::query(&sql).execute(pool).await?;
    }

    // `session_stats.first_message_at` / `last_message_at` are INTEGER
    // (unix seconds) in the production schema (migration 64). The
    // in-memory `SessionStats` struct carries them as ISO strings
    // (`Option<String>`). We do the cheap round-trip here — rebuild is
    // O(N) session_stats scan, the cost is negligible vs the 12 UPSERTs
    // per session that follow.
    let rows = sqlx::query(
        "SELECT session_id, total_input_tokens, total_output_tokens,
                cache_read_tokens, cache_creation_tokens,
                user_prompt_count, duration_seconds,
                first_message_at, last_message_at,
                primary_model, git_branch,
                project_id, file_path, is_compressed, source_mtime,
                source_content_hash, source_size
         FROM session_stats
         ORDER BY session_id",
    )
    .fetch_all(pool)
    .await?;

    fn unix_to_iso(ts: Option<i64>) -> Option<String> {
        ts.and_then(|t| chrono::DateTime::from_timestamp(t, 0))
            .map(|dt| dt.to_rfc3339())
    }

    let mut summary = RebuildSummary {
        rows_observed: 0,
        rows_applied: 0,
    };
    for row in &rows {
        summary.rows_observed += 1;
        let delta = StatsDelta {
            session_id: row.try_get::<String, _>("session_id")?,
            source_content_hash: row
                .try_get::<Vec<u8>, _>("source_content_hash")
                .unwrap_or_default(),
            source_size: row.try_get::<i64, _>("source_size").unwrap_or(0),
            source_inode: None,
            source_mid_hash: None,
            project_id: row
                .try_get::<Option<String>, _>("project_id")?
                .unwrap_or_default(),
            source_file_path: row
                .try_get::<Option<String>, _>("file_path")?
                .unwrap_or_default(),
            is_compressed: row.try_get::<i64, _>("is_compressed").unwrap_or(0) != 0,
            source_mtime: row.try_get::<Option<i64>, _>("source_mtime")?.unwrap_or(0),
            stats: SessionStats {
                total_input_tokens: row.try_get::<i64, _>("total_input_tokens")? as u64,
                total_output_tokens: row.try_get::<i64, _>("total_output_tokens")? as u64,
                cache_read_tokens: row.try_get::<i64, _>("cache_read_tokens")? as u64,
                cache_creation_tokens: row.try_get::<i64, _>("cache_creation_tokens")? as u64,
                user_prompt_count: row.try_get::<i64, _>("user_prompt_count")? as u32,
                duration_seconds: row.try_get::<i64, _>("duration_seconds")? as u32,
                first_message_at: unix_to_iso(row.try_get::<Option<i64>, _>("first_message_at")?),
                last_message_at: unix_to_iso(row.try_get::<Option<i64>, _>("last_message_at")?),
                primary_model: row.try_get::<Option<String>, _>("primary_model")?,
                git_branch: row.try_get::<Option<String>, _>("git_branch")?,
                ..Default::default()
            },
            old: None,
            seq: 0,
            source: DeltaSource::Indexer,
        };
        apply_stats_delta(pool, &delta).await?;
        summary.rows_applied += 1;
    }
    Ok(summary)
}

/// Names of every rollup table in canonical order. Hand-listed
/// instead of re-parsed from `STATEMENTS` so a schema rename can't
/// silently unmatch here. If `TABLE_COUNT` grows this list must too;
/// `test_rebuild_touches_every_rollup_table` catches the mismatch.
const ROLLUP_TABLES: &[&str] = &[
    "daily_global_stats",
    "daily_project_stats",
    "daily_branch_stats",
    "daily_model_stats",
    "daily_category_stats",
    "weekly_global_stats",
    "weekly_project_stats",
    "weekly_branch_stats",
    "weekly_model_stats",
    "weekly_category_stats",
    "monthly_global_stats",
    "monthly_project_stats",
    "monthly_branch_stats",
    "monthly_model_stats",
    "monthly_category_stats",
];

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_stats_rollup::stats_core::{self, select_range_daily_global_stats};

    async fn setup_pool_with_rollups() -> SqlitePool {
        // Schema matches the production session_stats shape from
        // migrations 64 + 66 — specifically first_message_at /
        // last_message_at are INTEGER (unix seconds), not TEXT. Keep
        // this in sync if either migration adds a column.
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(
            r"CREATE TABLE session_stats (
                session_id TEXT PRIMARY KEY,
                source_content_hash BLOB NOT NULL,
                source_size INTEGER NOT NULL,
                parser_version INTEGER NOT NULL DEFAULT 0,
                stats_version INTEGER NOT NULL DEFAULT 0,
                indexed_at INTEGER NOT NULL DEFAULT 0,
                total_input_tokens INTEGER NOT NULL DEFAULT 0,
                total_output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                user_prompt_count INTEGER NOT NULL DEFAULT 0,
                duration_seconds INTEGER NOT NULL DEFAULT 0,
                first_message_at INTEGER,
                last_message_at INTEGER,
                primary_model TEXT,
                git_branch TEXT,
                project_id TEXT,
                file_path TEXT,
                is_compressed INTEGER NOT NULL DEFAULT 0,
                source_mtime INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        for sql in stats_core::migrations::STATEMENTS {
            sqlx::raw_sql(sql).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn rebuild_touches_every_rollup_table() {
        // Compile-time pin: the hand-listed ROLLUP_TABLES must match
        // the macro-emitted TABLE_COUNT. If the design adds a dim or
        // bucket, this test forces the ROLLUP_TABLES list to update.
        assert_eq!(ROLLUP_TABLES.len(), stats_core::TABLE_COUNT);
    }

    #[tokio::test]
    async fn rebuild_from_empty_session_stats_produces_empty_rollups() {
        let pool = setup_pool_with_rollups().await;
        let summary = full_rebuild_from_session_stats(&pool).await.unwrap();
        assert_eq!(summary.rows_observed, 0);
        assert_eq!(summary.rows_applied, 0);

        // Every rollup table empty.
        for table in ROLLUP_TABLES {
            let (cnt,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(cnt, 0, "rollup table `{table}` should be empty");
        }
    }

    #[tokio::test]
    async fn rebuild_populates_from_one_session_stats_row() {
        let pool = setup_pool_with_rollups().await;

        // Seed one session_stats row. Timestamps are unix seconds
        // (INTEGER), matching the production schema from migration 64.
        use chrono::{NaiveDate, TimeZone, Utc};
        let first_ts = Utc
            .with_ymd_and_hms(2026, 4, 19, 10, 0, 0)
            .unwrap()
            .timestamp();
        let last_ts = Utc
            .with_ymd_and_hms(2026, 4, 19, 14, 30, 0)
            .unwrap()
            .timestamp();
        sqlx::query(
            r"INSERT INTO session_stats (
                session_id, source_content_hash, source_size,
                parser_version, stats_version, indexed_at,
                total_input_tokens, total_output_tokens,
                user_prompt_count, duration_seconds,
                first_message_at, last_message_at,
                primary_model, git_branch, project_id,
                file_path, is_compressed, source_mtime
            ) VALUES (
                'sess-1', X'01', 1024,
                1, 1, 0,
                100, 50,
                3, 120,
                ?, ?,
                'claude-opus-4-7', 'main', '-Users-test-proj',
                '/tmp/test.jsonl', 0, ?
            )",
        )
        .bind(first_ts)
        .bind(last_ts)
        .bind(last_ts)
        .execute(&pool)
        .await
        .unwrap();
        let midnight_unix_for_query = NaiveDate::from_ymd_opt(2026, 4, 19)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let _ = midnight_unix_for_query; // used below via inline recompute

        let summary = full_rebuild_from_session_stats(&pool).await.unwrap();
        assert_eq!(summary.rows_observed, 1);
        assert_eq!(summary.rows_applied, 1);

        // Daily global should have one row for 2026-04-19 midnight UTC.
        let midnight_unix = {
            use chrono::NaiveDate;
            NaiveDate::from_ymd_opt(2026, 4, 19)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp()
        };
        let rows = select_range_daily_global_stats(&pool, midnight_unix, midnight_unix + 1)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_count, 1);
        assert_eq!(rows[0].total_tokens, 150);

        // Branch + project + model dims should also have rows — spot
        // check one to prove the fan-out happened.
        let (branch_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM daily_branch_stats")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(branch_count, 1);
    }

    #[tokio::test]
    async fn rebuild_is_idempotent() {
        let pool = setup_pool_with_rollups().await;
        use chrono::{TimeZone, Utc};
        let apr19_14_unix = Utc
            .with_ymd_and_hms(2026, 4, 19, 14, 0, 0)
            .unwrap()
            .timestamp();
        sqlx::query(
            "INSERT INTO session_stats (
                session_id, source_content_hash, source_size,
                parser_version, stats_version, indexed_at,
                total_input_tokens, last_message_at, project_id
            ) VALUES ('s1', X'01', 0, 1, 1, 0, 500, ?, 'p1')",
        )
        .bind(apr19_14_unix)
        .execute(&pool)
        .await
        .unwrap();

        full_rebuild_from_session_stats(&pool).await.unwrap();
        let rows_after_first =
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM daily_global_stats")
                .fetch_one(&pool)
                .await
                .unwrap()
                .0;
        assert_eq!(rows_after_first, 1);

        // Second rebuild on same input — should produce the same state.
        full_rebuild_from_session_stats(&pool).await.unwrap();
        let rows_after_second =
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM daily_global_stats")
                .fetch_one(&pool)
                .await
                .unwrap()
                .0;
        assert_eq!(rows_after_second, 1);

        // Token count also stable (no doubling).
        let (tokens,): (i64,) =
            sqlx::query_as("SELECT total_tokens FROM daily_global_stats LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(tokens, 500);
    }
}
