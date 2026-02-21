// crates/db/src/queries/sessions.rs
// Session CRUD operations: insert, update, list, and indexer state management.

use crate::{Database, DbResult};
use chrono::Utc;
use std::collections::HashMap;
use claude_view_core::{ProjectInfo, SessionInfo};

use super::row_types::SessionRow;
use super::IndexerEntry;

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
                ?19, NULLIF(TRIM(?20), ''), ?21,
                ?22, ?23, ?24,
                ?25, ?26,
                ?27, ?28, ?29,
                ?30, ?31
            )
            ON CONFLICT(id) DO UPDATE SET
                project_id = excluded.project_id,
                preview = CASE WHEN excluded.preview IS NOT NULL AND excluded.preview <> '' THEN excluded.preview ELSE sessions.preview END,
                turn_count = excluded.turn_count,
                last_message_at = CASE WHEN excluded.last_message_at > 0 THEN excluded.last_message_at ELSE sessions.last_message_at END,
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
                summary = CASE WHEN excluded.summary IS NOT NULL AND excluded.summary <> '' THEN excluded.summary ELSE sessions.summary END,
                git_branch = COALESCE(NULLIF(TRIM(excluded.git_branch), ''), sessions.git_branch),
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

        // All token/model data is denormalized on the sessions table.
        // No LEFT JOIN on turns needed.
        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT
                s.id, s.project_id, s.preview, s.turn_count,
                s.last_message_at, s.file_path,
                s.project_path, s.project_display_name,
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
                s.category_l1, s.category_l2, s.category_l3,
                s.category_confidence, s.category_source, s.classified_at,
                s.prompt_word_count, s.correction_count, s.same_file_edit_count
            FROM sessions s
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
                ?9, NULLIF(TRIM(?10), ''), ?11,
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
                preview = CASE WHEN excluded.preview IS NOT NULL AND excluded.preview <> '' THEN excluded.preview ELSE sessions.preview END,
                summary = CASE WHEN excluded.summary IS NOT NULL AND excluded.summary <> '' THEN excluded.summary ELSE sessions.summary END,
                message_count = excluded.message_count,
                last_message_at = CASE WHEN excluded.last_message_at > 0 THEN excluded.last_message_at ELSE sessions.last_message_at END,
                git_branch = COALESCE(NULLIF(TRIM(excluded.git_branch), ''), sessions.git_branch),
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
        // LOC + work classification (must match _tx path)
        lines_added: i32,
        lines_removed: i32,
        loc_source: i32,
        ai_lines_added: i32,
        ai_lines_removed: i32,
        work_type: Option<&str>,
        git_branch: Option<&str>,
        primary_model: Option<&str>,
        last_message_at: Option<i64>,
        first_user_prompt: Option<&str>,
        total_cost_usd: f64,
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
                file_mtime_at_index = ?41,
                lines_added = ?42,
                lines_removed = ?43,
                loc_source = ?44,
                ai_lines_added = ?45,
                ai_lines_removed = ?46,
                work_type = ?47,
                git_branch = COALESCE(NULLIF(TRIM(?48), ''), git_branch),
                primary_model = ?49,
                last_message_at = COALESCE(?50, last_message_at),
                preview = CASE WHEN (preview IS NULL OR preview = '') AND ?51 IS NOT NULL THEN ?51 ELSE preview END,
                total_cost_usd = ?52
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
        .bind(lines_added)
        .bind(lines_removed)
        .bind(loc_source)
        .bind(ai_lines_added)
        .bind(ai_lines_removed)
        .bind(work_type)
        .bind(git_branch)
        .bind(primary_model)
        .bind(last_message_at)
        .bind(first_user_prompt)
        .bind(total_cost_usd)
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
    /// Tuple: `(id, file_path, file_size_at_index, file_mtime_at_index, deep_indexed_at, parse_version, project)`
    ///
    /// The `project` value is the display name (e.g. `claude-view`) rather than
    /// the encoded path (`-Users-foo-claude-view`), so that search qualifiers
    /// like `project:claude-view` match what users naturally type.
    pub async fn get_sessions_needing_deep_index(
        &self,
    ) -> DbResult<Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, i32, String)>> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, i32, String)> =
            sqlx::query_as(
                "SELECT id, file_path, file_size_at_index, file_mtime_at_index, deep_indexed_at, parse_version, COALESCE(project_display_name, project_id, '') FROM sessions WHERE file_path IS NOT NULL AND file_path != ''",
            )
            .fetch_all(self.pool())
            .await?;
        Ok(rows)
    }

    /// Mark all sessions for re-indexing by clearing their deep_indexed_at timestamps.
    ///
    /// This forces the next deep index pass to reprocess all sessions.
    /// Used by the "Rebuild Index" feature in the Settings UI.
    ///
    /// Returns the number of sessions marked for re-indexing.
    pub async fn mark_all_sessions_for_reindex(&self) -> DbResult<u64> {
        let result = sqlx::query(
            "UPDATE sessions SET deep_indexed_at = NULL, parse_version = 0 WHERE file_path IS NOT NULL AND file_path != ''",
        )
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }

    /// Look up a session's JSONL file path by session ID.
    ///
    /// Returns `None` if the session doesn't exist in the DB.
    /// The returned path is always absolute (set during indexing).
    pub async fn get_session_file_path(&self, session_id: &str) -> DbResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT file_path FROM sessions WHERE id = ?1",
        )
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(p,)| p))
    }

    /// Get all session file_paths from the database.
    ///
    /// Returns every non-empty `file_path` in the sessions table.
    /// Used by the stale-session pruning step to check which files still exist on disk.
    pub async fn get_all_session_file_paths(&self) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT file_path FROM sessions WHERE file_path IS NOT NULL AND file_path != ''",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|(path,)| path).collect())
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
