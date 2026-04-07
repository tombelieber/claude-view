// crates/db/src/snapshots/branches.rs
//! Branch breakdown and branch session queries.

use super::types::{BranchBreakdown, BranchSession, TimeRange};
use crate::{Database, DbResult};
use chrono::Local;

impl Database {
    /// Get contribution breakdown by branch.
    pub async fn get_branch_breakdown(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Vec<BranchBreakdown>> {
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

        // Query sessions grouped by branch for the time range
        if let Some(pid) = project_id {
            // Project-filtered: no need to return project info (it's redundant)
            let rows: Vec<(Option<String>, i64, i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
                r#"
                SELECT
                    s.git_branch,
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(s.ai_lines_added), 0) as lines_added,
                    COALESCE(SUM(s.ai_lines_removed), 0) as lines_removed,
                    COALESCE((SELECT COUNT(DISTINCT sc.commit_hash) FROM session_commits sc
                      INNER JOIN valid_sessions s2 ON sc.session_id = s2.id
                      WHERE (s2.project_id = ?1 OR (s2.git_root IS NOT NULL AND s2.git_root <> '' AND s2.git_root = ?1) OR (s2.project_path IS NOT NULL AND s2.project_path <> '' AND s2.project_path = ?1))
                        AND date(s2.last_message_at, 'unixepoch', 'localtime') >= ?2
                        AND date(s2.last_message_at, 'unixepoch', 'localtime') <= ?3
                        AND s2.git_branch IS s.git_branch
                    ), 0) as commits_count,
                    COALESCE(SUM(s.files_edited_count), 0) as files_edited,
                    MAX(s.last_message_at) as last_activity
                FROM valid_sessions s
                WHERE (s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1))
                  AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?3
                  AND (?4 IS NULL OR s.git_branch = ?4)
                GROUP BY s.git_branch
                ORDER BY sessions_count DESC
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_all(self.pool())
            .await?;

            Ok(rows
                .into_iter()
                .map(
                    |(
                        branch,
                        sessions_count,
                        lines_added,
                        lines_removed,
                        commits_count,
                        _files_edited,
                        last_activity,
                    )| {
                        BranchBreakdown {
                            branch: branch.unwrap_or_else(|| "(no branch)".to_string()),
                            sessions_count,
                            lines_added,
                            lines_removed,
                            commits_count,
                            ai_share: None,
                            last_activity,
                            project_id: None,
                            project_name: None,
                        }
                    },
                )
                .collect())
        } else {
            // Global: group by project + branch so frontend can group by project
            let rows: Vec<(
                Option<String>,
                i64,
                i64,
                i64,
                i64,
                i64,
                Option<i64>,
                Option<String>,
                Option<String>,
            )> = sqlx::query_as(
                r#"
                SELECT
                    s.git_branch,
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(s.ai_lines_added), 0) as lines_added,
                    COALESCE(SUM(s.ai_lines_removed), 0) as lines_removed,
                    COALESCE((SELECT COUNT(DISTINCT sc.commit_hash) FROM session_commits sc
                      INNER JOIN valid_sessions s2 ON sc.session_id = s2.id
                      WHERE date(s2.last_message_at, 'unixepoch', 'localtime') >= ?1
                        AND date(s2.last_message_at, 'unixepoch', 'localtime') <= ?2
                        AND s2.project_id IS s.project_id
                        AND s2.git_branch IS s.git_branch
                    ), 0) as commits_count,
                    COALESCE(SUM(s.files_edited_count), 0) as files_edited,
                    MAX(s.last_message_at) as last_activity,
                    s.project_id,
                    COALESCE(s.project_display_name, s.project_id) as project_name
                FROM valid_sessions s
                WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (?3 IS NULL OR s.git_branch = ?3)
                GROUP BY s.project_id, s.git_branch
                ORDER BY sessions_count DESC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_all(self.pool())
            .await?;

            Ok(rows
                .into_iter()
                .map(
                    |(
                        branch,
                        sessions_count,
                        lines_added,
                        lines_removed,
                        commits_count,
                        _files_edited,
                        last_activity,
                        pid,
                        pname,
                    )| {
                        BranchBreakdown {
                            branch: branch.unwrap_or_else(|| "(no branch)".to_string()),
                            sessions_count,
                            lines_added,
                            lines_removed,
                            commits_count,
                            ai_share: None,
                            last_activity,
                            project_id: pid,
                            project_name: pname,
                        }
                    },
                )
                .collect())
        }
    }

