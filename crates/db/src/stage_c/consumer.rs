//! Stage C incremental consumer — one `StatsDelta` → 12 UPSERTs.
//!
//! Fans out across `(Daily, Weekly, Monthly)` × `(global, project,
//! branch?, model?)`. Branch and model are conditional on whether the
//! observation has a git branch / primary model — absent values skip
//! that dimension rather than inserting with an empty-string key.
//!
//! Idempotency model: each dimension uses pointwise-sum `ON CONFLICT DO
//! UPDATE`. Stage C owns the delta computation (`StatsCore::delta_from`)
//! so the same session observed twice adds the difference, not the
//! absolute value, to existing rollup rows.
//!
//! Error handling: we fail fast on the first SQL error and bubble it
//! up. Upstream producers (indexer-v2, live-tail) log + drop on error
//! — they do not block their ingest loop on Stage C's slowness.

use chrono::{DateTime, TimeZone, Utc};
use claude_view_stats_rollup::stats_core::{
    upsert_daily_branch_stats, upsert_daily_global_stats, upsert_daily_model_stats,
    upsert_daily_project_stats, upsert_monthly_branch_stats, upsert_monthly_global_stats,
    upsert_monthly_model_stats, upsert_monthly_project_stats, upsert_weekly_branch_stats,
    upsert_weekly_global_stats, upsert_weekly_model_stats, upsert_weekly_project_stats,
    DailyBranchStats, DailyGlobalStats, DailyModelStats, DailyProjectStats, MonthlyBranchStats,
    MonthlyGlobalStats, MonthlyModelStats, MonthlyProjectStats, WeeklyBranchStats,
    WeeklyGlobalStats, WeeklyModelStats, WeeklyProjectStats,
};
use claude_view_stats_rollup::{Bucket, StatsCore};
use sqlx::SqlitePool;
use thiserror::Error;

use crate::indexer_v2::StatsDelta;

/// Errors raised by the Stage C incremental consumer.
#[derive(Debug, Error)]
pub enum StageCError {
    #[error("sqlx error while applying StatsDelta to rollup table: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// Apply one `StatsDelta` to all 12 affected rollup tables.
///
/// Called by the Phase 4b Stage C consumer task once per `recv()` from
/// the shared `mpsc::Receiver<StatsDelta>`. This function performs no
/// batching — the caller decides whether to serialize or parallelize
/// multiple deltas. Serial is fine in Phase 4; a fan-in batch is a
/// Phase 7 optimization tracked in §13.
pub async fn apply_stats_delta(pool: &SqlitePool, delta: &StatsDelta) -> Result<(), StageCError> {
    let core = StatsCore::delta_from(&delta.stats, delta.old.as_ref());
    let ts = resolve_observation_ts(delta);

    // Daily
    let period = Bucket::Daily.period_start_unix(ts);
    upsert_daily_global_stats(pool, &global_row(period, &core)).await?;
    upsert_daily_project_stats(pool, &project_row(period, &delta.project_id, &core)).await?;
    if let Some(branch) = &delta.stats.git_branch {
        upsert_daily_branch_stats(pool, &branch_row(period, &delta.project_id, branch, &core))
            .await?;
    }
    if let Some(model) = &delta.stats.primary_model {
        upsert_daily_model_stats(pool, &model_row(period, model, &core)).await?;
    }

    // Weekly
    let period = Bucket::Weekly.period_start_unix(ts);
    upsert_weekly_global_stats(pool, &weekly_global_row(period, &core)).await?;
    upsert_weekly_project_stats(pool, &weekly_project_row(period, &delta.project_id, &core))
        .await?;
    if let Some(branch) = &delta.stats.git_branch {
        upsert_weekly_branch_stats(
            pool,
            &weekly_branch_row(period, &delta.project_id, branch, &core),
        )
        .await?;
    }
    if let Some(model) = &delta.stats.primary_model {
        upsert_weekly_model_stats(pool, &weekly_model_row(period, model, &core)).await?;
    }

    // Monthly
    let period = Bucket::Monthly.period_start_unix(ts);
    upsert_monthly_global_stats(pool, &monthly_global_row(period, &core)).await?;
    upsert_monthly_project_stats(pool, &monthly_project_row(period, &delta.project_id, &core))
        .await?;
    if let Some(branch) = &delta.stats.git_branch {
        upsert_monthly_branch_stats(
            pool,
            &monthly_branch_row(period, &delta.project_id, branch, &core),
        )
        .await?;
    }
    if let Some(model) = &delta.stats.primary_model {
        upsert_monthly_model_stats(pool, &monthly_model_row(period, model, &core)).await?;
    }

    Ok(())
}

/// Pick the unix timestamp this observation should be attributed to.
///
/// Preference order:
///   1. `stats.last_message_at` (if parseable ISO-8601)
///   2. `stats.first_message_at` (same)
///   3. `source_mtime` (filesystem fallback)
///   4. `Utc::now()` — last-resort; rows attributed to a session with
///      no timestamps land in today's bucket.
///
/// Exposed publicly so `rebuild.rs` and future drift-healer paths can
/// share the resolution without duplicating it.
pub fn resolve_observation_ts(delta: &StatsDelta) -> i64 {
    if let Some(iso) = delta.stats.last_message_at.as_deref() {
        if let Some(t) = parse_iso8601(iso) {
            return t;
        }
    }
    if let Some(iso) = delta.stats.first_message_at.as_deref() {
        if let Some(t) = parse_iso8601(iso) {
            return t;
        }
    }
    if delta.source_mtime > 0 {
        return delta.source_mtime;
    }
    Utc::now().timestamp()
}

fn parse_iso8601(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp())
        .or_else(|| {
            // Some sessions store `YYYY-MM-DDTHH:MM:SS` without tz —
            // treat as UTC.
            chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| Utc.from_utc_datetime(&ndt).timestamp())
        })
}

