// crates/db/src/snapshots/aggregation.rs
//! Snapshot CRUD and aggregated contribution queries.

use super::helpers::usd_opt_to_cents;
use super::types::{AggregatedContributions, TimeRange};
use crate::{Database, DbResult};
use chrono::Local;

impl Database {
    /// Upsert a contribution snapshot.
    ///
    /// Uses INSERT OR REPLACE because SQLite's UNIQUE constraint doesn't work
    /// well with NULL values in ON CONFLICT.
    pub async fn upsert_snapshot(
        &self,
        date: &str,
        project_id: Option<&str>,
        branch: Option<&str>,
        sessions_count: i64,
        ai_lines_added: i64,
        ai_lines_removed: i64,
        commits_count: i64,
        commit_insertions: i64,
        commit_deletions: i64,
        tokens_used: i64,
        cost_cents: i64,
        files_edited_count: i64,
    ) -> DbResult<()> {
        // First try to find existing row with matching (date, project_id, branch)
        // We need special handling because NULL = NULL is false in SQL
        let existing_id: Option<(i64,)> = match (project_id, branch) {
            (None, None) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch IS NULL"
                )
                .bind(date)
                .fetch_optional(self.pool())
                .await?
            }
            (Some(pid), None) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id = ?2 AND branch IS NULL"
                )
                .bind(date)
                .bind(pid)
                .fetch_optional(self.pool())
                .await?
            }
            (None, Some(br)) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch = ?2"
                )
                .bind(date)
                .bind(br)
                .fetch_optional(self.pool())
                .await?
            }
            (Some(pid), Some(br)) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id = ?2 AND branch = ?3"
                )
                .bind(date)
                .bind(pid)
                .bind(br)
                .fetch_optional(self.pool())
                .await?
            }
        };

        if let Some((id,)) = existing_id {
            // Update existing row
            sqlx::query(
                r#"
                UPDATE contribution_snapshots SET
                    sessions_count = ?1,
                    ai_lines_added = ?2,
                    ai_lines_removed = ?3,
                    commits_count = ?4,
                    commit_insertions = ?5,
                    commit_deletions = ?6,
                    tokens_used = ?7,
                    cost_cents = ?8,
                    files_edited_count = ?9
                WHERE id = ?10
                "#,
            )
            .bind(sessions_count)
            .bind(ai_lines_added)
            .bind(ai_lines_removed)
            .bind(commits_count)
            .bind(commit_insertions)
            .bind(commit_deletions)
            .bind(tokens_used)
            .bind(cost_cents)
            .bind(files_edited_count)
            .bind(id)
            .execute(self.pool())
            .await?;
        } else {
            // Insert new row
            sqlx::query(
                r#"
                INSERT INTO contribution_snapshots (
                    date, project_id, branch,
                    sessions_count, ai_lines_added, ai_lines_removed,
                    commits_count, commit_insertions, commit_deletions,
                    tokens_used, cost_cents, files_edited_count
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
            )
            .bind(date)
            .bind(project_id)
            .bind(branch)
            .bind(sessions_count)
            .bind(ai_lines_added)
            .bind(ai_lines_removed)
            .bind(commits_count)
            .bind(commit_insertions)
            .bind(commit_deletions)
            .bind(tokens_used)
            .bind(cost_cents)
            .bind(files_edited_count)
            .execute(self.pool())
            .await?;
        }

        Ok(())
    }

    /// Get aggregated contributions for a time range.
    ///
    /// For `Today`, queries sessions directly (real-time).
    /// For other ranges, queries contribution_snapshots.
    pub async fn get_aggregated_contributions(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        match range {
            TimeRange::Today => self.get_today_contributions(project_id, branch).await,
            TimeRange::All => self.get_all_contributions(project_id, branch).await,
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01");
                let to_default = Local::now().format("%Y-%m-%d").to_string();
                let to = to_date.unwrap_or(&to_default);
                self.get_contributions_in_range(from, to, project_id, branch)
                    .await
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Local::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Local::now().format("%Y-%m-%d").to_string();
                self.get_contributions_in_range(&from, &to, project_id, branch)
                    .await
            }
        }
    }

    /// Get today's contributions from sessions directly (real-time query).
    async fn get_today_contributions(
        &self,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let today_start = format!("{} 00:00:00", today);

        let row: (i64, i64, i64, i64, i64, i64, Option<f64>) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                    COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(user_prompt_count), 0) as prompts,
                    COALESCE(SUM(files_edited_count), 0) as files_edited_count,
                    SUM(total_cost_usd) as total_cost_usd
                FROM valid_sessions
                WHERE (project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1))
                  AND datetime(last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND (?3 IS NULL OR git_branch = ?3)
                "#,
            )
            .bind(pid)
            .bind(&today_start)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                    COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(user_prompt_count), 0) as prompts,
                    COALESCE(SUM(files_edited_count), 0) as files_edited_count,
                    SUM(total_cost_usd) as total_cost_usd
                FROM valid_sessions
                WHERE datetime(last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND (?2 IS NULL OR git_branch = ?2)
                "#,
            )
            .bind(&today_start)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        };

        // Get commit counts for today (from session_commits joined with commits)
        let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) = if let Some(
            pid,
        ) =
            project_id
        {
            sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN valid_sessions s ON sc.session_id = s.id
                    WHERE (s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1))
                      AND datetime(s.last_message_at, 'unixepoch', 'localtime') >= ?2
                      AND (?3 IS NULL OR s.git_branch = ?3)
                    "#,
                )
                .bind(pid)
                .bind(&today_start)
                .bind(branch)
                .fetch_one(self.pool())
                .await?
        } else {
            sqlx::query_as(
                r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN valid_sessions s ON sc.session_id = s.id
                    WHERE datetime(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                      AND (?2 IS NULL OR s.git_branch = ?2)
                    "#,
            )
            .bind(&today_start)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        };

        let cost_cents = usd_opt_to_cents(row.6, row.0);

        Ok(AggregatedContributions {
            sessions_count: row.0,
            ai_lines_added: row.1,
            ai_lines_removed: row.2,
            commits_count,
            commit_insertions,
            commit_deletions,
            tokens_used: row.3,
            cost_cents,
            files_edited_count: row.5,
        })
    }
}
