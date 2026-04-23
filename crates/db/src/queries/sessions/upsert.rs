// crates/db/src/queries/sessions/upsert.rs
// Session upsert operations: full-parse upsert SQL, bind chain, and insert_session.

use crate::{Database, DbResult};
use chrono::Utc;
use claude_view_core::SessionInfo;
use claude_view_session_parser::{PARSER_VERSION, STATS_VERSION};

use crate::indexer_parallel::ParsedSession;

/// The SQL for upserting a fully-parsed session. Shared between
/// `upsert_parsed_session()` (pool executor) and `flush_batch()` (tx executor).
/// 65 bind parameters.
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
        longest_task_seconds, longest_task_preview, total_cost_usd,
        slug, entrypoint
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
        NULLIF(TRIM(?11), ''), ?12, ?13, ?14,
        ?15, ?16, ?17, ?18, ?19, ?20, ?21,
        ?22, ?14, ?23, ?24, ?25,
        ?26, ?27, ?28, ?29, ?30, ?31, ?32,
        ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40,
        ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48,
        ?49, ?50, ?51, ?52, ?53, ?54, ?55,
        ?56, ?57, ?58, ?59, ?60, ?61, ?62, ?63,
        ?64, ?65
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
        total_cost_usd = excluded.total_cost_usd,
        slug = excluded.slug,
        entrypoint = COALESCE(excluded.entrypoint, sessions.entrypoint)
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
        .bind(&s.id) // ?1
        .bind(&s.project_id) // ?2
        .bind(&s.project_display_name) // ?3
        .bind(&s.project_path) // ?4
        .bind(&s.file_path) // ?5
        .bind(&s.preview) // ?6
        .bind(&s.summary) // ?7
        .bind(s.message_count) // ?8
        .bind(s.last_message_at) // ?9
        .bind(s.first_message_at) // ?10
        .bind(&s.git_branch) // ?11
        .bind(s.is_sidechain) // ?12
        .bind(s.size_bytes) // ?13
        .bind(indexed_at) // ?14
        .bind(&s.last_message) // ?15
        .bind(&s.files_touched) // ?16
        .bind(&s.skills_used) // ?17
        .bind(s.tool_counts_edit) // ?18
        .bind(s.tool_counts_read) // ?19
        .bind(s.tool_counts_bash) // ?20
        .bind(s.tool_counts_write) // ?21
        .bind(s.turn_count) // ?22
        .bind(s.parse_version) // ?23
        .bind(s.file_size_at_index) // ?24
        .bind(s.file_mtime_at_index) // ?25
        .bind(s.user_prompt_count) // ?26
        .bind(s.api_call_count) // ?27
        .bind(s.tool_call_count) // ?28
        .bind(&s.files_read) // ?29
        .bind(&s.files_edited) // ?30
        .bind(s.files_read_count) // ?31
        .bind(s.files_edited_count) // ?32
        .bind(s.reedited_files_count) // ?33
        .bind(s.duration_seconds) // ?34
        .bind(s.commit_count) // ?35
        .bind(s.total_input_tokens) // ?36
        .bind(s.total_output_tokens) // ?37
        .bind(s.cache_read_tokens) // ?38
        .bind(s.cache_creation_tokens) // ?39
        .bind(s.thinking_block_count) // ?40
        .bind(s.turn_duration_avg_ms) // ?41
        .bind(s.turn_duration_max_ms) // ?42
        .bind(s.turn_duration_total_ms) // ?43
        .bind(s.api_error_count) // ?44
        .bind(s.api_retry_count) // ?45
        .bind(s.compaction_count) // ?46
        .bind(s.hook_blocked_count) // ?47
        .bind(s.agent_spawn_count) // ?48
        .bind(s.bash_progress_count) // ?49
        .bind(s.hook_progress_count) // ?50
        .bind(s.mcp_progress_count) // ?51
        .bind(&s.summary_text) // ?52
        .bind(s.lines_added) // ?53
        .bind(s.lines_removed) // ?54
        .bind(s.loc_source) // ?55
        .bind(s.ai_lines_added) // ?56
        .bind(s.ai_lines_removed) // ?57
        .bind(&s.work_type) // ?58
        .bind(&s.primary_model) // ?59
        .bind(s.total_task_time_seconds) // ?60
        .bind(s.longest_task_seconds) // ?61
        .bind(&s.longest_task_preview) // ?62
        .bind(s.total_cost_usd) // ?63
        .bind(&s.slug) // ?64
        .bind(&s.entrypoint) // ?65
        .execute(executor)
        .await?;

    Ok(())
}

