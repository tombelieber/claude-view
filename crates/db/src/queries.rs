// crates/db/src/queries.rs
// Session CRUD operations for the vibe-recall SQLite database.

use crate::{Database, DbResult};
use chrono::Utc;
use std::collections::HashMap;
use ts_rs::TS;
use vibe_recall_core::{
    parse_model_id, DashboardStats, DayActivity, ProjectInfo, ProjectStat, ProjectSummary,
    RawTurn, SessionDurationStat, SessionInfo, SessionsPage, SkillStat, ToolCounts,
};

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
    /// Upsert a session into the database.
    ///
    /// Uses `INSERT ... ON CONFLICT DO UPDATE` to preserve columns not listed in the upsert.
    /// `project_encoded` is the URL-encoded project name (stored as `project_id`).
    /// `project_display_name` is the human-readable project name.
    pub async fn insert_session(
        &self,
        session: &SessionInfo,
        project_encoded: &str,
        project_display_name: &str,
    ) -> DbResult<()> {
        let files_touched = serde_json::to_string(&session.files_touched)
            .unwrap_or_else(|_| "[]".to_string());
        let skills_used = serde_json::to_string(&session.skills_used)
            .unwrap_or_else(|_| "[]".to_string());
        let files_read = serde_json::to_string(&session.files_read)
            .unwrap_or_else(|_| "[]".to_string());
        let files_edited = serde_json::to_string(&session.files_edited)
            .unwrap_or_else(|_| "[]".to_string());
        let indexed_at = Utc::now().timestamp();
        let size_bytes = session.size_bytes as i64;
        let message_count = session.message_count as i32;
        let turn_count = session.turn_count as i32;
        let tool_edit = session.tool_counts.edit as i32;
        let tool_read = session.tool_counts.read as i32;
        let tool_bash = session.tool_counts.bash as i32;
        let tool_write = session.tool_counts.write as i32;

        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, project_id, preview, turn_count,
                last_message_at, file_path,
                indexed_at, project_path, project_display_name,
                size_bytes, last_message, files_touched, skills_used,
                tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
                message_count,
                summary, git_branch, is_sidechain,
                user_prompt_count, api_call_count, tool_call_count,
                files_read, files_edited,
                files_read_count, files_edited_count, reedited_files_count,
                duration_seconds, commit_count
            ) VALUES (
                ?1, ?2, ?3, ?4,
                ?5, ?6,
                ?7, ?8, ?9,
                ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17,
                ?18,
                ?19, ?20, ?21,
                ?22, ?23, ?24,
                ?25, ?26,
                ?27, ?28, ?29,
                ?30, ?31
            )
            ON CONFLICT(id) DO UPDATE SET
                project_id = excluded.project_id,
                preview = excluded.preview,
                turn_count = excluded.turn_count,
                last_message_at = excluded.last_message_at,
                file_path = excluded.file_path,
                indexed_at = excluded.indexed_at,
                project_path = excluded.project_path,
                project_display_name = excluded.project_display_name,
                size_bytes = excluded.size_bytes,
                last_message = excluded.last_message,
                files_touched = excluded.files_touched,
                skills_used = excluded.skills_used,
                tool_counts_edit = excluded.tool_counts_edit,
                tool_counts_read = excluded.tool_counts_read,
                tool_counts_bash = excluded.tool_counts_bash,
                tool_counts_write = excluded.tool_counts_write,
                message_count = excluded.message_count,
                summary = excluded.summary,
                git_branch = excluded.git_branch,
                is_sidechain = excluded.is_sidechain,
                user_prompt_count = excluded.user_prompt_count,
                api_call_count = excluded.api_call_count,
                tool_call_count = excluded.tool_call_count,
                files_read = excluded.files_read,
                files_edited = excluded.files_edited,
                files_read_count = excluded.files_read_count,
                files_edited_count = excluded.files_edited_count,
                reedited_files_count = excluded.reedited_files_count,
                duration_seconds = excluded.duration_seconds,
                commit_count = excluded.commit_count
            "#,
        )
        .bind(&session.id)
        .bind(project_encoded)
        .bind(&session.preview)
        .bind(turn_count)
        .bind(session.modified_at)
        .bind(&session.file_path)
        .bind(indexed_at)
        .bind(&session.project_path)
        .bind(project_display_name)
        .bind(size_bytes)
        .bind(&session.last_message)
        .bind(&files_touched)
        .bind(&skills_used)
        .bind(tool_edit)
        .bind(tool_read)
        .bind(tool_bash)
        .bind(tool_write)
        .bind(message_count)
        .bind(&session.summary)
        .bind(&session.git_branch)
        .bind(session.is_sidechain)
        .bind(session.user_prompt_count as i32)
        .bind(session.api_call_count as i32)
        .bind(session.tool_call_count as i32)
        .bind(&files_read)
        .bind(&files_edited)
        .bind(session.files_read_count as i32)
        .bind(session.files_edited_count as i32)
        .bind(session.reedited_files_count as i32)
        .bind(session.duration_seconds as i32)
        .bind(session.commit_count as i32)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// List all projects with their sessions, grouped by project_id.
    ///
    /// Sessions within each project are sorted by `last_message_at` DESC.
    /// `active_count` is calculated as sessions with `last_message_at` within
    /// the last 5 minutes (300 seconds).
    pub async fn list_projects(&self) -> DbResult<Vec<ProjectInfo>> {
        let now = Utc::now().timestamp();
        let active_threshold = now - 300;

        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT
                s.id, s.project_id, s.preview, s.turn_count,
                s.last_message_at, s.file_path,
                s.project_path, s.project_display_name,
                s.size_bytes, s.last_message, s.files_touched, s.skills_used,
                s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
                s.message_count,
                s.summary, s.git_branch, s.is_sidechain, s.deep_indexed_at,
                tok.total_input_tokens,
                tok.total_output_tokens,
                tok.total_cache_read_tokens,
                tok.total_cache_creation_tokens,
                tok.turn_count_api,
                tok.primary_model,
                s.user_prompt_count, s.api_call_count, s.tool_call_count,
                s.files_read, s.files_edited,
                s.files_read_count, s.files_edited_count, s.reedited_files_count,
                s.duration_seconds, s.first_message_at, s.commit_count,
                s.thinking_block_count, s.turn_duration_avg_ms, s.turn_duration_max_ms,
                s.api_error_count, s.compaction_count, s.agent_spawn_count,
                s.bash_progress_count, s.hook_progress_count, s.mcp_progress_count,
                s.summary_text, s.parse_version,
                s.category_l1, s.category_l2, s.category_l3,
                s.category_confidence, s.category_source, s.classified_at,
                s.prompt_word_count, s.correction_count, s.same_file_edit_count
            FROM sessions s
            LEFT JOIN (
                SELECT session_id,
                       SUM(input_tokens) as total_input_tokens,
                       SUM(output_tokens) as total_output_tokens,
                       SUM(cache_read_tokens) as total_cache_read_tokens,
                       SUM(cache_creation_tokens) as total_cache_creation_tokens,
                       COUNT(*) as turn_count_api,
                       (SELECT model_id FROM turns t2
                        WHERE t2.session_id = t.session_id
                        GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 1
                       ) as primary_model
                FROM turns t
                GROUP BY session_id
            ) tok ON tok.session_id = s.id
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

    /// Check if a file needs re-indexing by retrieving its indexer state.
    pub async fn get_indexer_state(&self, file_path: &str) -> DbResult<Option<IndexerEntry>> {
        let row: Option<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT file_path, file_size, modified_at, indexed_at FROM indexer_state WHERE file_path = ?1",
        )
        .bind(file_path)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|(file_path, file_size, modified_at, indexed_at)| IndexerEntry {
            file_path,
            file_size,
            modified_at,
            indexed_at,
        }))
    }

    /// Batch-load all indexer states into a HashMap keyed by file_path.
    ///
    /// This avoids the N+1 query pattern when diffing many files against the DB.
    pub async fn get_all_indexer_states(&self) -> DbResult<HashMap<String, IndexerEntry>> {
        let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT file_path, file_size, modified_at, indexed_at FROM indexer_state",
        )
        .fetch_all(self.pool())
        .await?;

        let map = rows
            .into_iter()
            .map(|(file_path, file_size, modified_at, indexed_at)| {
                let entry = IndexerEntry {
                    file_path: file_path.clone(),
                    file_size,
                    modified_at,
                    indexed_at,
                };
                (file_path, entry)
            })
            .collect();

        Ok(map)
    }

    /// Mark a file as indexed with the given size and modification time.
    pub async fn update_indexer_state(
        &self,
        file_path: &str,
        size: i64,
        mtime: i64,
    ) -> DbResult<()> {
        let indexed_at = Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO indexer_state (file_path, file_size, modified_at, indexed_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(file_path)
        .bind(size)
        .bind(mtime)
        .bind(indexed_at)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Insert a lightweight session from sessions-index.json (Pass 1).
    ///
    /// Inserts Pass 1 fields. On conflict, updates only the Pass 1 fields
    /// and does NOT overwrite Pass 2 fields (tool_counts, files_touched, etc.)
    /// if they already have data.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_session_from_index(
        &self,
        id: &str,
        project_encoded: &str,
        project_display_name: &str,
        project_path: &str,
        file_path: &str,
        preview: &str,
        summary: Option<&str>,
        message_count: i32,
        modified_at: i64,
        git_branch: Option<&str>,
        is_sidechain: bool,
        size_bytes: i64,
    ) -> DbResult<()> {
        let indexed_at = Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, project_id, project_display_name, project_path,
                file_path, preview, summary, message_count,
                last_message_at, git_branch, is_sidechain,
                size_bytes, indexed_at,
                last_message, files_touched, skills_used,
                tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
                turn_count
            ) VALUES (
                ?1, ?2, ?3, ?4,
                ?5, ?6, ?7, ?8,
                ?9, ?10, ?11,
                ?12, ?13,
                '', '[]', '[]',
                0, 0, 0, 0,
                0
            )
            ON CONFLICT(id) DO UPDATE SET
                project_id = excluded.project_id,
                project_display_name = excluded.project_display_name,
                project_path = excluded.project_path,
                file_path = excluded.file_path,
                preview = excluded.preview,
                summary = excluded.summary,
                message_count = excluded.message_count,
                last_message_at = excluded.last_message_at,
                git_branch = excluded.git_branch,
                is_sidechain = excluded.is_sidechain,
                size_bytes = excluded.size_bytes,
                indexed_at = excluded.indexed_at
            "#,
        )
        .bind(id)
        .bind(project_encoded)
        .bind(project_display_name)
        .bind(project_path)
        .bind(file_path)
        .bind(preview)
        .bind(summary)
        .bind(message_count)
        .bind(modified_at)
        .bind(git_branch)
        .bind(is_sidechain)
        .bind(size_bytes)
        .bind(indexed_at)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Update extended metadata fields from Pass 2 deep indexing.
    ///
    /// Sets `deep_indexed_at` to the current timestamp to mark the session
    /// as having been fully indexed. Includes all Phase 3 atomic unit metrics.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_session_deep_fields(
        &self,
        id: &str,
        last_message: &str,
        turn_count: i32,
        tool_edit: i32,
        tool_read: i32,
        tool_bash: i32,
        tool_write: i32,
        files_touched: &str,
        skills_used: &str,
        // Phase 3: Atomic unit metrics
        user_prompt_count: i32,
        api_call_count: i32,
        tool_call_count: i32,
        files_read: &str,
        files_edited: &str,
        files_read_count: i32,
        files_edited_count: i32,
        reedited_files_count: i32,
        duration_seconds: i32,
        commit_count: i32,
        first_message_at: Option<i64>,
        // Full parser metrics (Phase 3.5)
        total_input_tokens: i64,
        total_output_tokens: i64,
        cache_read_tokens: i64,
        cache_creation_tokens: i64,
        thinking_block_count: i32,
        turn_duration_avg_ms: Option<i64>,
        turn_duration_max_ms: Option<i64>,
        turn_duration_total_ms: Option<i64>,
        api_error_count: i32,
        api_retry_count: i32,
        compaction_count: i32,
        hook_blocked_count: i32,
        agent_spawn_count: i32,
        bash_progress_count: i32,
        hook_progress_count: i32,
        mcp_progress_count: i32,
        summary_text: Option<&str>,
        parse_version: i32,
        file_size: i64,
        file_mtime: i64,
    ) -> DbResult<()> {
        let deep_indexed_at = Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE sessions SET
                last_message = ?2,
                turn_count = ?3,
                tool_counts_edit = ?4,
                tool_counts_read = ?5,
                tool_counts_bash = ?6,
                tool_counts_write = ?7,
                files_touched = ?8,
                skills_used = ?9,
                deep_indexed_at = ?10,
                user_prompt_count = ?11,
                api_call_count = ?12,
                tool_call_count = ?13,
                files_read = ?14,
                files_edited = ?15,
                files_read_count = ?16,
                files_edited_count = ?17,
                reedited_files_count = ?18,
                duration_seconds = ?19,
                commit_count = ?20,
                first_message_at = ?21,
                total_input_tokens = ?22,
                total_output_tokens = ?23,
                cache_read_tokens = ?24,
                cache_creation_tokens = ?25,
                thinking_block_count = ?26,
                turn_duration_avg_ms = ?27,
                turn_duration_max_ms = ?28,
                turn_duration_total_ms = ?29,
                api_error_count = ?30,
                api_retry_count = ?31,
                compaction_count = ?32,
                hook_blocked_count = ?33,
                agent_spawn_count = ?34,
                bash_progress_count = ?35,
                hook_progress_count = ?36,
                mcp_progress_count = ?37,
                summary_text = ?38,
                parse_version = ?39,
                file_size_at_index = ?40,
                file_mtime_at_index = ?41
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(last_message)
        .bind(turn_count)
        .bind(tool_edit)
        .bind(tool_read)
        .bind(tool_bash)
        .bind(tool_write)
        .bind(files_touched)
        .bind(skills_used)
        .bind(deep_indexed_at)
        .bind(user_prompt_count)
        .bind(api_call_count)
        .bind(tool_call_count)
        .bind(files_read)
        .bind(files_edited)
        .bind(files_read_count)
        .bind(files_edited_count)
        .bind(reedited_files_count)
        .bind(duration_seconds)
        .bind(commit_count)
        .bind(first_message_at)
        .bind(total_input_tokens)
        .bind(total_output_tokens)
        .bind(cache_read_tokens)
        .bind(cache_creation_tokens)
        .bind(thinking_block_count)
        .bind(turn_duration_avg_ms)
        .bind(turn_duration_max_ms)
        .bind(turn_duration_total_ms)
        .bind(api_error_count)
        .bind(api_retry_count)
        .bind(compaction_count)
        .bind(hook_blocked_count)
        .bind(agent_spawn_count)
        .bind(bash_progress_count)
        .bind(hook_progress_count)
        .bind(mcp_progress_count)
        .bind(summary_text)
        .bind(parse_version)
        .bind(file_size)
        .bind(file_mtime)
        .execute(self.pool())
        .await?;

        Ok(())
    }
    /// Get all sessions with their file paths and stored file metadata.
    ///
    /// Returns all sessions so the caller can decide which ones need re-indexing
    /// based on: (1) never deep-indexed, (2) stale parse version, (3) file changed
    /// since last index (size or mtime differs).
    ///
    /// Tuple: `(id, file_path, file_size_at_index, file_mtime_at_index, deep_indexed_at, parse_version)`
    pub async fn get_sessions_needing_deep_index(
        &self,
    ) -> DbResult<Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, i32)>> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, i32)> =
            sqlx::query_as(
                "SELECT id, file_path, file_size_at_index, file_mtime_at_index, deep_indexed_at, parse_version FROM sessions WHERE file_path IS NOT NULL AND file_path != ''",
            )
            .fetch_all(self.pool())
            .await?;
        Ok(rows)
    }

    // ========================================================================
    // Invocable + Invocation CRUD
    // ========================================================================

    /// Insert or update a single invocable.
    ///
    /// Uses `INSERT ... ON CONFLICT(id) DO UPDATE SET` to upsert.
    pub async fn upsert_invocable(
        &self,
        id: &str,
        plugin_name: Option<&str>,
        name: &str,
        kind: &str,
        description: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO invocables (id, plugin_name, name, kind, description, status)
            VALUES (?1, ?2, ?3, ?4, ?5, 'enabled')
            ON CONFLICT(id) DO UPDATE SET
                plugin_name = excluded.plugin_name,
                name = excluded.name,
                kind = excluded.kind,
                description = excluded.description
            "#,
        )
        .bind(id)
        .bind(plugin_name)
        .bind(name)
        .bind(kind)
        .bind(description)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Batch insert invocations in a single transaction.
    ///
    /// Each tuple is `(source_file, byte_offset, invocable_id, session_id, project, timestamp)`.
    /// Uses `INSERT OR IGNORE` so re-indexing skips duplicates (PK is source_file + byte_offset).
    /// Returns the number of rows actually inserted.
    pub async fn batch_insert_invocations(
        &self,
        invocations: &[(String, i64, String, String, String, i64)],
    ) -> DbResult<u64> {
        if invocations.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        let inserted = batch_insert_invocations_tx(&mut tx, invocations).await?;
        tx.commit().await?;
        Ok(inserted)
    }

    /// List all invocables with their invocation counts.
    ///
    /// Results are ordered by invocation_count DESC, then name ASC.
    pub async fn list_invocables_with_counts(&self) -> DbResult<Vec<InvocableWithCount>> {
        let rows: Vec<InvocableWithCount> = sqlx::query_as(
            r#"
            SELECT
                i.id, i.plugin_name, i.name, i.kind, i.description,
                COALESCE(COUNT(inv.invocable_id), 0) as invocation_count,
                MAX(inv.timestamp) as last_used_at
            FROM invocables i
            LEFT JOIN invocations inv ON i.id = inv.invocable_id
            GROUP BY i.id
            ORDER BY invocation_count DESC, i.name ASC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    /// Batch insert/update invocables from a registry snapshot.
    ///
    /// Each tuple is `(id, plugin_name, name, kind, description)`.
    /// Uses `INSERT ... ON CONFLICT(id) DO UPDATE SET` for upsert semantics.
    /// Returns the number of rows affected.
    pub async fn batch_upsert_invocables(
        &self,
        invocables: &[(String, Option<String>, String, String, String)],
    ) -> DbResult<u64> {
        let mut tx = self.pool().begin().await?;
        let mut affected: u64 = 0;

        for (id, plugin_name, name, kind, description) in invocables {
            let result = sqlx::query(
                r#"
                INSERT INTO invocables (id, plugin_name, name, kind, description, status)
                VALUES (?1, ?2, ?3, ?4, ?5, 'enabled')
                ON CONFLICT(id) DO UPDATE SET
                    plugin_name = excluded.plugin_name,
                    name = excluded.name,
                    kind = excluded.kind,
                    description = excluded.description
                "#,
            )
            .bind(id)
            .bind(plugin_name)
            .bind(name)
            .bind(kind)
            .bind(description)
            .execute(&mut *tx)
            .await?;

            affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(affected)
    }

    /// Get aggregate statistics overview.
    ///
    /// Returns total sessions, total invocations, unique invocables used,
    /// and the top 10 invocables by usage count.
    pub async fn get_stats_overview(&self) -> DbResult<StatsOverview> {
        let (total_sessions,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions")
                .fetch_one(self.pool())
                .await?;

        let (total_invocations,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invocations")
                .fetch_one(self.pool())
                .await?;

        let (unique_invocables_used,): (i64,) =
            sqlx::query_as("SELECT COUNT(DISTINCT invocable_id) FROM invocations")
                .fetch_one(self.pool())
                .await?;

        let all = self.list_invocables_with_counts().await?;
        let top_invocables: Vec<InvocableWithCount> = all.into_iter().take(10).collect();

        Ok(StatsOverview {
            total_sessions,
            total_invocations,
            unique_invocables_used,
            top_invocables,
        })
    }

    // ========================================================================
    // Model + Turn CRUD (Phase 2B)
    // ========================================================================

    /// Batch upsert models: INSERT OR IGNORE + UPDATE last_seen.
    ///
    /// Each `model_id` is parsed via `parse_model_id()` to derive provider/family.
    /// `seen_at` is the unix timestamp when the model was observed.
    pub async fn batch_upsert_models(
        &self,
        model_ids: &[String],
        seen_at: i64,
    ) -> DbResult<u64> {
        if model_ids.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        let affected = batch_upsert_models_tx(&mut tx, model_ids, seen_at).await?;
        tx.commit().await?;
        Ok(affected)
    }

    /// Batch insert turns using INSERT OR IGNORE (UUID PK = free dedup on re-index).
    ///
    /// Returns the number of rows actually inserted.
    pub async fn batch_insert_turns(
        &self,
        session_id: &str,
        turns: &[RawTurn],
    ) -> DbResult<u64> {
        if turns.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        let inserted = batch_insert_turns_tx(&mut tx, session_id, turns).await?;
        tx.commit().await?;
        Ok(inserted)
    }

    /// Get all models with usage counts (for GET /api/models).
    pub async fn get_all_models(&self) -> DbResult<Vec<ModelWithStats>> {
        let rows: Vec<ModelWithStats> = sqlx::query_as(
            r#"
            SELECT m.id, m.provider, m.family, m.first_seen, m.last_seen,
                   COUNT(t.uuid) as total_turns,
                   COUNT(DISTINCT t.session_id) as total_sessions
            FROM models m
            LEFT JOIN turns t ON t.model_id = m.id
            GROUP BY m.id
            ORDER BY total_turns DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    /// Get aggregate token statistics (for GET /api/stats/tokens).
    pub async fn get_token_stats(&self) -> DbResult<TokenStats> {
        let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_creation_tokens), 0),
                COUNT(*),
                COUNT(DISTINCT session_id)
            FROM turns
            "#,
        )
        .fetch_one(self.pool())
        .await?;

        let total_input = row.0 as u64;
        let total_cache_read = row.2 as u64;
        let total_cache_creation = row.3 as u64;
        let denominator = total_input + total_cache_creation;
        let cache_hit_ratio = if denominator > 0 {
            total_cache_read as f64 / denominator as f64
        } else {
            0.0
        };

        Ok(TokenStats {
            total_input_tokens: total_input,
            total_output_tokens: row.1 as u64,
            total_cache_read_tokens: total_cache_read,
            total_cache_creation_tokens: total_cache_creation,
            cache_hit_ratio,
            turns_count: row.4 as u64,
            sessions_count: row.5 as u64,
        })
    }

    // ========================================================================
    // Phase 2C: Project Summaries, Paginated Sessions, Dashboard Stats
    // ========================================================================

    /// List lightweight project summaries (no sessions array).
    /// Returns ProjectSummary with counts only — sidebar payload.
    pub async fn list_project_summaries(&self) -> DbResult<Vec<ProjectSummary>> {
        let now = Utc::now().timestamp();
        let active_threshold = now - 300; // 5 minutes

        let rows: Vec<(String, String, String, i64, i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT
                project_id,
                COALESCE(project_display_name, project_id),
                COALESCE(project_path, ''),
                COUNT(*) as session_count,
                SUM(CASE WHEN last_message_at > ?1 THEN 1 ELSE 0 END) as active_count,
                MAX(last_message_at) as last_activity_at
            FROM sessions
            WHERE is_sidechain = 0
            GROUP BY project_id
            ORDER BY last_activity_at DESC
            "#,
        )
        .bind(active_threshold)
        .fetch_all(self.pool())
        .await?;

        let summaries = rows
            .into_iter()
            .map(|(name, display_name, path, session_count, active_count, last_activity_at)| {
                ProjectSummary {
                    name,
                    display_name,
                    path,
                    session_count: session_count as usize,
                    active_count: active_count as usize,
                    last_activity_at,
                }
            })
            .collect();

        Ok(summaries)
    }

    /// List paginated sessions for a specific project.
    ///
    /// Supports sorting (recent, oldest, messages), branch filtering,
    /// and sidechain inclusion.
    pub async fn list_sessions_for_project(
        &self,
        project_id: &str,
        limit: i64,
        offset: i64,
        sort: &str,
        branch: Option<&str>,
        include_sidechains: bool,
    ) -> DbResult<SessionsPage> {
        // Build WHERE clause dynamically
        let mut conditions = vec!["s.project_id = ?1".to_string()];
        if !include_sidechains {
            conditions.push("s.is_sidechain = 0".to_string());
        }
        if branch.is_some() {
            conditions.push("s.git_branch = ?4".to_string());
        }

        let where_clause = conditions.join(" AND ");

        let order_clause = match sort {
            "oldest" => "s.last_message_at ASC",
            "messages" => "s.message_count DESC",
            _ => "s.last_message_at DESC", // "recent" is default
        };

        // Count total matching sessions
        let count_sql = format!(
            "SELECT COUNT(*) FROM sessions s WHERE {}",
            where_clause
        );
        let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql)
            .bind(project_id);
        if let Some(b) = branch {
            count_query = count_query.bind(b);
        }
        let (total,) = count_query.fetch_one(self.pool()).await?;

        // Fetch paginated sessions with token LEFT JOIN
        let select_sql = format!(
            r#"
            SELECT
                s.id, s.project_id, s.preview, s.turn_count,
                s.last_message_at, s.file_path,
                s.project_path, s.project_display_name,
                s.size_bytes, s.last_message, s.files_touched, s.skills_used,
                s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
                s.message_count,
                s.summary, s.git_branch, s.is_sidechain, s.deep_indexed_at,
                tok.total_input_tokens,
                tok.total_output_tokens,
                tok.total_cache_read_tokens,
                tok.total_cache_creation_tokens,
                tok.turn_count_api,
                tok.primary_model,
                s.user_prompt_count, s.api_call_count, s.tool_call_count,
                s.files_read, s.files_edited,
                s.files_read_count, s.files_edited_count, s.reedited_files_count,
                s.duration_seconds, s.first_message_at, s.commit_count,
                s.thinking_block_count, s.turn_duration_avg_ms, s.turn_duration_max_ms,
                s.api_error_count, s.compaction_count, s.agent_spawn_count,
                s.bash_progress_count, s.hook_progress_count, s.mcp_progress_count,
                s.summary_text, s.parse_version,
                s.category_l1, s.category_l2, s.category_l3,
                s.category_confidence, s.category_source, s.classified_at,
                s.prompt_word_count, s.correction_count, s.same_file_edit_count
            FROM sessions s
            LEFT JOIN (
                SELECT session_id,
                       SUM(input_tokens) as total_input_tokens,
                       SUM(output_tokens) as total_output_tokens,
                       SUM(cache_read_tokens) as total_cache_read_tokens,
                       SUM(cache_creation_tokens) as total_cache_creation_tokens,
                       COUNT(*) as turn_count_api,
                       (SELECT model_id FROM turns t2
                        WHERE t2.session_id = t.session_id
                        GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 1
                       ) as primary_model
                FROM turns t
                GROUP BY session_id
            ) tok ON tok.session_id = s.id
            WHERE {}
            ORDER BY {}
            LIMIT ?2 OFFSET ?3
            "#,
            where_clause, order_clause
        );

        let mut query = sqlx::query_as::<_, SessionRow>(&select_sql)
            .bind(project_id)
            .bind(limit)
            .bind(offset);
        if let Some(b) = branch {
            query = query.bind(b);
        }

        let rows: Vec<SessionRow> = query.fetch_all(self.pool()).await?;

        let sessions: Vec<SessionInfo> = rows
            .into_iter()
            .map(|r| {
                let pid = project_id.to_string();
                r.into_session_info(&pid)
            })
            .collect();

        Ok(SessionsPage {
            sessions,
            total: total as usize,
        })
    }

    /// Fetch top 10 invocables by kind from the invocations table.
    async fn top_invocables_by_kind(&self, kind: &str) -> DbResult<Vec<SkillStat>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT inv.name, COUNT(*) as cnt
            FROM invocations i
            JOIN invocables inv ON i.invocable_id = inv.id
            WHERE inv.kind = ?1
            GROUP BY inv.name
            ORDER BY cnt DESC
            LIMIT 10
            "#,
        )
        .bind(kind)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(name, count)| SkillStat {
                name,
                count: count as usize,
            })
            .collect())
    }

    /// Get pre-computed dashboard statistics.
    ///
    /// Returns heatmap (90 days), top 10 invocables per kind, top 5 projects, tool totals.
    pub async fn get_dashboard_stats(&self) -> DbResult<DashboardStats> {
        // Total sessions and projects
        let (total_sessions,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0")
                .fetch_one(self.pool())
                .await?;

        let (total_projects,): (i64,) =
            sqlx::query_as("SELECT COUNT(DISTINCT project_id) FROM sessions WHERE is_sidechain = 0")
                .fetch_one(self.pool())
                .await?;

        // Heatmap: 90-day activity (sessions per day)
        let now = Utc::now().timestamp();
        let ninety_days_ago = now - (90 * 86400);
        let heatmap_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT date(last_message_at, 'unixepoch') as day, COUNT(*) as cnt
            FROM sessions
            WHERE last_message_at >= ?1 AND is_sidechain = 0
            GROUP BY day
            ORDER BY day ASC
            "#,
        )
        .bind(ninety_days_ago)
        .fetch_all(self.pool())
        .await?;

        let heatmap: Vec<DayActivity> = heatmap_rows
            .into_iter()
            .map(|(date, count)| DayActivity {
                date,
                count: count as usize,
            })
            .collect();

        // Top invocables by kind (from Phase 2A-2 invocations table)
        let top_skills = self.top_invocables_by_kind("skill").await?;
        let top_commands = self.top_invocables_by_kind("command").await?;
        let top_mcp_tools = self.top_invocables_by_kind("mcp_tool").await?;
        let top_agents = self.top_invocables_by_kind("agent").await?;

        // Top 5 projects by session count
        let project_rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT project_id, COALESCE(project_display_name, project_id), COUNT(*) as cnt
            FROM sessions
            WHERE is_sidechain = 0
            GROUP BY project_id
            ORDER BY cnt DESC
            LIMIT 5
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        let top_projects: Vec<ProjectStat> = project_rows
            .into_iter()
            .map(|(name, display_name, session_count)| ProjectStat {
                name,
                display_name,
                session_count: session_count as usize,
            })
            .collect();

        // Top 5 longest sessions by duration
        let longest_rows: Vec<(String, String, String, String, i32)> = sqlx::query_as(
            r#"
            SELECT id, preview, project_id, COALESCE(project_display_name, project_id), duration_seconds
            FROM sessions
            WHERE is_sidechain = 0 AND duration_seconds > 0
            ORDER BY duration_seconds DESC
            LIMIT 5
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        let longest_sessions: Vec<SessionDurationStat> = longest_rows
            .into_iter()
            .map(|(id, preview, project_name, project_display_name, duration_seconds)| {
                SessionDurationStat {
                    id,
                    preview,
                    project_name,
                    project_display_name,
                    duration_seconds: duration_seconds as u32,
                }
            })
            .collect();

        // Tool totals (aggregate across all non-sidechain sessions)
        let (edit, read, bash, write): (i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(tool_counts_edit), 0),
                COALESCE(SUM(tool_counts_read), 0),
                COALESCE(SUM(tool_counts_bash), 0),
                COALESCE(SUM(tool_counts_write), 0)
            FROM sessions
            WHERE is_sidechain = 0
            "#,
        )
        .fetch_one(self.pool())
        .await?;

        Ok(DashboardStats {
            total_sessions: total_sessions as usize,
            total_projects: total_projects as usize,
            heatmap,
            top_skills,
            top_commands,
            top_mcp_tools,
            top_agents,
            top_projects,
            tool_totals: ToolCounts {
                edit: edit as usize,
                read: read as usize,
                bash: bash as usize,
                write: write as usize,
            },
            longest_sessions,
        })
    }

    /// Remove sessions whose file_path is NOT in the given list of valid paths.
    /// Also cleans up corresponding indexer_state entries.
    /// Both deletes run in a transaction for consistency.
    pub async fn remove_stale_sessions(&self, valid_paths: &[String]) -> DbResult<u64> {
        let mut tx = self.pool().begin().await?;

        if valid_paths.is_empty() {
            let result = sqlx::query("DELETE FROM sessions")
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM indexer_state")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(result.rows_affected());
        }

        // Build placeholders for the IN clause
        let placeholders: Vec<String> = (1..=valid_paths.len()).map(|i| format!("?{}", i)).collect();
        let in_clause = placeholders.join(", ");

        let delete_sessions_sql = format!(
            "DELETE FROM sessions WHERE file_path NOT IN ({})",
            in_clause
        );
        let delete_indexer_sql = format!(
            "DELETE FROM indexer_state WHERE file_path NOT IN ({})",
            in_clause
        );

        let mut query = sqlx::query(&delete_sessions_sql);
        for path in valid_paths {
            query = query.bind(path);
        }
        let result = query.execute(&mut *tx).await?;

        let mut query = sqlx::query(&delete_indexer_sql);
        for path in valid_paths {
            query = query.bind(path);
        }
        query.execute(&mut *tx).await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }

    // ========================================================================
    // Theme 4: Classification Job CRUD
    // ========================================================================

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

    // ========================================================================
    // Theme 4: Index Run CRUD
    // ========================================================================

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

    /// Update session classification fields.
    pub async fn update_session_classification(
        &self,
        session_id: &str,
        category_l1: &str,
        category_l2: &str,
        category_l3: &str,
        confidence: f64,
        source: &str,
    ) -> DbResult<()> {
        let classified_at = Utc::now().to_rfc3339();
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
        .bind(category_l1)
        .bind(category_l2)
        .bind(category_l3)
        .bind(confidence)
        .bind(source)
        .bind(&classified_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}

// ============================================================================
// Theme 4: Internal row types for classification_jobs and index_runs
// ============================================================================

#[derive(Debug)]
struct ClassificationJobRow {
    id: i64,
    started_at: String,
    completed_at: Option<String>,
    total_sessions: i64,
    classified_count: i64,
    skipped_count: i64,
    failed_count: i64,
    provider: String,
    model: String,
    status: String,
    error_message: Option<String>,
    cost_estimate_cents: Option<i64>,
    actual_cost_cents: Option<i64>,
    tokens_used: Option<i64>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for ClassificationJobRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
            total_sessions: row.try_get("total_sessions")?,
            classified_count: row.try_get("classified_count")?,
            skipped_count: row.try_get("skipped_count")?,
            failed_count: row.try_get("failed_count")?,
            provider: row.try_get("provider")?,
            model: row.try_get("model")?,
            status: row.try_get("status")?,
            error_message: row.try_get("error_message")?,
            cost_estimate_cents: row.try_get("cost_estimate_cents")?,
            actual_cost_cents: row.try_get("actual_cost_cents")?,
            tokens_used: row.try_get("tokens_used")?,
        })
    }
}

