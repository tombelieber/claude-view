// crates/db/src/queries/classification.rs
// Classification job and index run CRUD operations (Theme 4).

use crate::{Database, DbResult};
use chrono::Utc;
use super::row_types::{ClassificationJobRow, IndexRunRow};

impl Database {
    /// Create a new classification job. Returns the new job ID.
    pub async fn create_classification_job(
        &self,
        total_sessions: i64,
        provider: &str,
        model: &str,
        cost_estimate_cents: Option<i64>,
    ) -> DbResult<i64> {
        let started_at = Utc::now().to_rfc3339();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO classification_jobs (started_at, total_sessions, provider, model, cost_estimate_cents)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id
            "#,
        )
        .bind(&started_at)
        .bind(total_sessions)
        .bind(provider)
        .bind(model)
        .bind(cost_estimate_cents)
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }

    /// Get the currently running classification job, if any.
    pub async fn get_active_classification_job(&self) -> DbResult<Option<vibe_recall_core::ClassificationJob>> {
        let row: Option<ClassificationJobRow> = sqlx::query_as(
            "SELECT * FROM classification_jobs WHERE status = 'running' ORDER BY started_at DESC LIMIT 1",
        )
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|r| r.into_classification_job()))
    }

    /// Update classification job progress counters.
    pub async fn update_classification_job_progress(
        &self,
        job_id: i64,
        classified_count: i64,
        skipped_count: i64,
        failed_count: i64,
        tokens_used: Option<i64>,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE classification_jobs SET
                classified_count = ?2,
                skipped_count = ?3,
                failed_count = ?4,
                tokens_used = ?5
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(classified_count)
        .bind(skipped_count)
        .bind(failed_count)
        .bind(tokens_used)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Mark a classification job as completed.
    pub async fn complete_classification_job(
        &self,
        job_id: i64,
        actual_cost_cents: Option<i64>,
    ) -> DbResult<()> {
        let completed_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE classification_jobs SET
                status = 'completed',
                completed_at = ?2,
                actual_cost_cents = ?3
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(&completed_at)
        .bind(actual_cost_cents)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Cancel a running classification job.
    pub async fn cancel_classification_job(&self, job_id: i64) -> DbResult<()> {
        let completed_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE classification_jobs SET
                status = 'cancelled',
                completed_at = ?2
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(&completed_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Fail a classification job with an error message.
    pub async fn fail_classification_job(&self, job_id: i64, error: &str) -> DbResult<()> {
        let completed_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE classification_jobs SET
                status = 'failed',
                completed_at = ?2,
                error_message = ?3
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(&completed_at)
        .bind(error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Get recent classification jobs (last 10).
    pub async fn get_recent_classification_jobs(&self) -> DbResult<Vec<vibe_recall_core::ClassificationJob>> {
        let rows: Vec<ClassificationJobRow> = sqlx::query_as(
            "SELECT * FROM classification_jobs ORDER BY started_at DESC LIMIT 10",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|r| r.into_classification_job()).collect())
    }

    /// Create a new index run. Returns the new run ID.
    pub async fn create_index_run(
        &self,
        run_type: &str,
        sessions_before: Option<i64>,
    ) -> DbResult<i64> {
        let started_at = Utc::now().to_rfc3339();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO index_runs (started_at, type, sessions_before)
            VALUES (?1, ?2, ?3)
            RETURNING id
            "#,
        )
        .bind(&started_at)
        .bind(run_type)
        .bind(sessions_before)
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }

    /// Mark an index run as completed.
    pub async fn complete_index_run(
        &self,
        run_id: i64,
        sessions_after: Option<i64>,
        duration_ms: i64,
        throughput_mb_per_sec: Option<f64>,
    ) -> DbResult<()> {
        let completed_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE index_runs SET
                status = 'completed',
                completed_at = ?2,
                sessions_after = ?3,
                duration_ms = ?4,
                throughput_mb_per_sec = ?5
            WHERE id = ?1
            "#,
        )
        .bind(run_id)
        .bind(&completed_at)
        .bind(sessions_after)
        .bind(duration_ms)
        .bind(throughput_mb_per_sec)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Fail an index run with an error message.
    pub async fn fail_index_run(&self, run_id: i64, error: &str) -> DbResult<()> {
        let completed_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE index_runs SET
                status = 'failed',
                completed_at = ?2,
                error_message = ?3
            WHERE id = ?1
            "#,
        )
        .bind(run_id)
        .bind(&completed_at)
        .bind(error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Get recent index runs (last 20).
    pub async fn get_recent_index_runs(&self) -> DbResult<Vec<vibe_recall_core::IndexRun>> {
        let rows: Vec<IndexRunRow> = sqlx::query_as(
            "SELECT * FROM index_runs ORDER BY started_at DESC LIMIT 20",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|r| r.into_index_run()).collect())
    }

    /// Get unclassified sessions (id + preview + skills_used) for classification.
    /// Returns sessions where category_l1 IS NULL, limited to `limit` rows.
    pub async fn get_unclassified_sessions(
        &self,
        limit: i64,
    ) -> DbResult<Vec<(String, String, String)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT id, preview, skills_used
            FROM sessions
            WHERE category_l1 IS NULL
            ORDER BY last_message_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    /// Get ALL sessions (id + preview + skills_used) for reclassification.
    /// Returns all sessions, limited to `limit` rows.
    pub async fn get_all_sessions_for_classification(
        &self,
        limit: i64,
    ) -> DbResult<Vec<(String, String, String)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT id, preview, skills_used
            FROM sessions
            ORDER BY last_message_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    /// Count unclassified sessions.
    pub async fn count_unclassified_sessions(&self) -> DbResult<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE category_l1 IS NULL",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }

    /// Count all sessions.
    pub async fn count_all_sessions(&self) -> DbResult<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }

    /// Count classified sessions.
    pub async fn count_classified_sessions(&self) -> DbResult<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE category_l1 IS NOT NULL",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }

    /// Batch update session classifications (within a single transaction).
    pub async fn batch_update_session_classifications(
        &self,
        updates: &[(String, String, String, String, f64, String)],
    ) -> DbResult<()> {
        let classified_at = Utc::now().to_rfc3339();
        let mut tx = self.pool().begin().await?;
        for (session_id, l1, l2, l3, confidence, source) in updates {
            sqlx::query(
                r#"
                UPDATE sessions SET
                    category_l1 = ?2,
                    category_l2 = ?3,
                    category_l3 = ?4,
                    category_confidence = ?5,
                    category_source = ?6,
                    classified_at = ?7
                WHERE id = ?1
                "#,
            )
            .bind(session_id)
            .bind(l1)
            .bind(l2)
            .bind(l3)
            .bind(confidence)
            .bind(source)
            .bind(&classified_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Get a classification job by ID.
    pub async fn get_classification_job(&self, job_id: i64) -> DbResult<Option<vibe_recall_core::ClassificationJob>> {
        let row: Option<ClassificationJobRow> = sqlx::query_as(
            "SELECT * FROM classification_jobs WHERE id = ?1",
        )
        .bind(job_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|r| r.into_classification_job()))
    }

    /// Get the most recent completed/cancelled/failed classification job.
    pub async fn get_last_completed_classification_job(&self) -> DbResult<Option<vibe_recall_core::ClassificationJob>> {
        let row: Option<ClassificationJobRow> = sqlx::query_as(
            "SELECT * FROM classification_jobs WHERE status IN ('completed', 'cancelled', 'failed') ORDER BY completed_at DESC LIMIT 1",
        )
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|r| r.into_classification_job()))
    }

    /// Mark stale running classification jobs as failed (for server restart recovery).
    pub async fn recover_stale_classification_jobs(&self) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE classification_jobs
            SET status = 'failed',
                error_message = 'Server restart interrupted job',
                completed_at = datetime('now')
            WHERE status = 'running'
            "#,
        )
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }
}
