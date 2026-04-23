// crates/db/src/queries/sessions/upsert_stats.rs
// CQRS Phase 7.h.2: full-row `session_stats` UPSERT from a ParsedSession.
// Extracted from `upsert.rs` in Phase 7.h.3 to stay under the 600-line
// per-file hard stop (Decompose, Don't Monolith).

use chrono::Utc;
use claude_view_session_parser::{PARSER_VERSION, STATS_VERSION};

use crate::indexer_parallel::ParsedSession;

/// Full-row `session_stats` UPSERT from a parsed session.
/// Writes every column the legacy `sessions` table carried plus the
/// `session_stats`-specific header fields. Source of truth is `ParsedSession`
/// (which the parser already populates in full), so this writer needs no new
/// parser work — only a matching SQL statement.
///
/// Coexistence contract with [`crate::indexer_v2::writer::upsert_session_stats`]:
/// - This writer owns the 47 Phase 7.h columns (project_display_name, summary,
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
/// stable) plus one trailing bind for `stats_version`.
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
/// Works against any executor (pool or transaction). Used from every
/// production upsert path in parallel with `execute_upsert_parsed_session`
/// so the two tables stay synchronised during the Phase 7.h cutover.
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
    // retired — documented here because the writer-ownership registry keys
    // off this constant even though the column binds use `s.parse_version`.
    let _ = PARSER_VERSION;

    Ok(())
}
