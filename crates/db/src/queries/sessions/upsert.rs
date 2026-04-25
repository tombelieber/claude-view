// crates/db/src/queries/sessions/upsert.rs
// Session upsert operations. CQRS Phase 7.h retired the legacy `sessions`
// table; compatibility entry points now write the full row to `session_stats`.

use super::upsert_stats::execute_upsert_session_stats_from_parsed;
use crate::indexer_parallel::{ParsedSession, CURRENT_PARSE_VERSION};
use crate::{Database, DbResult};
use claude_view_core::SessionInfo;

/// Compatibility wrapper for callers that predate the Phase 7.h cutover.
pub async fn execute_upsert_parsed_session<'e, E>(
    executor: E,
    s: &ParsedSession,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    execute_upsert_session_stats_from_parsed(executor, s).await
}

impl Database {
    /// Upsert a fully-parsed session into `session_stats`.
    pub async fn upsert_parsed_session(&self, s: &ParsedSession) -> DbResult<()> {
        execute_upsert_session_stats_from_parsed(self.pool(), s).await?;
        Ok(())
    }

    /// Upsert a SessionInfo-shaped row into `session_stats`.
    ///
    /// This is the legacy route/test helper shape; the parser path should use
    /// `upsert_parsed_session` so every extracted field is populated.
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
        let size_bytes = session.size_bytes as i64;
        let modified_at = session.modified_at;

        let parsed = ParsedSession {
            id: session.id.clone(),
            project_id: project_encoded.to_string(),
            project_display_name: project_display_name.to_string(),
            project_path: session.project_path.clone(),
            file_path: session.file_path.clone(),
            preview: session.preview.clone(),
            summary: session.summary.clone(),
            message_count: session.message_count as i32,
            last_message_at: modified_at,
            first_message_at: session.first_message_at.unwrap_or(modified_at),
            git_branch: session.git_branch.clone(),
            is_sidechain: session.is_sidechain,
            size_bytes,
            last_message: session.last_message.clone(),
            turn_count: session.turn_count as i32,
            tool_counts_edit: session.tool_counts.edit as i32,
            tool_counts_read: session.tool_counts.read as i32,
            tool_counts_bash: session.tool_counts.bash as i32,
            tool_counts_write: session.tool_counts.write as i32,
            files_touched,
            skills_used,
            user_prompt_count: session.user_prompt_count as i32,
            api_call_count: session.api_call_count as i32,
            tool_call_count: session.tool_call_count as i32,
            files_read,
            files_edited,
            files_read_count: session.files_read_count as i32,
            files_edited_count: session.files_edited_count as i32,
            reedited_files_count: session.reedited_files_count as i32,
            duration_seconds: session.duration_seconds as i64,
            commit_count: session.commit_count as i32,
            total_input_tokens: session.total_input_tokens.unwrap_or(0) as i64,
            total_output_tokens: session.total_output_tokens.unwrap_or(0) as i64,
            cache_read_tokens: session.total_cache_read_tokens.unwrap_or(0) as i64,
            cache_creation_tokens: session.total_cache_creation_tokens.unwrap_or(0) as i64,
            thinking_block_count: session.thinking_block_count as i32,
            turn_duration_avg_ms: session.turn_duration_avg_ms.map(|n| n as i64),
            turn_duration_max_ms: session.turn_duration_max_ms.map(|n| n as i64),
            turn_duration_total_ms: None,
            api_error_count: session.api_error_count as i32,
            api_retry_count: 0,
            compaction_count: session.compaction_count as i32,
            hook_blocked_count: 0,
            agent_spawn_count: session.agent_spawn_count as i32,
            bash_progress_count: session.bash_progress_count as i32,
            hook_progress_count: session.hook_progress_count as i32,
            mcp_progress_count: session.mcp_progress_count as i32,
            summary_text: None,
            parse_version: if session.parse_version == 0 {
                CURRENT_PARSE_VERSION
            } else {
                session.parse_version as i32
            },
            file_size_at_index: size_bytes,
            file_mtime_at_index: modified_at,
            lines_added: session.lines_added as i64,
            lines_removed: session.lines_removed as i64,
            loc_source: session.loc_source as i32,
            ai_lines_added: 0,
            ai_lines_removed: 0,
            work_type: None,
            primary_model: session.primary_model.clone(),
            total_task_time_seconds: session.total_task_time_seconds.map(|n| n as i64),
            longest_task_seconds: session.longest_task_seconds.map(|n| n as i64),
            longest_task_preview: session.longest_task_preview.clone(),
            total_cost_usd: session.total_cost_usd,
            slug: session.slug.clone(),
            entrypoint: session.entrypoint.clone(),
        };

        execute_upsert_session_stats_from_parsed(self.pool(), &parsed).await?;

        if session.git_root.is_some() {
            sqlx::query("UPDATE session_stats SET git_root = ?1 WHERE session_id = ?2")
                .bind(session.git_root.as_deref())
                .bind(&session.id)
                .execute(self.pool())
                .await?;
        }

        Ok(())
    }
}
