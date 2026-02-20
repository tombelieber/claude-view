// crates/db/src/queries/system.rs
// System-level queries: storage stats, health, classification status, reset.

use crate::{Database, DbResult};
use chrono::Utc;
use super::{StorageStats, HealthStats, HealthStatus, ClassificationStatus};

impl Database {
    /// Get the oldest session date (Unix timestamp).
    pub async fn get_oldest_session_date(&self, project: Option<&str>, branch: Option<&str>) -> DbResult<Option<i64>> {
        let result: (Option<i64>,) = sqlx::query_as(
            "SELECT MIN(last_message_at) FROM valid_sessions WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)",
        )
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;
        Ok(result.0)
    }

    /// Get all storage-related counts in a single query (replaces 4 separate queries).
    ///
    /// Returns (session_count, project_count, commit_count, oldest_session_date).
    pub async fn get_storage_counts(&self) -> DbResult<(i64, i64, i64, Option<i64>)> {
        let (session_count, project_count, commit_count, oldest_date): (i64, i64, i64, Option<i64>) =
            sqlx::query_as(
                r#"
                SELECT
                  (SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0),
                  (SELECT COUNT(DISTINCT project_id) FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0),
                  (SELECT COUNT(DISTINCT commit_hash) FROM session_commits),
                  (SELECT MIN(last_message_at) FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0)
                "#,
            )
            .fetch_one(self.pool())
            .await?;

        Ok((session_count, project_count, commit_count, oldest_date))
    }

    /// Get the SQLite database file size in bytes.
    /// Uses SQLite pragma to calculate page_count * page_size.
    pub async fn get_database_size(&self) -> DbResult<i64> {
        let (page_count,): (i64,) = sqlx::query_as("SELECT page_count FROM pragma_page_count()")
            .fetch_one(self.pool())
            .await?;
        let (page_size,): (i64,) = sqlx::query_as("SELECT page_size FROM pragma_page_size()")
            .fetch_one(self.pool())
            .await?;
        Ok(page_count * page_size)
    }

    /// Set the primary model for a session (used for testing and indexing).
    pub async fn set_session_primary_model(&self, session_id: &str, model: &str) -> DbResult<()> {
        sqlx::query("UPDATE sessions SET primary_model = ?1 WHERE id = ?2")
            .bind(model)
            .bind(session_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Backfill primary_model from turns table for sessions that were deep-indexed
    /// before primary_model was populated during indexing.
    pub async fn backfill_primary_models(&self) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE sessions SET primary_model = (
                SELECT model_id FROM turns
                WHERE turns.session_id = sessions.id
                GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 1
            )
            WHERE primary_model IS NULL AND deep_indexed_at IS NOT NULL
            "#,
        )
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }

    // ========================================================================
    // Theme 4 Phase 3: System Page Queries
    // ========================================================================

    /// Get storage statistics for the system page.
    ///
    /// Returns sizes for JSONL files (from indexer_state), database file,
    /// and computed totals. Index and cache sizes are set to 0 here and
    /// can be augmented by the server layer with filesystem checks.
    pub async fn get_storage_stats(&self) -> DbResult<StorageStats> {
        // Sum of JSONL file sizes from indexer_state
        let (jsonl_bytes,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(file_size), 0) FROM indexer_state",
        )
        .fetch_one(self.pool())
        .await?;

        // Database file size
        let db_bytes = if self.db_path().exists() && !self.db_path().as_os_str().is_empty() {
            std::fs::metadata(self.db_path())
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        // Index and cache sizes are intentionally 0 here — computed at the server
        // layer via filesystem scan (see crates/server/src/routes/stats.rs).
        let index_bytes: u64 = 0;
        let cache_bytes: u64 = 0;

        let total_bytes = jsonl_bytes as u64 + index_bytes + db_bytes + cache_bytes;

        Ok(StorageStats {
            jsonl_bytes: jsonl_bytes as u64,
            index_bytes,
            db_bytes,
            cache_bytes,
            total_bytes,
        })
    }

