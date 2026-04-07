// crates/db/src/snapshots/generation.rs
//! Snapshot generation (daily job) and weekly rollup.

use super::helpers::usd_opt_to_cents;
use super::types::SnapshotStats;
use crate::{Database, DbResult};
use chrono::Local;

impl Database {
    /// Generate a daily snapshot for a specific date.
    ///
    /// This aggregates session data and commit data for the given date
    /// and upserts it into the contribution_snapshots table.
    pub async fn generate_daily_snapshot(&self, date: &str) -> DbResult<()> {
        // Get session aggregates for the date (global)
        let session_agg: (i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as sessions_count,
                COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                COALESCE(SUM(files_edited_count), 0) as files_edited_count,
                SUM(total_cost_usd) as total_cost_usd
            FROM valid_sessions
            WHERE date(last_message_at, 'unixepoch', 'localtime') = ?1
            "#,
        )
        .bind(date)
        .fetch_one(self.pool())
        .await?;

        // Get commit aggregates for the date (from commits linked to sessions on that date)
        let commit_agg: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(DISTINCT c.hash) as commits_count,
                COALESCE(SUM(c.insertions), 0) as commit_insertions,
                COALESCE(SUM(c.deletions), 0) as commit_deletions
            FROM session_commits sc
            JOIN commits c ON sc.commit_hash = c.hash
            JOIN valid_sessions s ON sc.session_id = s.id
            WHERE date(s.last_message_at, 'unixepoch', 'localtime') = ?1
            "#,
        )
        .bind(date)
        .fetch_one(self.pool())
        .await?;

        let cost_cents = usd_opt_to_cents(session_agg.5, session_agg.0);

        // Upsert global snapshot (project_id = NULL, branch = NULL)
        self.upsert_snapshot(
            date,
            None, // global
            None, // all branches
            session_agg.0,
            session_agg.1,
            session_agg.2,
            commit_agg.0,
            commit_agg.1,
            commit_agg.2,
            session_agg.3,
            cost_cents,
            session_agg.4,
        )
        .await?;

