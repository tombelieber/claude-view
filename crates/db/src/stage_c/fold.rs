//! Phase 4 PR 4.8 — contribution_snapshots fold into rollup tables.
//!
//! `contribution_snapshots` is a pre-aggregated daily table populated
//! by the existing snapshot pipeline (nightly job + live contribution
//! tracker). Each row captures `(date, project_id, branch)` totals for:
//!
//! - `sessions_count`, `tokens_used`, `cost_cents` — **owned by Stage C**
//!   (session_stats → rollup path). Not folded.
//! - `ai_lines_added`, `ai_lines_removed` — **owned by Phase 5 flag
//!   fold** long-term. Folded here to unblock PR 4.5/4.6 until Phase 5
//!   lands.
//! - `commits_count`, `commit_insertions`, `commit_deletions` —
//!   **owned by Phase 5 flag fold** long-term. Folded here for the same
//!   reason.
//!
//! ## Why a partial fold
//!
//! If this code folded `sessions_count` / `tokens_used` it would
//! double-count against Stage C's `session_stats` replay, because every
//! session a snapshot row summarises also exists in `session_stats`
//! and drives a `session_count += 1` + `total_tokens += N` through
//! the consumer. Partial fold keeps the arithmetic honest: Stage C
//! owns `{session_count, total_tokens, total_cost_cents, prompt_count,
//! duration_*}`; fold owns `{lines_added, lines_removed, commit_count,
//! commit_insertions, commit_deletions}`. The two writer paths are
//! disjoint on field set and therefore commute under the pointwise-sum
//! UPSERT.
//!
//! ## Idempotency
//!
//! The fold is safe to run repeatedly only because
//! `full_rebuild_from_session_stats` truncates every rollup table
//! first. Running fold twice without a truncate would double the
//! line/commit counts. `stage_c/mod.rs` exposes
//! `full_rebuild_with_snapshots` as the composed entry point —
//! callers should not mix a raw
//! `fold_contribution_snapshots_into_rollups` into a non-truncated
//! rollup state.
//!
//! ## Bucket fan-out
//!
//! Each snapshot row lands in three daily × two project-level
//! dimensions = 6 UPSERTs (daily/weekly/monthly × project/branch).
//! Branch dim is conditional on `branch IS NOT NULL`. Global dim is
//! skipped (Stage C owns it; snapshots' global row is a Stage C superset
//! anyway). Category dim is out of scope here (Phase 5 SessionFlags).
//!
//! ## Related
//!
//! - Design: `2026-04-17-cqrs-phase-1-7-design.md` §6.2 PR 4.8
//! - SOTA: `2026-04-17-cqrs-stats-redesign-sota.md` §10 Phase 4c + D6

use chrono::NaiveDate;
use claude_view_stats_rollup::stats_core::{
    upsert_daily_branch_stats, upsert_daily_project_stats, upsert_monthly_branch_stats,
    upsert_monthly_project_stats, upsert_weekly_branch_stats, upsert_weekly_project_stats,
};
use claude_view_stats_rollup::{Bucket, StatsCore};
use sqlx::{Row, SqlitePool};

use crate::stage_c::consumer::StageCError;

/// Summary of a `fold_contribution_snapshots_into_rollups` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FoldSummary {
    /// Rows read from `contribution_snapshots`.
    pub rows_observed: u64,
    /// Rows that contributed to rollup tables. Lower than
    /// `rows_observed` when a row was skipped (missing `project_id` or
    /// unparseable `date`).
    pub rows_applied: u64,
    /// Rows skipped because `project_id` was NULL — no project-level
    /// rollup target.
    pub rows_skipped_no_project: u64,
    /// Rows skipped because the `date` field did not parse as
    /// `YYYY-MM-DD`.
    pub rows_skipped_bad_date: u64,
}

