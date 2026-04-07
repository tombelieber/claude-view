//! Statusline-derived fields with typed merge strategies.

use crate::live::mutation::merge::{Latest, Monotonic, Transient};
use serde::Serialize;
use ts_rs::TS;

/// Statusline-derived fields, grouped by merge strategy.
///
/// `#[serde(flatten)]` on the parent ensures JSON keys are identical to the old
/// flat layout. Each field uses `Monotonic<T>`, `Latest<T>`, or `Transient<T>`
/// to enforce correct merge semantics at compile time.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct StatuslineFields {
    // -- Monotonic: value only goes up within a session --
    // NOTE: All fields use #[ts(type = "...")] to bypass TS trait bounds on newtypes.
    /// Claude Code's own total cost in USD, from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_cost_usd: Monotonic<f64>,

    /// Wall-clock session duration from statusline cost.total_duration_ms.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_total_duration_ms: Monotonic<u64>,

    /// API-only duration from statusline cost.total_api_duration_ms.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_api_duration_ms: Monotonic<u64>,

    /// Total lines added from statusline cost.total_lines_added.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_lines_added: Monotonic<u64>,

    /// Total lines removed from statusline cost.total_lines_removed.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_lines_removed: Monotonic<u64>,

    /// Cumulative input tokens across the session from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_total_input_tokens: Monotonic<u64>,

    /// Cumulative output tokens across the session from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_total_output_tokens: Monotonic<u64>,

    // -- Latest: newest non-null wins --
    /// Authoritative context window size from statusline (200_000 or 1_000_000).
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_context_window_size: Latest<u32>,

    /// Authoritative context used percentage from statusline (0.0-100.0).
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_used_pct: Latest<f32>,

    /// Remaining context window percentage from statusline (0.0-100.0).
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_remaining_pct: Latest<f32>,

    /// Working directory from statusline workspace.current_dir.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_cwd: Latest<String>,

    /// Project directory from statusline workspace.project_dir.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_project_dir: Latest<String>,

    /// Claude Code version from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_version: Latest<String>,

    /// Transcript path from statusline (used for session dedup).
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_transcript_path: Latest<String>,

    /// 5-hour rate limit used percentage from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_5h_pct: Latest<f64>,

    /// 5-hour rate limit reset timestamp (Unix seconds) from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_5h_resets_at: Latest<i64>,

    /// 7-day rate limit used percentage from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_7d_pct: Latest<f64>,

    /// 7-day rate limit reset timestamp (Unix seconds) from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_7d_resets_at: Latest<i64>,

    // -- Transient: absence = cleared --
    /// Current turn input tokens from statusline current_usage.input_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_input_tokens: Transient<u64>,

    /// Current turn output tokens from statusline current_usage.output_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_output_tokens: Transient<u64>,

    /// Cache read tokens from statusline current_usage.cache_read_input_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_cache_read_tokens: Transient<u64>,

    /// Cache creation tokens from statusline current_usage.cache_creation_input_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_cache_creation_tokens: Transient<u64>,

    /// Whether the session exceeds 200K tokens (from statusline).
    #[ts(type = "boolean | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub exceeds_200k_tokens: Transient<bool>,

    /// Output style name from statusline (e.g. "default", "concise").
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_output_style: Transient<String>,

    /// Vim mode from statusline (e.g. "NORMAL", "INSERT").
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_vim_mode: Transient<String>,

    /// Subagent name from statusline (e.g. "code-reviewer").
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_agent_name: Transient<String>,

    /// Worktree name from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_name: Transient<String>,

    /// Worktree path from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_path: Transient<String>,

    /// Worktree branch from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_branch: Transient<String>,

    /// Worktree original cwd from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_original_cwd: Transient<String>,

    /// Worktree original branch from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_original_branch: Transient<String>,

    // -- Raw: rolling debug buffer (not serialized) --
    /// Last N raw statusline payloads for debugging. NOT serialized to SSE.
    /// Newest at back. Capped at MAX_STATUSLINE_DEBUG_ENTRIES.
    #[serde(skip)]
    #[ts(skip)]
    pub statusline_debug_log: std::collections::VecDeque<StatuslineDebugEntry>,
}

/// Max entries in the per-session statusline debug ring buffer.
pub const MAX_STATUSLINE_DEBUG_ENTRIES: usize = 20;

/// A timestamped raw statusline payload for debugging.
#[derive(Debug, Clone)]
pub struct StatuslineDebugEntry {
    pub received_at: i64,
    pub payload: serde_json::Value,
    /// Which top-level blocks were present (quick scan without reading payload).
    pub blocks_present: Vec<String>,
}