/// CQRS Phase 7.h.2: full-row `session_stats` UPSERT from a parsed session.
/// Writes every column the legacy `sessions` table carried plus the
/// `session_stats`-specific header fields. Source of truth is `ParsedSession`
/// (which the parser already populates in full), so this writer needs no new
/// parser work — only a matching SQL statement.
///
/// Coexistence contract with [`crate::indexer_v2::writer::upsert_session_stats`]:
/// - This writer owns the 42 Phase 7.h columns (project_display_name, summary,
///   tool_counts_*, files_touched, turn_duration_*, compaction/hook/mcp progress
///   counters, loc fields, task-time fields, slug/entrypoint/git_root, etc.) and
///   the common fields the parser can populate (tokens, counts, timestamps,
///   git_branch, primary_model).
/// - The StatsDelta writer owns `source_content_hash`, `source_inode`,
///   `source_mid_hash`, `line_count`, `cache_creation_5m_tokens`,
///   `cache_creation_1hr_tokens`, `per_model_tokens_json`, `invocation_counts`.
///   When writing via this function we leave them as DEFAULT / NULL in the
///   INSERT and DO NOT overwrite them in the ON CONFLICT clause, so whichever
///   writer got there first wins for those columns.
///
/// 65 bind parameters (same shape as UPSERT_SESSION_SQL to keep mental model
/// stable for 7.h.3 where callers flip).
pub const UPSERT_SESSION_STATS_FROM_PARSED_SQL: &str = r#"
    INSERT INTO session_stats (
        session_id, project_id, project_display_name, project_path,
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
        longest_task_seconds, longest_task_preview, total_cost_usd,
        slug, entrypoint,
        -- session_stats header columns the StatsDelta writer owns. We set
        -- them to safe defaults on INSERT so the NOT NULL constraints are
        -- satisfied; ON CONFLICT DO NOT update them (coexistence contract).
        source_content_hash, source_size,
        parser_version, stats_version,
        bash_count,
        source_mtime
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
        NULLIF(TRIM(?11), ''), ?12, ?13, ?14,
        ?15, ?16, ?17, ?18, ?19, ?20, ?21,
        ?22, ?14, ?23, ?24, ?25,
        ?26, ?27, ?28, ?29, ?30, ?31, ?32,
        ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40,
        ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48,
        ?49, ?50, ?51, ?52, ?53, ?54, ?55,
        ?56, ?57, ?58, ?59, ?60, ?61, ?62, ?63,
        ?64, ?65,
        X'', ?13,
        ?23, ?66,
        ?20,
        ?25
    )
    ON CONFLICT(session_id) DO UPDATE SET
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
        total_cost_usd = excluded.total_cost_usd,
        slug = excluded.slug,
        entrypoint = COALESCE(excluded.entrypoint, session_stats.entrypoint),
        bash_count = excluded.bash_count
"#;