/// Fold every `contribution_snapshots` row into the rollup tables.
///
/// Populates only the fields Stage C's `apply_stats_delta` leaves at 0:
/// `lines_added`, `lines_removed`, `commit_count`, `commit_insertions`,
/// `commit_deletions`. All other rollup fields are left to the Stage C
/// path — this function writes ZERO to them so the UPSERT's
/// pointwise-sum leaves them unchanged.
///
/// Callers must ensure rollup tables are in a known state (truncated
/// then re-populated by Stage C, OR empty) before calling. The
/// composed entry point `full_rebuild_with_snapshots` handles that.
pub async fn fold_contribution_snapshots_into_rollups(
    pool: &SqlitePool,
) -> Result<FoldSummary, StageCError> {
    let rows = sqlx::query(
        r"SELECT date, project_id, branch,
                 COALESCE(ai_lines_added,   0) AS ai_lines_added,
                 COALESCE(ai_lines_removed, 0) AS ai_lines_removed,
                 COALESCE(commits_count,    0) AS commits_count,
                 COALESCE(commit_insertions,0) AS commit_insertions,
                 COALESCE(commit_deletions, 0) AS commit_deletions
         FROM contribution_snapshots",
    )
    .fetch_all(pool)
    .await?;

    let mut summary = FoldSummary::default();
    for row in &rows {
        summary.rows_observed += 1;

        let project_id: Option<String> = row.try_get("project_id").ok().flatten();
        let Some(project_id) = project_id else {
            summary.rows_skipped_no_project += 1;
            continue;
        };

        let date_str: String = row.try_get("date")?;
        let Some(day_unix) = parse_day_midnight_unix(&date_str) else {
            summary.rows_skipped_bad_date += 1;
            continue;
        };

        let core = snapshot_to_stats_core(row)?;

        let branch: Option<String> = row.try_get("branch").ok().flatten();

        for bucket in [Bucket::Daily, Bucket::Weekly, Bucket::Monthly] {
            let period = bucket.period_start_unix(day_unix);
            apply_project(pool, bucket, period, &project_id, &core).await?;
            if let Some(branch) = &branch {
                apply_branch(pool, bucket, period, &project_id, branch, &core).await?;
            }
        }
        summary.rows_applied += 1;
    }
    Ok(summary)
}

/// Build a partial `StatsCore` carrying only the fold-owned fields.
/// All Stage-C-owned fields stay zero so pointwise-sum leaves the
/// existing rollup value intact.
fn snapshot_to_stats_core(row: &sqlx::sqlite::SqliteRow) -> Result<StatsCore, StageCError> {
    let ai_add: i64 = row.try_get("ai_lines_added")?;
    let ai_rem: i64 = row.try_get("ai_lines_removed")?;
    let commits: i64 = row.try_get("commits_count")?;
    let ins: i64 = row.try_get("commit_insertions")?;
    let dels: i64 = row.try_get("commit_deletions")?;
    Ok(StatsCore {
        session_count: 0,
        total_tokens: 0,
        total_cost_cents: 0,
        prompt_count: 0,
        file_count: 0,
        lines_added: ai_add.max(0) as u64,
        lines_removed: ai_rem.max(0) as u64,
        commit_count: commits.max(0) as u64,
        commit_insertions: ins.max(0) as u64,
        commit_deletions: dels.max(0) as u64,
        duration_sum_ms: 0,
        duration_count: 0,
        reedit_rate_sum: 0.0,
        reedit_rate_count: 0,
    })
}

async fn apply_project(
    pool: &SqlitePool,
    bucket: Bucket,
    period: i64,
    project_id: &str,
    core: &StatsCore,
) -> Result<(), StageCError> {
    match bucket {
        Bucket::Daily => {
            upsert_daily_project_stats(
                pool,
                &claude_view_stats_rollup::stats_core::DailyProjectStats {
                    period_start: period,
                    project_id: project_id.to_owned(),
                    session_count: core.session_count,
                    total_tokens: core.total_tokens,
                    total_cost_cents: core.total_cost_cents,
                    prompt_count: core.prompt_count,
                    file_count: core.file_count,
                    lines_added: core.lines_added,
                    lines_removed: core.lines_removed,
                    commit_count: core.commit_count,
                    commit_insertions: core.commit_insertions,
                    commit_deletions: core.commit_deletions,
                    duration_sum_ms: core.duration_sum_ms,
                    duration_count: core.duration_count,
                    reedit_rate_sum: core.reedit_rate_sum,
                    reedit_rate_count: core.reedit_rate_count,
                },
            )
            .await
        }
        Bucket::Weekly => {
            upsert_weekly_project_stats(
                pool,
                &claude_view_stats_rollup::stats_core::WeeklyProjectStats {
                    period_start: period,
                    project_id: project_id.to_owned(),
                    session_count: core.session_count,
                    total_tokens: core.total_tokens,
                    total_cost_cents: core.total_cost_cents,
                    prompt_count: core.prompt_count,
                    file_count: core.file_count,
                    lines_added: core.lines_added,
                    lines_removed: core.lines_removed,
                    commit_count: core.commit_count,
                    commit_insertions: core.commit_insertions,
                    commit_deletions: core.commit_deletions,
                    duration_sum_ms: core.duration_sum_ms,
                    duration_count: core.duration_count,
                    reedit_rate_sum: core.reedit_rate_sum,
                    reedit_rate_count: core.reedit_rate_count,
                },
            )
            .await
        }
        Bucket::Monthly => {
            upsert_monthly_project_stats(
                pool,
                &claude_view_stats_rollup::stats_core::MonthlyProjectStats {
                    period_start: period,
                    project_id: project_id.to_owned(),
                    session_count: core.session_count,
                    total_tokens: core.total_tokens,
                    total_cost_cents: core.total_cost_cents,
                    prompt_count: core.prompt_count,
                    file_count: core.file_count,
                    lines_added: core.lines_added,
                    lines_removed: core.lines_removed,
                    commit_count: core.commit_count,
                    commit_insertions: core.commit_insertions,
                    commit_deletions: core.commit_deletions,
                    duration_sum_ms: core.duration_sum_ms,
                    duration_count: core.duration_count,
                    reedit_rate_sum: core.reedit_rate_sum,
                    reedit_rate_count: core.reedit_rate_count,
                },
            )
            .await
        }
    }?;
    Ok(())
}

