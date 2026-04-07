//! Response types for the per-turn breakdown endpoint.

use serde::Serialize;
use ts_rs::TS;

/// A single turn in the session breakdown.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TurnInfo {
    /// 1-based turn index.
    pub index: u32,
    /// Unix timestamp (seconds) when the turn started (user prompt).
    #[ts(type = "number")]
    pub started_at: i64,
    /// Wall-clock seconds from turn start to turn end (last message before next turn or EOF).
    #[ts(type = "number")]
    pub wall_clock_seconds: i64,
    /// Claude Code reported turn duration in milliseconds (from `turn_duration` system message).
    /// Null if no `turn_duration` message follows this turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub cc_duration_ms: Option<u64>,
    /// First 60 characters of the user prompt text.
    pub prompt_preview: String,
}
