// crates/db/src/queries/sessions.rs
// Session CRUD operations: insert, update, list, and indexer state management.

use crate::{Database, DbResult};
use chrono::Utc;
use std::collections::HashMap;
use claude_view_core::{ProjectInfo, SessionInfo};

use super::row_types::SessionRow;
use super::IndexerEntry;
use crate::indexer_parallel::ParsedSession;

/// The SQL for upserting a fully-parsed session. Shared between
/// `upsert_parsed_session()` (pool executor) and `flush_batch()` (tx executor).
/// 63 bind parameters.
pub const UPSERT_SESSION_SQL: &str = r#"
    INSERT INTO sessions (
        id, project_id, project_display_name, project_path,
        file_path, preview, summary, message_count,
        last_message_at, first_message_at, git_branch, is_sidechain,
        size_bytes, indexed_at,
        last_message, files_touched, skills_used,
        tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
        turn_count, deep_indexed_at, parse_version,
        file_size_at_index, file_mtime_at_index,
        user_prompt_count, api_call_count, tool_call_count,
        files_read, files_edited, files_read_count, files_edited_count,
        reedited_files_count, duration_seconds, commit_count,
        total_input_tokens, total_output_tokens,
        cache_read_tokens, cache_creation_tokens,
        thinking_block_count,
        turn_duration_avg_ms, turn_duration_max_ms, turn_duration_total_ms,
        api_error_count, api_retry_count, compaction_count,
        hook_blocked_count, agent_spawn_count,
        bash_progress_count, hook_progress_count, mcp_progress_count,
        summary_text, lines_added, lines_removed, loc_source,
        ai_lines_added, ai_lines_removed, work_type,
        primary_model, total_task_time_seconds,
        longest_task_seconds, longest_task_preview, total_cost_usd
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
        NULLIF(TRIM(?11), ''), ?12, ?13, ?14,
        ?15, ?16, ?17, ?18, ?19, ?20, ?21,
        ?22, ?14, ?23, ?24, ?25,
        ?26, ?27, ?28, ?29, ?30, ?31, ?32,
        ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40,
        ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48,
        ?49, ?50, ?51, ?52, ?53, ?54, ?55,
        ?56, ?57, ?58, ?59, ?60, ?61, ?62, ?63
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
        first_message_at = excluded.first_message_at,
        git_branch = excluded.git_branch,
        is_sidechain = excluded.is_sidechain,
        size_bytes = excluded.size_bytes,
        indexed_at = excluded.indexed_at,
        last_message = excluded.last_message,
        files_touched = excluded.files_touched,
        skills_used = excluded.skills_used,
        tool_counts_edit = excluded.tool_counts_edit,
        tool_counts_read = excluded.tool_counts_read,
        tool_counts_bash = excluded.tool_counts_bash,
        tool_counts_write = excluded.tool_counts_write,
        turn_count = excluded.turn_count,
        deep_indexed_at = excluded.deep_indexed_at,
        parse_version = excluded.parse_version,
        file_size_at_index = excluded.file_size_at_index,
        file_mtime_at_index = excluded.file_mtime_at_index,
        user_prompt_count = excluded.user_prompt_count,
        api_call_count = excluded.api_call_count,
        tool_call_count = excluded.tool_call_count,
        files_read = excluded.files_read,
        files_edited = excluded.files_edited,
        files_read_count = excluded.files_read_count,
        files_edited_count = excluded.files_edited_count,
        reedited_files_count = excluded.reedited_files_count,
        duration_seconds = excluded.duration_seconds,
        commit_count = excluded.commit_count,
        total_input_tokens = excluded.total_input_tokens,
        total_output_tokens = excluded.total_output_tokens,
        cache_read_tokens = excluded.cache_read_tokens,
        cache_creation_tokens = excluded.cache_creation_tokens,
        thinking_block_count = excluded.thinking_block_count,
        turn_duration_avg_ms = excluded.turn_duration_avg_ms,
        turn_duration_max_ms = excluded.turn_duration_max_ms,
        turn_duration_total_ms = excluded.turn_duration_total_ms,
        api_error_count = excluded.api_error_count,
        api_retry_count = excluded.api_retry_count,
        compaction_count = excluded.compaction_count,
        hook_blocked_count = excluded.hook_blocked_count,
        agent_spawn_count = excluded.agent_spawn_count,
        bash_progress_count = excluded.bash_progress_count,
        hook_progress_count = excluded.hook_progress_count,
        mcp_progress_count = excluded.mcp_progress_count,
        summary_text = excluded.summary_text,
        lines_added = excluded.lines_added,
        lines_removed = excluded.lines_removed,
        loc_source = excluded.loc_source,
        ai_lines_added = excluded.ai_lines_added,
        ai_lines_removed = excluded.ai_lines_removed,
        work_type = excluded.work_type,
        primary_model = excluded.primary_model,
        total_task_time_seconds = excluded.total_task_time_seconds,
        longest_task_seconds = excluded.longest_task_seconds,
        longest_task_preview = excluded.longest_task_preview,
        total_cost_usd = excluded.total_cost_usd
"#;

/// Execute the upsert SQL for a single ParsedSession against any sqlx executor.
/// Works with both `&SqlitePool` and `&mut SqliteConnection` (transaction).
///
/// This is the single place where bind ordering is defined — both
/// `upsert_parsed_session()` and `flush_batch()` call this.
pub async fn execute_upsert_parsed_session<'e, E>(
    executor: E,
    s: &ParsedSession,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let indexed_at = chrono::Utc::now().timestamp();

    sqlx::query(UPSERT_SESSION_SQL)
        .bind(&s.id)              // ?1
        .bind(&s.project_id)      // ?2
        .bind(&s.project_display_name) // ?3
        .bind(&s.project_path)    // ?4
        .bind(&s.file_path)       // ?5
        .bind(&s.preview)         // ?6
        .bind(&s.summary)         // ?7
        .bind(s.message_count)    // ?8
        .bind(s.last_message_at)  // ?9
        .bind(s.first_message_at) // ?10
        .bind(&s.git_branch)      // ?11
        .bind(s.is_sidechain)     // ?12
        .bind(s.size_bytes)       // ?13
        .bind(indexed_at)         // ?14
        .bind(&s.last_message)    // ?15
        .bind(&s.files_touched)   // ?16
        .bind(&s.skills_used)     // ?17
        .bind(s.tool_counts_edit) // ?18
        .bind(s.tool_counts_read) // ?19
        .bind(s.tool_counts_bash) // ?20
        .bind(s.tool_counts_write) // ?21
        .bind(s.turn_count)       // ?22
        .bind(s.parse_version)    // ?23
        .bind(s.file_size_at_index) // ?24
        .bind(s.file_mtime_at_index) // ?25
        .bind(s.user_prompt_count) // ?26
        .bind(s.api_call_count)   // ?27
        .bind(s.tool_call_count)  // ?28
        .bind(&s.files_read)      // ?29
        .bind(&s.files_edited)    // ?30
        .bind(s.files_read_count) // ?31
        .bind(s.files_edited_count) // ?32
        .bind(s.reedited_files_count) // ?33
        .bind(s.duration_seconds) // ?34
        .bind(s.commit_count)     // ?35
        .bind(s.total_input_tokens) // ?36
        .bind(s.total_output_tokens) // ?37
        .bind(s.cache_read_tokens) // ?38
        .bind(s.cache_creation_tokens) // ?39
        .bind(s.thinking_block_count) // ?40
        .bind(s.turn_duration_avg_ms) // ?41
        .bind(s.turn_duration_max_ms) // ?42
        .bind(s.turn_duration_total_ms) // ?43
        .bind(s.api_error_count)  // ?44
        .bind(s.api_retry_count)  // ?45
        .bind(s.compaction_count) // ?46
        .bind(s.hook_blocked_count) // ?47
        .bind(s.agent_spawn_count) // ?48
        .bind(s.bash_progress_count) // ?49
        .bind(s.hook_progress_count) // ?50
        .bind(s.mcp_progress_count) // ?51
        .bind(&s.summary_text)    // ?52
        .bind(s.lines_added)      // ?53
        .bind(s.lines_removed)    // ?54
        .bind(s.loc_source)       // ?55
        .bind(s.ai_lines_added)   // ?56
        .bind(s.ai_lines_removed) // ?57
        .bind(&s.work_type)       // ?58
        .bind(&s.primary_model)   // ?59
        .bind(s.total_task_time_seconds) // ?60
        .bind(s.longest_task_seconds)    // ?61
        .bind(&s.longest_task_preview)   // ?62
        .bind(s.total_cost_usd)          // ?63
        .execute(executor)
        .await?;

    Ok(())
}