async fn apply_branch(
    pool: &SqlitePool,
    bucket: Bucket,
    period: i64,
    project_id: &str,
    branch: &str,
    core: &StatsCore,
) -> Result<(), StageCError> {
    match bucket {
        Bucket::Daily => {
            upsert_daily_branch_stats(
                pool,
                &claude_view_stats_rollup::stats_core::DailyBranchStats {
                    period_start: period,
                    project_id: project_id.to_owned(),
                    branch: branch.to_owned(),
                    session_count: core.session_count,
                    total_tokens: core.total_tokens,
                    total_cost_cents: core.total_cost_cents,
                    prompt_count: core.prompt_count,
                    file_count: core.file_count,
                    lines_added: core.lines_added,
                    lines_removed: core.lines_removed,
                    commit_count: core.commit_count,
                    commit_insertions: core.commit_insertions,
                    commit_deletions: core.commit_deletions,
                    duration_sum_ms: core.duration_sum_ms,
                    duration_count: core.duration_count,
                    reedit_rate_sum: core.reedit_rate_sum,
                    reedit_rate_count: core.reedit_rate_count,
                },
            )
            .await
        }
        Bucket::Weekly => {
            upsert_weekly_branch_stats(
                pool,
                &claude_view_stats_rollup::stats_core::WeeklyBranchStats {
                    period_start: period,
                    project_id: project_id.to_owned(),
                    branch: branch.to_owned(),
                    session_count: core.session_count,
                    total_tokens: core.total_tokens,
                    total_cost_cents: core.total_cost_cents,
                    prompt_count: core.prompt_count,
                    file_count: core.file_count,
                    lines_added: core.lines_added,
                    lines_removed: core.lines_removed,
                    commit_count: core.commit_count,
                    commit_insertions: core.commit_insertions,
                    commit_deletions: core.commit_deletions,
                    duration_sum_ms: core.duration_sum_ms,
                    duration_count: core.duration_count,
                    reedit_rate_sum: core.reedit_rate_sum,
                    reedit_rate_count: core.reedit_rate_count,
                },
            )
            .await
        }
        Bucket::Monthly => {
            upsert_monthly_branch_stats(
                pool,
                &claude_view_stats_rollup::stats_core::MonthlyBranchStats {
                    period_start: period,
                    project_id: project_id.to_owned(),
                    branch: branch.to_owned(),
                    session_count: core.session_count,
                    total_tokens: core.total_tokens,
                    total_cost_cents: core.total_cost_cents,
                    prompt_count: core.prompt_count,
                    file_count: core.file_count,
                    lines_added: core.lines_added,
                    lines_removed: core.lines_removed,
                    commit_count: core.commit_count,
                    commit_insertions: core.commit_insertions,
                    commit_deletions: core.commit_deletions,
                    duration_sum_ms: core.duration_sum_ms,
                    duration_count: core.duration_count,
                    reedit_rate_sum: core.reedit_rate_sum,
                    reedit_rate_count: core.reedit_rate_count,
                },
            )
            .await
        }
    }?;
    Ok(())
}