impl ClassificationJobRow {
    fn into_classification_job(self) -> vibe_recall_core::ClassificationJob {
        vibe_recall_core::ClassificationJob {
            id: self.id,
            started_at: self.started_at,
            completed_at: self.completed_at,
            total_sessions: self.total_sessions,
            classified_count: self.classified_count,
            skipped_count: self.skipped_count,
            failed_count: self.failed_count,
            provider: self.provider,
            model: self.model,
            status: vibe_recall_core::ClassificationJobStatus::from_db_str(&self.status),
            error_message: self.error_message,
            cost_estimate_cents: self.cost_estimate_cents,
            actual_cost_cents: self.actual_cost_cents,
            tokens_used: self.tokens_used,
        }
    }
}

#[derive(Debug)]
struct IndexRunRow {
    id: i64,
    started_at: String,
    completed_at: Option<String>,
    run_type: String,
    sessions_before: Option<i64>,
    sessions_after: Option<i64>,
    duration_ms: Option<i64>,
    throughput_mb_per_sec: Option<f64>,
    status: String,
    error_message: Option<String>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for IndexRunRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
            run_type: row.try_get("type")?,
            sessions_before: row.try_get("sessions_before")?,
            sessions_after: row.try_get("sessions_after")?,
            duration_ms: row.try_get("duration_ms")?,
            throughput_mb_per_sec: row.try_get("throughput_mb_per_sec")?,
            status: row.try_get("status")?,
            error_message: row.try_get("error_message")?,
        })
    }
}

