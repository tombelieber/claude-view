// crates/db/src/trends.rs
//! Trend calculations and index metadata management.
//!
//! This module implements:
//! - Week period bounds (current week, previous week)
//! - TrendMetric calculation with delta and delta_percent
//! - Week-over-week trend aggregations
//! - index_metadata CRUD operations

use crate::{Database, DbResult};
use chrono::{Datelike, Utc};
use serde::Serialize;
use ts_rs::TS;

// ============================================================================
// Time Period Functions
// ============================================================================

/// Get the bounds for the current week (Monday 00:00 UTC to now).
///
/// Returns `(start_timestamp, end_timestamp)` as Unix seconds.
pub fn current_week_bounds() -> (i64, i64) {
    let now = Utc::now();
    let days_since_monday = now.weekday().num_days_from_monday() as i64;
    let monday = now - chrono::Duration::days(days_since_monday);
    let start = monday
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end = now.timestamp();
    (start, end)
}

/// Get the bounds for the previous week (Monday 00:00 to Sunday 23:59:59 UTC).
///
/// Returns `(start_timestamp, end_timestamp)` as Unix seconds.
pub fn previous_week_bounds() -> (i64, i64) {
    let now = Utc::now();
    let days_since_monday = now.weekday().num_days_from_monday() as i64;
    let this_monday = now - chrono::Duration::days(days_since_monday);
    let prev_monday = this_monday - chrono::Duration::days(7);
    // Previous week ends at Sunday 23:59:59, which is this Monday 00:00:00 - 1 second
    let start = prev_monday
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end = this_monday
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
        - 1;
    (start, end)
}

// ============================================================================
// Trend Metric Types
// ============================================================================

/// A single trend metric comparing current vs previous period.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TrendMetric {
    /// Current period value.
    #[ts(type = "number")]
    pub current: i64,
    /// Previous period value.
    #[ts(type = "number")]
    pub previous: i64,
    /// Absolute change (current - previous).
    #[ts(type = "number")]
    pub delta: i64,
    /// Percentage change, rounded to 1 decimal place.
    /// None if previous == 0 (cannot calculate percentage).
    pub delta_percent: Option<f64>,
}

impl TrendMetric {
    /// Create a new TrendMetric from current and previous values.
    ///
    /// Calculates delta and delta_percent automatically.
    /// delta_percent is None if previous is 0.
    pub fn new(current: i64, previous: i64) -> Self {
        let delta = current - previous;
        let delta_percent = if previous == 0 {
            None
        } else {
            // Round to 1 decimal place
            let percent = (delta as f64 / previous as f64) * 100.0;
            Some((percent * 10.0).round() / 10.0)
        };
        Self {
            current,
            previous,
            delta,
            delta_percent,
        }
    }
}

/// Collection of all week-over-week trend metrics.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct WeekTrends {
    /// Session count trend.
    pub session_count: TrendMetric,
    /// Total tokens (input + output) trend.
    pub total_tokens: TrendMetric,
    /// Average tokens per prompt (weighted average).
    /// None if no prompts in either period.
    pub avg_tokens_per_prompt: TrendMetric,
    /// Total files edited trend.
    pub total_files_edited: TrendMetric,
    /// Average re-edit rate (weighted average) * 100 for percentage display.
    /// None if no files edited in either period.
    pub avg_reedit_rate: TrendMetric,
    /// Commit link count trend.
    pub commit_link_count: TrendMetric,
}

/// Index metadata for data freshness tracking.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct IndexMetadata {
    /// Unix timestamp of last successful index completion.
    #[ts(type = "number | null")]
    pub last_indexed_at: Option<i64>,
    /// Duration of last successful index in milliseconds.
    #[ts(type = "number | null")]
    pub last_index_duration_ms: Option<i64>,
    /// Number of sessions indexed in last run.
    #[ts(type = "number")]
    pub sessions_indexed: i64,
    /// Number of projects indexed in last run.
    #[ts(type = "number")]
    pub projects_indexed: i64,
    /// Unix timestamp of last successful git sync.
    #[ts(type = "number | null")]
    pub last_git_sync_at: Option<i64>,
    /// Number of commits found in last git sync.
    #[ts(type = "number")]
    pub commits_found: i64,
    /// Number of session-commit links created in last git sync.
    #[ts(type = "number")]
    pub links_created: i64,
    /// Unix timestamp of last metadata update.
    #[ts(type = "number")]
    pub updated_at: i64,
    /// User-configurable git sync interval in seconds (default 60).
    #[ts(type = "number")]
    pub git_sync_interval_secs: i64,
}

