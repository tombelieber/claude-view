// crates/db/src/snapshots/contributions.rs
//! All-time and date-range contribution queries.

use super::helpers::usd_opt_to_cents;
use super::types::AggregatedContributions;
use crate::{Database, DbResult};

impl Database {
    /// Get all-time contributions by querying sessions directly.
    ///
    /// Uses `valid_sessions` (`is_sidechain = 0`) to match dashboard
    /// primary-session semantics.
    pub(crate) async fn get_all_contributions(
        &self,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        if let Some(pid) = project_id {
            // Project-filtered: query sessions directly (snapshots only have global data)
            let row: (i64, i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
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
                  AND (?2 IS NULL OR git_branch = ?2)
                "#,
            )
            .bind(pid)
            .bind(branch)
            .fetch_one(self.pool())
            .await?;

            let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) =
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
                      AND (?2 IS NULL OR s.git_branch = ?2)
                    "#,
                )
                .bind(pid)
                .bind(branch)
                .fetch_one(self.pool())
                .await?;

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
        } else if branch.is_some() {
            // Global + branch filter: query sessions directly (snapshots lack branch column)
            let row: (i64, i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
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
                WHERE git_branch = ?1
                "#,
            )
            .bind(branch)
            .fetch_one(self.pool())
            .await?;

            let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) =
                sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN valid_sessions s ON sc.session_id = s.id
                    WHERE s.git_branch = ?1
                    "#,
                )
                .bind(branch)
                .fetch_one(self.pool())
                .await?;

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
        } else {
            // Global: query sessions directly (consistent with dashboard canonical filter)
            let row: (i64, i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
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
                "#,
            )
            .fetch_one(self.pool())
            .await?;

            let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) =
                sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN valid_sessions s ON sc.session_id = s.id
                    "#,
                )
                .fetch_one(self.pool())
                .await?;

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

    /// Get contributions in a date range.
    ///
    /// All branches query `valid_sessions` directly (view pre-filters
    /// `is_sidechain = 0`) to match the dashboard.
    pub(crate) async fn get_contributions_in_range(
        &self,
        from: &str,
        to: &str,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        if let Some(pid) = project_id {
            // Project-filtered: query sessions directly (snapshots only have global data)
            let row: (i64, i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
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
                  AND date(last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?3
                  AND (?4 IS NULL OR git_branch = ?4)
                "#,
            )
            .bind(pid)
            .bind(from)
            .bind(to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?;

            let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) =
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
                      AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?2
                      AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?3
                      AND (?4 IS NULL OR s.git_branch = ?4)
                    "#,
                )
                .bind(pid)
                .bind(from)
                .bind(to)
                .bind(branch)
                .fetch_one(self.pool())
                .await?;

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
        } else if branch.is_some() {
            // Global + branch filter: query sessions directly (snapshots lack branch column)
            let row: (i64, i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
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
                WHERE date(last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND git_branch = ?3
                "#,
            )
            .bind(from)
            .bind(to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?;

            let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) =
                sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN valid_sessions s ON sc.session_id = s.id
                    WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                      AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                      AND s.git_branch = ?3
                    "#,
                )
                .bind(from)
                .bind(to)
                .bind(branch)
                .fetch_one(self.pool())
                .await?;

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
        } else {
            // Global: query sessions directly (consistent with dashboard canonical filter)
            let row: (i64, i64, i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
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
                WHERE date(last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?2
                "#,
            )
            .bind(from)
            .bind(to)
            .fetch_one(self.pool())
            .await?;

            let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) =
                sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN valid_sessions s ON sc.session_id = s.id
                    WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                      AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                    "#,
                )
                .bind(from)
                .bind(to)
                .fetch_one(self.pool())
                .await?;

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
}