impl IndexRunRow {
    fn into_index_run(self) -> vibe_recall_core::IndexRun {
        vibe_recall_core::IndexRun {
            id: self.id,
            started_at: self.started_at,
            completed_at: self.completed_at,
            run_type: vibe_recall_core::IndexRunType::from_db_str(&self.run_type),
            sessions_before: self.sessions_before,
            sessions_after: self.sessions_after,
            duration_ms: self.duration_ms,
            throughput_mb_per_sec: self.throughput_mb_per_sec,
            status: vibe_recall_core::IndexRunStatus::from_db_str(&self.status),
            error_message: self.error_message,
        }
    }
}

// ============================================================================
// Transaction-accepting variants for batch writes (collect-then-write pattern)
// ============================================================================

/// Update extended metadata fields within an existing transaction.
///
/// Same SQL as `Database::update_session_deep_fields` but executes on the
/// provided transaction instead of acquiring a new connection from the pool.
#[allow(clippy::too_many_arguments)]
pub async fn update_session_deep_fields_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
    last_message: &str,
    turn_count: i32,
    tool_edit: i32,
    tool_read: i32,
    tool_bash: i32,
    tool_write: i32,
    files_touched: &str,
    skills_used: &str,
    user_prompt_count: i32,
    api_call_count: i32,
    tool_call_count: i32,
    files_read: &str,
    files_edited: &str,
    files_read_count: i32,
    files_edited_count: i32,
    reedited_files_count: i32,
    duration_seconds: i32,
    commit_count: i32,
    first_message_at: Option<i64>,
    total_input_tokens: i64,
    total_output_tokens: i64,
    cache_read_tokens: i64,
    cache_creation_tokens: i64,
    thinking_block_count: i32,
    turn_duration_avg_ms: Option<i64>,
    turn_duration_max_ms: Option<i64>,
    turn_duration_total_ms: Option<i64>,
    api_error_count: i32,
    api_retry_count: i32,
    compaction_count: i32,
    hook_blocked_count: i32,
    agent_spawn_count: i32,
    bash_progress_count: i32,
    hook_progress_count: i32,
    mcp_progress_count: i32,
    summary_text: Option<&str>,
    parse_version: i32,
    file_size: i64,
    file_mtime: i64,
) -> DbResult<()> {
    let deep_indexed_at = Utc::now().timestamp();

    sqlx::query(
        r#"
        UPDATE sessions SET
            last_message = ?2,
            turn_count = ?3,
            tool_counts_edit = ?4,
            tool_counts_read = ?5,
            tool_counts_bash = ?6,
            tool_counts_write = ?7,
            files_touched = ?8,
            skills_used = ?9,
            deep_indexed_at = ?10,
            user_prompt_count = ?11,
            api_call_count = ?12,
            tool_call_count = ?13,
            files_read = ?14,
            files_edited = ?15,
            files_read_count = ?16,
            files_edited_count = ?17,
            reedited_files_count = ?18,
            duration_seconds = ?19,
            commit_count = ?20,
            first_message_at = ?21,
            total_input_tokens = ?22,
            total_output_tokens = ?23,
            cache_read_tokens = ?24,
            cache_creation_tokens = ?25,
            thinking_block_count = ?26,
            turn_duration_avg_ms = ?27,
            turn_duration_max_ms = ?28,
            turn_duration_total_ms = ?29,
            api_error_count = ?30,
            api_retry_count = ?31,
            compaction_count = ?32,
            hook_blocked_count = ?33,
            agent_spawn_count = ?34,
            bash_progress_count = ?35,
            hook_progress_count = ?36,
            mcp_progress_count = ?37,
            summary_text = ?38,
            parse_version = ?39,
            file_size_at_index = ?40,
            file_mtime_at_index = ?41
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .bind(last_message)
    .bind(turn_count)
    .bind(tool_edit)
    .bind(tool_read)
    .bind(tool_bash)
    .bind(tool_write)
    .bind(files_touched)
    .bind(skills_used)
    .bind(deep_indexed_at)
    .bind(user_prompt_count)
    .bind(api_call_count)
    .bind(tool_call_count)
    .bind(files_read)
    .bind(files_edited)
    .bind(files_read_count)
    .bind(files_edited_count)
    .bind(reedited_files_count)
    .bind(duration_seconds)
    .bind(commit_count)
    .bind(first_message_at)
    .bind(total_input_tokens)
    .bind(total_output_tokens)
    .bind(cache_read_tokens)
    .bind(cache_creation_tokens)
    .bind(thinking_block_count)
    .bind(turn_duration_avg_ms)
    .bind(turn_duration_max_ms)
    .bind(turn_duration_total_ms)
    .bind(api_error_count)
    .bind(api_retry_count)
    .bind(compaction_count)
    .bind(hook_blocked_count)
    .bind(agent_spawn_count)
    .bind(bash_progress_count)
    .bind(hook_progress_count)
    .bind(mcp_progress_count)
    .bind(summary_text)
    .bind(parse_version)
    .bind(file_size)
    .bind(file_mtime)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Batch insert invocations within an existing transaction (no BEGIN/COMMIT).
pub async fn batch_insert_invocations_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    invocations: &[(String, i64, String, String, String, i64)],
) -> DbResult<u64> {
    let mut inserted: u64 = 0;

    for (source_file, byte_offset, invocable_id, session_id, project, timestamp) in invocations {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO invocations
                (source_file, byte_offset, invocable_id, session_id, project, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(source_file)
        .bind(byte_offset)
        .bind(invocable_id)
        .bind(session_id)
        .bind(project)
        .bind(timestamp)
        .execute(&mut **tx)
        .await?;

        inserted += result.rows_affected();
    }

    Ok(inserted)
}

/// Batch upsert models within an existing transaction (no BEGIN/COMMIT).
pub async fn batch_upsert_models_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    model_ids: &[String],
    seen_at: i64,
) -> DbResult<u64> {
    if model_ids.is_empty() {
        return Ok(0);
    }
    let mut affected: u64 = 0;

    for model_id in model_ids {
        let (provider, family) = parse_model_id(model_id);
        let result = sqlx::query(
            r#"
            INSERT INTO models (id, provider, family, first_seen, last_seen)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                last_seen = MAX(models.last_seen, excluded.last_seen)
            "#,
        )
        .bind(model_id)
        .bind(provider)
        .bind(family)
        .bind(seen_at)
        .bind(seen_at)
        .execute(&mut **tx)
        .await?;

        affected += result.rows_affected();
    }

    Ok(affected)
}

/// Batch insert turns within an existing transaction (no BEGIN/COMMIT).
pub async fn batch_insert_turns_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: &str,
    turns: &[RawTurn],
) -> DbResult<u64> {
    if turns.is_empty() {
        return Ok(0);
    }
    let mut inserted: u64 = 0;

    for turn in turns {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO turns (
                session_id, uuid, seq, model_id, parent_uuid,
                content_type, input_tokens, output_tokens,
                cache_read_tokens, cache_creation_tokens,
                service_tier, timestamp
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8,
                ?9, ?10,
                ?11, ?12
            )
            "#,
        )
        .bind(session_id)
        .bind(&turn.uuid)
        .bind(turn.seq)
        .bind(&turn.model_id)
        .bind(&turn.parent_uuid)
        .bind(&turn.content_type)
        .bind(turn.input_tokens.map(|v| v as i64))
        .bind(turn.output_tokens.map(|v| v as i64))
        .bind(turn.cache_read_tokens.map(|v| v as i64))
        .bind(turn.cache_creation_tokens.map(|v| v as i64))
        .bind(&turn.service_tier)
        .bind(turn.timestamp)
        .execute(&mut **tx)
        .await?;

        inserted += result.rows_affected();
    }

    Ok(inserted)
}