// ============================================================================
// Database Queries
// ============================================================================

impl Database {
    /// Get trend metrics for a custom time range.
    ///
    /// The comparison period is automatically calculated as the equivalent
    /// duration immediately preceding the requested period.
    ///
    /// For example, if `from` to `to` is 7 days, the comparison period
    /// is the 7 days before `from`.
    pub async fn get_trends_with_range(&self, from: i64, to: i64) -> DbResult<WeekTrends> {
        let duration = to - from;
        let comp_end = from - 1;
        let comp_start = comp_end - duration;

        self.get_trends_for_periods(from, to, comp_start, comp_end).await
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

        self.get_trends_for_periods(curr_start, curr_end, prev_start, prev_end).await
    }

    /// Internal: Get trend metrics comparing two arbitrary periods.
    async fn get_trends_for_periods(
        &self,
        curr_start: i64,
        curr_end: i64,
        prev_start: i64,
        prev_end: i64,
    ) -> DbResult<WeekTrends> {

        // Session count
        let (curr_sessions,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(curr_start)
        .bind(curr_end)
        .fetch_one(self.pool())
        .await?;

        let (prev_sessions,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(prev_start)
        .bind(prev_end)
        .fetch_one(self.pool())
        .await?;

        // Total tokens (from turns table, joined with sessions for period filter)
        let (curr_tokens,): (i64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(COALESCE(t.input_tokens, 0) + COALESCE(t.output_tokens, 0)), 0)
            FROM turns t
            INNER JOIN sessions s ON t.session_id = s.id
            WHERE s.is_sidechain = 0 AND s.last_message_at >= ?1 AND s.last_message_at <= ?2
            "#,
        )
        .bind(curr_start)
        .bind(curr_end)
        .fetch_one(self.pool())
        .await?;

        let (prev_tokens,): (i64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(COALESCE(t.input_tokens, 0) + COALESCE(t.output_tokens, 0)), 0)
            FROM turns t
            INNER JOIN sessions s ON t.session_id = s.id
            WHERE s.is_sidechain = 0 AND s.last_message_at >= ?1 AND s.last_message_at <= ?2
            "#,
        )
        .bind(prev_start)
        .bind(prev_end)
        .fetch_one(self.pool())
        .await?;

        // Weighted avg tokens per prompt: SUM(tokens) / SUM(user_prompt_count)
        let (curr_prompts,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(user_prompt_count), 0) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(curr_start)
        .bind(curr_end)
        .fetch_one(self.pool())
        .await?;

        let (prev_prompts,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(user_prompt_count), 0) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(prev_start)
        .bind(prev_end)
        .fetch_one(self.pool())
        .await?;

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

        // Total files edited
        let (curr_files_edited,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(files_edited_count), 0) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(curr_start)
        .bind(curr_end)
        .fetch_one(self.pool())
        .await?;

        let (prev_files_edited,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(files_edited_count), 0) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(prev_start)
        .bind(prev_end)
        .fetch_one(self.pool())
        .await?;

        // Weighted avg re-edit rate: SUM(reedited_files_count) / SUM(files_edited_count) * 100
        let (curr_reedited,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(reedited_files_count), 0) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(curr_start)
        .bind(curr_end)
        .fetch_one(self.pool())
        .await?;

        let (prev_reedited,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(reedited_files_count), 0) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
        )
        .bind(prev_start)
        .bind(prev_end)
        .fetch_one(self.pool())
        .await?;

        // Calculate re-edit rate as percentage (0-100)
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

        // Commit link count (from session_commits joined with sessions for period filter)
        let (curr_commits,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM session_commits sc
            INNER JOIN sessions s ON sc.session_id = s.id
            WHERE s.is_sidechain = 0 AND s.last_message_at >= ?1 AND s.last_message_at <= ?2
            "#,
        )
        .bind(curr_start)
        .bind(curr_end)
        .fetch_one(self.pool())
        .await?;

