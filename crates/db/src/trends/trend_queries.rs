//! Week-over-week trend query implementations.

use super::time_periods::{current_week_bounds, previous_week_bounds};
use super::types::{TrendMetric, WeekTrends};
use crate::{Database, DbResult};

impl Database {
    /// Get trend metrics for a custom time range.
    ///
    /// The comparison period is automatically calculated as the equivalent
    /// duration immediately preceding the requested period.
    ///
    /// For example, if `from` to `to` is 7 days, the comparison period
    /// is the 7 days before `from`.
    pub async fn get_trends_with_range(
        &self,
        from: i64,
        to: i64,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<WeekTrends> {
        let duration = to - from;
        let comp_end = from - 1;
        // H5: Clamp to 0 to prevent negative timestamps for new users
        let comp_start = (comp_end - duration).max(0);

        self.get_trends_for_periods(from, to, comp_start, comp_end, project, branch)
            .await
    }

    /// Get week-over-week trend metrics.
    ///
    /// Computes trends for:
    /// - Session count
    /// - Total tokens (input + output)
    /// - Avg tokens per prompt (weighted average)
    /// - Total files edited
    /// - Avg re-edit rate (weighted average)
    /// - Commit link count
    pub async fn get_week_trends(&self) -> DbResult<WeekTrends> {
        let (curr_start, curr_end) = current_week_bounds();
        let (prev_start, prev_end) = previous_week_bounds();

        self.get_trends_for_periods(curr_start, curr_end, prev_start, prev_end, None, None)
            .await
    }

    /// Internal: Get trend metrics comparing two arbitrary periods.
    ///
    /// Consolidated from 12 sequential queries to 3 via conditional aggregation.
    /// Query A: sessions table metrics (sessions, prompts, files_edited, reedited) x 2 periods
    /// Query B: tokens from turns table x 2 periods
    /// Query C: commits from session_commits x 2 periods
    async fn get_trends_for_periods(
        &self,
        curr_start: i64,
        curr_end: i64,
        prev_start: i64,
        prev_end: i64,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<WeekTrends> {
        // Query A — session-table metrics for both periods in one scan (replaces 8 queries)
        let (
            curr_sessions, curr_prompts, curr_files_edited, curr_reedited,
            prev_sessions, prev_prompts, prev_files_edited, prev_reedited,
        ): (i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COALESCE(SUM(CASE WHEN last_message_at >= ?1 AND last_message_at <= ?2 THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN last_message_at >= ?1 AND last_message_at <= ?2 THEN user_prompt_count ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN last_message_at >= ?1 AND last_message_at <= ?2 THEN files_edited_count ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN last_message_at >= ?1 AND last_message_at <= ?2 THEN reedited_files_count ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN last_message_at >= ?3 AND last_message_at <= ?4 THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN last_message_at >= ?3 AND last_message_at <= ?4 THEN user_prompt_count ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN last_message_at >= ?3 AND last_message_at <= ?4 THEN files_edited_count ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN last_message_at >= ?3 AND last_message_at <= ?4 THEN reedited_files_count ELSE 0 END), 0)
            FROM valid_sessions
            WHERE last_message_at >= ?3 AND last_message_at <= ?2
              AND (?5 IS NULL OR project_id = ?5 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?5) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?5))
              AND (?6 IS NULL OR git_branch = ?6)
            "#,
        )
        .bind(curr_start)  // ?1
        .bind(curr_end)    // ?2
        .bind(prev_start)  // ?3
        .bind(prev_end)    // ?4
        .bind(project)     // ?5
        .bind(branch)      // ?6
        .fetch_one(self.pool())
        .await?;

        // Query B — tokens from sessions table for both periods (replaces 2 queries)
        let (curr_tokens, prev_tokens): (i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COALESCE(SUM(CASE WHEN s.last_message_at >= ?1 AND s.last_message_at <= ?2
                THEN COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0) ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN s.last_message_at >= ?3 AND s.last_message_at <= ?4
                THEN COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0) ELSE 0 END), 0)
            FROM valid_sessions s
            WHERE s.last_message_at >= ?3 AND s.last_message_at <= ?2
              AND (?5 IS NULL OR s.project_id = ?5 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?5) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?5))
              AND (?6 IS NULL OR s.git_branch = ?6)
            "#,
        )
        .bind(curr_start)
        .bind(curr_end)
        .bind(prev_start)
        .bind(prev_end)
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;

        // Query C — commits for both periods (replaces 2 queries)
        let (curr_commits, prev_commits): (i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COUNT(DISTINCT CASE WHEN s.last_message_at >= ?1 AND s.last_message_at <= ?2 THEN sc.commit_hash END),
              COUNT(DISTINCT CASE WHEN s.last_message_at >= ?3 AND s.last_message_at <= ?4 THEN sc.commit_hash END)
            FROM session_commits sc
            INNER JOIN valid_sessions s ON sc.session_id = s.id
            WHERE s.last_message_at >= ?3 AND s.last_message_at <= ?2
              AND (?5 IS NULL OR s.project_id = ?5 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?5) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?5))
              AND (?6 IS NULL OR s.git_branch = ?6)
            "#,
        )
        .bind(curr_start)
        .bind(curr_end)
        .bind(prev_start)
        .bind(prev_end)
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;

        // Derived metrics
        let curr_avg_tokens = if curr_prompts > 0 {
            curr_tokens / curr_prompts
        } else {
            0
        };
        let prev_avg_tokens = if prev_prompts > 0 {
            prev_tokens / prev_prompts
        } else {
            0
        };

        let curr_reedit_rate = if curr_files_edited > 0 {
            ((curr_reedited as f64 / curr_files_edited as f64) * 100.0).round() as i64
        } else {
            0
        };
        let prev_reedit_rate = if prev_files_edited > 0 {
            ((prev_reedited as f64 / prev_files_edited as f64) * 100.0).round() as i64
        } else {
            0
        };

        Ok(WeekTrends {
            session_count: TrendMetric::new(curr_sessions, prev_sessions),
            total_tokens: TrendMetric::new(curr_tokens, prev_tokens),
            avg_tokens_per_prompt: TrendMetric::new(curr_avg_tokens, prev_avg_tokens),
            total_files_edited: TrendMetric::new(curr_files_edited, prev_files_edited),
            avg_reedit_rate: TrendMetric::new(curr_reedit_rate, prev_reedit_rate),
            commit_link_count: TrendMetric::new(curr_commits, prev_commits),
        })
    }
}