// Internal row type for reading sessions from SQLite.
#[derive(Debug)]
struct SessionRow {
    id: String,
    project_id: String,
    preview: String,
    turn_count: i32,
    last_message_at: Option<i64>,
    file_path: String,
    project_path: String,
    project_display_name: String,
    size_bytes: i64,
    last_message: String,
    files_touched: String,
    skills_used: String,
    tool_counts_edit: i32,
    tool_counts_read: i32,
    tool_counts_bash: i32,
    tool_counts_write: i32,
    message_count: i32,
    summary: Option<String>,
    git_branch: Option<String>,
    is_sidechain: bool,
    deep_indexed_at: Option<i64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    total_cache_read_tokens: Option<i64>,
    total_cache_creation_tokens: Option<i64>,
    turn_count_api: Option<i64>,
    primary_model: Option<String>,
    // Phase 3: Atomic unit metrics
    user_prompt_count: i32,
    api_call_count: i32,
    tool_call_count: i32,
    files_read: String,
    files_edited: String,
    files_read_count: i32,
    files_edited_count: i32,
    reedited_files_count: i32,
    duration_seconds: i32,
    #[allow(dead_code)] // Used internally by git sync queries, not by into_session_info()
    first_message_at: Option<i64>,
    commit_count: i32,
    // Phase 3.5: Full parser metrics
    thinking_block_count: i32,
    turn_duration_avg_ms: Option<i64>,
    turn_duration_max_ms: Option<i64>,
    api_error_count: i32,
    compaction_count: i32,
    agent_spawn_count: i32,
    bash_progress_count: i32,
    hook_progress_count: i32,
    mcp_progress_count: i32,
    summary_text: Option<String>,
    parse_version: i32,
    // Theme 4: Classification
    category_l1: Option<String>,
    category_l2: Option<String>,
    category_l3: Option<String>,
    category_confidence: Option<f64>,
    category_source: Option<String>,
    classified_at: Option<String>,
    // Theme 4: Behavioral metrics
    prompt_word_count: Option<i32>,
    correction_count: i32,
    same_file_edit_count: i32,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for SessionRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            preview: row.try_get("preview")?,
            turn_count: row.try_get("turn_count")?,
            last_message_at: row.try_get("last_message_at")?,
            file_path: row.try_get("file_path")?,
            project_path: row.try_get("project_path")?,
            project_display_name: row.try_get("project_display_name")?,
            size_bytes: row.try_get("size_bytes")?,
            last_message: row.try_get("last_message")?,
            files_touched: row.try_get("files_touched")?,
            skills_used: row.try_get("skills_used")?,
            tool_counts_edit: row.try_get("tool_counts_edit")?,
            tool_counts_read: row.try_get("tool_counts_read")?,
            tool_counts_bash: row.try_get("tool_counts_bash")?,
            tool_counts_write: row.try_get("tool_counts_write")?,
            message_count: row.try_get("message_count")?,
            summary: row.try_get("summary")?,
            git_branch: row.try_get("git_branch")?,
            is_sidechain: row.try_get("is_sidechain")?,
            deep_indexed_at: row.try_get("deep_indexed_at")?,
            total_input_tokens: row.try_get("total_input_tokens").ok().flatten(),
            total_output_tokens: row.try_get("total_output_tokens").ok().flatten(),
            total_cache_read_tokens: row.try_get("total_cache_read_tokens").ok().flatten(),
            total_cache_creation_tokens: row.try_get("total_cache_creation_tokens").ok().flatten(),
            turn_count_api: row.try_get("turn_count_api").ok().flatten(),
            primary_model: row.try_get("primary_model").ok().flatten(),
            // Phase 3: Atomic unit metrics
            user_prompt_count: row.try_get("user_prompt_count")?,
            api_call_count: row.try_get("api_call_count")?,
            tool_call_count: row.try_get("tool_call_count")?,
            files_read: row.try_get("files_read")?,
            files_edited: row.try_get("files_edited")?,
            files_read_count: row.try_get("files_read_count")?,
            files_edited_count: row.try_get("files_edited_count")?,
            reedited_files_count: row.try_get("reedited_files_count")?,
            duration_seconds: row.try_get("duration_seconds")?,
            first_message_at: row.try_get("first_message_at")?,
            commit_count: row.try_get("commit_count")?,
            // Phase 3.5: Full parser metrics
            thinking_block_count: row.try_get("thinking_block_count")?,
            turn_duration_avg_ms: row.try_get("turn_duration_avg_ms").ok().flatten(),
            turn_duration_max_ms: row.try_get("turn_duration_max_ms").ok().flatten(),
            api_error_count: row.try_get("api_error_count")?,
            compaction_count: row.try_get("compaction_count")?,
            agent_spawn_count: row.try_get("agent_spawn_count")?,
            bash_progress_count: row.try_get("bash_progress_count")?,
            hook_progress_count: row.try_get("hook_progress_count")?,
            mcp_progress_count: row.try_get("mcp_progress_count")?,
            summary_text: row.try_get("summary_text")?,
            parse_version: row.try_get("parse_version")?,
            // Theme 4: Classification
            category_l1: row.try_get("category_l1").ok().flatten(),
            category_l2: row.try_get("category_l2").ok().flatten(),
            category_l3: row.try_get("category_l3").ok().flatten(),
            category_confidence: row.try_get("category_confidence").ok().flatten(),
            category_source: row.try_get("category_source").ok().flatten(),
            classified_at: row.try_get("classified_at").ok().flatten(),
            // Theme 4: Behavioral metrics
            prompt_word_count: row.try_get("prompt_word_count").ok().flatten(),
            correction_count: row.try_get("correction_count").unwrap_or(0),
            same_file_edit_count: row.try_get("same_file_edit_count").unwrap_or(0),
        })
    }
}