        let (prev_commits,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM session_commits sc
            INNER JOIN sessions s ON sc.session_id = s.id
            WHERE s.is_sidechain = 0 AND s.last_message_at >= ?1 AND s.last_message_at <= ?2
            "#,
        )
        .bind(prev_start)
        .bind(prev_end)
        .fetch_one(self.pool())
        .await?;

        Ok(WeekTrends {
            session_count: TrendMetric::new(curr_sessions, prev_sessions),
            total_tokens: TrendMetric::new(curr_tokens, prev_tokens),
            avg_tokens_per_prompt: TrendMetric::new(curr_avg_tokens, prev_avg_tokens),
            total_files_edited: TrendMetric::new(curr_files_edited, prev_files_edited),
            avg_reedit_rate: TrendMetric::new(curr_reedit_rate, prev_reedit_rate),
            commit_link_count: TrendMetric::new(curr_commits, prev_commits),
        })
    }

    /// Update index metadata after a successful index operation.
    ///
    /// Only call this when indexing completes successfully.
    /// Do NOT call on failure — preserve the last successful timestamp.
    pub async fn update_index_metadata_on_success(
        &self,
        duration_ms: i64,
        sessions_indexed: i64,
        projects_indexed: i64,
    ) -> DbResult<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE index_metadata SET
                last_indexed_at = ?1,
                last_index_duration_ms = ?2,
                sessions_indexed = ?3,
                projects_indexed = ?4,
                updated_at = ?5
            WHERE id = 1
            "#,
        )
        .bind(now)
        .bind(duration_ms)
        .bind(sessions_indexed)
        .bind(projects_indexed)
        .bind(now)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Update git sync metadata after a successful git sync operation.
    ///
    /// Only call this when git sync completes successfully.
    /// Do NOT call on failure — preserve the last successful timestamp.
    pub async fn update_git_sync_metadata_on_success(
        &self,
        commits_found: i64,
        links_created: i64,
    ) -> DbResult<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE index_metadata SET
                last_git_sync_at = ?1,
                commits_found = ?2,
                links_created = ?3,
                updated_at = ?4
            WHERE id = 1
            "#,
        )
        .bind(now)
        .bind(commits_found)
        .bind(links_created)
        .bind(now)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Get the current index metadata.
    pub async fn get_index_metadata(&self) -> DbResult<IndexMetadata> {
        let row: (
            Option<i64>,
            Option<i64>,
            i64,
            i64,
            Option<i64>,
            i64,
            i64,
            i64,
            i64,
        ) = sqlx::query_as(
            r#"
            SELECT
                last_indexed_at,
                last_index_duration_ms,
                sessions_indexed,
                projects_indexed,
                last_git_sync_at,
                commits_found,
                links_created,
                updated_at,
                git_sync_interval_secs
            FROM index_metadata
            WHERE id = 1
            "#,
        )
        .fetch_one(self.pool())
        .await?;

        Ok(IndexMetadata {
            last_indexed_at: row.0,
            last_index_duration_ms: row.1,
            sessions_indexed: row.2,
            projects_indexed: row.3,
            last_git_sync_at: row.4,
            commits_found: row.5,
            links_created: row.6,
            updated_at: row.7,
            git_sync_interval_secs: row.8,
        })
    }

    /// Get the git sync interval in seconds.
    pub async fn get_git_sync_interval(&self) -> DbResult<u64> {
        let (interval,): (i64,) = sqlx::query_as(
            "SELECT git_sync_interval_secs FROM index_metadata WHERE id = 1",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(interval as u64)
    }

    /// Set the git sync interval in seconds.
    pub async fn set_git_sync_interval(&self, seconds: u64) -> DbResult<()> {
        sqlx::query(
            "UPDATE index_metadata SET git_sync_interval_secs = ?1 WHERE id = 1",
        )
        .bind(seconds as i64)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use chrono::{TimeZone, Timelike, Weekday};

    // ========================================================================
    // TrendMetric unit tests (A4.3 acceptance tests)
    // ========================================================================

    #[test]
    fn test_trend_metric_positive_delta() {
        // 120 vs 100 → delta 20, percent 20.0
        let metric = TrendMetric::new(120, 100);
        assert_eq!(metric.current, 120);
        assert_eq!(metric.previous, 100);
        assert_eq!(metric.delta, 20);
        assert_eq!(metric.delta_percent, Some(20.0));
    }

    #[test]
    fn test_trend_metric_negative_delta() {
        // 100 vs 120 → delta -20, percent -16.7 (rounded)
        let metric = TrendMetric::new(100, 120);
        assert_eq!(metric.current, 100);
        assert_eq!(metric.previous, 120);
        assert_eq!(metric.delta, -20);
        assert_eq!(metric.delta_percent, Some(-16.7));
    }

    #[test]
    fn test_trend_metric_previous_zero() {
        // 50 vs 0 → delta 50, percent None
        let metric = TrendMetric::new(50, 0);
        assert_eq!(metric.current, 50);
        assert_eq!(metric.previous, 0);
        assert_eq!(metric.delta, 50);
        assert_eq!(metric.delta_percent, None);
    }

    #[test]
    fn test_trend_metric_both_zero() {
        // 0 vs 0 → delta 0, percent None
        let metric = TrendMetric::new(0, 0);
        assert_eq!(metric.current, 0);
        assert_eq!(metric.previous, 0);
        assert_eq!(metric.delta, 0);
        assert_eq!(metric.delta_percent, None);
    }

    #[test]
    fn test_trend_metric_negative_hundred_percent() {
        // 0 vs 50 → delta -50, percent -100.0
        let metric = TrendMetric::new(0, 50);
        assert_eq!(metric.current, 0);
        assert_eq!(metric.previous, 50);
        assert_eq!(metric.delta, -50);
        assert_eq!(metric.delta_percent, Some(-100.0));
    }

    #[test]
    fn test_trend_metric_fractional_percent_rounds() {
        // 133 vs 100 → delta 33, percent 33.0 (not 33.333...)
        let metric = TrendMetric::new(133, 100);
        assert_eq!(metric.delta, 33);
        assert_eq!(metric.delta_percent, Some(33.0));

        // 125 vs 100 → delta 25, percent 25.0
        let metric = TrendMetric::new(125, 100);
        assert_eq!(metric.delta_percent, Some(25.0));

        // 115 vs 100 → delta 15, percent 15.0
        let metric = TrendMetric::new(115, 100);
        assert_eq!(metric.delta_percent, Some(15.0));
    }

    // ========================================================================
    // Time bounds tests
    // ========================================================================

    #[test]
    fn test_current_week_bounds_format() {
        let (start, end) = current_week_bounds();

        // Start should be before end
        assert!(start < end, "Start should be before end");

        // Start should be on a Monday at midnight
        let start_dt = Utc.timestamp_opt(start, 0).unwrap();
        assert_eq!(
            start_dt.weekday(),
            Weekday::Mon,
            "Start should be a Monday"
        );
        assert_eq!(start_dt.hour(), 0, "Start should be at 00:00");
        assert_eq!(start_dt.minute(), 0);
        assert_eq!(start_dt.second(), 0);

        // End should be approximately now (within 5 seconds)
        let now = Utc::now().timestamp();
        assert!(
            (end - now).abs() < 5,
            "End should be approximately now"
        );
    }

    #[test]
    fn test_previous_week_bounds_format() {
        let (start, end) = previous_week_bounds();

        // Start should be before end
        assert!(start < end, "Start should be before end");

        // Start should be on a Monday at midnight
        let start_dt = Utc.timestamp_opt(start, 0).unwrap();
        assert_eq!(
            start_dt.weekday(),
            Weekday::Mon,
            "Start should be a Monday"
        );
        assert_eq!(start_dt.hour(), 0, "Start should be at 00:00");

        // End should be on a Sunday at 23:59:59
        let end_dt = Utc.timestamp_opt(end, 0).unwrap();
        assert_eq!(end_dt.weekday(), Weekday::Sun, "End should be a Sunday");
        assert_eq!(end_dt.hour(), 23, "End should be at 23:59:59");
        assert_eq!(end_dt.minute(), 59);
        assert_eq!(end_dt.second(), 59);

        // Duration should be exactly 7 days minus 1 second
        let duration = end - start;
        assert_eq!(
            duration,
            7 * 24 * 60 * 60 - 1,
            "Duration should be 7 days minus 1 second"
        );
    }

    #[test]
    fn test_week_bounds_relationship() {
        let (curr_start, _curr_end) = current_week_bounds();
        let (_prev_start, prev_end) = previous_week_bounds();

        // Previous week should end exactly 1 second before current week starts
        assert_eq!(
            prev_end + 1,
            curr_start,
            "Previous week should end 1 second before current week starts"
        );
    }

    // ========================================================================
    // Database tests for trends
    // ========================================================================

    #[tokio::test]
    async fn test_get_week_trends_empty_db() {
        let db = Database::new_in_memory().await.unwrap();

        let trends = db.get_week_trends().await.unwrap();

        // All metrics should be 0/0
        assert_eq!(trends.session_count.current, 0);
        assert_eq!(trends.session_count.previous, 0);
        assert_eq!(trends.total_tokens.current, 0);
        assert_eq!(trends.total_files_edited.current, 0);
        assert_eq!(trends.commit_link_count.current, 0);
    }

    #[tokio::test]
    async fn test_get_week_trends_with_data() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert sessions in current week
        let (curr_start, _) = current_week_bounds();

        // Insert a session in current week
        db.insert_session_from_index(
            "sess-curr-1",
            "project-a",
            "Project A",
            "/tmp/project-a",
            "/tmp/curr1.jsonl",
            "Current week session",
            None,
            5,
            curr_start + 3600, // 1 hour into current week
            None,
            false,
            1000,
        )
        .await
        .unwrap();

        // Update with Phase 3 metrics
        db.update_session_deep_fields(
            "sess-curr-1",
            "Last message",
            3,  // turn_count
            2,  // tool_edit
            5,  // tool_read
            1,  // tool_bash
            1,  // tool_write
            "[]",
            "[]",
            10, // user_prompt_count
            8,  // api_call_count
            15, // tool_call_count
            r#"["/a.rs"]"#,
            r#"["/b.rs", "/c.rs"]"#,
            1,  // files_read_count
            2,  // files_edited_count
            0,  // reedited_files_count
            600,
            1,
            None, // first_message_at
            // Phase 3.5: Full parser metrics
            0, 0, 0, 0, // token counts
            0,           // thinking_block_count
            None, None, None, // turn durations
            0, 0, 0, 0, // error/retry/compaction/hook_blocked
            0, 0, 0, 0, // progress counts
            None,        // summary_text
            1,           // parse_version
            1000,        // file_size
            1706200000,  // file_mtime
        )
        .await
        .unwrap();

        let trends = db.get_week_trends().await.unwrap();

        // Should have 1 session in current week
        assert_eq!(trends.session_count.current, 1);
        assert_eq!(trends.session_count.previous, 0);
        assert_eq!(trends.session_count.delta, 1);
        assert_eq!(trends.session_count.delta_percent, None); // prev is 0

        // Should have files edited
        assert_eq!(trends.total_files_edited.current, 2);
    }

    // ========================================================================
    // Index metadata tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_index_metadata_default() {
        let db = Database::new_in_memory().await.unwrap();

        let metadata = db.get_index_metadata().await.unwrap();

        // Default values
        assert_eq!(metadata.last_indexed_at, None);
        assert_eq!(metadata.last_index_duration_ms, None);
        assert_eq!(metadata.sessions_indexed, 0);
        assert_eq!(metadata.projects_indexed, 0);
        assert_eq!(metadata.last_git_sync_at, None);
        assert_eq!(metadata.commits_found, 0);
        assert_eq!(metadata.links_created, 0);
        assert!(metadata.updated_at > 0);
    }

    #[tokio::test]
    async fn test_update_index_metadata_on_success() {
        let db = Database::new_in_memory().await.unwrap();

        // Update index metadata
        db.update_index_metadata_on_success(1500, 100, 5)
            .await
            .unwrap();

        let metadata = db.get_index_metadata().await.unwrap();

        assert!(metadata.last_indexed_at.is_some());
        assert_eq!(metadata.last_index_duration_ms, Some(1500));
        assert_eq!(metadata.sessions_indexed, 100);
        assert_eq!(metadata.projects_indexed, 5);
        // Git sync should still be None (not updated)
        assert_eq!(metadata.last_git_sync_at, None);
    }

    #[tokio::test]
    async fn test_update_git_sync_metadata_on_success() {
        let db = Database::new_in_memory().await.unwrap();

        // Update git sync metadata
        db.update_git_sync_metadata_on_success(250, 45)
            .await
            .unwrap();

        let metadata = db.get_index_metadata().await.unwrap();

        assert!(metadata.last_git_sync_at.is_some());
        assert_eq!(metadata.commits_found, 250);
        assert_eq!(metadata.links_created, 45);
        // Index metadata should still be None (not updated)
        assert_eq!(metadata.last_indexed_at, None);
    }

    #[tokio::test]
    async fn test_update_both_metadata() {
        let db = Database::new_in_memory().await.unwrap();

        // Update index first
        db.update_index_metadata_on_success(1200, 50, 3)
            .await
            .unwrap();

        // Then update git sync
        db.update_git_sync_metadata_on_success(100, 20)
            .await
            .unwrap();

        let metadata = db.get_index_metadata().await.unwrap();

        // Both should be set
        assert!(metadata.last_indexed_at.is_some());
        assert_eq!(metadata.last_index_duration_ms, Some(1200));
        assert_eq!(metadata.sessions_indexed, 50);
        assert_eq!(metadata.projects_indexed, 3);

        assert!(metadata.last_git_sync_at.is_some());
        assert_eq!(metadata.commits_found, 100);
        assert_eq!(metadata.links_created, 20);
    }

    #[tokio::test]
    async fn test_metadata_updates_preserve_other_fields() {
        let db = Database::new_in_memory().await.unwrap();

        // Set initial values for both
        db.update_index_metadata_on_success(1000, 30, 2)
            .await
            .unwrap();
        db.update_git_sync_metadata_on_success(80, 15)
            .await
            .unwrap();

        let first_metadata = db.get_index_metadata().await.unwrap();

        // Update only index metadata again
        db.update_index_metadata_on_success(2000, 60, 4)
            .await
            .unwrap();

        let second_metadata = db.get_index_metadata().await.unwrap();

        // Index metadata should be updated
        assert_eq!(second_metadata.last_index_duration_ms, Some(2000));
        assert_eq!(second_metadata.sessions_indexed, 60);
        assert_eq!(second_metadata.projects_indexed, 4);

        // Git sync metadata should be preserved
        assert_eq!(second_metadata.commits_found, 80);
        assert_eq!(second_metadata.links_created, 15);
        // Note: last_git_sync_at timestamp might change due to updated_at, but the data is preserved
        assert_eq!(
            second_metadata.commits_found,
            first_metadata.commits_found
        );
    }

    #[tokio::test]
    async fn test_index_metadata_serializes_correctly() {
        let db = Database::new_in_memory().await.unwrap();

        db.update_index_metadata_on_success(1500, 100, 5)
            .await
            .unwrap();

        let metadata = db.get_index_metadata().await.unwrap();
        let json = serde_json::to_string(&metadata).unwrap();

        // Should use camelCase
        assert!(json.contains("\"lastIndexedAt\""));
        assert!(json.contains("\"lastIndexDurationMs\""));
        assert!(json.contains("\"sessionsIndexed\""));
        assert!(json.contains("\"projectsIndexed\""));
        assert!(json.contains("\"lastGitSyncAt\""));
        assert!(json.contains("\"commitsFound\""));
        assert!(json.contains("\"linksCreated\""));
        assert!(json.contains("\"updatedAt\""));
    }

    #[tokio::test]
    async fn test_trend_metric_serializes_correctly() {
        let metric = TrendMetric::new(120, 100);
        let json = serde_json::to_string(&metric).unwrap();

        // Should use camelCase
        assert!(json.contains("\"current\":120"));
        assert!(json.contains("\"previous\":100"));
        assert!(json.contains("\"delta\":20"));
        assert!(json.contains("\"deltaPercent\":20.0"));
    }

    #[tokio::test]
    async fn test_trend_metric_null_delta_percent_serializes() {
        let metric = TrendMetric::new(50, 0);
        let json = serde_json::to_string(&metric).unwrap();

        // deltaPercent should be null
        assert!(json.contains("\"deltaPercent\":null"));
    }
}
