// crates/db/src/queries/dashboard/session_queries.rs
// Flat session listing across all projects.
//
// The filtered/paginated list path was removed once `/api/sessions` became
// JSONL-first (see `crates/server/src/routes/sessions/list.rs`). The DB
// is consulted for enrichment only — see `routes/sessions/enrichment.rs`.

use super::super::row_types::SessionRow;
use crate::{Database, DbResult};
use claude_view_core::SessionInfo;

impl Database {
    /// List all non-sidechain sessions across all projects.
    ///
    /// Flat query — no project grouping, no turns JOIN.
    /// Returns sessions sorted by `last_message_at` DESC.
    pub async fn list_all_sessions(&self) -> DbResult<Vec<SessionInfo>> {
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
                s.total_task_time_seconds, s.longest_task_seconds, s.longest_task_preview,
                s.total_cost_usd,
                s.slug,
                s.entrypoint
            FROM valid_sessions s
            ORDER BY s.last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        let sessions = rows
            .into_iter()
            .map(|r| {
                let pid = r.project_id.clone();
                r.into_session_info(&pid)
            })
            .collect();

        Ok(sessions)
    }
}
