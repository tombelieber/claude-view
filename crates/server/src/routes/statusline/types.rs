//! Statusline payload types.
//!
//! These structs represent the JSON shape that Claude Code sends on every
//! assistant turn. All fields are optional except `session_id`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslinePayload {
    pub session_id: String,
    pub model: Option<StatuslineModel>,
    pub cwd: Option<String>,
    pub workspace: Option<StatuslineWorkspace>,
    pub cost: Option<StatuslineCost>,
    pub context_window: Option<StatuslineContextWindow>,
    pub exceeds_200k_tokens: Option<bool>,
    pub transcript_path: Option<String>,
    pub version: Option<String>,
    pub output_style: Option<StatuslineOutputStyle>,
    pub vim: Option<StatuslineVim>,
    pub agent: Option<StatuslineAgent>,
    pub worktree: Option<StatuslineWorktree>,
    pub rate_limits: Option<StatuslineRateLimits>,
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineModel {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineContextWindow {
    /// The real context window limit in tokens (200_000 or 1_000_000).
    /// This is the authoritative value -- no guessing needed.
    pub context_window_size: Option<u32>,
    /// Pre-computed percentage used (0.0-100.0). Null early in session.
    pub used_percentage: Option<f64>,
    /// Pre-computed percentage remaining (0.0-100.0).
    pub remaining_percentage: Option<f64>,
    /// Current turn token usage breakdown. Claude Code sends this as an object
    /// with per-category token counts, not a single integer.
    pub current_usage: Option<StatuslineCurrentUsage>,
    /// Cumulative input tokens across the session.
    pub total_input_tokens: Option<u64>,
    /// Cumulative output tokens across the session.
    pub total_output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineCurrentUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineCost {
    /// Claude Code's own total cost calculation in USD.
    pub total_cost_usd: Option<f64>,
    /// Total wall-clock session duration in milliseconds.
    pub total_duration_ms: Option<u64>,
    /// API-only duration in milliseconds.
    pub total_api_duration_ms: Option<u64>,
    /// Total lines added across the session.
    pub total_lines_added: Option<u64>,
    /// Total lines removed across the session.
    pub total_lines_removed: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineWorkspace {
    pub current_dir: Option<String>,
    pub project_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineOutputStyle {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineVim {
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineAgent {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineWorktree {
    pub name: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
    pub original_cwd: Option<String>,
    pub original_branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineRateLimits {
    pub five_hour: Option<StatuslineRateLimitWindow>,
    pub seven_day: Option<StatuslineRateLimitWindow>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineRateLimitWindow {
    pub used_percentage: Option<f64>,
    pub resets_at: Option<i64>,
}
