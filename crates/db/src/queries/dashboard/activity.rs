// crates/db/src/queries/dashboard/activity.rs
// Activity histogram and rich activity aggregation queries.

use super::types::{ActivityPoint, ActivitySummaryRow, ProjectActivityRow, RichActivityResponse};
use crate::{Database, DbResult};

impl Database {
    /// Activity histogram for sparkline chart.
    /// Auto-buckets by day/week/month based on data span.
    /// Optional time_after/time_before filter to match page context.
    /// Returns (Vec<ActivityPoint>, bucket_name).
    pub async fn session_activity_histogram(
        &self,
        time_after: Option<i64>,
        time_before: Option<i64>,
    ) -> DbResult<(Vec<ActivityPoint>, String)> {
        // Build time filter clause
        let time_clause = match (time_after, time_before) {
            (Some(ta), Some(tb)) => format!("last_message_at >= {ta} AND last_message_at <= {tb}"),
            (Some(ta), None) => format!("last_message_at >= {ta}"),
            (None, Some(tb)) => format!("last_message_at > 0 AND last_message_at <= {tb}"),
            (None, None) => "last_message_at > 0".to_string(),
        };

        // 1. Determine span (guard ts <= 0 — timestamp 0 is a data bug)
        let span_sql = format!(
            "SELECT COALESCE(MIN(last_message_at), 0), COALESCE(MAX(last_message_at), 0) \
             FROM valid_sessions WHERE {time_clause}"
        );
        let row: (i64, i64) = sqlx::query_as(&span_sql).fetch_one(self.pool()).await?;

        let span_days = (row.1 - row.0) / 86400;
        let (group_expr, bucket) = if span_days > 365 {
            ("strftime('%Y-%m', last_message_at, 'unixepoch')", "month")
        } else if span_days > 60 {
            ("strftime('%Y-W%W', last_message_at, 'unixepoch')", "week")
        } else {
            ("DATE(last_message_at, 'unixepoch')", "day")
        };

        // 2. Run grouped count + total_seconds
        let sql = format!(
            "SELECT {group_expr} AS date, COUNT(*) AS count, \
                COALESCE(SUM(duration_seconds), 0) AS total_seconds \
             FROM valid_sessions WHERE {time_clause} \
             GROUP BY date ORDER BY date"
        );

        let raw_rows: Vec<(String, i64, i64)> = sqlx::query_as(&sql).fetch_all(self.pool()).await?;

        let rows: Vec<ActivityPoint> = raw_rows
            .into_iter()
            .map(|(date, count, total_seconds)| ActivityPoint {
                date,
                count,
                total_seconds,
            })
            .collect();

        Ok((rows, bucket.to_string()))
    }