impl Database {
    /// Upsert a fully-parsed session into the DB.
    ///
    /// This is the ONLY function that writes session data. Every field comes
    /// from the parser — no stubs, no zeros, no partial rows. On conflict,
    /// ALL fields are overwritten unconditionally because the parser is the
    /// single source of truth.
    ///
    /// Delegates to `execute_upsert_parsed_session()` which holds the SQL
    /// and bind chain — shared with `flush_batch()` in the live manager.
    pub async fn upsert_parsed_session(&self, s: &ParsedSession) -> DbResult<()> {
        execute_upsert_parsed_session(self.pool(), s).await?;
        Ok(())
    }

    /// Partial update from live tail — only updates fields that can be
    /// observed from appended JSONL lines. Does NOT overwrite fields
    /// that require a full parse (e.g., duration_seconds, commit_count,
    /// files_touched, skills_used, lines_added/removed).
    ///
    /// Called from the live session manager after each `parse_tail()` poll.
    /// Parameters match the accumulated state from `SessionAccumulator`.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_session_from_tail(
        &self,
        session_id: &str,
        message_count: i32,
        turn_count: i32,
        last_message_at: i64,
        last_message: &str,
        size_bytes: i64,
        file_size_at_index: i64,
        file_mtime_at_index: i64,
        total_input_tokens: i64,
        total_output_tokens: i64,
        cache_read_tokens: i64,
        cache_creation_tokens: i64,
        tool_counts_edit: i32,
        tool_counts_read: i32,
        tool_counts_bash: i32,
        tool_counts_write: i32,
    ) -> DbResult<()> {
        sqlx::query(
            "UPDATE sessions SET
                message_count = ?2, turn_count = ?3, last_message_at = ?4,
                last_message = ?5, size_bytes = ?6, file_size_at_index = ?7,
                file_mtime_at_index = ?8,
                total_input_tokens = ?9, total_output_tokens = ?10,
                cache_read_tokens = ?11, cache_creation_tokens = ?12,
                tool_counts_edit = ?13, tool_counts_read = ?14,
                tool_counts_bash = ?15, tool_counts_write = ?16
            WHERE id = ?1",
        )
        .bind(session_id)           // ?1
        .bind(message_count)        // ?2
        .bind(turn_count)           // ?3
        .bind(last_message_at)      // ?4
        .bind(last_message)         // ?5
        .bind(size_bytes)           // ?6
        .bind(file_size_at_index)   // ?7
        .bind(file_mtime_at_index)  // ?8
        .bind(total_input_tokens)   // ?9
        .bind(total_output_tokens)  // ?10
        .bind(cache_read_tokens)    // ?11
        .bind(cache_creation_tokens) // ?12
        .bind(tool_counts_edit)     // ?13
        .bind(tool_counts_read)     // ?14
        .bind(tool_counts_bash)     // ?15
        .bind(tool_counts_write)    // ?16
        .execute(self.pool())
        .await?;
        Ok(())
    }

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
                duration_seconds, commit_count,
                git_root
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
                ?30, ?31,
                ?32
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
                commit_count = excluded.commit_count,
                git_root = COALESCE(excluded.git_root, sessions.git_root)
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
        .bind(session.git_root.as_deref())
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
                s.category_l1, s.category_l2, s.category_l3,
                s.category_confidence, s.category_source, s.classified_at,
                s.prompt_word_count, s.correction_count, s.same_file_edit_count,
                s.total_task_time_seconds, s.longest_task_seconds, s.longest_task_preview
            FROM valid_sessions s
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

    /// Update session topology fields discovered via content classification.
    /// Called after insert_session_from_index for sessions discovered by
    /// discover_orphan_sessions() which have cwd and parent_id from JSONL.
    pub async fn update_session_topology(
        &self,
        id: &str,
        session_cwd: Option<&str>,
        parent_session_id: Option<&str>,
        git_root: Option<&str>,
    ) -> DbResult<()> {
        sqlx::query(
            "UPDATE sessions SET \
             session_cwd = COALESCE(?1, session_cwd), \
             parent_session_id = COALESCE(?2, parent_session_id), \
             git_root = COALESCE(?3, git_root) \
             WHERE id = ?4",
        )
        .bind(session_cwd)
        .bind(parent_session_id)
        .bind(git_root)
        .bind(id)
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
        total_task_time_seconds: i32,
        longest_task_seconds: Option<i32>,
        longest_task_preview: Option<&str>,
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
                total_task_time_seconds = ?52,
                longest_task_seconds = ?53,
                longest_task_preview = ?54,
                total_cost_usd = ?55
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
        .bind(total_task_time_seconds)
        .bind(longest_task_seconds)
        .bind(longest_task_preview)
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

    /// Fetch all sessions that have session_cwd but no git_root yet.
    /// Returns (id, session_cwd) pairs.
    pub async fn fetch_sessions_needing_git_root(
        &self,
        limit: i64,
    ) -> DbResult<Vec<(String, String)>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT id, session_cwd FROM sessions \
             WHERE git_root IS NULL AND session_cwd IS NOT NULL \
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    /// Set git_root for a single session by id.
    pub async fn set_git_root(&self, id: &str, git_root: &str) -> DbResult<()> {
        sqlx::query("UPDATE sessions SET git_root = ?1 WHERE id = ?2")
            .bind(git_root)
            .bind(id)
            .execute(self.pool())
            .await?;
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

#[cfg(test)]
mod upsert_tests {
    use crate::Database;
    use crate::indexer_parallel::{ParsedSession, CURRENT_PARSE_VERSION};

    fn make_parsed_session(id: &str, message_count: i32) -> ParsedSession {
        ParsedSession {
            id: id.to_string(),
            project_id: "test-project".to_string(),
            project_display_name: "Test Project".to_string(),
            project_path: "/test/project".to_string(),
            file_path: "/test/session.jsonl".to_string(),
            preview: "Hello world".to_string(),
            summary: None,
            message_count,
            last_message_at: 1700000000,
            first_message_at: 1699999000,
            git_branch: None,
            is_sidechain: false,
            size_bytes: 1024,
            last_message: "test message".to_string(),
            turn_count: 5,
            tool_counts_edit: 1,
            tool_counts_read: 2,
            tool_counts_bash: 3,
            tool_counts_write: 0,
            files_touched: "[]".to_string(),
            skills_used: "[]".to_string(),
            user_prompt_count: 5,
            api_call_count: 5,
            tool_call_count: 6,
            files_read: "[]".to_string(),
            files_edited: "[]".to_string(),
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 300,
            commit_count: 1,
            total_input_tokens: 10000,
            total_output_tokens: 5000,
            cache_read_tokens: 2000,
            cache_creation_tokens: 1000,
            thinking_block_count: 3,
            turn_duration_avg_ms: 5000,
            turn_duration_max_ms: 12000,
            turn_duration_total_ms: 25000,
            api_error_count: 0,
            api_retry_count: 0,
            compaction_count: 0,
            hook_blocked_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            summary_text: None,
            parse_version: CURRENT_PARSE_VERSION,
            file_size_at_index: 1024,
            file_mtime_at_index: 1700000000,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            ai_lines_added: 0,
            ai_lines_removed: 0,
            work_type: None,
            primary_model: Some("claude-sonnet-4-5-20250929".to_string()),
            total_task_time_seconds: Some(0),
            longest_task_seconds: Some(0),
            longest_task_preview: None,
            total_cost_usd: 0.05,
        }
    }

    #[tokio::test]
    async fn upsert_inserts_new_session_with_all_fields() {
        let db = Database::new_in_memory().await.unwrap();
        let session = make_parsed_session("sess-001", 42);
        db.upsert_parsed_session(&session).await.unwrap();

        let row = sqlx::query_as::<_, (i32, i32, i64, i64)>(
            "SELECT message_count, turn_count, total_input_tokens, total_output_tokens FROM sessions WHERE id = ?1"
        )
        .bind("sess-001")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 42);    // message_count
        assert_eq!(row.1, 5);     // turn_count
        assert_eq!(row.2, 10000); // total_input_tokens
        assert_eq!(row.3, 5000);  // total_output_tokens
    }

    #[tokio::test]
    async fn upsert_overwrites_all_fields_on_conflict() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert initial
        let session1 = make_parsed_session("sess-002", 10);
        db.upsert_parsed_session(&session1).await.unwrap();

        // Upsert with updated data — simulates re-parse
        let mut session2 = make_parsed_session("sess-002", 50);
        session2.turn_count = 25;
        session2.total_input_tokens = 99999;
        db.upsert_parsed_session(&session2).await.unwrap();

        let row = sqlx::query_as::<_, (i32, i32, i64)>(
            "SELECT message_count, turn_count, total_input_tokens FROM sessions WHERE id = ?1"
        )
        .bind("sess-002")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 50);    // message_count updated, not stuck at 10
        assert_eq!(row.1, 25);    // turn_count updated
        assert_eq!(row.2, 99999); // tokens updated
    }

    #[tokio::test]
    async fn no_ghost_sessions_after_upsert() {
        // Proves the ghost bug is impossible: every row has real data
        let db = Database::new_in_memory().await.unwrap();
        let session = make_parsed_session("sess-003", 42);
        db.upsert_parsed_session(&session).await.unwrap();

        // Query via valid_sessions — must be visible
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM valid_sessions WHERE id = ?1"
        )
        .bind("sess-003")
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(count.0, 1);

        // Verify no zero-count rows exist
        let ghosts: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE message_count = 0 AND last_message_at > 0"
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(ghosts.0, 0);
    }
}