        Ok(())
    }

    /// Generate daily snapshots for all dates in the range.
    ///
    /// Always refreshes snapshots (DELETE + INSERT) so that data stays
    /// current after incremental re-indexing updates session metrics.
    /// Includes today (i=0) so the trend chart shows the current day.
    pub async fn generate_missing_snapshots(&self, days_back: i64) -> DbResult<u32> {
        let today = Local::now().date_naive();

        // Collect all dates in the range (including today)
        let dates: Vec<String> = (0..=days_back)
            .map(|i| {
                (today - chrono::Duration::days(i))
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .collect();

        if dates.is_empty() {
            return Ok(0);
        }

        // Batch all snapshot generation in a single transaction
        let mut tx = self.pool().begin().await?;
        let mut count = 0u32;

        for date in &dates {
            // Inline the snapshot generation to use the transaction
            let session_agg: (i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                    COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(files_edited_count), 0) as files_edited_count,
                    SUM(total_cost_usd) as total_cost_usd
                FROM valid_sessions
                WHERE date(last_message_at, 'unixepoch', 'localtime') = ?1
                "#,
            )
            .bind(date)
            .fetch_one(&mut *tx)
            .await?;

            let commit_agg: (i64, i64, i64) = sqlx::query_as(
                r#"
                SELECT
                    COUNT(DISTINCT c.hash) as commits_count,
                    COALESCE(SUM(c.insertions), 0) as commit_insertions,
                    COALESCE(SUM(c.deletions), 0) as commit_deletions
                FROM session_commits sc
                JOIN commits c ON sc.commit_hash = c.hash
                JOIN valid_sessions s ON sc.session_id = s.id
                WHERE date(s.last_message_at, 'unixepoch', 'localtime') = ?1
                "#,
            )
            .bind(date)
            .fetch_one(&mut *tx)
            .await?;

            // Skip dates with no session activity
            if session_agg.0 == 0 && commit_agg.0 == 0 {
                continue;
            }

            let cost_cents = usd_opt_to_cents(session_agg.5, session_agg.0);

            // Delete any existing row for this (date, NULL, NULL) combo,
            // since UNIQUE(date, project_id, branch) doesn't catch NULL duplicates.
            sqlx::query(
                "DELETE FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch IS NULL",
            )
            .bind(date)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO contribution_snapshots
                    (date, project_id, branch, sessions_count, ai_lines_added, ai_lines_removed,
                     commits_count, commit_insertions, commit_deletions, tokens_used, cost_cents, files_edited_count)
                VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
            )
            .bind(date)
            .bind(session_agg.0)
            .bind(session_agg.1)
            .bind(session_agg.2)
            .bind(commit_agg.0)
            .bind(commit_agg.1)
            .bind(commit_agg.2)
            .bind(session_agg.3)
            .bind(cost_cents)
            .bind(session_agg.4)
            .execute(&mut *tx)
            .await?;

            count += 1;
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Roll up daily snapshots into weekly aggregates.
    ///
    /// This function aggregates daily snapshots older than `retention_days` into
    /// weekly buckets, reducing storage while preserving historical data.
    ///
    /// Weekly snapshots use the Monday of each week as the date key.
    ///
    /// # Arguments
    /// * `retention_days` - Keep daily granularity for this many days (default: 30)
    ///
    /// # Returns
    /// * Number of weekly snapshots created
    pub async fn rollup_weekly_snapshots(&self, retention_days: i64) -> DbResult<u32> {
        let cutoff_date = (Local::now() - chrono::Duration::days(retention_days))
            .format("%Y-%m-%d")
            .to_string();

        // Find weeks with daily snapshots that need rollup (global snapshots only)
        // Group by ISO week (Monday as start of week)
        let weeks: Vec<(String, i64, i64, i64, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                -- Get the Monday of the week (ISO week starts on Monday)
                date(date, 'weekday 0', '-6 days') as week_start,
                COALESCE(SUM(sessions_count), 0),
                COALESCE(SUM(ai_lines_added), 0),
                COALESCE(SUM(ai_lines_removed), 0),
                COALESCE(SUM(commits_count), 0),
                COALESCE(SUM(commit_insertions), 0),
                COALESCE(SUM(commit_deletions), 0),
                COALESCE(SUM(tokens_used), 0),
                COALESCE(SUM(cost_cents), 0),
                COALESCE(SUM(files_edited_count), 0)
            FROM contribution_snapshots
            WHERE project_id IS NULL
              AND branch IS NULL
              AND date < ?1
              AND length(date) = 10  -- Only daily snapshots (YYYY-MM-DD format)
            GROUP BY week_start
            HAVING COUNT(*) > 0
            ORDER BY week_start ASC
            "#,
        )
        .bind(&cutoff_date)
        .fetch_all(self.pool())
        .await?;

        if weeks.is_empty() {
            return Ok(0);
        }

        // Batch all inserts and deletes in a single transaction for performance
        let mut tx = self.pool().begin().await?;
        let mut count = 0u32;

        for (
            week_start,
            sessions,
            lines_added,
            lines_removed,
            commits,
            insertions,
            deletions,
            tokens,
            cost,
            files_edited,
        ) in weeks
        {
            // Create the week key format: "W:YYYY-MM-DD" to distinguish from daily
            let week_key = format!("W:{}", week_start);

            // Check if weekly rollup already exists
            let existing: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch IS NULL",
            )
            .bind(&week_key)
            .fetch_one(&mut *tx)
            .await?;

            if existing.0 == 0 {
                // Insert weekly rollup
                sqlx::query(
                    r#"
                    INSERT INTO contribution_snapshots (
                        date, project_id, branch,
                        sessions_count, ai_lines_added, ai_lines_removed,
                        commits_count, commit_insertions, commit_deletions,
                        tokens_used, cost_cents, files_edited_count
                    ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    "#,
                )
                .bind(&week_key)
                .bind(sessions)
                .bind(lines_added)
                .bind(lines_removed)
                .bind(commits)
                .bind(insertions)
                .bind(deletions)
                .bind(tokens)
                .bind(cost)
                .bind(files_edited)
                .execute(&mut *tx)
                .await?;

                count += 1;
            }

            // Delete the daily snapshots that were rolled up (keeping weekly)
            sqlx::query(
                r#"
                DELETE FROM contribution_snapshots
                WHERE project_id IS NULL
                  AND branch IS NULL
                  AND date >= ?1
                  AND date < date(?1, '+7 days')
                  AND length(date) = 10  -- Only delete daily snapshots
                "#,
            )
            .bind(&week_start)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Get snapshot retention statistics.
    ///
    /// Returns counts of daily vs weekly snapshots for monitoring.
    pub async fn get_snapshot_stats(&self) -> DbResult<SnapshotStats> {
        let (daily_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE length(date) = 10 AND project_id IS NULL",
        )
        .fetch_one(self.pool())
        .await?;

        let (weekly_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE date LIKE 'W:%' AND project_id IS NULL",
        )
        .fetch_one(self.pool())
        .await?;

        let oldest_daily: Option<(String,)> = sqlx::query_as(
            "SELECT MIN(date) FROM contribution_snapshots WHERE length(date) = 10 AND project_id IS NULL",
        )
        .fetch_optional(self.pool())
        .await?;

        let oldest_weekly: Option<(String,)> = sqlx::query_as(
            "SELECT MIN(date) FROM contribution_snapshots WHERE date LIKE 'W:%' AND project_id IS NULL",
        )
        .fetch_optional(self.pool())
        .await?;

        Ok(SnapshotStats {
            daily_count,
            weekly_count,
            oldest_daily: oldest_daily.and_then(|r| r.0.into()),
            oldest_weekly: oldest_weekly.map(|r| r.0.replace("W:", "")),
        })
    }
}
