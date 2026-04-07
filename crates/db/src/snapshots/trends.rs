// crates/db/src/snapshots/trends.rs
//! Daily trend data queries for charts.

use super::helpers::{fill_date_gaps, usd_to_cents};
use super::types::{DailyTrendPoint, TimeRange};
use crate::{Database, DbResult};
use chrono::Local;

impl Database {
    /// Get daily trend data for charting.
    pub async fn get_contribution_trend(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Vec<DailyTrendPoint>> {
        let (from, to) = match range {
            TimeRange::Today => {
                let today = Local::now().format("%Y-%m-%d").to_string();
                (today.clone(), today)
            }
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

        let sparse = if let Some(pid) = project_id {
            // Project-filtered: query sessions directly grouped by date
            // (snapshots only have global data)
            let rows: Vec<(String, i64, i64, i64, i64, i64, f64)> = sqlx::query_as(
                r#"
                SELECT
                    date(s.last_message_at, 'unixepoch', 'localtime') as date,
                    COALESCE(SUM(s.ai_lines_added), 0) as lines_added,
                    COALESCE(SUM(s.ai_lines_removed), 0) as lines_removed,
                    COALESCE((SELECT COUNT(DISTINCT sc.commit_hash) FROM session_commits sc
                      INNER JOIN valid_sessions s2 ON sc.session_id = s2.id
                      WHERE (s2.project_id = ?1 OR (s2.git_root IS NOT NULL AND s2.git_root <> '' AND s2.git_root = ?1) OR (s2.project_path IS NOT NULL AND s2.project_path <> '' AND s2.project_path = ?1))
                        AND s2.is_sidechain = 0
                        AND (?4 IS NULL OR s2.git_branch = ?4)
                        AND date(s2.last_message_at, 'unixepoch', 'localtime') = date(s.last_message_at, 'unixepoch', 'localtime')
                    ), 0) as commits_count,
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(s.total_input_tokens + s.total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(s.total_cost_usd), 0.0) as total_cost_usd
                FROM valid_sessions s
                WHERE (s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1))
                  AND s.is_sidechain = 0
                  AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?2
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?3
                  AND (?4 IS NULL OR s.git_branch = ?4)
                GROUP BY date(s.last_message_at, 'unixepoch', 'localtime')
                ORDER BY date ASC
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_all(self.pool())
            .await?;

            rows.into_iter()
                .map(
                    |(
                        date,
                        lines_added,
                        lines_removed,
                        commits,
                        sessions,
                        tokens_used,
                        cost_usd,
                    )| {
                        DailyTrendPoint {
                            date,
                            lines_added,
                            lines_removed,
                            commits,
                            sessions,
                            tokens_used,
                            cost_cents: usd_to_cents(cost_usd),
                        }
                    },
                )
                .collect()
        } else if branch.is_some() {
            // Global + branch filter: query sessions directly (snapshots lack branch column)
            let rows: Vec<(String, i64, i64, i64, i64, i64, f64)> = sqlx::query_as(
                r#"
                SELECT
                    date(s.last_message_at, 'unixepoch', 'localtime') as date,
                    COALESCE(SUM(s.ai_lines_added), 0) as lines_added,
                    COALESCE(SUM(s.ai_lines_removed), 0) as lines_removed,
                    COALESCE((SELECT COUNT(DISTINCT sc.commit_hash) FROM session_commits sc
                      INNER JOIN valid_sessions s2 ON sc.session_id = s2.id
                      WHERE s2.git_branch = ?3
                        AND date(s2.last_message_at, 'unixepoch', 'localtime') = date(s.last_message_at, 'unixepoch', 'localtime')
                    ), 0) as commits_count,
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(s.total_input_tokens + s.total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(s.total_cost_usd), 0.0) as total_cost_usd
                FROM valid_sessions s
                WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND s.git_branch = ?3
                GROUP BY date(s.last_message_at, 'unixepoch', 'localtime')
                ORDER BY date ASC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_all(self.pool())
            .await?;

            rows.into_iter()
                .map(
                    |(
                        date,
                        lines_added,
                        lines_removed,
                        commits,
                        sessions,
                        tokens_used,
                        cost_usd,
                    )| {
                        DailyTrendPoint {
                            date,
                            lines_added,
                            lines_removed,
                            commits,
                            sessions,
                            tokens_used,
                            cost_cents: usd_to_cents(cost_usd),
                        }
                    },
                )
                .collect()
        } else {
            // Global: use pre-aggregated snapshots
            let rows: Vec<(String, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
                r#"
                SELECT
                    date,
                    ai_lines_added,
                    ai_lines_removed,
                    commits_count,
                    sessions_count,
                    tokens_used,
                    cost_cents
                FROM contribution_snapshots
                WHERE project_id IS NULL AND date >= ?1 AND date <= ?2
                ORDER BY date ASC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?;

            rows.into_iter()
                .map(
                    |(
                        date,
                        lines_added,
                        lines_removed,
                        commits,
                        sessions,
                        tokens_used,
                        cost_cents,
                    )| DailyTrendPoint {
                        date,
                        lines_added,
                        lines_removed,
                        commits,
                        sessions,
                        tokens_used,
                        cost_cents,
                    },
                )
                .collect()
        };

        // Fill in zero-value entries for days with no sessions so the trend
        // array covers the full date range (charts render correctly, no gaps).
        Ok(fill_date_gaps(sparse, &from, &to))
    }
}
