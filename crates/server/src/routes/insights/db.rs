//! Database row types and conversion helpers for insights queries.

use std::sync::Arc;

use claude_view_core::types::SessionInfo;
use claude_view_core::{AnalyticsScopeMeta, AnalyticsSessionBreakdown};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ============================================================================
// Lightweight DB row types (used for aggregate queries)
// ============================================================================

/// Lightweight session data for pattern computation (no full JSONL parse).
pub(super) struct LightSession {
    pub id: String,
    pub project_id: String,
    pub project_path: String,
    pub project_display_name: String,
    pub file_path: String,
    pub last_message_at: Option<i64>,
    pub duration_seconds: i32,
    pub files_edited_count: i32,
    pub files_read_count: i32,
    pub reedited_files_count: i32,
    pub user_prompt_count: i32,
    pub api_call_count: i32,
    pub tool_call_count: i32,
    pub commit_count: i32,
    pub turn_count: i32,
    pub tool_counts_edit: i32,
    pub tool_counts_read: i32,
    pub tool_counts_bash: i32,
    pub tool_counts_write: i32,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
    pub primary_model: Option<String>,
    pub git_branch: Option<String>,
    pub files_edited: String,
    pub files_read: String,
    pub category_l1: Option<String>,
    pub size_bytes: i64,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for LightSession {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            project_path: row.try_get("project_path")?,
            project_display_name: row.try_get("project_display_name")?,
            file_path: row.try_get("file_path")?,
            last_message_at: row.try_get("last_message_at")?,
            duration_seconds: row.try_get("duration_seconds")?,
            files_edited_count: row.try_get("files_edited_count")?,
            files_read_count: row.try_get("files_read_count")?,
            reedited_files_count: row.try_get("reedited_files_count")?,
            user_prompt_count: row.try_get("user_prompt_count")?,
            api_call_count: row.try_get("api_call_count")?,
            tool_call_count: row.try_get("tool_call_count")?,
            commit_count: row.try_get("commit_count")?,
            turn_count: row.try_get("turn_count")?,
            tool_counts_edit: row.try_get("tool_counts_edit")?,
            tool_counts_read: row.try_get("tool_counts_read")?,
            tool_counts_bash: row.try_get("tool_counts_bash")?,
            tool_counts_write: row.try_get("tool_counts_write")?,
            total_input_tokens: row.try_get("total_input_tokens").ok().flatten(),
            total_output_tokens: row.try_get("total_output_tokens").ok().flatten(),
            primary_model: row.try_get("primary_model").ok().flatten(),
            git_branch: row.try_get("git_branch").ok().flatten(),
            files_edited: row.try_get("files_edited")?,
            files_read: row.try_get("files_read")?,
            category_l1: row.try_get("category_l1").ok().flatten(),
            size_bytes: row.try_get("size_bytes")?,
        })
    }
}

impl LightSession {
    /// Convert to SessionInfo for the pattern engine.
    pub(super) fn into_session_info(self) -> SessionInfo {
        let files_edited: Vec<String> = match serde_json::from_str(&self.files_edited) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "corrupt files_edited in DB, using empty default");
                Vec::new()
            }
        };
        let files_read: Vec<String> = match serde_json::from_str(&self.files_read) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "corrupt files_read in DB, using empty default");
                Vec::new()
            }
        };

        SessionInfo {
            id: self.id,
            project: self.project_id.clone(),
            project_path: self.project_path,
            display_name: self.project_display_name,
            git_root: None,
            file_path: self.file_path,
            modified_at: self.last_message_at.filter(|&ts| ts > 0).unwrap_or(0),
            size_bytes: self.size_bytes as u64,
            preview: String::new(),
            last_message: String::new(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: claude_view_core::ToolCounts {
                edit: self.tool_counts_edit as usize,
                read: self.tool_counts_read as usize,
                bash: self.tool_counts_bash as usize,
                write: self.tool_counts_write as usize,
            },
            message_count: 0,
            turn_count: self.turn_count as usize,
            summary: None,
            git_branch: self.git_branch,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: self.total_input_tokens.map(|v| v as u64),
            total_output_tokens: self.total_output_tokens.map(|v| v as u64),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: self.primary_model,
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
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            parse_version: 0,
            category_l1: self.category_l1,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        }
    }
}

/// Fetch analytics scope metadata (primary vs sidechain breakdown) for a time range.
pub(super) async fn fetch_analytics_scope_meta_for_range(
    state: &Arc<AppState>,
    from: i64,
    to: i64,
) -> ApiResult<AnalyticsScopeMeta> {
    // CQRS Phase 7.c — is_sidechain now reads from session_stats; archived_at from session_flags.
    let (primary_sessions, sidechain_sessions): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN ss.is_sidechain = 0 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN ss.is_sidechain = 1 THEN 1 ELSE 0 END), 0)
        FROM session_stats ss
        LEFT JOIN session_flags sf ON sf.session_id = ss.session_id
        WHERE sf.archived_at IS NULL
          AND ss.last_message_at >= ?1
          AND ss.last_message_at <= ?2
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch insights session breakdown: {e}")))?;

    Ok(AnalyticsScopeMeta::new(AnalyticsSessionBreakdown::new(
        primary_sessions,
        sidechain_sessions,
    )))
}
