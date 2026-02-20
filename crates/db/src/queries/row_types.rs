// crates/db/src/queries/row_types.rs
// Internal row types and transaction-accepting helper functions.
// Extracted from queries/mod.rs for reduced merge conflicts.

use crate::DbResult;
use chrono::Utc;
use sqlx::Row;
use claude_view_core::{
    parse_model_id, ClassificationJob, ClassificationJobStatus, IndexRun, IndexRunStatus,
    IndexRunType, RawTurn, SessionInfo, ToolCounts,
};

// ============================================================================
// Theme 4: Internal row types for classification_jobs and index_runs
// ============================================================================

#[derive(Debug)]
pub(crate) struct ClassificationJobRow {
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
    pub(crate) fn into_classification_job(self) -> ClassificationJob {
        ClassificationJob {
            id: self.id,
            started_at: self.started_at,
            completed_at: self.completed_at,
            total_sessions: self.total_sessions,
            classified_count: self.classified_count,
            skipped_count: self.skipped_count,
            failed_count: self.failed_count,
            provider: self.provider,
            model: self.model,
            status: ClassificationJobStatus::from_db_str(&self.status),
            error_message: self.error_message,
            cost_estimate_cents: self.cost_estimate_cents,
            actual_cost_cents: self.actual_cost_cents,
            tokens_used: self.tokens_used,
        }
    }
}

#[derive(Debug)]
pub(crate) struct IndexRunRow {
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
    pub(crate) fn into_index_run(self) -> IndexRun {
        IndexRun {
            id: self.id,
            started_at: self.started_at,
            completed_at: self.completed_at,
            run_type: IndexRunType::from_db_str(&self.run_type),
            sessions_before: self.sessions_before,
            sessions_after: self.sessions_after,
            duration_ms: self.duration_ms,
            throughput_mb_per_sec: self.throughput_mb_per_sec,
            status: IndexRunStatus::from_db_str(&self.status),
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
            longest_task_preview = ?54
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
pub(crate) struct SessionRow {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) preview: String,
    pub(crate) turn_count: i32,
    pub(crate) last_message_at: Option<i64>,
    pub(crate) file_path: String,
    pub(crate) project_path: String,
    pub(crate) project_display_name: String,
    pub(crate) size_bytes: i64,
    pub(crate) last_message: String,
    pub(crate) files_touched: String,
    pub(crate) skills_used: String,
    pub(crate) tool_counts_edit: i32,
    pub(crate) tool_counts_read: i32,
    pub(crate) tool_counts_bash: i32,
    pub(crate) tool_counts_write: i32,
    pub(crate) message_count: i32,
    pub(crate) summary: Option<String>,
    pub(crate) git_branch: Option<String>,
    pub(crate) is_sidechain: bool,
    pub(crate) deep_indexed_at: Option<i64>,
    pub(crate) total_input_tokens: Option<i64>,
    pub(crate) total_output_tokens: Option<i64>,
    pub(crate) total_cache_read_tokens: Option<i64>,
    pub(crate) total_cache_creation_tokens: Option<i64>,
    pub(crate) turn_count_api: Option<i64>,
    pub(crate) primary_model: Option<String>,
    // Phase 3: Atomic unit metrics
    pub(crate) user_prompt_count: i32,
    pub(crate) api_call_count: i32,
    pub(crate) tool_call_count: i32,
    pub(crate) files_read: String,
    pub(crate) files_edited: String,
    pub(crate) files_read_count: i32,
    pub(crate) files_edited_count: i32,
    pub(crate) reedited_files_count: i32,
    pub(crate) duration_seconds: i32,
    #[allow(dead_code)] // Used internally by git sync queries, not by into_session_info()
    pub(crate) first_message_at: Option<i64>,
    pub(crate) commit_count: i32,
    // Phase 3.5: Full parser metrics
    pub(crate) thinking_block_count: i32,
    pub(crate) turn_duration_avg_ms: Option<i64>,
    pub(crate) turn_duration_max_ms: Option<i64>,
    pub(crate) api_error_count: i32,
    pub(crate) compaction_count: i32,
    pub(crate) agent_spawn_count: i32,
    pub(crate) bash_progress_count: i32,
    pub(crate) hook_progress_count: i32,
    pub(crate) mcp_progress_count: i32,
    pub(crate) parse_version: i32,
    // Phase C: LOC estimation
    pub(crate) lines_added: i32,
    pub(crate) lines_removed: i32,
    pub(crate) loc_source: i32,
    // Theme 4: Classification
    pub(crate) category_l1: Option<String>,
    pub(crate) category_l2: Option<String>,
    pub(crate) category_l3: Option<String>,
    pub(crate) category_confidence: Option<f64>,
    pub(crate) category_source: Option<String>,
    pub(crate) classified_at: Option<String>,
    // Theme 4: Behavioral metrics
    pub(crate) prompt_word_count: Option<i32>,
    pub(crate) correction_count: i32,
    pub(crate) same_file_edit_count: i32,
    // Wall-clock task time metrics
    pub(crate) total_task_time_seconds: Option<i32>,
    pub(crate) longest_task_seconds: Option<i32>,
    pub(crate) longest_task_preview: Option<String>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for SessionRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
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
            parse_version: row.try_get("parse_version")?,
            // Phase C: LOC estimation
            lines_added: row.try_get("lines_added")?,
            lines_removed: row.try_get("lines_removed")?,
            loc_source: row.try_get("loc_source")?,
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
            // Wall-clock task time metrics
            total_task_time_seconds: row.try_get("total_task_time_seconds").ok().flatten(),
            longest_task_seconds: row.try_get("longest_task_seconds").ok().flatten(),
            longest_task_preview: row.try_get("longest_task_preview").ok().flatten(),
        })
    }
}

impl SessionRow {
    pub(crate) fn into_session_info(self, project_encoded: &str) -> SessionInfo {
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
            parse_version: self.parse_version as u32,
            // Phase C: LOC estimation
            lines_added: self.lines_added as u32,
            lines_removed: self.lines_removed as u32,
            loc_source: self.loc_source as u8,
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
            // Wall-clock task time metrics
            total_task_time_seconds: self.total_task_time_seconds.map(|v| v as u32),
            longest_task_seconds: self.longest_task_seconds.map(|v| v as u32),
            longest_task_preview: self.longest_task_preview,
        }
    }
}
