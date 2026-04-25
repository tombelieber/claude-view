// crates/db/src/queries/sessions/listing.rs
// Session listing and lookup queries.

use crate::{Database, DbResult};
use chrono::Utc;
use claude_view_core::{ProjectInfo, SessionInfo};
use std::collections::HashMap;

use super::super::row_types::SessionRow;

impl Database {
    /// List all projects with their sessions, grouped by project_id.
    ///
    /// Sessions within each project are sorted by `last_message_at` DESC.
    /// `active_count` is calculated as sessions with `last_message_at` within
    /// the last 5 minutes (300 seconds).
    pub async fn list_projects(&self) -> DbResult<Vec<ProjectInfo>> {
        let now = Utc::now().timestamp();
        let active_threshold = now - 300;

        // All token/model data is denormalized on session_stats.
        // No LEFT JOIN on turns needed.
        //
        // CQRS Phase 5.5bc / D.3 — `category_*` and `classified_at` now
        // live on `session_flags`; reads join + alias to the legacy
        // column names so `SessionRow::from_row` keeps working. Timestamp
        // conversion ms→RFC3339 preserves the existing string shape.
        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT
                s.id, s.project_id, s.preview, s.turn_count,
                s.last_message_at, s.file_path,
                s.project_path, s.git_root, s.project_display_name,
                s.size_bytes, s.last_message, s.files_touched, s.skills_used,
                s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
                s.message_count,
                COALESCE(s.summary_text, s.summary) AS summary,
                s.git_branch, s.is_sidechain, s.deep_indexed_at,
                s.total_input_tokens,
                s.total_output_tokens,
                s.cache_read_tokens AS total_cache_read_tokens,
                s.cache_creation_tokens AS total_cache_creation_tokens,
                s.api_call_count AS turn_count_api,
                s.primary_model,
                s.user_prompt_count, s.api_call_count, s.tool_call_count,
                s.files_read, s.files_edited,
                s.files_read_count, s.files_edited_count, s.reedited_files_count,
                s.duration_seconds, s.first_message_at, s.commit_count,
                s.thinking_block_count, s.turn_duration_avg_ms, s.turn_duration_max_ms,
                s.api_error_count, s.compaction_count, s.agent_spawn_count,
                s.bash_progress_count, s.hook_progress_count, s.mcp_progress_count,
                s.lines_added, s.lines_removed, s.loc_source,
                s.summary_text, s.parse_version,
                sf.category_l1 AS category_l1,
                sf.category_l2 AS category_l2,
                sf.category_l3 AS category_l3,
                sf.category_confidence AS category_confidence,
                sf.category_source AS category_source,
                CASE
                    WHEN sf.classified_at IS NULL THEN NULL
                    ELSE strftime('%Y-%m-%dT%H:%M:%fZ', sf.classified_at / 1000.0, 'unixepoch')
                END AS classified_at,
                s.total_task_time_seconds, s.longest_task_seconds, s.longest_task_preview,
                s.total_cost_usd,
                s.slug,
                s.entrypoint
            FROM valid_sessions s
            LEFT JOIN session_flags sf ON sf.session_id = s.id
            ORDER BY s.last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        // Group rows by project_id
        let mut project_map: HashMap<String, Vec<SessionRow>> = HashMap::new();
        for row in rows {
            project_map
                .entry(row.project_id.clone())
                .or_default()
                .push(row);
        }

        let mut projects: Vec<ProjectInfo> = project_map
            .into_iter()
            .map(|(project_id, rows)| {
                let display_name = rows
                    .first()
                    .map(|r| r.project_display_name.clone())
                    .unwrap_or_default();
                let path = rows
                    .first()
                    .map(|r| r.project_path.clone())
                    .unwrap_or_default();

                let active_count = rows
                    .iter()
                    .filter(|r| r.last_message_at.unwrap_or(0) > active_threshold)
                    .count();

                let sessions: Vec<SessionInfo> = rows
                    .into_iter()
                    .map(|r| r.into_session_info(&project_id))
                    .collect();

                ProjectInfo {
                    name: project_id,
                    display_name,
                    path,
                    sessions,
                    active_count,
                }
            })
            .collect();

        // Sort projects by most recent session activity
        projects.sort_by(|a, b| {
            let a_latest = a.sessions.first().map(|s| s.modified_at).unwrap_or(0);
            let b_latest = b.sessions.first().map(|s| s.modified_at).unwrap_or(0);
            b_latest.cmp(&a_latest)
        });

        Ok(projects)
    }

