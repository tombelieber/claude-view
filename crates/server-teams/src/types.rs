//! API response types and cost types for teams.
//!
//! All types are generated to TypeScript via ts-rs and documented via utoipa.

use claude_view_core::pricing::{CostBreakdown, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

// ============================================================================
// API Response Types (generated to TypeScript via ts-rs)
// ============================================================================

#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub lead_session_id: String,
    #[ts(type = "number")]
    pub member_count: u32,
    #[ts(type = "number")]
    pub message_count: u32,
    #[ts(type = "number | null")]
    pub duration_estimate_secs: Option<u32>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TeamDetail {
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub lead_session_id: String,
    pub members: Vec<TeamMember>,
}

#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct TeamMember {
    pub agent_id: String,
    pub name: String,
    pub agent_type: String,
    pub model: String,
    pub prompt: Option<String>,
    pub color: String,
    pub backend_type: Option<String>,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InboxMessage {
    pub from: String,
    pub text: String,
    pub timestamp: String,
    pub message_type: InboxMessageType,
    pub read: bool,
    pub color: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub enum InboxMessageType {
    PlainText,
    TaskAssignment,
    IdleNotification,
    ShutdownRequest,
    ShutdownApproved,
}

// ============================================================================
// Team Cost Types
// ============================================================================

/// Per-member cost data for the team cost breakdown.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TeamMemberCost {
    pub name: String,
    pub color: String,
    pub model: String,
    pub agent_type: String,
    /// Resolved session ID (None for in-process members whose cost is in the lead session).
    pub session_id: Option<String>,
    /// True when member runs in-process -- cost is included in the coordinator total.
    pub in_process: bool,
    /// Total cost in USD (None if session not found or not yet resolved).
    #[ts(type = "number | null")]
    pub cost_usd: Option<f64>,
    /// Token usage breakdown (None if session not found).
    pub tokens: Option<TokenUsage>,
    /// Full cost breakdown (None if session not found).
    pub cost: Option<CostBreakdown>,
}

/// Aggregated cost breakdown for an entire team.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TeamCostBreakdown {
    pub team_name: String,
    #[ts(type = "number")]
    pub total_cost_usd: f64,
    /// Lead session cost (the coordinator).
    #[ts(type = "number")]
    pub lead_cost_usd: f64,
    pub members: Vec<TeamMemberCost>,
}

// ============================================================================
// JSONL Fallback Index Types
// ============================================================================

/// Reference to a session JSONL file that contains data for a team.
/// Used when the filesystem team directory (`~/.claude/teams/<name>/`) no longer exists.
#[derive(Debug, Clone)]
pub struct TeamJSONLRef {
    pub session_id: String,
    pub jsonl_path: std::path::PathBuf,
}

/// Index type: team_name -> list of JSONL refs (a team may appear across multiple sessions).
pub type TeamJSONLIndex = HashMap<String, Vec<TeamJSONLRef>>;

// ============================================================================
// Raw deserialization types (match on-disk JSON shape)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawTeamConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub lead_session_id: String,
    #[serde(default)]
    pub members: Vec<RawTeamMember>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields deserialized from on-disk JSON but not all are mapped to API types
pub(super) struct RawTeamMember {
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub agent_type: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub backend_type: Option<String>,
    #[serde(default)]
    pub plan_mode_required: bool,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub joined_at: i64,
    #[serde(default)]
    pub tmux_pane_id: String,
    #[serde(default)]
    pub subscriptions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawInboxMessage {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

// ============================================================================
// Team Member Sidechain Types
// ============================================================================

/// A single sidechain instance for a team member.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct TeamMemberSidechain {
    /// Hex agent ID -- used with `/api/sessions/{sid}/subagents/{hex}/messages`.
    pub hex_id: String,
    /// Agent name from meta.json (e.g., "js-advocate").
    pub member_name: String,
    /// Number of JSONL lines (proxy for amount of work done).
    #[ts(type = "number")]
    pub line_count: u32,
    /// File size in bytes.
    #[ts(type = "number")]
    pub file_size_bytes: u64,
    /// Model used by this sidechain (e.g., "claude-opus-4-6").
    pub model: String,
    /// ISO 8601 timestamp of the first JSONL entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// ISO 8601 timestamp of the last JSONL entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Duration in seconds (derived from started_at -> ended_at).
    #[ts(type = "number")]
    pub duration_seconds: u32,
    /// Cost in USD (computed from JSONL token usage + pricing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Token usage breakdown (input, output, cache read, cache creation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenUsage>,
}

// ============================================================================
// JSONL Spawn Data Parsing (typed serde)
// ============================================================================

/// Resolved metadata for a team member extracted from the lead JSONL.
#[derive(Debug, Clone, Default)]
pub struct ResolvedMemberInfo {
    /// Session ID or agent ID (e.g. UUID or "name@team").
    pub agent_id: String,
    /// Model used by this member (from toolUseResult).
    pub model: Option<String>,
    /// True when tmux_pane_id == "in-process" -- cost is embedded in the lead session.
    pub in_process: bool,
}

/// A single JSONL line -- we only care about the top-level `toolUseResult`.
#[derive(Deserialize)]
pub(super) struct JsonlLine {
    #[serde(default, rename = "toolUseResult")]
    pub tool_use_result: Option<SpawnResult>,
}

/// The `toolUseResult` object written by Claude Code when spawning a teammate.
#[derive(Deserialize)]
pub(super) struct SpawnResult {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub team_name: String,
    #[serde(default)]
    pub tmux_pane_id: String,
}