    /// Get sessions for a specific branch.
    ///
    /// Returns lightweight session summaries for display when a branch is expanded.
    /// Sessions are ordered by last_message_at descending (most recent first).
    pub async fn get_branch_sessions(
        &self,
        branch: &str,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        limit: i64,
    ) -> DbResult<Vec<BranchSession>> {
        let (from, to) = self.date_range_from_time_range(range, from_date, to_date);

        // Handle "(no branch)" special case
        let branch_filter = if branch == "(no branch)" {
            None
        } else {
            Some(branch)
        };

        let rows: Vec<(String, Option<String>, i64, i64, i64, i64, i64)> = if let Some(pid) =
            project_id
        {
            if let Some(b) = branch_filter {
                sqlx::query_as(
                        r#"
                    SELECT
                        id,
                        work_type,
                        duration_seconds,
                        COALESCE(ai_lines_added, 0),
                        COALESCE(ai_lines_removed, 0),
                        COALESCE(commit_count, 0),
                        last_message_at
                    FROM valid_sessions
                    WHERE (project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1))
                      AND git_branch = ?2
                      AND date(last_message_at, 'unixepoch', 'localtime') >= ?3
                      AND date(last_message_at, 'unixepoch', 'localtime') <= ?4
                    ORDER BY last_message_at DESC
                    LIMIT ?5
                    "#,
                    )
                    .bind(pid)
                    .bind(b)
                    .bind(&from)
                    .bind(&to)
                    .bind(limit)
                    .fetch_all(self.pool())
                    .await?
            } else {
                sqlx::query_as(
                        r#"
                    SELECT
                        id,
                        work_type,
                        duration_seconds,
                        COALESCE(ai_lines_added, 0),
                        COALESCE(ai_lines_removed, 0),
                        COALESCE(commit_count, 0),
                        last_message_at
                    FROM valid_sessions
                    WHERE (project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1))
                      AND git_branch IS NULL
                      AND date(last_message_at, 'unixepoch', 'localtime') >= ?2
                      AND date(last_message_at, 'unixepoch', 'localtime') <= ?3
                    ORDER BY last_message_at DESC
                    LIMIT ?4
                    "#,
                    )
                    .bind(pid)
                    .bind(&from)
                    .bind(&to)
                    .bind(limit)
                    .fetch_all(self.pool())
                    .await?
            }
        } else if let Some(b) = branch_filter {
            sqlx::query_as(
                r#"
                SELECT
                    id,
                    work_type,
                    duration_seconds,
                    COALESCE(ai_lines_added, 0),
                    COALESCE(ai_lines_removed, 0),
                    COALESCE(commit_count, 0),
                    last_message_at
                FROM valid_sessions
                WHERE git_branch = ?1
                  AND date(last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?3
                ORDER BY last_message_at DESC
                LIMIT ?4
                "#,
            )
            .bind(b)
            .bind(&from)
            .bind(&to)
            .bind(limit)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    id,
                    work_type,
                    duration_seconds,
                    COALESCE(ai_lines_added, 0),
                    COALESCE(ai_lines_removed, 0),
                    COALESCE(commit_count, 0),
                    last_message_at
                FROM valid_sessions
                WHERE git_branch IS NULL
                  AND date(last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(last_message_at, 'unixepoch', 'localtime') <= ?2
                ORDER BY last_message_at DESC
                LIMIT ?3
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(limit)
            .fetch_all(self.pool())
            .await?
        };

        Ok(rows
            .into_iter()
            .map(
                |(
                    session_id,
                    work_type,
                    duration_seconds,
                    ai_lines_added,
                    ai_lines_removed,
                    commit_count,
                    last_message_at,
                )| {
                    BranchSession {
                        session_id,
                        work_type,
                        duration_seconds,
                        ai_lines_added,
                        ai_lines_removed,
                        commit_count,
                        last_message_at,
                    }
                },
            )
            .collect())
    }
}