/// Parse a `YYYY-MM-DD` string to the unix timestamp of that day's
/// UTC midnight. Returns `None` for unparseable input (incl. empty).
fn parse_day_midnight_unix(date_str: &str) -> Option<i64> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| ndt.and_utc().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_stats_rollup::stats_core;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        // Minimal contribution_snapshots schema. Matches
        // `Database::ensure_schema_columns`.
        sqlx::raw_sql(
            r"CREATE TABLE contribution_snapshots (
                id INTEGER PRIMARY KEY,
                date TEXT NOT NULL,
                project_id TEXT,
                branch TEXT,
                sessions_count INTEGER DEFAULT 0,
                ai_lines_added INTEGER DEFAULT 0,
                ai_lines_removed INTEGER DEFAULT 0,
                commits_count INTEGER DEFAULT 0,
                commit_insertions INTEGER DEFAULT 0,
                commit_deletions INTEGER DEFAULT 0,
                tokens_used INTEGER DEFAULT 0,
                cost_cents INTEGER DEFAULT 0,
                UNIQUE(date, project_id, branch)
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

    async fn seed_snapshot(
        pool: &SqlitePool,
        date: &str,
        project_id: Option<&str>,
        branch: Option<&str>,
        ai_add: i64,
        ai_rem: i64,
        commits: i64,
        ins: i64,
        dels: i64,
    ) {
        sqlx::query(
            r"INSERT INTO contribution_snapshots (
                date, project_id, branch,
                ai_lines_added, ai_lines_removed,
                commits_count, commit_insertions, commit_deletions
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(date)
        .bind(project_id)
        .bind(branch)
        .bind(ai_add)
        .bind(ai_rem)
        .bind(commits)
        .bind(ins)
        .bind(dels)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn daily_project_lines(pool: &SqlitePool, project_id: &str) -> (i64, i64) {
        let (added, removed): (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(lines_added),0), COALESCE(SUM(lines_removed),0)
             FROM daily_project_stats WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
        (added, removed)
    }

    #[tokio::test]
    async fn empty_snapshots_fold_is_no_op() {
        let pool = setup_pool().await;
        let summary = fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();
        assert_eq!(summary.rows_observed, 0);
        assert_eq!(summary.rows_applied, 0);
    }

    #[tokio::test]
    async fn fold_populates_line_and_commit_fields_on_project_rollup() {
        let pool = setup_pool().await;
        seed_snapshot(
            &pool,
            "2026-04-19",
            Some("p-a"),
            Some("main"),
            100,
            20,
            3,
            150,
            30,
        )
        .await;

        let summary = fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();
        assert_eq!(summary.rows_observed, 1);
        assert_eq!(summary.rows_applied, 1);

        let (added, removed) = daily_project_lines(&pool, "p-a").await;
        assert_eq!(added, 100);
        assert_eq!(removed, 20);

        // Branch rollup also populated.
        let (b_added, b_removed, commits, ins, dels): (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, commit_count, commit_insertions, commit_deletions
             FROM daily_branch_stats WHERE project_id='p-a' AND branch='main'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (b_added, b_removed, commits, ins, dels),
            (100, 20, 3, 150, 30)
        );
    }

    #[tokio::test]
    async fn fold_skips_rows_with_null_project_id() {
        let pool = setup_pool().await;
        seed_snapshot(&pool, "2026-04-19", None, None, 100, 20, 1, 10, 5).await;
        let summary = fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();
        assert_eq!(summary.rows_observed, 1);
        assert_eq!(summary.rows_applied, 0);
        assert_eq!(summary.rows_skipped_no_project, 1);
    }

    #[tokio::test]
    async fn fold_skips_rows_with_malformed_date() {
        let pool = setup_pool().await;
        seed_snapshot(&pool, "not-a-date", Some("p-a"), None, 10, 5, 0, 0, 0).await;
        let summary = fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();
        assert_eq!(summary.rows_observed, 1);
        assert_eq!(summary.rows_applied, 0);
        assert_eq!(summary.rows_skipped_bad_date, 1);
    }

    #[tokio::test]
    async fn fold_accumulates_across_multiple_days_in_same_week_and_month() {
        let pool = setup_pool().await;
        // Three snapshots in the same week (2026-04-20 is a Monday;
        // 2026-04-21, 2026-04-22 fall in the same week bucket).
        seed_snapshot(&pool, "2026-04-20", Some("p-week"), None, 10, 0, 1, 50, 0).await;
        seed_snapshot(&pool, "2026-04-21", Some("p-week"), None, 20, 0, 2, 30, 0).await;
        seed_snapshot(&pool, "2026-04-22", Some("p-week"), None, 30, 0, 0, 0, 0).await;

        fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();

        // Weekly project rollup: sum across the three days.
        let (week_added, week_commits): (i64, i64) = sqlx::query_as(
            "SELECT lines_added, commit_count FROM weekly_project_stats
             WHERE project_id='p-week'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(week_added, 60);
        assert_eq!(week_commits, 3);

        // Monthly project rollup: also summed.
        let (month_added,): (i64,) = sqlx::query_as(
            "SELECT lines_added FROM monthly_project_stats WHERE project_id='p-week'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(month_added, 60);
    }

    #[tokio::test]
    async fn fold_skips_branch_fanout_when_branch_is_null() {
        let pool = setup_pool().await;
        seed_snapshot(
            &pool,
            "2026-04-19",
            Some("p-nobranch"),
            None,
            100,
            0,
            0,
            0,
            0,
        )
        .await;
        fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();

        // Project row created
        let (added,): (i64,) = sqlx::query_as(
            "SELECT lines_added FROM daily_project_stats WHERE project_id='p-nobranch'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(added, 100);

        // Branch row not created
        let (cnt,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM daily_branch_stats WHERE project_id='p-nobranch'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cnt, 0);
    }

    #[tokio::test]
    async fn fold_leaves_stage_c_owned_fields_at_zero() {
        let pool = setup_pool().await;
        seed_snapshot(&pool, "2026-04-19", Some("p-a"), None, 10, 5, 1, 10, 5).await;
        fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();

        // Stage-C-owned fields must all be zero after a pure fold.
        let (sessions, tokens, prompts, cost): (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT session_count, total_tokens, prompt_count, total_cost_cents
             FROM daily_project_stats WHERE project_id='p-a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (sessions, tokens, prompts, cost),
            (0, 0, 0, 0),
            "fold must not write to Stage-C-owned fields"
        );
    }

    #[tokio::test]
    async fn fold_composes_additively_over_stage_c_written_rows() {
        // Simulate Stage C having already written session_count +
        // tokens for a row; fold should then add only lines / commits
        // without disturbing Stage C's values.
        let pool = setup_pool().await;

        // Seed a Stage-C-shaped row: 2 sessions, 500 tokens, zero lines.
        let apr19_mid = NaiveDate::from_ymd_opt(2026, 4, 19)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        stats_core::upsert_daily_project_stats(
            &pool,
            &stats_core::DailyProjectStats {
                period_start: apr19_mid,
                project_id: "p-compose".into(),
                session_count: 2,
                total_tokens: 500,
                total_cost_cents: 0,
                prompt_count: 5,
                file_count: 0,
                lines_added: 0,
                lines_removed: 0,
                commit_count: 0,
                commit_insertions: 0,
                commit_deletions: 0,
                duration_sum_ms: 12_000,
                duration_count: 2,
                reedit_rate_sum: 0.0,
                reedit_rate_count: 0,
            },
        )
        .await
        .unwrap();

        seed_snapshot(
            &pool,
            "2026-04-19",
            Some("p-compose"),
            None,
            100,
            20,
            3,
            150,
            30,
        )
        .await;
        fold_contribution_snapshots_into_rollups(&pool)
            .await
            .unwrap();

        let (sessions, tokens, prompts, dur_ms, dur_n, lines_add, lines_rem, commits, ins, dels): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = sqlx::query_as(
            "SELECT session_count, total_tokens, prompt_count,
                    duration_sum_ms, duration_count,
                    lines_added, lines_removed,
                    commit_count, commit_insertions, commit_deletions
             FROM daily_project_stats WHERE project_id='p-compose'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        // Stage-C-owned fields preserved
        assert_eq!(
            sessions, 2,
            "fold must not clobber Stage-C-owned session_count"
        );
        assert_eq!(tokens, 500);
        assert_eq!(prompts, 5);
        assert_eq!(dur_ms, 12_000);
        assert_eq!(dur_n, 2);

        // Fold-owned fields populated
        assert_eq!(lines_add, 100);
        assert_eq!(lines_rem, 20);
        assert_eq!(commits, 3);
        assert_eq!(ins, 150);
        assert_eq!(dels, 30);
    }
}
