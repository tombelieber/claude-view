// crates/db/src/queries/sessions/update.rs
// Session update operations: tail updates, topology, deep fields, classification.

use crate::queries::action_log::insert_action_log_tx;
use crate::{Database, DbResult};
use chrono::Utc;

impl Database {
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
            "UPDATE session_stats SET \
             session_cwd = COALESCE(?1, session_cwd), \
             parent_session_id = COALESCE(?2, parent_session_id), \
             git_root = COALESCE(?3, git_root) \
             WHERE session_id = ?4",
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
    #[deprecated(
        note = "Legacy two-pass pipeline. Use scan_and_index_all + upsert_parsed_session instead."
    )]
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
        total_cost_usd: Option<f64>,
    ) -> DbResult<()> {
        let deep_indexed_at = Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE session_stats SET
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
            WHERE session_id = ?1
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
        let classified_at_ms = Utc::now().timestamp_millis();
        let payload = serde_json::json!({
            "l1": category_l1,
            "l2": category_l2,
            "l3": category_l3,
            "confidence": confidence,
            "source": source,
        })
        .to_string();
        let actor = format!("classifier:{source}");

        let mut tx = self.pool().begin().await?;
        insert_action_log_tx(
            &mut *tx,
            session_id,
            "classify",
            &payload,
            &actor,
            classified_at_ms,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Fetch all sessions that have session_cwd but no git_root yet.
    /// Returns (id, session_cwd) pairs.
    pub async fn fetch_sessions_needing_git_root(
        &self,
        limit: i64,
    ) -> DbResult<Vec<(String, String)>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT session_id, session_cwd FROM session_stats \
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
        sqlx::query("UPDATE session_stats SET git_root = ?1 WHERE session_id = ?2")
            .bind(git_root)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Update a session's file_path in the DB (e.g. after moving to/from archive).
    pub async fn update_session_file_path(
        &self,
        session_id: &str,
        new_path: &str,
    ) -> DbResult<bool> {
        let rows = sqlx::query("UPDATE session_stats SET file_path = ?1 WHERE session_id = ?2")
            .bind(new_path)
            .bind(session_id)
            .execute(self.pool())
            .await?
            .rows_affected();
        Ok(rows > 0)
    }
}