impl SessionRow {
    fn into_session_info(self, project_encoded: &str) -> SessionInfo {
        let files_touched: Vec<String> =
            serde_json::from_str(&self.files_touched).unwrap_or_default();
        let skills_used: Vec<String> =
            serde_json::from_str(&self.skills_used).unwrap_or_default();
        // Phase 3: Deserialize files_read and files_edited from JSON
        let files_read: Vec<String> =
            serde_json::from_str(&self.files_read).unwrap_or_default();
        let files_edited: Vec<String> =
            serde_json::from_str(&self.files_edited).unwrap_or_default();

        SessionInfo {
            id: self.id,
            project: project_encoded.to_string(),
            project_path: self.project_path,
            file_path: self.file_path,
            modified_at: self.last_message_at.unwrap_or(0),
            size_bytes: self.size_bytes as u64,
            preview: self.preview,
            last_message: self.last_message,
            files_touched,
            skills_used,
            tool_counts: ToolCounts {
                edit: self.tool_counts_edit as usize,
                read: self.tool_counts_read as usize,
                bash: self.tool_counts_bash as usize,
                write: self.tool_counts_write as usize,
            },
            message_count: self.message_count as usize,
            turn_count: self.turn_count as usize,
            summary: self.summary,
            git_branch: self.git_branch,
            is_sidechain: self.is_sidechain,
            deep_indexed: self.deep_indexed_at.is_some(),
            total_input_tokens: self.total_input_tokens.map(|v| v as u64),
            total_output_tokens: self.total_output_tokens.map(|v| v as u64),
            total_cache_read_tokens: self.total_cache_read_tokens.map(|v| v as u64),
            total_cache_creation_tokens: self.total_cache_creation_tokens.map(|v| v as u64),
            turn_count_api: self.turn_count_api.map(|v| v as u64),
            primary_model: self.primary_model.clone(),
            // Phase 3: Atomic unit metrics loaded from DB
            user_prompt_count: self.user_prompt_count as u32,
            api_call_count: self.api_call_count as u32,
            tool_call_count: self.tool_call_count as u32,
            files_read,
            files_edited,
            files_read_count: self.files_read_count as u32,
            files_edited_count: self.files_edited_count as u32,
            reedited_files_count: self.reedited_files_count as u32,
            duration_seconds: self.duration_seconds as u32,
            commit_count: self.commit_count as u32,
            // Phase 3.5
            thinking_block_count: self.thinking_block_count as u32,
            turn_duration_avg_ms: self.turn_duration_avg_ms.map(|v| v as u64),
            turn_duration_max_ms: self.turn_duration_max_ms.map(|v| v as u64),
            api_error_count: self.api_error_count as u32,
            compaction_count: self.compaction_count as u32,
            agent_spawn_count: self.agent_spawn_count as u32,
            bash_progress_count: self.bash_progress_count as u32,
            hook_progress_count: self.hook_progress_count as u32,
            mcp_progress_count: self.mcp_progress_count as u32,
            summary_text: self.summary_text,
            parse_version: self.parse_version as u32,
            // Theme 4: Classification
            category_l1: self.category_l1,
            category_l2: self.category_l2,
            category_l3: self.category_l3,
            category_confidence: self.category_confidence,
            category_source: self.category_source,
            classified_at: self.classified_at,
            // Theme 4: Behavioral metrics
            prompt_word_count: self.prompt_word_count.map(|v| v as u32),
            correction_count: self.correction_count as u32,
            same_file_edit_count: self.same_file_edit_count as u32,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test SessionInfo with sensible defaults.
    fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: project.to_string(),
            project_path: format!("/home/user/{}", project),
            file_path: format!(
                "/home/user/.claude/projects/{}/{}.jsonl",
                project, id
            ),
            modified_at,
            size_bytes: 2048,
            preview: format!("Preview for {}", id),
            last_message: format!("Last message for {}", id),
            files_touched: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
            skills_used: vec!["/commit".to_string()],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            // Phase 3: Atomic unit metrics
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            summary_text: None,
            parse_version: 0,
            // Theme 4: Classification
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        }
    }

