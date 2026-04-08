// crates/db/src/snapshots/rates.rs
//! Rate calculation queries: re-edit rate, commit rate, total prompts.

use super::types::TimeRange;
use crate::{Database, DbResult};
use chrono::Local;

impl Database {
    /// Calculate weighted average re-edit rate for a time range.
    ///
    /// Re-edit rate = total_reedited_files / total_files_edited
    pub async fn get_reedit_rate(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Option<f64>> {
        let (from, to) = match range {
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => (
                "1970-01-01".to_string(),
                Local::now().format("%Y-%m-%d").to_string(),
            ),
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Local::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Local::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        let row: (i64, i64) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(reedited_files_count), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM valid_sessions
                WHERE (project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1))
                  AND date(last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?3
                  AND (?4 IS NULL OR git_branch = ?4)
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(reedited_files_count), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM valid_sessions
                WHERE date(last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (?3 IS NULL OR git_branch = ?3)
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        };

        if row.1 == 0 {
            Ok(None)
        } else {
            Ok(Some(row.0 as f64 / row.1 as f64))
        }
    }

    /// Calculate commit rate for a time range.
    ///
    /// Commit rate = sessions_with_commits / total_sessions
    pub async fn get_commit_rate(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Option<f64>> {
        let (from, to) = match range {
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => (
                "1970-01-01".to_string(),
                Local::now().format("%Y-%m-%d").to_string(),
            ),
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Local::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Local::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        let row: (i64, i64) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    SUM(CASE WHEN commit_count > 0 THEN 1 ELSE 0 END),
                    COUNT(*)
                FROM valid_sessions
                WHERE (project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1))
                  AND date(last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?3
                  AND (?4 IS NULL OR git_branch = ?4)
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    SUM(CASE WHEN commit_count > 0 THEN 1 ELSE 0 END),
                    COUNT(*)
                FROM valid_sessions
                WHERE date(last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (?3 IS NULL OR git_branch = ?3)
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        };

        if row.1 == 0 {
            Ok(None)
        } else {
            Ok(Some(row.0 as f64 / row.1 as f64))
        }
    }

    /// Get total user prompt count for a time range.
    ///
    /// Queries sessions table directly since prompts are not stored in snapshots.
    pub async fn get_total_prompts(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<i64> {
        let (from, to) = match range {
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => (
                "1970-01-01".to_string(),
                Local::now().format("%Y-%m-%d").to_string(),
            ),
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Local::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Local::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        let row: (i64,) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT COALESCE(SUM(user_prompt_count), 0)
                FROM valid_sessions
                WHERE (project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1))
                  AND date(last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?3
                  AND (?4 IS NULL OR git_branch = ?4)
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT COALESCE(SUM(user_prompt_count), 0)
                FROM valid_sessions
                WHERE date(last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (?3 IS NULL OR git_branch = ?3)
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_one(self.pool())
            .await?
        };

        Ok(row.0)
    }
}
