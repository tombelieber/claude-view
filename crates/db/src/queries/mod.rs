// crates/db/src/queries/mod.rs
// Session CRUD operations for the vibe-recall SQLite database.

pub(crate) mod row_types;
mod classification;
mod dashboard;
mod invocables;
mod models;
mod sessions;

// Re-export _tx functions for indexer_parallel.rs (crate::queries::*_tx paths)
pub use row_types::{
    batch_insert_invocations_tx, batch_insert_turns_tx, batch_upsert_models_tx,
    update_session_deep_fields_tx,
};

// Re-export row types for sibling sub-modules

use crate::{Database, DbResult};
use chrono::Utc;
use serde::Serialize;
use ts_rs::TS;

/// Branch count for a project.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BranchCount {
    pub branch: Option<String>,
    #[ts(type = "number")]
    pub count: i64,
}

/// Indexer state entry returned from the database.
#[derive(Debug, Clone)]
pub struct IndexerEntry {
    pub file_path: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub indexed_at: i64,
}

/// An invocable (tool/skill/MCP) with its aggregated invocation count.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InvocableWithCount {
    pub id: String,
    pub plugin_name: Option<String>,
    pub name: String,
    pub kind: String,
    pub description: String,
    #[ts(type = "number")]
    pub invocation_count: i64,
    #[ts(type = "number | null")]
    pub last_used_at: Option<i64>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for InvocableWithCount {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            plugin_name: row.try_get("plugin_name")?,
            name: row.try_get("name")?,
            kind: row.try_get("kind")?,
            description: row.try_get("description")?,
            invocation_count: row.try_get("invocation_count")?,
            last_used_at: row.try_get("last_used_at")?,
        })
    }
}

/// A model record with aggregated usage stats (for GET /api/models).
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ModelWithStats {
    pub id: String,
    pub provider: Option<String>,
    pub family: Option<String>,
    #[ts(type = "number | null")]
    pub first_seen: Option<i64>,
    #[ts(type = "number | null")]
    pub last_seen: Option<i64>,
    #[ts(type = "number")]
    pub total_turns: i64,
    #[ts(type = "number")]
    pub total_sessions: i64,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for ModelWithStats {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            provider: row.try_get("provider")?,
            family: row.try_get("family")?,
            first_seen: row.try_get("first_seen")?,
            last_seen: row.try_get("last_seen")?,
            total_turns: row.try_get("total_turns")?,
            total_sessions: row.try_get("total_sessions")?,
        })
    }
}

/// Aggregate token usage statistics (for GET /api/stats/tokens).
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TokenStats {
    #[ts(type = "number")]
    pub total_input_tokens: u64,
    #[ts(type = "number")]
    pub total_output_tokens: u64,
    #[ts(type = "number")]
    pub total_cache_read_tokens: u64,
    #[ts(type = "number")]
    pub total_cache_creation_tokens: u64,
    pub cache_hit_ratio: f64,
    #[ts(type = "number")]
    pub turns_count: u64,
    #[ts(type = "number")]
    pub sessions_count: u64,
}

/// Token usage breakdown by model.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TokensByModel {
    pub model: String,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
}

/// Token usage breakdown by project.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TokensByProject {
    pub project: String,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
}

/// AI Generation statistics (for GET /api/stats/ai-generation).
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct AIGenerationStats {
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub files_created: i64,
    #[ts(type = "number")]
    pub total_input_tokens: i64,
    #[ts(type = "number")]
    pub total_output_tokens: i64,
    pub tokens_by_model: Vec<TokensByModel>,
    pub tokens_by_project: Vec<TokensByProject>,
}

/// Storage statistics for the system page.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    #[ts(type = "number")]
    pub jsonl_bytes: u64,
    #[ts(type = "number")]
    pub index_bytes: u64,
    #[ts(type = "number")]
    pub db_bytes: u64,
    #[ts(type = "number")]
    pub cache_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

/// Health status enum for the system page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Warning,
    Error,
}

/// Health statistics for the system page.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct HealthStats {
    #[ts(type = "number")]
    pub sessions_count: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    #[ts(type = "number")]
    pub projects_count: i64,
    #[ts(type = "number")]
    pub errors_count: i64,
    #[ts(type = "number | null")]
    pub last_sync_at: Option<i64>,
    pub status: HealthStatus,
}

/// Classification status summary for the system page.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassificationStatus {
    #[ts(type = "number")]
    pub classified_count: i64,
    #[ts(type = "number")]
    pub unclassified_count: i64,
    pub last_run_at: Option<String>,
    #[ts(type = "number | null")]
    pub last_run_duration_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub last_run_cost_cents: Option<i64>,
    pub provider: String,
    pub model: String,
    pub is_running: bool,
    #[ts(type = "number | null")]
    pub progress: Option<i64>,
}