// ── Row constructors — 12 near-identical builders, kept as plain
//    functions for readability. A local macro_rules! would hide the
//    dim-key shape; stay explicit. ────────────────────────────────

fn global_row(period: i64, c: &StatsCore) -> DailyGlobalStats {
    DailyGlobalStats {
        period_start: period,
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn weekly_global_row(period: i64, c: &StatsCore) -> WeeklyGlobalStats {
    WeeklyGlobalStats {
        period_start: period,
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn monthly_global_row(period: i64, c: &StatsCore) -> MonthlyGlobalStats {
    MonthlyGlobalStats {
        period_start: period,
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn project_row(period: i64, project_id: &str, c: &StatsCore) -> DailyProjectStats {
    DailyProjectStats {
        period_start: period,
        project_id: project_id.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn weekly_project_row(period: i64, project_id: &str, c: &StatsCore) -> WeeklyProjectStats {
    WeeklyProjectStats {
        period_start: period,
        project_id: project_id.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn monthly_project_row(period: i64, project_id: &str, c: &StatsCore) -> MonthlyProjectStats {
    MonthlyProjectStats {
        period_start: period,
        project_id: project_id.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn branch_row(period: i64, project_id: &str, branch: &str, c: &StatsCore) -> DailyBranchStats {
    DailyBranchStats {
        period_start: period,
        project_id: project_id.to_owned(),
        branch: branch.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn weekly_branch_row(
    period: i64,
    project_id: &str,
    branch: &str,
    c: &StatsCore,
) -> WeeklyBranchStats {
    WeeklyBranchStats {
        period_start: period,
        project_id: project_id.to_owned(),
        branch: branch.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn monthly_branch_row(
    period: i64,
    project_id: &str,
    branch: &str,
    c: &StatsCore,
) -> MonthlyBranchStats {
    MonthlyBranchStats {
        period_start: period,
        project_id: project_id.to_owned(),
        branch: branch.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn model_row(period: i64, model_id: &str, c: &StatsCore) -> DailyModelStats {
    DailyModelStats {
        period_start: period,
        model_id: model_id.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn weekly_model_row(period: i64, model_id: &str, c: &StatsCore) -> WeeklyModelStats {
    WeeklyModelStats {
        period_start: period,
        model_id: model_id.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

fn monthly_model_row(period: i64, model_id: &str, c: &StatsCore) -> MonthlyModelStats {
    MonthlyModelStats {
        period_start: period,
        model_id: model_id.to_owned(),
        session_count: c.session_count,
        total_tokens: c.total_tokens,
        total_cost_cents: c.total_cost_cents,
        prompt_count: c.prompt_count,
        file_count: c.file_count,
        lines_added: c.lines_added,
        lines_removed: c.lines_removed,
        commit_count: c.commit_count,
        commit_insertions: c.commit_insertions,
        commit_deletions: c.commit_deletions,
        duration_sum_ms: c.duration_sum_ms,
        duration_count: c.duration_count,
        reedit_rate_sum: c.reedit_rate_sum,
        reedit_rate_count: c.reedit_rate_count,
    }
}

// Re-export the `stats_core` module under this name so tests can access
// the select_range_* helpers via a short path.
#[allow(unused_imports)]
use claude_view_stats_rollup::stats_core as _stats_core;

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_core::session_stats::SessionStats;
    use claude_view_stats_rollup::stats_core::select_range_daily_global_stats;

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for sql in claude_view_stats_rollup::stats_core::migrations::STATEMENTS {
            sqlx::raw_sql(sql).execute(&pool).await.unwrap();
        }
        pool
    }

    fn sample_stats() -> SessionStats {
        SessionStats {
            total_input_tokens: 100,
            total_output_tokens: 50,
            cache_read_tokens: 10,
            cache_creation_tokens: 5,
            user_prompt_count: 3,
            duration_seconds: 120,
            first_message_at: Some("2026-04-19T10:00:00Z".to_string()),
            last_message_at: Some("2026-04-19T14:30:00Z".to_string()),
            primary_model: Some("claude-opus-4-7".to_string()),
            git_branch: Some("main".to_string()),
            ..Default::default()
        }
    }

    fn sample_delta(project: &str) -> StatsDelta {
        StatsDelta {
            session_id: "test-session".to_string(),
            source_content_hash: vec![1, 2, 3],
            source_size: 1024,
            source_inode: None,
            source_mid_hash: None,
            project_id: project.to_string(),
            source_file_path: format!("/Users/test/.claude/projects/{project}/test-session.jsonl"),
            is_compressed: false,
            source_mtime: 1_776_055_800,
            stats: sample_stats(),
            old: None,
            seq: 0,
            source: crate::indexer_v2::DeltaSource::Indexer,
        }
    }

    #[tokio::test]
    async fn apply_stats_delta_populates_daily_global() {
        let pool = setup_test_pool().await;
        let delta = sample_delta("-Users-test-proj");
        apply_stats_delta(&pool, &delta).await.unwrap();

        // Daily global row should exist for 2026-04-19 midnight UTC.
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
        assert_eq!(rows.len(), 1, "exactly one row for the bucket");
        let row = &rows[0];
        assert_eq!(row.session_count, 1);
        assert_eq!(row.total_tokens, 165); // 100+50+10+5
        assert_eq!(row.prompt_count, 3);
        assert_eq!(row.duration_sum_ms, 120_000);
        assert_eq!(row.duration_count, 1);
    }

    #[tokio::test]
    async fn second_observation_of_same_session_adds_delta_not_double() {
        let pool = setup_test_pool().await;
        let mut first = sample_delta("-Users-test-proj");
        apply_stats_delta(&pool, &first).await.unwrap();

        // Second observation: 50 more input tokens, 1 more prompt.
        let new_stats = SessionStats {
            total_input_tokens: 150,
            user_prompt_count: 4,
            ..first.stats.clone()
        };
        first.old = Some(first.stats.clone());
        first.stats = new_stats;
        apply_stats_delta(&pool, &first).await.unwrap();

        // Use midnight directly to avoid tz drift.
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
        let row = &rows[0];
        assert_eq!(row.session_count, 1, "same session, count stays 1");
        // total_tokens delta: (150-100) more input = +50.
        // Daily row should now hold 165 + 50 = 215.
        assert_eq!(row.total_tokens, 215);
        assert_eq!(row.prompt_count, 4);
    }

    #[tokio::test]
    async fn observation_with_no_branch_skips_branch_dim() {
        let pool = setup_test_pool().await;
        let mut delta = sample_delta("-Users-test-proj");
        delta.stats.git_branch = None;
        apply_stats_delta(&pool, &delta).await.unwrap();

        let (daily_branch_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM daily_branch_stats")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(daily_branch_count, 0, "no branch → no branch rollup row");

        // Global and project rows still exist.
        let (daily_global_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM daily_global_stats")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(daily_global_count, 1);
    }
}