/// Execute the session_stats full-row UPSERT from a ParsedSession.
///
/// Bind chain identical to [`execute_upsert_parsed_session`] plus one
/// trailing bind for `stats_version`. Works against any executor
/// (pool or transaction). No caller wires this up yet — Phase 7.h.3
/// flips `upsert_parsed_session` to call this instead of the legacy
/// `INSERT INTO sessions` path.
pub async fn execute_upsert_session_stats_from_parsed<'e, E>(
    executor: E,
    s: &ParsedSession,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let indexed_at = Utc::now().timestamp();

    sqlx::query(UPSERT_SESSION_STATS_FROM_PARSED_SQL)
        .bind(&s.id) // ?1
        .bind(&s.project_id) // ?2
        .bind(&s.project_display_name) // ?3
        .bind(&s.project_path) // ?4
        .bind(&s.file_path) // ?5
        .bind(&s.preview) // ?6
        .bind(&s.summary) // ?7
        .bind(s.message_count) // ?8
        .bind(s.last_message_at) // ?9
        .bind(s.first_message_at) // ?10
        .bind(&s.git_branch) // ?11
        .bind(s.is_sidechain) // ?12
        .bind(s.size_bytes) // ?13  (reused for source_size default)
        .bind(indexed_at) // ?14
        .bind(&s.last_message) // ?15
        .bind(&s.files_touched) // ?16
        .bind(&s.skills_used) // ?17
        .bind(s.tool_counts_edit) // ?18
        .bind(s.tool_counts_read) // ?19
        .bind(s.tool_counts_bash) // ?20 (reused for bash_count default)
        .bind(s.tool_counts_write) // ?21
        .bind(s.turn_count) // ?22
        .bind(s.parse_version) // ?23 (reused for parser_version default)
        .bind(s.file_size_at_index) // ?24
        .bind(s.file_mtime_at_index) // ?25 (reused for source_mtime default)
        .bind(s.user_prompt_count) // ?26
        .bind(s.api_call_count) // ?27
        .bind(s.tool_call_count) // ?28
        .bind(&s.files_read) // ?29
        .bind(&s.files_edited) // ?30
        .bind(s.files_read_count) // ?31
        .bind(s.files_edited_count) // ?32
        .bind(s.reedited_files_count) // ?33
        .bind(s.duration_seconds) // ?34
        .bind(s.commit_count) // ?35
        .bind(s.total_input_tokens) // ?36
        .bind(s.total_output_tokens) // ?37
        .bind(s.cache_read_tokens) // ?38
        .bind(s.cache_creation_tokens) // ?39
        .bind(s.thinking_block_count) // ?40
        .bind(s.turn_duration_avg_ms) // ?41
        .bind(s.turn_duration_max_ms) // ?42
        .bind(s.turn_duration_total_ms) // ?43
        .bind(s.api_error_count) // ?44
        .bind(s.api_retry_count) // ?45
        .bind(s.compaction_count) // ?46
        .bind(s.hook_blocked_count) // ?47
        .bind(s.agent_spawn_count) // ?48
        .bind(s.bash_progress_count) // ?49
        .bind(s.hook_progress_count) // ?50
        .bind(s.mcp_progress_count) // ?51
        .bind(&s.summary_text) // ?52
        .bind(s.lines_added) // ?53
        .bind(s.lines_removed) // ?54
        .bind(s.loc_source) // ?55
        .bind(s.ai_lines_added) // ?56
        .bind(s.ai_lines_removed) // ?57
        .bind(&s.work_type) // ?58
        .bind(&s.primary_model) // ?59
        .bind(s.total_task_time_seconds) // ?60
        .bind(s.longest_task_seconds) // ?61
        .bind(&s.longest_task_preview) // ?62
        .bind(s.total_cost_usd) // ?63
        .bind(&s.slug) // ?64
        .bind(&s.entrypoint) // ?65
        .bind(i64::from(STATS_VERSION.0)) // ?66 stats_version default for INSERT
        .execute(executor)
        .await?;

    // Touch PARSER_VERSION so the build fails loudly if the constant is
    // retired — we only bind the session-level `parse_version` here, but
    // the writer ownership registry also depends on the canonical parser
    // version for downstream consumers (docs + gate scripts key off it).
    let _ = PARSER_VERSION;

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
    /// CQRS Phase 7.h.3: dual-writes to legacy `sessions` AND `session_stats`.
    /// The session_stats write is additive (leaves StatsDelta-owned header
    /// columns alone on conflict) so the two writers coexist cleanly. Phase
    /// 7.h.4 drops the sessions write once readers flip to session_stats.
    pub async fn upsert_parsed_session(&self, s: &ParsedSession) -> DbResult<()> {
        execute_upsert_parsed_session(self.pool(), s).await?;
        execute_upsert_session_stats_from_parsed(self.pool(), s).await?;
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
        let files_touched =
            serde_json::to_string(&session.files_touched).unwrap_or_else(|_| "[]".to_string());
        let skills_used =
            serde_json::to_string(&session.skills_used).unwrap_or_else(|_| "[]".to_string());
        let files_read =
            serde_json::to_string(&session.files_read).unwrap_or_else(|_| "[]".to_string());
        let files_edited =
            serde_json::to_string(&session.files_edited).unwrap_or_else(|_| "[]".to_string());
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
                git_root, entrypoint
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
                ?32, ?33
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
                git_root = COALESCE(excluded.git_root, sessions.git_root),
                entrypoint = COALESCE(excluded.entrypoint, sessions.entrypoint)
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
        .bind(session.entrypoint.as_deref())
        .execute(self.pool())
        .await?;

        // Also insert minimal session_stats row for Phase 7.c queries
        // (reader code now reads from session_stats as primary table)
        sqlx::query(
            r#"
            INSERT INTO session_stats (
                session_id, source_content_hash, source_size, parser_version,
                stats_version, indexed_at, last_message_at, is_sidechain,
                commit_count, reedited_files_count, files_edited_count, git_branch,
                skills_used
            )
            VALUES (?1, X'00', ?2, 1, 3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(session_id) DO UPDATE SET
                last_message_at = excluded.last_message_at,
                is_sidechain = excluded.is_sidechain,
                commit_count = excluded.commit_count,
                reedited_files_count = excluded.reedited_files_count,
                files_edited_count = excluded.files_edited_count,
                git_branch = excluded.git_branch,
                skills_used = excluded.skills_used
            "#,
        )
        .bind(&session.id)
        .bind(size_bytes)
        .bind(indexed_at)
        .bind(session.modified_at)
        .bind(session.is_sidechain as i64)
        .bind(session.commit_count as i64)
        .bind(session.reedited_files_count as i64)
        .bind(session.files_edited_count as i64)
        .bind(&session.git_branch)
        .bind(&skills_used)
        .execute(self.pool())
        .await?;

        Ok(())
    }
}
