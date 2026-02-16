// crates/core/src/subagent.rs
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Status of a sub-agent within a live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub enum SubAgentStatus {
    Running,
    Complete,
    Error,
}

/// Information about a sub-agent spawned via the Task tool.
///
/// Note: ts-rs exports i64 as TypeScript `number` (safe for Unix timestamps).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SubAgentInfo {
    /// The tool_use_id from the spawning Task call. Used to match
    /// the tool_result that signals completion.
    pub tool_use_id: String,

    /// 7-character short hash agent identifier from `toolUseResult.agentId`.
    /// Matches the `agent-{id}.jsonl` filename in the subagents directory.
    /// None while status is Running (only available on completion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    /// Agent type label extracted from `subagent_type` field.
    /// Examples: "Explore", "code-reviewer", "search", "edit-files".
    /// Falls back to "Task" if subagent_type is absent.
    pub agent_type: String,

    /// Human-readable description from the Task tool's `description` input.
    pub description: String,

    /// Current execution status.
    pub status: SubAgentStatus,

    /// Unix timestamp (seconds) when the sub-agent was spawned.
    /// Parsed from the ISO 8601 `timestamp` field on the JSONL line
    /// via `chrono::DateTime::parse_from_rfc3339`.
    pub started_at: i64,

    /// Unix timestamp (seconds) when the sub-agent completed or errored.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,

    /// Duration in milliseconds from `toolUseResult.totalDurationMs`.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Number of tool calls the sub-agent made, from `toolUseResult.totalToolUseCount`.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_count: Option<u32>,

    /// Cost in USD attributed to this sub-agent's execution.
    /// Computed from `toolUseResult.usage` token counts via the pricing table.
    /// None while status is Running or if pricing data unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}
