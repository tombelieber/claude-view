//! Phase 4 dashboard read helpers — single-call SUM aggregation over
//! rollup rows for a time range. Used by `/api/stats/dashboard` and
//! `/api/insights/*` endpoint cutovers once their handler code swaps
//! from legacy GROUP BY over `session_stats` to pre-folded rollup
//! reads.
//!
//! These helpers are thin wrappers over the macro-generated
//! `select_range_*` functions — they add "SUM the rows" because the
//! endpoints want a single aggregate, not a per-bucket time series.
//!
//! Current readiness:
//!   - All StatsCore token / session / prompt / duration fields land
//!     via Phase 4 Stage C. ✅
//!   - Flag-derived fields (lines_*, commit_*, reedit_*, file_count,
//!     category) stay at 0 until Phase 5 `SessionFlags` fold. ⚠️
//!
//! Handler cutovers should display the fields that are populated
//! and fall back to the legacy path for fields that aren't. Once
//! Phase 5 + PR 4.8 close the remaining gaps, the handler can read
//! *only* from rollups.

use crate::stats_core::{
    select_range_daily_global_stats, select_range_monthly_global_stats,
    select_range_weekly_global_stats,
};
use crate::{Bucket, StatsCore};
use sqlx::SqlitePool;

/// Sum every `daily_global_stats` row with `period_start` in
/// `[start, end)` into a single `StatsCore`.
///
/// Returns `StatsCore::ZERO` when the range has no rows.
///
/// Caller supplies the bucket choice because the same handler might
/// want weekly / monthly views for wider date ranges (SQLite's
/// per-query cost dominates; scanning 12 monthly rows is cheaper than
/// 365 daily rows for a 1-year view).
pub async fn sum_global_stats_in_range(
    pool: &SqlitePool,
    bucket: Bucket,
    start_unix: i64,
    end_unix: i64,
) -> Result<StatsCore, sqlx::Error> {
    let mut acc = StatsCore::ZERO;
    match bucket {
        Bucket::Daily => {
            for row in select_range_daily_global_stats(pool, start_unix, end_unix).await? {
                fold_into(&mut acc, row);
            }
        }
        Bucket::Weekly => {
            for row in select_range_weekly_global_stats(pool, start_unix, end_unix).await? {
                fold_into_weekly(&mut acc, row);
            }
        }
        Bucket::Monthly => {
            for row in select_range_monthly_global_stats(pool, start_unix, end_unix).await? {
                fold_into_monthly(&mut acc, row);
            }
        }
    }
    Ok(acc)
}

fn fold_into(acc: &mut StatsCore, row: crate::stats_core::DailyGlobalStats) {
    acc.session_count += row.session_count;
    acc.total_tokens += row.total_tokens;
    acc.total_cost_cents += row.total_cost_cents;
    acc.prompt_count += row.prompt_count;
    acc.file_count += row.file_count;
    acc.lines_added += row.lines_added;
    acc.lines_removed += row.lines_removed;
    acc.commit_count += row.commit_count;
    acc.commit_insertions += row.commit_insertions;
    acc.commit_deletions += row.commit_deletions;
    acc.duration_sum_ms += row.duration_sum_ms;
    acc.duration_count += row.duration_count;
    acc.reedit_rate_sum += row.reedit_rate_sum;
    acc.reedit_rate_count += row.reedit_rate_count;
}

fn fold_into_weekly(acc: &mut StatsCore, row: crate::stats_core::WeeklyGlobalStats) {
    acc.session_count += row.session_count;
    acc.total_tokens += row.total_tokens;
    acc.total_cost_cents += row.total_cost_cents;
    acc.prompt_count += row.prompt_count;
    acc.file_count += row.file_count;
    acc.lines_added += row.lines_added;
    acc.lines_removed += row.lines_removed;
    acc.commit_count += row.commit_count;
    acc.commit_insertions += row.commit_insertions;
    acc.commit_deletions += row.commit_deletions;
    acc.duration_sum_ms += row.duration_sum_ms;
    acc.duration_count += row.duration_count;
    acc.reedit_rate_sum += row.reedit_rate_sum;
    acc.reedit_rate_count += row.reedit_rate_count;
}

fn fold_into_monthly(acc: &mut StatsCore, row: crate::stats_core::MonthlyGlobalStats) {
    acc.session_count += row.session_count;
    acc.total_tokens += row.total_tokens;
    acc.total_cost_cents += row.total_cost_cents;
    acc.prompt_count += row.prompt_count;
    acc.file_count += row.file_count;
    acc.lines_added += row.lines_added;
    acc.lines_removed += row.lines_removed;
    acc.commit_count += row.commit_count;
    acc.commit_insertions += row.commit_insertions;
    acc.commit_deletions += row.commit_deletions;
    acc.duration_sum_ms += row.duration_sum_ms;
    acc.duration_count += row.duration_count;
    acc.reedit_rate_sum += row.reedit_rate_sum;
    acc.reedit_rate_count += row.reedit_rate_count;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_core::{self, upsert_daily_global_stats, DailyGlobalStats};

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for sql in stats_core::migrations::STATEMENTS {
            sqlx::raw_sql(sql).execute(&pool).await.unwrap();
        }
        pool
    }

    fn row(period: i64, sessions: u64, tokens: u64) -> DailyGlobalStats {
        DailyGlobalStats {
            period_start: period,
            session_count: sessions,
            total_tokens: tokens,
            total_cost_cents: 0,
            prompt_count: 0,
            file_count: 0,
            lines_added: 0,
            lines_removed: 0,
            commit_count: 0,
            commit_insertions: 0,
            commit_deletions: 0,
            duration_sum_ms: 0,
            duration_count: 0,
            reedit_rate_sum: 0.0,
            reedit_rate_count: 0,
        }
    }

    #[tokio::test]
    async fn sum_across_three_daily_rows() {
        let pool = setup().await;
        upsert_daily_global_stats(&pool, &row(1_000_000, 5, 500))
            .await
            .unwrap();
        upsert_daily_global_stats(&pool, &row(1_086_400, 3, 300))
            .await
            .unwrap();
        upsert_daily_global_stats(&pool, &row(1_172_800, 2, 200))
            .await
            .unwrap();

        let sum = sum_global_stats_in_range(&pool, Bucket::Daily, 0, i64::MAX)
            .await
            .unwrap();
        assert_eq!(sum.session_count, 10);
        assert_eq!(sum.total_tokens, 1000);
    }

    #[tokio::test]
    async fn range_filter_excludes_outside_rows() {
        let pool = setup().await;
        upsert_daily_global_stats(&pool, &row(1_000_000, 5, 500))
            .await
            .unwrap();
        upsert_daily_global_stats(&pool, &row(2_000_000, 3, 300))
            .await
            .unwrap();

        // Only the first row falls inside [1_000_000, 1_500_000).
        let sum = sum_global_stats_in_range(&pool, Bucket::Daily, 1_000_000, 1_500_000)
            .await
            .unwrap();
        assert_eq!(sum.session_count, 5);
        assert_eq!(sum.total_tokens, 500);
    }

    #[tokio::test]
    async fn empty_range_returns_zero() {
        let pool = setup().await;
        let sum = sum_global_stats_in_range(&pool, Bucket::Daily, 0, 1_000_000)
            .await
            .unwrap();
        assert_eq!(sum, StatsCore::ZERO);
    }
}
