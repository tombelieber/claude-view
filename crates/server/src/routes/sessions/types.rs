//! Request/response types, query structs, and constants for session endpoints.

use claude_view_core::task_files::TaskItem;
use claude_view_core::todo_files::AgentTodos;
use claude_view_core::SessionInfo;
use claude_view_db::git_correlation::GitCommit;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Archive request/response types
// ============================================================================

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkArchiveRequest {
    pub ids: Vec<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ArchiveResponse {
    pub archived: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BulkArchiveResponse {
    pub archived_count: usize,
}

// ============================================================================
// Filter and Sort Enums
// ============================================================================

/// Valid filter values for GET /api/sessions
pub(crate) const VALID_FILTERS: &[&str] = &["all", "has_commits", "high_reedit", "long_session"];

/// Valid sort values for GET /api/sessions
pub(crate) const VALID_SORTS: &[&str] =
    &["recent", "tokens", "prompts", "files_edited", "duration"];

/// Query parameters for GET /api/sessions
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
#[serde(default)]
pub struct SessionsListQuery {
    /// Filter: all (default), has_commits, high_reedit, long_session (kept for backward compat)
    pub filter: Option<String>,
    /// Sort: recent (default), tokens, prompts, files_edited, duration
    pub sort: Option<String>,
    /// Pagination limit (default 30)
    pub limit: Option<i64>,
    /// Pagination offset (default 0)
    pub offset: Option<i64>,
    /// Text search across preview, last_message, project name
    pub q: Option<String>,
    // New multi-facet filters
    /// Comma-separated list of branches to filter by
    pub branches: Option<String>,
    /// Comma-separated list of models to filter by
    pub models: Option<String>,
    /// Filter sessions with commits (true) or without (false)
    pub has_commits: Option<bool>,
    /// Filter sessions with skills (true) or without (false)
    pub has_skills: Option<bool>,
    /// Minimum duration in seconds
    pub min_duration: Option<i64>,
    /// Minimum number of files edited
    pub min_files: Option<i64>,
    /// Minimum total tokens (input + output)
    pub min_tokens: Option<i64>,
    /// Filter sessions with high re-edit rate (> 0.2)
    pub high_reedit: Option<bool>,
    /// Filter sessions after this timestamp (unix seconds)
    pub time_after: Option<i64>,
    /// Filter sessions before this timestamp (unix seconds)
    pub time_before: Option<i64>,
    /// Optional project filter (matches project_id or git_root)
    pub project: Option<String>,
    /// Include archived sessions (queries `sessions` table instead of `valid_sessions` view)
    pub show_archived: Option<bool>,
}

/// Response for GET /api/sessions with pagination
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionsListResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
    pub has_more: bool,
    pub filter: String,
    pub sort: String,
}

/// Response for GET /api/sessions/activity
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionActivityResponse {
    pub activity: Vec<claude_view_db::ActivityPoint>,
    pub bucket: String,
    /// True total from valid_sessions (includes sessions with last_message_at=0
    /// that can't be placed on the chart axis).
    pub total: usize,
}

// ============================================================================
// Session Detail Types (Step 21)
// ============================================================================

/// Extended session detail with commits (for GET /api/sessions/:id)
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    #[serde(flatten)]
    pub info: SessionInfo,
    pub commits: Vec<CommitWithTier>,
    pub derived_metrics: DerivedMetrics,
    /// Persistent task data from ~/.claude/tasks/{sessionId}/*.json
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskItem>,
    /// Agent-level todo checklists from ~/.claude/todos/{sessionId}-agent-{agentId}.json
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub todos: Vec<AgentTodos>,
    /// Whether plan files exist for this session's slug
    pub has_plans: bool,
    /// Warnings for non-fatal data read failures (e.g. task/plan file errors)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// A commit linked to a session with its confidence tier
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CommitWithTier {
    pub hash: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[ts(type = "number")]
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Tier 1 = high confidence (commit skill), Tier 2 = medium (during session)
    pub tier: i32,
}

impl From<(GitCommit, i32, String)> for CommitWithTier {
    fn from((commit, tier, _evidence): (GitCommit, i32, String)) -> Self {
        Self {
            hash: commit.hash,
            message: commit.message,
            author: commit.author,
            timestamp: commit.timestamp,
            branch: commit.branch,
            tier,
        }
    }
}

/// Derived metrics calculated from atomic units
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DerivedMetrics {
    /// Tokens per prompt: (total_input + total_output) / user_prompt_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_prompt: Option<f64>,
    /// Re-edit rate: reedited_files_count / files_edited_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reedit_rate: Option<f64>,
    /// Tool density: tool_call_count / api_call_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_density: Option<f64>,
    /// Edit velocity: files_edited_count / (duration_seconds / 60)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_velocity: Option<f64>,
    /// Read-to-edit ratio: files_read_count / files_edited_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_to_edit_ratio: Option<f64>,
}

impl From<&SessionInfo> for DerivedMetrics {
    fn from(s: &SessionInfo) -> Self {
        Self {
            tokens_per_prompt: s.tokens_per_prompt(),
            reedit_rate: s.reedit_rate(),
            tool_density: s.tool_density(),
            edit_velocity: s.edit_velocity(),
            read_to_edit_ratio: s.read_to_edit_ratio(),
        }
    }
}

// ============================================================================
// Paginated Messages Query
// ============================================================================

/// Query parameters for GET /api/sessions/:id/messages
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
#[serde(default)]
pub struct SessionMessagesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub raw: bool,
    /// "block" → return ConversationBlock[], otherwise legacy Message[]
    pub format: Option<String>,
}

/// Paginated response for `?format=block`
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedBlocks {
    pub blocks: Vec<claude_view_core::ConversationBlock>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
}

// ============================================================================
// Cost Estimation types
// ============================================================================

/// Request body for cost estimation.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct EstimateRequest {
    pub session_id: String,
    pub model: Option<String>,
}

/// Cost estimation response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CostEstimate {
    pub session_id: String,
    pub history_tokens: u64,
    pub cache_warm: bool,
    pub first_message_cost: Option<f64>,
    pub per_message_cost: Option<f64>,
    pub has_pricing: bool,
    pub model: String,
    pub explanation: String,
    pub session_title: Option<String>,
    pub project_name: Option<String>,
    pub turn_count: u32,
    pub files_edited: u32,
    pub last_active_secs_ago: i64,
}

/// Query parameters for the sparkline activity histogram.
#[derive(Debug, serde::Deserialize)]
pub struct SparklineActivityParams {
    pub time_after: Option<i64>,
    pub time_before: Option<i64>,
}

/// Query parameters for rich activity endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct RichActivityParams {
    pub time_after: Option<i64>,
    pub time_before: Option<i64>,
    pub project: Option<String>,
    pub branch: Option<String>,
}