    #[tokio::test]
    async fn test_insert_and_list_projects() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert 3 sessions across 2 projects
        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 2000);
        let s3 = make_session("sess-3", "project-b", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 2, "Should have 2 projects");

        // Projects should be sorted by most recent activity (project-b first)
        assert_eq!(projects[0].name, "project-b");
        assert_eq!(projects[0].sessions.len(), 1);
        assert_eq!(projects[0].display_name, "Project B");

        assert_eq!(projects[1].name, "project-a");
        assert_eq!(projects[1].sessions.len(), 2);
        assert_eq!(projects[1].display_name, "Project A");

        // Within project-a, sessions should be sorted by last_message_at DESC
        assert_eq!(projects[1].sessions[0].id, "sess-2");
        assert_eq!(projects[1].sessions[1].id, "sess-1");

        // Verify JSON fields deserialized correctly
        assert_eq!(
            projects[1].sessions[0].files_touched,
            vec!["src/main.rs", "Cargo.toml"]
        );
        assert_eq!(projects[1].sessions[0].skills_used, vec!["/commit"]);
        assert_eq!(projects[1].sessions[0].tool_counts.edit, 5);
    }

    #[tokio::test]
    async fn test_upsert_session() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = make_session("sess-1", "project-a", 1000);
        db.insert_session(&s1, "project-a", "Project A").await.unwrap();

        // Update same session with new data
        let s1_updated = SessionInfo {
            preview: "Updated preview".to_string(),
            modified_at: 5000,
            message_count: 50,
            ..s1
        };
        db.insert_session(&s1_updated, "project-a", "Project A")
            .await
            .unwrap();

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1, "Should still have 1 project");
        assert_eq!(projects[0].sessions.len(), 1, "Should still have 1 session (upsert, not duplicate)");
        assert_eq!(projects[0].sessions[0].preview, "Updated preview");
        assert_eq!(projects[0].sessions[0].modified_at, 5000);
        assert_eq!(projects[0].sessions[0].message_count, 50);
    }

    #[tokio::test]
    async fn test_indexer_state_roundtrip() {
        let db = Database::new_in_memory().await.unwrap();

        let path = "/home/user/.claude/projects/test/session.jsonl";

        // Initially no state
        let state = db.get_indexer_state(path).await.unwrap();
        assert!(state.is_none(), "Should have no state initially");

        // Set state
        db.update_indexer_state(path, 4096, 1234567890).await.unwrap();

        // Read back
        let state = db.get_indexer_state(path).await.unwrap();
        assert!(state.is_some(), "Should have state after update");
        let entry = state.unwrap();
        assert_eq!(entry.file_path, path);
        assert_eq!(entry.file_size, 4096);
        assert_eq!(entry.modified_at, 1234567890);
        assert!(entry.indexed_at > 0, "indexed_at should be set");

        // Update state (upsert)
        db.update_indexer_state(path, 8192, 1234567999).await.unwrap();
        let entry = db.get_indexer_state(path).await.unwrap().unwrap();
        assert_eq!(entry.file_size, 8192);
        assert_eq!(entry.modified_at, 1234567999);
    }

    #[tokio::test]
    async fn test_remove_stale_sessions() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 2000);
        let s3 = make_session("sess-3", "project-b", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        // Also add indexer state for the sessions
        db.update_indexer_state(&s1.file_path, 2048, 1000).await.unwrap();
        db.update_indexer_state(&s2.file_path, 2048, 2000).await.unwrap();
        db.update_indexer_state(&s3.file_path, 2048, 3000).await.unwrap();

        // Keep only sess-1's file path; sess-2 and sess-3 are stale
        let valid = vec![s1.file_path.clone()];
        let removed = db.remove_stale_sessions(&valid).await.unwrap();
        assert_eq!(removed, 2, "Should have removed 2 stale sessions");

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1, "Should have 1 project left");
        assert_eq!(projects[0].sessions.len(), 1);
        assert_eq!(projects[0].sessions[0].id, "sess-1");

        // Indexer state should also be cleaned up
        assert!(db.get_indexer_state(&s2.file_path).await.unwrap().is_none());
        assert!(db.get_indexer_state(&s3.file_path).await.unwrap().is_none());
        // The valid file should still have its indexer state
        assert!(db.get_indexer_state(&s1.file_path).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_active_count_calculation() {
        let db = Database::new_in_memory().await.unwrap();
        let now = Utc::now().timestamp();

        // Session within the 5-minute window (active)
        let s_active = SessionInfo {
            modified_at: now - 60, // 1 minute ago
            ..make_session("active-sess", "project-a", now - 60)
        };

        // Session outside the 5-minute window (inactive)
        let s_old = SessionInfo {
            modified_at: now - 600, // 10 minutes ago
            ..make_session("old-sess", "project-a", now - 600)
        };

        db.insert_session(&s_active, "project-a", "Project A").await.unwrap();
        db.insert_session(&s_old, "project-a", "Project A").await.unwrap();

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].active_count, 1, "Only 1 session should be active (within 5 min)");
        assert_eq!(projects[0].sessions.len(), 2, "Both sessions should be listed");
    }

    #[tokio::test]
    async fn test_get_all_indexer_states() {
        let db = Database::new_in_memory().await.unwrap();

        // Initially empty
        let states = db.get_all_indexer_states().await.unwrap();
        assert!(states.is_empty(), "Should be empty initially");

        // Insert some indexer state entries
        let path_a = "/home/user/.claude/projects/test/a.jsonl";
        let path_b = "/home/user/.claude/projects/test/b.jsonl";
        let path_c = "/home/user/.claude/projects/test/c.jsonl";

        db.update_indexer_state(path_a, 1000, 100).await.unwrap();
        db.update_indexer_state(path_b, 2000, 200).await.unwrap();
        db.update_indexer_state(path_c, 3000, 300).await.unwrap();

        // Fetch all states
        let states = db.get_all_indexer_states().await.unwrap();
        assert_eq!(states.len(), 3, "Should have 3 entries");

        // Verify each entry is keyed correctly and has correct values
        let a = states.get(path_a).expect("Should contain path_a");
        assert_eq!(a.file_size, 1000);
        assert_eq!(a.modified_at, 100);

        let b = states.get(path_b).expect("Should contain path_b");
        assert_eq!(b.file_size, 2000);
        assert_eq!(b.modified_at, 200);

        let c = states.get(path_c).expect("Should contain path_c");
        assert_eq!(c.file_size, 3000);
        assert_eq!(c.modified_at, 300);

        // All entries should have indexed_at set
        assert!(a.indexed_at > 0);
        assert!(b.indexed_at > 0);
        assert!(c.indexed_at > 0);
    }

    #[tokio::test]
    async fn test_list_projects_returns_camelcase_json() {
        let db = Database::new_in_memory().await.unwrap();
        let now = Utc::now().timestamp();

        let s1 = make_session("sess-1", "project-a", now);
        db.insert_session(&s1, "project-a", "Project A").await.unwrap();

        let projects = db.list_projects().await.unwrap();
        let json = serde_json::to_string(&projects).unwrap();

        // Verify camelCase keys in ProjectInfo
        assert!(json.contains("\"displayName\""), "Should use camelCase: displayName");
        assert!(json.contains("\"activeCount\""), "Should use camelCase: activeCount");

        // Verify camelCase keys in SessionInfo
        assert!(json.contains("\"projectPath\""), "Should use camelCase: projectPath");
        assert!(json.contains("\"filePath\""), "Should use camelCase: filePath");
        assert!(json.contains("\"modifiedAt\""), "Should use camelCase: modifiedAt");
        assert!(json.contains("\"sizeBytes\""), "Should use camelCase: sizeBytes");
        assert!(json.contains("\"lastMessage\""), "Should use camelCase: lastMessage");
        assert!(json.contains("\"filesTouched\""), "Should use camelCase: filesTouched");
        assert!(json.contains("\"skillsUsed\""), "Should use camelCase: skillsUsed");
        assert!(json.contains("\"toolCounts\""), "Should use camelCase: toolCounts");
        assert!(json.contains("\"messageCount\""), "Should use camelCase: messageCount");
        assert!(json.contains("\"turnCount\""), "Should use camelCase: turnCount");

        // Verify new fields use camelCase
        assert!(json.contains("\"isSidechain\""), "Should use camelCase: isSidechain");
        assert!(json.contains("\"deepIndexed\""), "Should use camelCase: deepIndexed");
        // summary and git_branch are None, so they should be omitted (skip_serializing_if)
        assert!(!json.contains("\"summary\""), "summary=None should be omitted");
        assert!(!json.contains("\"gitBranch\""), "gitBranch=None should be omitted");

        // modifiedAt should be a Unix timestamp number (not an ISO string)
        let expected_fragment = format!("\"modifiedAt\":{}", now);
        assert!(
            json.contains(&expected_fragment),
            "modifiedAt should be a number: {}",
            json
        );
    }

    // ========================================================================
    // Invocable + Invocation CRUD tests
    // ========================================================================

    #[tokio::test]
    async fn test_upsert_invocable() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a new invocable
        db.upsert_invocable("tool::Read", Some("core"), "Read", "tool", "Read files")
            .await
            .unwrap();

        let items = db.list_invocables_with_counts().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "tool::Read");
        assert_eq!(items[0].plugin_name, Some("core".to_string()));
        assert_eq!(items[0].description, "Read files");

        // Upsert same id with a different description
        db.upsert_invocable("tool::Read", Some("core"), "Read", "tool", "Read files from disk")
            .await
            .unwrap();

        let items = db.list_invocables_with_counts().await.unwrap();
        assert_eq!(items.len(), 1, "Should still be 1 invocable after upsert");
        assert_eq!(items[0].description, "Read files from disk");
    }

    #[tokio::test]
    async fn test_batch_insert_invocations() {
        let db = Database::new_in_memory().await.unwrap();

        // Must insert invocables first (FK constraint)
        db.upsert_invocable("tool::Read", None, "Read", "tool", "")
            .await
            .unwrap();
        db.upsert_invocable("tool::Edit", None, "Edit", "tool", "")
            .await
            .unwrap();

        let invocations = vec![
            ("file1.jsonl".to_string(), 100, "tool::Read".to_string(), "sess-1".to_string(), "proj-a".to_string(), 1000),
            ("file1.jsonl".to_string(), 200, "tool::Edit".to_string(), "sess-1".to_string(), "proj-a".to_string(), 1001),
            ("file2.jsonl".to_string(), 50, "tool::Read".to_string(), "sess-2".to_string(), "proj-a".to_string(), 2000),
        ];

        let inserted = db.batch_insert_invocations(&invocations).await.unwrap();
        assert_eq!(inserted, 3, "Should insert 3 rows");
    }

    #[tokio::test]
    async fn test_batch_insert_invocations_ignores_duplicates() {
        let db = Database::new_in_memory().await.unwrap();

        db.upsert_invocable("tool::Read", None, "Read", "tool", "")
            .await
            .unwrap();

        let invocations = vec![
            ("file1.jsonl".to_string(), 100, "tool::Read".to_string(), "sess-1".to_string(), "proj-a".to_string(), 1000),
        ];

        let inserted = db.batch_insert_invocations(&invocations).await.unwrap();
        assert_eq!(inserted, 1);

        // Insert same (source_file, byte_offset) again — should be ignored
        let inserted2 = db.batch_insert_invocations(&invocations).await.unwrap();
        assert_eq!(inserted2, 0, "Duplicate should be ignored (INSERT OR IGNORE)");
    }

    #[tokio::test]
    async fn test_list_invocables_with_counts() {
        let db = Database::new_in_memory().await.unwrap();

        db.upsert_invocable("tool::Read", None, "Read", "tool", "Read files")
            .await
            .unwrap();
        db.upsert_invocable("tool::Edit", None, "Edit", "tool", "Edit files")
            .await
            .unwrap();
        db.upsert_invocable("tool::Bash", None, "Bash", "tool", "Run commands")
            .await
            .unwrap();

        // Add invocations: Read x3, Edit x1, Bash x0
        let invocations = vec![
            ("f1.jsonl".to_string(), 10, "tool::Read".to_string(), "s1".to_string(), "p".to_string(), 1000),
            ("f1.jsonl".to_string(), 20, "tool::Read".to_string(), "s1".to_string(), "p".to_string(), 2000),
            ("f2.jsonl".to_string(), 10, "tool::Read".to_string(), "s2".to_string(), "p".to_string(), 3000),
            ("f2.jsonl".to_string(), 20, "tool::Edit".to_string(), "s2".to_string(), "p".to_string(), 3001),
        ];
        db.batch_insert_invocations(&invocations).await.unwrap();

        let items = db.list_invocables_with_counts().await.unwrap();
        assert_eq!(items.len(), 3);

        // Ordered by invocation_count DESC, then name ASC
        assert_eq!(items[0].id, "tool::Read");
        assert_eq!(items[0].invocation_count, 3);
        assert_eq!(items[0].last_used_at, Some(3000));

        assert_eq!(items[1].id, "tool::Edit");
        assert_eq!(items[1].invocation_count, 1);

        assert_eq!(items[2].id, "tool::Bash");
        assert_eq!(items[2].invocation_count, 0);
        assert_eq!(items[2].last_used_at, None);
    }

    #[tokio::test]
    async fn test_batch_upsert_invocables() {
        let db = Database::new_in_memory().await.unwrap();

        let batch = vec![
            ("tool::Read".to_string(), Some("core".to_string()), "Read".to_string(), "tool".to_string(), "Read files".to_string()),
            ("tool::Edit".to_string(), None, "Edit".to_string(), "tool".to_string(), "Edit files".to_string()),
            ("skill::commit".to_string(), Some("git".to_string()), "commit".to_string(), "skill".to_string(), "Git commit".to_string()),
        ];

        let affected = db.batch_upsert_invocables(&batch).await.unwrap();
        assert_eq!(affected, 3);

        let items = db.list_invocables_with_counts().await.unwrap();
        assert_eq!(items.len(), 3, "All 3 invocables should be present");

        // Verify one of them
        let commit = items.iter().find(|i| i.id == "skill::commit").unwrap();
        assert_eq!(commit.plugin_name, Some("git".to_string()));
        assert_eq!(commit.kind, "skill");
    }

    #[tokio::test]
    async fn test_get_stats_overview() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a session so total_sessions > 0
        let s1 = make_session("sess-1", "project-a", 1000);
        db.insert_session(&s1, "project-a", "Project A").await.unwrap();

        // Insert invocables
        db.upsert_invocable("tool::Read", None, "Read", "tool", "")
            .await
            .unwrap();
        db.upsert_invocable("tool::Edit", None, "Edit", "tool", "")
            .await
            .unwrap();

        // Insert invocations
        let invocations = vec![
            ("f1.jsonl".to_string(), 10, "tool::Read".to_string(), "sess-1".to_string(), "p".to_string(), 1000),
            ("f1.jsonl".to_string(), 20, "tool::Read".to_string(), "sess-1".to_string(), "p".to_string(), 1001),
            ("f1.jsonl".to_string(), 30, "tool::Edit".to_string(), "sess-1".to_string(), "p".to_string(), 1002),
        ];
        db.batch_insert_invocations(&invocations).await.unwrap();

        let stats = db.get_stats_overview().await.unwrap();
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.total_invocations, 3);
        assert_eq!(stats.unique_invocables_used, 2);
        assert!(stats.top_invocables.len() <= 10);
        assert_eq!(stats.top_invocables[0].id, "tool::Read");
        assert_eq!(stats.top_invocables[0].invocation_count, 2);
    }

    // ========================================================================
    // Phase 2C: Project Summaries, Paginated Sessions, Dashboard Stats
    // ========================================================================

    #[tokio::test]
    async fn test_list_project_summaries() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 2000);
        let s3 = make_session("sess-3", "project-b", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        let summaries = db.list_project_summaries().await.unwrap();
        assert_eq!(summaries.len(), 2);

        // Sorted by last_activity_at DESC
        assert_eq!(summaries[0].name, "project-b");
        assert_eq!(summaries[0].session_count, 1);
        assert_eq!(summaries[0].display_name, "Project B");

        assert_eq!(summaries[1].name, "project-a");
        assert_eq!(summaries[1].session_count, 2);

        // No sessions array on summaries
        let json = serde_json::to_string(&summaries).unwrap();
        assert!(!json.contains("\"sessions\""), "Summaries should NOT include sessions array");
        assert!(json.contains("\"sessionCount\""), "Should have sessionCount field");
    }

    #[tokio::test]
    async fn test_list_sessions_for_project_pagination() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert 5 sessions for project-a
        for i in 1..=5 {
            let s = make_session(&format!("sess-{}", i), "project-a", i as i64 * 1000);
            db.insert_session(&s, "project-a", "Project A").await.unwrap();
        }

        // Page 1: limit=2, offset=0
        let page1 = db.list_sessions_for_project("project-a", 2, 0, "recent", None, false).await.unwrap();
        assert_eq!(page1.total, 5);
        assert_eq!(page1.sessions.len(), 2);
        assert_eq!(page1.sessions[0].id, "sess-5"); // Most recent first

        // Page 2: limit=2, offset=2
        let page2 = db.list_sessions_for_project("project-a", 2, 2, "recent", None, false).await.unwrap();
        assert_eq!(page2.total, 5);
        assert_eq!(page2.sessions.len(), 2);
        assert_eq!(page2.sessions[0].id, "sess-3");
    }

    #[tokio::test]
    async fn test_list_sessions_for_project_sort() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = SessionInfo { message_count: 100, ..make_session("sess-1", "project-a", 1000) };
        let s2 = SessionInfo { message_count: 5, ..make_session("sess-2", "project-a", 3000) };
        let s3 = SessionInfo { message_count: 50, ..make_session("sess-3", "project-a", 2000) };

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-a", "Project A").await.unwrap();

        // Sort by oldest
        let oldest = db.list_sessions_for_project("project-a", 10, 0, "oldest", None, false).await.unwrap();
        assert_eq!(oldest.sessions[0].id, "sess-1");

        // Sort by messages
        let by_msg = db.list_sessions_for_project("project-a", 10, 0, "messages", None, false).await.unwrap();
        assert_eq!(by_msg.sessions[0].id, "sess-1"); // 100 messages
    }

    #[tokio::test]
    async fn test_list_sessions_excludes_sidechains() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = SessionInfo { is_sidechain: true, ..make_session("sess-2", "project-a", 2000) };

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();

        // Default: exclude sidechains
        let page = db.list_sessions_for_project("project-a", 10, 0, "recent", None, false).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.sessions[0].id, "sess-1");

        // Include sidechains
        let page = db.list_sessions_for_project("project-a", 10, 0, "recent", None, true).await.unwrap();
        assert_eq!(page.total, 2);
    }

    #[tokio::test]
    async fn test_get_dashboard_stats() {
        let db = Database::new_in_memory().await.unwrap();

        let now = Utc::now().timestamp();
        let s1 = SessionInfo { modified_at: now - 86400, ..make_session("sess-1", "project-a", now - 86400) };
        let s2 = SessionInfo { modified_at: now - 172800, ..make_session("sess-2", "project-a", now - 172800) };
        let s3 = SessionInfo { modified_at: now - 86400, ..make_session("sess-3", "project-b", now - 86400) };

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        let stats = db.get_dashboard_stats().await.unwrap();
        assert_eq!(stats.total_sessions, 3);
        assert_eq!(stats.total_projects, 2);
        assert!(!stats.heatmap.is_empty());
        assert!(!stats.top_projects.is_empty());
        assert_eq!(stats.top_projects[0].session_count, 2); // project-a has most
        assert!(stats.tool_totals.edit > 0); // sessions have tool counts
    }

    #[tokio::test]
    async fn test_project_summaries_exclude_sidechains() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = SessionInfo { is_sidechain: true, ..make_session("sess-2", "project-a", 2000) };

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();

        let summaries = db.list_project_summaries().await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].session_count, 1, "Sidechain should be excluded from count");
    }

    // ========================================================================
    // Phase 3: Deep fields integration tests
    // ========================================================================

    #[tokio::test]
    async fn test_update_session_deep_fields_phase3() {
        let db = Database::new_in_memory().await.unwrap();

        // First insert a session via Pass 1 (from index)
        db.insert_session_from_index(
            "test-sess-deep",
            "project-deep",
            "Project Deep",
            "/tmp/project-deep",
            "/tmp/test-deep.jsonl",
            "Test preview",
            None,
            10,
            1000,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        // Update with Pass 2 deep fields including Phase 3 metrics
        let files_read = r#"["/path/to/file1.rs", "/path/to/file2.rs"]"#;
        let files_edited = r#"["/path/to/file1.rs", "/path/to/file1.rs", "/path/to/file3.rs"]"#;

        db.update_session_deep_fields(
            "test-sess-deep",
            "Last message content",
            5,   // turn_count
            10,  // tool_edit
            15,  // tool_read
            3,   // tool_bash
            2,   // tool_write
            r#"["/path/to/file1.rs", "/path/to/file2.rs", "/path/to/file3.rs"]"#, // files_touched
            r#"["/commit", "/review"]"#, // skills_used
            // Phase 3: Atomic unit metrics
            8,   // user_prompt_count
            12,  // api_call_count
            25,  // tool_call_count
            files_read,
            files_edited,
            2,   // files_read_count
            2,   // files_edited_count (unique: file1.rs, file3.rs)
            1,   // reedited_files_count (file1.rs edited twice)
            600, // duration_seconds (10 minutes)
            3,   // commit_count
            Some(1000), // first_message_at
            // Phase 3.5: Full parser metrics
            5000,  // total_input_tokens
            3000,  // total_output_tokens
            1000,  // cache_read_tokens
            500,   // cache_creation_tokens
            2,     // thinking_block_count
            Some(150), // turn_duration_avg_ms
            Some(300), // turn_duration_max_ms
            Some(750), // turn_duration_total_ms
            1,     // api_error_count
            0,     // api_retry_count
            0,     // compaction_count
            0,     // hook_blocked_count
            1,     // agent_spawn_count
            2,     // bash_progress_count
            0,     // hook_progress_count
            1,     // mcp_progress_count
            Some("Session summary text"), // summary_text
            1,     // parse_version
            5000,  // file_size
            1706200000, // file_mtime
        )
        .await
        .unwrap();

        // Retrieve session and verify all Phase 3 fields
        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].sessions.len(), 1);

        let session = &projects[0].sessions[0];
        assert_eq!(session.id, "test-sess-deep");
        assert_eq!(session.last_message, "Last message content");
        assert_eq!(session.turn_count, 5);
        assert!(session.deep_indexed, "Session should be marked as deep_indexed");

        // Verify Phase 3 atomic unit metrics
        assert_eq!(session.user_prompt_count, 8, "user_prompt_count mismatch");
        assert_eq!(session.api_call_count, 12, "api_call_count mismatch");
        assert_eq!(session.tool_call_count, 25, "tool_call_count mismatch");
        assert_eq!(session.files_read.len(), 2, "files_read count mismatch");
        assert_eq!(session.files_read, vec!["/path/to/file1.rs", "/path/to/file2.rs"]);
        assert_eq!(session.files_edited.len(), 3, "files_edited count mismatch (includes duplicates)");
        assert_eq!(session.files_read_count, 2, "files_read_count mismatch");
        assert_eq!(session.files_edited_count, 2, "files_edited_count mismatch");
        assert_eq!(session.reedited_files_count, 1, "reedited_files_count mismatch");
        assert_eq!(session.duration_seconds, 600, "duration_seconds mismatch");
        assert_eq!(session.commit_count, 3, "commit_count mismatch");
    }

    #[tokio::test]
    async fn test_list_sessions_for_project_includes_phase3_fields() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert via Pass 1
        db.insert_session_from_index(
            "phase3-paginated",
            "proj-paginated",
            "Project Paginated",
            "/tmp/proj-paginated",
            "/tmp/paginated.jsonl",
            "Preview",
            None,
            5,
            2000,
            None,
            false,
            1000,
        )
        .await
        .unwrap();

        // Update via Pass 2 with Phase 3 fields
        db.update_session_deep_fields(
            "phase3-paginated",
            "Last msg",
            3,   // turn_count
            2, 4, 1, 0,  // tool counts
            "[]", "[]", // files_touched, skills_used
            15, 20, 30, // user_prompt_count, api_call_count, tool_call_count
            r#"["/a.rs"]"#, r#"["/b.rs"]"#, // files_read, files_edited
            1, 1, 0,    // counts
            120, 2,     // duration_seconds, commit_count
            None, // first_message_at
            // Phase 3.5: Full parser metrics
            0, 0, 0, 0, // token counts
            0,           // thinking_block_count
            None, None, None, // turn durations
            0, 0, 0, 0, // error/retry/compaction/hook_blocked
            0, 0, 0, 0, // progress counts
            None,        // summary_text
            1,           // parse_version
            1000,        // file_size
            1706200000,  // file_mtime
        )
        .await
        .unwrap();

        // Test paginated retrieval includes Phase 3 fields
        let page = db.list_sessions_for_project("proj-paginated", 10, 0, "recent", None, false).await.unwrap();
        assert_eq!(page.sessions.len(), 1);

        let session = &page.sessions[0];
        assert_eq!(session.user_prompt_count, 15);
        assert_eq!(session.api_call_count, 20);
        assert_eq!(session.tool_call_count, 30);
        assert_eq!(session.files_read, vec!["/a.rs"]);
        assert_eq!(session.files_edited, vec!["/b.rs"]);
        assert_eq!(session.duration_seconds, 120);
        assert_eq!(session.commit_count, 2);
    }

    #[tokio::test]
    async fn test_phase3_fields_default_to_zero() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a session but don't run Pass 2
        db.insert_session_from_index(
            "no-deep-index",
            "proj-no-deep",
            "Project No Deep",
            "/tmp/proj",
            "/tmp/no-deep.jsonl",
            "Preview",
            None,
            5,
            1000,
            None,
            false,
            500,
        )
        .await
        .unwrap();

        // Retrieve and verify Phase 3 fields default to 0/empty
        let projects = db.list_projects().await.unwrap();
        let session = &projects[0].sessions[0];

        assert!(!session.deep_indexed, "Session should not be deep_indexed yet");
        assert_eq!(session.user_prompt_count, 0);
        assert_eq!(session.api_call_count, 0);
        assert_eq!(session.tool_call_count, 0);
        assert!(session.files_read.is_empty());
        assert!(session.files_edited.is_empty());
        assert_eq!(session.files_read_count, 0);
        assert_eq!(session.files_edited_count, 0);
        assert_eq!(session.reedited_files_count, 0);
        assert_eq!(session.duration_seconds, 0);
        assert_eq!(session.commit_count, 0);
    }
}