    /// Get health statistics for the system page.
    pub async fn get_health_stats(&self) -> DbResult<HealthStats> {
        // Count sessions (excluding sidechains)
        let (sessions_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0",
        )
        .fetch_one(self.pool())
        .await?;

        // Count unique commits
        let (commits_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM commits")
                .fetch_one(self.pool())
                .await?;

        // Count unique projects
        let (projects_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT project_id) FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0",
        )
        .fetch_one(self.pool())
        .await?;

        // Count parsing errors from last index run (failed index_runs entries)
        let (errors_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM index_runs WHERE status = 'failed'",
        )
        .fetch_one(self.pool())
        .await?;

        // Get last sync timestamp
        let metadata = self.get_index_metadata().await?;
        let last_sync_at = metadata.last_indexed_at;

        // Determine status
        let status = Self::calculate_health_status(errors_count, last_sync_at);

        Ok(HealthStats {
            sessions_count,
            commits_count,
            projects_count,
            errors_count,
            last_sync_at,
            status,
        })
    }

    /// Calculate health status based on errors and staleness.
    fn calculate_health_status(
        errors_count: i64,
        last_sync_at: Option<i64>,
    ) -> HealthStatus {
        // Error: 10+ errors or index stale > 24 hours
        if errors_count >= 10 {
            return HealthStatus::Error;
        }

        if let Some(ts) = last_sync_at {
            let now = Utc::now().timestamp();
            let hours_stale = (now - ts) / 3600;
            if hours_stale >= 24 {
                return HealthStatus::Error;
            }
        }

        // Warning: any errors
        if errors_count > 0 {
            return HealthStatus::Warning;
        }

        HealthStatus::Healthy
    }

    /// Get classification status summary for the system page.
    pub async fn get_classification_status(&self) -> DbResult<ClassificationStatus> {
        // Count classified sessions
        let (classified_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE classified_at IS NOT NULL AND is_sidechain = 0",
        )
        .fetch_one(self.pool())
        .await?;

        // Count unclassified sessions
        let (unclassified_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE classified_at IS NULL AND is_sidechain = 0",
        )
        .fetch_one(self.pool())
        .await?;

        // Get the most recent completed job
        #[allow(clippy::type_complexity)]
        let last_job: Option<(String, Option<String>, Option<i64>, String, String)> = sqlx::query_as(
            r#"
            SELECT started_at, completed_at, actual_cost_cents, provider, model
            FROM classification_jobs
            WHERE status = 'completed'
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(self.pool())
        .await?;

        // Check for active job
        let active_job = self.get_active_classification_job().await?;

        let (last_run_at, last_run_duration_ms, last_run_cost_cents, provider, model) =
            if let Some((started, completed, cost, prov, mdl)) = last_job {
                // Calculate duration from started_at to completed_at
                let duration = if let Some(ref completed_at) = completed {
                    // Both are RFC3339 strings; parse and compute diff
                    let start = chrono::DateTime::parse_from_rfc3339(&started).ok();
                    let end = chrono::DateTime::parse_from_rfc3339(completed_at).ok();
                    match (start, end) {
                        (Some(s), Some(e)) => Some((e - s).num_milliseconds()),
                        _ => None,
                    }
                } else {
                    None
                };
                (Some(started), duration, cost, prov, mdl)
            } else {
                (
                    None,
                    None,
                    None,
                    "claude-cli".to_string(),
                    "claude-3-haiku-20240307".to_string(),
                )
            };

        let is_running = active_job.is_some();
        let progress = active_job.as_ref().map(|j| {
            if j.total_sessions > 0 {
                ((j.classified_count as f64 / j.total_sessions as f64) * 100.0) as i64
            } else {
                0
            }
        });

        Ok(ClassificationStatus {
            classified_count,
            unclassified_count,
            last_run_at,
            last_run_duration_ms,
            last_run_cost_cents,
            provider,
            model,
            is_running,
            progress,
        })
    }

    /// Reset all application data (factory reset).
    /// Clears sessions, commits, invocables, index runs, etc.
    /// Does NOT delete original JSONL files.
    pub async fn reset_all_data(&self) -> DbResult<()> {
        // Use a single transaction for atomicity
        let mut tx = self.pool().begin().await?;

        // Order matters due to foreign key constraints
        sqlx::query("DELETE FROM session_commits")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM turn_metrics")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM api_errors")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM turns")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM invocations")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM invocables")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM commits")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM sessions")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM models")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM indexer_state")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM classification_jobs")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM index_runs")
            .execute(&mut *tx)
            .await?;

        // Reset index_metadata to defaults
        sqlx::query(
            r#"
            UPDATE index_metadata SET
                last_indexed_at = NULL,
                last_index_duration_ms = NULL,
                sessions_indexed = 0,
                projects_indexed = 0,
                last_git_sync_at = NULL,
                commits_found = 0,
                links_created = 0,
                updated_at = strftime('%s', 'now')
            WHERE id = 1
            "#,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}
