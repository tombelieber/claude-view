//! Report CRUD queries and preview aggregation.

use crate::{Database, DbResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A saved report row.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ReportRow {
    #[ts(type = "number")]
    pub id: i64,
    pub report_type: String,
    pub date_start: String,
    pub date_end: String,
    pub content_md: String,
    pub context_digest: Option<String>,
    #[ts(type = "number")]
    pub session_count: i64,
    #[ts(type = "number")]
    pub project_count: i64,
    #[ts(type = "number")]
    pub total_duration_secs: i64,
    #[ts(type = "number")]
    pub total_cost_cents: i64,
    #[ts(type = "number | null")]
    pub generation_ms: Option<i64>,
    pub created_at: String,
}

/// Preview stats for a date range (no AI, pure DB aggregation).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ReportPreview {
    #[ts(type = "number")]
    pub session_count: i64,
    #[ts(type = "number")]
    pub project_count: i64,
    #[ts(type = "number")]
    pub total_duration_secs: i64,
    #[ts(type = "number")]
    pub total_cost_cents: i64,
    pub projects: Vec<ProjectPreview>,
}

/// Per-project summary in the preview.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreview {
    pub name: String,
    #[ts(type = "number")]
    pub session_count: i64,
}

impl Database {
    /// Insert a new report and return its id.
    pub async fn insert_report(
        &self,
        report_type: &str,
        date_start: &str,
        date_end: &str,
        content_md: &str,
        context_digest: Option<&str>,
        session_count: i64,
        project_count: i64,
        total_duration_secs: i64,
        total_cost_cents: i64,
        generation_ms: Option<i64>,
    ) -> DbResult<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"INSERT INTO reports (report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               RETURNING id"#,
        )
        .bind(report_type)
        .bind(date_start)
        .bind(date_end)
        .bind(content_md)
        .bind(context_digest)
        .bind(session_count)
        .bind(project_count)
        .bind(total_duration_secs)
        .bind(total_cost_cents)
        .bind(generation_ms)
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }

    /// List all reports, newest first.
    pub async fn list_reports(&self) -> DbResult<Vec<ReportRow>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, String, Option<String>, i64, i64, i64, i64, Option<i64>, String)>(
            "SELECT id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at FROM reports ORDER BY created_at DESC, id DESC"
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at)| ReportRow {
                id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at,
            })
            .collect())
    }

    /// Get a single report by id.
    pub async fn get_report(&self, id: i64) -> DbResult<Option<ReportRow>> {
        let row = sqlx::query_as::<_, (i64, String, String, String, String, Option<String>, i64, i64, i64, i64, Option<i64>, String)>(
            "SELECT id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at FROM reports WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|(id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at)| ReportRow {
            id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at,
        }))
    }

    /// Delete a report by id. Returns true if a row was deleted.
    pub async fn delete_report(&self, id: i64) -> DbResult<bool> {
        let result = sqlx::query("DELETE FROM reports WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Query sessions in a date range for report context building.
    /// Returns Vec of (id, project_display_name, preview, category_l2, duration_seconds, git_branch).
    pub async fn get_sessions_in_range(
        &self,
        start_ts: i64,
        end_ts: i64,
    ) -> DbResult<Vec<(String, String, String, Option<String>, i64, Option<String>)>> {
        let rows = sqlx::query_as(
            r#"SELECT id, project_display_name, preview, category_l2, duration_seconds, git_branch
               FROM valid_sessions
               WHERE first_message_at >= ? AND first_message_at <= ?
               ORDER BY project_display_name, git_branch, first_message_at"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    /// Query commit counts per project in a date range.
    /// Returns Vec of (project_display_name, commit_count).
    pub async fn get_commit_counts_in_range(
        &self,
        start_ts: i64,
        end_ts: i64,
    ) -> DbResult<Vec<(String, i64)>> {
        let rows = sqlx::query_as(
            r#"SELECT s.project_display_name, COUNT(DISTINCT sc.commit_hash)
               FROM valid_sessions s
               INNER JOIN session_commits sc ON sc.session_id = s.id
               WHERE s.first_message_at >= ?1 AND s.first_message_at <= ?2
               GROUP BY s.project_display_name"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    /// Query top tools used in a date range.
    /// Returns Vec of tool names ordered by usage count descending.
    pub async fn get_top_tools_in_range(
        &self,
        start_ts: i64,
        end_ts: i64,
        limit: i64,
    ) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"SELECT i.name
               FROM invocations i
               JOIN valid_sessions s ON i.session_id = s.id
               WHERE s.first_message_at >= ? AND s.first_message_at <= ?
                 AND i.type = 'tool'
               GROUP BY i.name
               ORDER BY SUM(i.count) DESC
               LIMIT ?"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }

    /// Query top skills used in a date range.
    /// Returns Vec of skill names ordered by usage count descending.
    pub async fn get_top_skills_in_range(
        &self,
        start_ts: i64,
        end_ts: i64,
        limit: i64,
    ) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"SELECT i.name
               FROM invocations i
               JOIN valid_sessions s ON i.session_id = s.id
               WHERE s.first_message_at >= ? AND s.first_message_at <= ?
                 AND i.type = 'skill'
               GROUP BY i.name
               ORDER BY SUM(i.count) DESC
               LIMIT ?"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }

    /// Query total input and output tokens for sessions in a date range.
    pub async fn get_token_totals_in_range(
        &self,
        start_ts: i64,
        end_ts: i64,
    ) -> DbResult<(i64, i64)> {
        let row: (i64, i64) = sqlx::query_as(
            r#"SELECT
                COALESCE(SUM(total_input_tokens), 0),
                COALESCE(SUM(total_output_tokens), 0)
               FROM valid_sessions
               WHERE first_message_at >= ? AND first_message_at <= ?"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_one(self.pool())
        .await?;
        Ok(row)
    }

    /// Aggregate preview stats for sessions in a date range.
    ///
    /// Uses `first_message_at` (unix timestamp) for filtering.
    /// `start_ts` and `end_ts` are unix timestamps for the range bounds.
    pub async fn get_report_preview(&self, start_ts: i64, end_ts: i64) -> DbResult<ReportPreview> {
        // Aggregate stats
        let stats: (i64, i64, i64, i64) = sqlx::query_as(
            r#"SELECT
                COUNT(*) as session_count,
                COUNT(DISTINCT project_display_name) as project_count,
                COALESCE(SUM(duration_seconds), 0) as total_duration,
                COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as total_tokens
            FROM valid_sessions
            WHERE first_message_at >= ? AND first_message_at <= ?"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_one(self.pool())
        .await?;

        // Per-project breakdown
        let project_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT project_display_name, COUNT(*) as cnt
               FROM valid_sessions
               WHERE first_message_at >= ? AND first_message_at <= ?
               GROUP BY project_display_name
               ORDER BY cnt DESC"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(self.pool())
        .await?;

        let projects = project_rows
            .into_iter()
            .map(|(name, session_count)| ProjectPreview { name, session_count })
            .collect();

        // Estimate cost from total tokens using blended rate (~$2.50/M tokens = 0.00025 cents/token)
        let total_tokens = stats.3;
        let total_cost_cents = (total_tokens as f64 * 0.00025).round() as i64;

        Ok(ReportPreview {
            session_count: stats.0,
            project_count: stats.1,
            total_duration_secs: stats.2,
            total_cost_cents,
            projects,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[tokio::test]
    async fn test_insert_and_get_report() {
        let db = Database::new_in_memory().await.unwrap();
        let id = db
            .insert_report("daily", "2026-02-21", "2026-02-21", "- Shipped search", None, 8, 3, 15120, 680, Some(14200))
            .await
            .unwrap();
        assert!(id > 0);

        let report = db.get_report(id).await.unwrap().unwrap();
        assert_eq!(report.report_type, "daily");
        assert_eq!(report.content_md, "- Shipped search");
        assert_eq!(report.session_count, 8);
    }

    #[tokio::test]
    async fn test_list_reports_newest_first() {
        let db = Database::new_in_memory().await.unwrap();
        db.insert_report("daily", "2026-02-20", "2026-02-20", "day 1", None, 5, 2, 3600, 100, None).await.unwrap();
        db.insert_report("daily", "2026-02-21", "2026-02-21", "day 2", None, 8, 3, 7200, 200, None).await.unwrap();

        let reports = db.list_reports().await.unwrap();
        assert_eq!(reports.len(), 2);
        // Newest first
        assert_eq!(reports[0].date_start, "2026-02-21");
    }

    #[tokio::test]
    async fn test_delete_report() {
        let db = Database::new_in_memory().await.unwrap();
        let id = db.insert_report("weekly", "2026-02-17", "2026-02-21", "week summary", None, 32, 5, 64800, 2450, None).await.unwrap();

        assert!(db.delete_report(id).await.unwrap());
        assert!(db.get_report(id).await.unwrap().is_none());
        assert!(!db.delete_report(id).await.unwrap()); // already deleted
    }

    #[tokio::test]
    async fn test_get_report_preview_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let preview = db.get_report_preview(0, i64::MAX).await.unwrap();
        assert_eq!(preview.session_count, 0);
        assert_eq!(preview.project_count, 0);
        assert!(preview.projects.is_empty());
    }

    #[tokio::test]
    async fn test_get_token_totals_in_range() {
        let db = Database::new_in_memory().await.unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, last_message_at, duration_seconds, total_input_tokens, total_output_tokens) VALUES ('s1', 'p1', '/tmp/s1.jsonl', 'Test', 'proj', 1000, 1100, 100, 500000, 80000)"
        ).execute(db.pool()).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, last_message_at, duration_seconds, total_input_tokens, total_output_tokens) VALUES ('s2', 'p1', '/tmp/s2.jsonl', 'Test', 'proj', 1100, 1300, 200, 347000, 44000)"
        ).execute(db.pool()).await.unwrap();

        let (input, output) = db.get_token_totals_in_range(0, 2000).await.unwrap();
        assert_eq!(input, 847000);
        assert_eq!(output, 124000);
    }

    #[tokio::test]
    async fn test_get_token_totals_empty_range() {
        let db = Database::new_in_memory().await.unwrap();
        let (input, output) = db.get_token_totals_in_range(0, 2000).await.unwrap();
        assert_eq!(input, 0);
        assert_eq!(output, 0);
    }

    #[tokio::test]
    async fn test_get_report_preview_with_sessions() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert test sessions
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, last_message_at, duration_seconds) VALUES ('s1', 'p1', '/tmp/s1.jsonl', 'Test', 'claude-view', 1000, 1100, 3600)"
        ).execute(db.pool()).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, last_message_at, duration_seconds) VALUES ('s2', 'p1', '/tmp/s2.jsonl', 'Test', 'claude-view', 1100, 1200, 1800)"
        ).execute(db.pool()).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, last_message_at, duration_seconds) VALUES ('s3', 'p2', '/tmp/s3.jsonl', 'Test', 'vicky-wiki', 1200, 1300, 900)"
        ).execute(db.pool()).await.unwrap();

        let preview = db.get_report_preview(0, 2000).await.unwrap();
        assert_eq!(preview.session_count, 3);
        assert_eq!(preview.project_count, 2);
        assert_eq!(preview.total_duration_secs, 6300);
        assert_eq!(preview.projects.len(), 2);
        // claude-view should be first (most sessions)
        assert_eq!(preview.projects[0].name, "claude-view");
        assert_eq!(preview.projects[0].session_count, 2);
    }
}