    /// Look up a single session by its UUID.
    ///
    /// Used by the cost estimation endpoint to fetch session metadata
    /// (token counts, model, timestamps) for a specific session.
    ///
    /// CQRS Phase D.3 — category / classified fields join from
    /// `session_flags`; timestamp formatting matches the legacy RFC3339
    /// shape for API compatibility.
    pub async fn get_session_by_id(&self, id: &str) -> DbResult<Option<SessionInfo>> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"SELECT
                s.session_id AS id, s.project_id, s.preview, s.turn_count,
                s.last_message_at, s.file_path,
                s.project_path, s.git_root, s.project_display_name,
                s.size_bytes, s.last_message, s.files_touched, s.skills_used,
                s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
                s.message_count,
                COALESCE(s.summary_text, s.summary) AS summary,
                s.git_branch, s.is_sidechain, s.deep_indexed_at,
                s.total_input_tokens,
                s.total_output_tokens,
                s.cache_read_tokens AS total_cache_read_tokens,
                s.cache_creation_tokens AS total_cache_creation_tokens,
                s.api_call_count AS turn_count_api,
                s.primary_model,
                s.user_prompt_count, s.api_call_count, s.tool_call_count,
                s.files_read, s.files_edited,
                s.files_read_count, s.files_edited_count, s.reedited_files_count,
                s.duration_seconds, s.first_message_at, s.commit_count,
                s.thinking_block_count, s.turn_duration_avg_ms, s.turn_duration_max_ms,
                s.api_error_count, s.compaction_count, s.agent_spawn_count,
                s.bash_progress_count, s.hook_progress_count, s.mcp_progress_count,
                s.lines_added, s.lines_removed, s.loc_source,
                s.summary_text, s.parse_version,
                sf.category_l1 AS category_l1,
                sf.category_l2 AS category_l2,
                sf.category_l3 AS category_l3,
                sf.category_confidence AS category_confidence,
                sf.category_source AS category_source,
                CASE
                    WHEN sf.classified_at IS NULL THEN NULL
                    ELSE strftime('%Y-%m-%dT%H:%M:%fZ', sf.classified_at / 1000.0, 'unixepoch')
                END AS classified_at,
                s.total_task_time_seconds, s.longest_task_seconds, s.longest_task_preview,
                s.total_cost_usd,
                s.slug,
                s.entrypoint
            FROM session_stats s
            LEFT JOIN session_flags sf ON sf.session_id = s.session_id
            WHERE s.session_id = ?1"#,
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|r| {
            let pid = r.project_id.clone();
            r.into_session_info(&pid)
        }))
    }

    /// Look up a session's JSONL file path by session ID.
    ///
    /// Returns `None` if the session doesn't exist in the DB.
    /// The returned path is always absolute (set during indexing).
    pub async fn get_session_file_path(&self, session_id: &str) -> DbResult<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT file_path FROM session_stats WHERE session_id = ?1")
                .bind(session_id)
                .fetch_optional(self.pool())
                .await?;
        Ok(row.map(|(p,)| p))
    }

    /// Get all session IDs in the database (for backup dedup).
    pub async fn get_all_session_ids(&self) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT session_id FROM session_stats")
            .fetch_all(self.pool())
            .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Get all session file_paths from the database.
    ///
    /// Returns every non-empty `file_path` in `session_stats`.
    /// Used by the stale-session pruning step to check which files still exist on disk.
    pub async fn get_all_session_file_paths(&self) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT file_path FROM session_stats WHERE file_path IS NOT NULL AND file_path != ''",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|(path,)| path).collect())
    }
}