    /// Rich activity aggregation — histogram + project breakdown + summary stats.
    /// Replaces the client-side `useActivityData` pagination loop.
    pub async fn rich_activity(
        &self,
        time_after: Option<i64>,
        time_before: Option<i64>,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<RichActivityResponse> {
        // Build WHERE clause fragments
        let mut conditions = vec!["last_message_at > 0".to_string()];
        if let Some(ta) = time_after {
            conditions.push(format!("last_message_at >= {ta}"));
        }
        if let Some(tb) = time_before {
            conditions.push(format!("last_message_at <= {tb}"));
        }
        if let Some(p) = project {
            conditions.push(format!(
                "(project_id = '{p}' OR (git_root IS NOT NULL AND git_root != '' AND git_root = '{p}') \
                 OR (project_path IS NOT NULL AND project_path != '' AND project_path = '{p}'))"
            ));
        }
        if let Some(b) = branch {
            conditions.push(format!("git_branch = '{b}'"));
        }
        let where_clause = conditions.join(" AND ");

        // 1. Histogram — always daily buckets for CalendarHeatmap (needs YYYY-MM-DD).
        // The older session_activity_histogram (sparkline) can still auto-bucket.
        // needs YYYY-MM-DD dates regardless of span. The older
        // session_activity_histogram (sparkline) can still auto-bucket.
        let bucket = "day";

        let hist_sql = format!(
            "SELECT DATE(last_message_at, 'unixepoch') AS date, COUNT(*) AS count, \
                COALESCE(SUM(duration_seconds), 0) AS total_seconds \
             FROM valid_sessions WHERE {where_clause} \
             GROUP BY date ORDER BY date"
        );
        let histogram: Vec<ActivityPoint> = sqlx::query_as::<_, (String, i64, i64)>(&hist_sql)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(|(date, count, total_seconds)| ActivityPoint {
                date,
                count,
                total_seconds,
            })
            .collect();

        // 2. Project breakdown (top 20 by session count)
        let proj_sql = format!(
            "SELECT \
                COALESCE(NULLIF(git_root, ''), NULLIF(project_path, ''), project_id) AS effective_path, \
                CASE WHEN git_root IS NOT NULL AND git_root != '' \
                     THEN REPLACE(REPLACE(git_root, RTRIM(git_root, REPLACE(git_root, '/', '')), ''), '/', '') \
                     ELSE COALESCE(project_display_name, project_id) END AS display_name, \
                COUNT(*) AS session_count, \
                COALESCE(SUM(duration_seconds), 0) AS total_seconds, \
                COALESCE(SUM(total_cost_usd), 0.0) AS total_cost_usd \
             FROM valid_sessions WHERE {where_clause} \
             GROUP BY effective_path ORDER BY session_count DESC LIMIT 20"
        );
        let projects: Vec<ProjectActivityRow> =
            sqlx::query_as::<_, (String, String, i64, i64, f64)>(&proj_sql)
                .fetch_all(self.pool())
                .await?
                .into_iter()
                .map(|(path, name, count, secs, cost)| ProjectActivityRow {
                    project_path: path,
                    display_name: name,
                    session_count: count,
                    total_seconds: secs,
                    total_cost_usd: cost,
                })
                .collect();

        // 3. Summary stats
        let summary_sql = format!(
            "SELECT \
                COALESCE(SUM(duration_seconds), 0), \
                COUNT(*), \
                COALESCE(SUM(tool_call_count), 0), \
                COALESCE(SUM(agent_spawn_count), 0), \
                COALESCE(SUM(mcp_progress_count), 0) \
             FROM valid_sessions WHERE {where_clause}"
        );
        let (total_seconds, session_count, total_tool_calls, total_agent_spawns, total_mcp_calls): (
            i64, i64, i64, i64, i64,
        ) = sqlx::query_as(&summary_sql)
            .fetch_one(self.pool())
            .await?;

        // Unique skills count
        let skills_sql = format!(
            "SELECT COUNT(DISTINCT value) FROM valid_sessions, json_each(skills_used) \
             WHERE {where_clause} AND value IS NOT NULL AND value != ''"
        );
        let (unique_skills,): (i64,) = sqlx::query_as(&skills_sql)
            .fetch_one(self.pool())
            .await
            .unwrap_or((0,));

        // Longest session
        let longest_sql = format!(
            "SELECT id, duration_seconds, \
                COALESCE(NULLIF(git_root, ''), NULLIF(project_path, ''), project_id), \
                COALESCE(summary, preview, '') \
             FROM valid_sessions WHERE {where_clause} AND duration_seconds > 0 \
             ORDER BY duration_seconds DESC LIMIT 1"
        );
        let longest: Option<(String, i64, String, String)> = sqlx::query_as(&longest_sql)
            .fetch_optional(self.pool())
            .await?;

        let summary = ActivitySummaryRow {
            total_seconds,
            session_count,
            total_tool_calls,
            total_agent_spawns,
            total_mcp_calls,
            unique_skills,
            longest_session_id: longest.as_ref().map(|r| r.0.clone()),
            longest_session_seconds: longest.as_ref().map(|r| r.1).unwrap_or(0),
            longest_session_project: longest.as_ref().map(|r| r.2.clone()),
            longest_session_title: longest.as_ref().map(|r| r.3.clone()),
        };

        Ok(RichActivityResponse {
            histogram,
            bucket: bucket.to_string(),
            projects,
            summary,
            total: session_count,
        })
    }
}

#[cfg(test)]
mod rich_activity_tests {
    use crate::Database;
    use claude_view_core::{SessionInfo, ToolCounts};

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn make_session(id: &str, project: &str, modified_at: i64, duration: u32) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: project.to_string(),
            project_path: format!("/home/user/{project}"),
            display_name: project.to_string(),
            git_root: None,
            file_path: format!("/path/{id}.jsonl"),
            modified_at,
            size_bytes: 1024,
            preview: String::new(),
            last_message: String::new(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 5,
            turn_count: 3,
            summary: Some(format!("Session {id}")),
            git_branch: None,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: Some(1000),
            total_output_tokens: Some(500),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: Some(3),
            primary_model: Some("claude-sonnet-4".to_string()),
            user_prompt_count: 3,
            api_call_count: 5,
            tool_call_count: 10,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 2,
            files_edited_count: 1,
            reedited_files_count: 0,
            duration_seconds: duration,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 2,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 3,
            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: Some(0.05),
            slug: None,
            entrypoint: None,
        }
    }

    async fn insert(db: &Database, s: &SessionInfo) {
        db.insert_session(s, &s.project, &s.display_name)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rich_activity_basic() {
        let db = test_db().await;
        let ts_day1 = 1700000000_i64; // 2023-11-14
        let ts_day2 = ts_day1 + 86400; // 2023-11-15
        insert(&db, &make_session("s1", "project-a", ts_day1, 300)).await;
        insert(&db, &make_session("s2", "project-a", ts_day1 + 3600, 600)).await;
        insert(&db, &make_session("s3", "project-b", ts_day2, 1200)).await;

        let result = db.rich_activity(None, None, None, None).await.unwrap();

        assert_eq!(result.total, 3);
        assert_eq!(result.summary.session_count, 3);
        assert_eq!(result.summary.total_seconds, 300 + 600 + 1200);
        // tool_call_count is inserted by insert_session; agent_spawn/mcp are not
        assert_eq!(result.summary.total_tool_calls, 30); // 10 × 3
        assert_eq!(result.summary.longest_session_seconds, 1200);
        assert_eq!(result.summary.longest_session_id.as_deref(), Some("s3"));
        assert_eq!(result.projects.len(), 2);
        assert_eq!(result.projects[0].session_count, 2); // project-a
        assert_eq!(result.projects[1].session_count, 1); // project-b
        assert!(result.histogram.len() >= 2);
        assert_eq!(result.bucket, "day");
    }

    #[tokio::test]
    async fn test_rich_activity_time_filter() {
        let db = test_db().await;
        let ts1 = 1700000000_i64;
        let ts2 = ts1 + 86400 * 7;
        insert(&db, &make_session("s1", "p", ts1, 100)).await;
        insert(&db, &make_session("s2", "p", ts2, 200)).await;

        let result = db
            .rich_activity(Some(ts2 - 1), None, None, None)
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.summary.total_seconds, 200);
    }

    #[tokio::test]
    async fn test_rich_activity_empty() {
        let db = test_db().await;
        let result = db.rich_activity(None, None, None, None).await.unwrap();
        assert_eq!(result.total, 0);
        assert_eq!(result.summary.total_seconds, 0);
        assert!(result.histogram.is_empty());
        assert!(result.projects.is_empty());
    }
}