/// Aggregate statistics overview for the API.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    #[ts(type = "number")]
    pub total_sessions: i64,
    #[ts(type = "number")]
    pub total_invocations: i64,
    #[ts(type = "number")]
    pub unique_invocables_used: i64,
    pub top_invocables: Vec<InvocableWithCount>,
}

impl Database {
    /// Get the oldest session date (Unix timestamp).
    pub async fn get_oldest_session_date(&self, project: Option<&str>, branch: Option<&str>) -> DbResult<Option<i64>> {
        let result: (Option<i64>,) = sqlx::query_as(
            "SELECT MIN(last_message_at) FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0 AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)",
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
                  (SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0),
                  (SELECT COUNT(DISTINCT project_id) FROM sessions WHERE is_sidechain = 0),
                  (SELECT COUNT(*) FROM session_commits),
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

        // Index and cache sizes are computed at the server layer via filesystem
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
            "SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0",
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
            "SELECT COUNT(DISTINCT project_id) FROM sessions",
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

    // ========================================================================
    // AI Generation Statistics (for dashboard AI generation breakdown)
    // ========================================================================

    /// Get AI generation statistics with optional time range filter.
    pub async fn get_ai_generation_stats(
        &self,
        from: Option<i64>,
        to: Option<i64>,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<AIGenerationStats> {
        let from = from.unwrap_or(1);
        let to = to.unwrap_or(i64::MAX);

        let (files_created, total_input_tokens, total_output_tokens): (i64, i64, i64) =
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(files_edited_count), 0),
                    COALESCE(SUM(total_input_tokens), 0),
                    COALESCE(SUM(total_output_tokens), 0)
                FROM sessions
                WHERE is_sidechain = 0
                  AND last_message_at >= ?1
                  AND last_message_at <= ?2
                  AND (?3 IS NULL OR project_id = ?3)
                  AND (?4 IS NULL OR git_branch = ?4)
                "#,
            )
            .bind(from)
            .bind(to)
            .bind(project)
            .bind(branch)
            .fetch_one(self.pool())
            .await?;

        let model_rows: Vec<(Option<String>, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                primary_model,
                COALESCE(SUM(total_input_tokens), 0) as input_tokens,
                COALESCE(SUM(total_output_tokens), 0) as output_tokens
            FROM sessions
            WHERE is_sidechain = 0
              AND last_message_at >= ?1
              AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3)
              AND (?4 IS NULL OR git_branch = ?4)
              AND primary_model IS NOT NULL
            GROUP BY primary_model
            ORDER BY (input_tokens + output_tokens) DESC
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let tokens_by_model: Vec<TokensByModel> = model_rows
            .into_iter()
            .filter_map(|(model, input_tokens, output_tokens)| {
                model.map(|m| TokensByModel {
                    model: m,
                    input_tokens,
                    output_tokens,
                })
            })
            .collect();

        let project_rows: Vec<(String, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                COALESCE(project_display_name, project_id) as project,
                COALESCE(SUM(total_input_tokens), 0) as input_tokens,
                COALESCE(SUM(total_output_tokens), 0) as output_tokens
            FROM sessions
            WHERE is_sidechain = 0
              AND last_message_at >= ?1
              AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3)
              AND (?4 IS NULL OR git_branch = ?4)
            GROUP BY project_id
            ORDER BY (input_tokens + output_tokens) DESC
            LIMIT 6
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let tokens_by_project: Vec<TokensByProject> = if project_rows.len() > 5 {
            let mut result: Vec<TokensByProject> = project_rows
                .iter()
                .take(5)
                .map(|(project, input_tokens, output_tokens)| TokensByProject {
                    project: project.clone(),
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                })
                .collect();

            let top5_input: i64 = result.iter().map(|p| p.input_tokens).sum();
            let top5_output: i64 = result.iter().map(|p| p.output_tokens).sum();
            let others_input = (total_input_tokens - top5_input).max(0);
            let others_output = (total_output_tokens - top5_output).max(0);

            if others_input > 0 || others_output > 0 {
                result.push(TokensByProject {
                    project: "Others".to_string(),
                    input_tokens: others_input,
                    output_tokens: others_output,
                });
            }
            result
        } else {
            project_rows
                .into_iter()
                .map(|(project, input_tokens, output_tokens)| TokensByProject {
                    project,
                    input_tokens,
                    output_tokens,
                })
                .collect()
        };

        Ok(AIGenerationStats {
            lines_added: 0,
            lines_removed: 0,
            files_created,
            total_input_tokens,
            total_output_tokens,
            tokens_by_model,
            tokens_by_project,
        })
    }
}

