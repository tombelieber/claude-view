//! Database query methods for insights trends (metric timeseries, category evolution, heatmap).

use crate::{Database, DbResult};

use super::types::{CategoryDataPoint, HeatmapCell, MetricDataPoint};

impl Database {
    /// Get time-series data for a metric, grouped by the specified granularity.
    pub async fn get_metric_timeseries(
        &self,
        metric: &str,
        from: i64,
        to: i64,
        granularity: &str,
    ) -> DbResult<Vec<MetricDataPoint>> {
        let group_by = match granularity {
            "day" => "date(last_message_at, 'unixepoch')",
            "week" => "strftime('%Y-W%W', last_message_at, 'unixepoch')",
            "month" => "strftime('%Y-%m', last_message_at, 'unixepoch')",
            _ => "strftime('%Y-W%W', last_message_at, 'unixepoch')",
        };

        let value_expr = match metric {
            "reedit_rate" => {
                "COALESCE(CAST(SUM(reedited_files_count) AS REAL) / NULLIF(SUM(files_edited_count), 0), 0.0)"
            }
            "sessions" => "CAST(COUNT(*) AS REAL)",
            "lines" => {
                "CAST(SUM(COALESCE(ai_lines_added, 0) + COALESCE(ai_lines_removed, 0)) AS REAL)"
            }
            "cost_per_line" => {
                "COALESCE(SUM(total_cost_usd) / NULLIF(SUM(CASE WHEN total_cost_usd IS NOT NULL THEN (COALESCE(ai_lines_added, 0) + COALESCE(ai_lines_removed, 0)) ELSE 0 END), 0), 0.0)"
            }
            "prompts" => "COALESCE(CAST(SUM(user_prompt_count) AS REAL) / NULLIF(COUNT(*), 0), 0.0)",
            _ => {
                "COALESCE(CAST(SUM(reedited_files_count) AS REAL) / NULLIF(SUM(files_edited_count), 0), 0.0)"
            }
        };

        let sql = format!(
            r#"
            SELECT
                {group_by} as period,
                {value_expr} as value
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND last_message_at <= ?2
            GROUP BY period
            ORDER BY period
            "#,
            group_by = group_by,
            value_expr = value_expr
        );

        let rows: Vec<(String, f64)> = sqlx::query_as(&sql)
            .bind(from)
            .bind(to)
            .fetch_all(self.pool())
            .await?;

        Ok(rows
            .into_iter()
            .map(|(date, value)| MetricDataPoint { date, value })
            .collect())
    }

    /// Get category evolution data (requires classification).
    /// Returns `None` if no sessions have been classified.
    pub async fn get_category_evolution(
        &self,
        from: i64,
        to: i64,
        granularity: &str,
    ) -> DbResult<Option<Vec<CategoryDataPoint>>> {
        // Check if any sessions have classification data
        let (classified_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM valid_sessions WHERE category_l1 IS NOT NULL")
                .fetch_one(self.pool())
                .await?;

        if classified_count == 0 {
            return Ok(None);
        }

        let group_by = match granularity {
            "day" => "date(last_message_at, 'unixepoch')",
            "week" => "strftime('%Y-W%W', last_message_at, 'unixepoch')",
            "month" => "strftime('%Y-%m', last_message_at, 'unixepoch')",
            _ => "strftime('%Y-W%W', last_message_at, 'unixepoch')",
        };

        let sql = format!(
            r#"
            SELECT
                {group_by} as period,
                CAST(SUM(CASE WHEN category_l1 = 'code_work' THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as code_work,
                CAST(SUM(CASE WHEN category_l1 = 'support_work' THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as support_work,
                CAST(SUM(CASE WHEN category_l1 = 'thinking_work' THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as thinking_work
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND last_message_at <= ?2
              AND category_l1 IS NOT NULL
            GROUP BY period
            ORDER BY period
            "#,
            group_by = group_by
        );

        let rows: Vec<(String, f64, f64, f64)> = sqlx::query_as(&sql)
            .bind(from)
            .bind(to)
            .fetch_all(self.pool())
            .await?;

        Ok(Some(
            rows.into_iter()
                .map(|(date, code, support, thinking)| CategoryDataPoint {
                    date,
                    code_work: code,
                    support_work: support,
                    thinking_work: thinking,
                })
                .collect(),
        ))
    }

    /// Get activity heatmap data: session count and avg re-edit rate by day-of-week and hour.
    pub async fn get_activity_heatmap(&self, from: i64, to: i64) -> DbResult<Vec<HeatmapCell>> {
        let rows: Vec<(i64, i64, i64, f64)> = sqlx::query_as(
            r#"
            SELECT
                CAST(strftime('%w', last_message_at, 'unixepoch') AS INTEGER) as dow,
                CAST(strftime('%H', last_message_at, 'unixepoch') AS INTEGER) as hour,
                COUNT(*) as sessions,
                COALESCE(
                    CAST(SUM(reedited_files_count) AS REAL) / NULLIF(SUM(files_edited_count), 0),
                    0.0
                ) as avg_reedit
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND last_message_at <= ?2
            GROUP BY dow, hour
            ORDER BY dow, hour
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(dow, hour, sessions, avg_reedit)| {
                // Convert SQLite's dow (0=Sunday) to our format (0=Monday)
                let adjusted_dow = if dow == 0 { 6 } else { dow - 1 };
                HeatmapCell {
                    day_of_week: adjusted_dow as u8,
                    hour_of_day: hour as u8,
                    sessions,
                    avg_reedit_rate: avg_reedit,
                }
            })
            .collect())
    }

    /// Get total session count within a time range.
    pub async fn get_session_count_in_range(&self, from: i64, to: i64) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM valid_sessions WHERE last_message_at >= ?1 AND last_message_at <= ?2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(self.pool())
        .await?;

        Ok(count)
    }
}
