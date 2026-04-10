// crates/core/src/subagent.rs
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Status of a sub-agent within a live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub enum SubAgentStatus {
    Running,
    Complete,
    Error,
}

/// Information about a sub-agent spawned via the Task tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentInfo {
    /// The tool_use_id from the spawning Task call. Used to match
    /// the tool_result that signals completion.
    pub tool_use_id: String,

    /// Agent identifier from `toolUseResult.agentId` (variable length hex string).
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
    #[ts(type = "number")]
    pub started_at: i64,

    /// Unix timestamp (seconds) when the sub-agent completed or errored.
    /// None while status is Running.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,

    /// Duration in milliseconds from `toolUseResult.totalDurationMs`.
    /// None while status is Running.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Number of tool calls the sub-agent made, from `toolUseResult.totalToolUseCount`.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_count: Option<u32>,

    /// Model used by this sub-agent (alias or full ID, e.g., "haiku", "claude-haiku-4-5-20251001").
    /// Populated from spawn input `model` field; overridden by `toolUseResult.model` if present.
    /// None means the sub-agent inherited the parent session's model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Token usage breakdown from `toolUseResult.usage`.
    /// None while status is Running.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,

    /// Cost in USD attributed to this sub-agent's execution.
    /// Computed from `toolUseResult.usage` token counts via the pricing table,
    /// using the sub-agent's own model for pricing (falls back to parent model if unknown).
    /// None while status is Running or if pricing data unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,

    /// Current tool the sub-agent is using (e.g., "Read", "Grep", "Edit").
    /// Populated from progress events while status is Running.
    /// Cleared to None when status transitions to Complete/Error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_activity: Option<String>,

    /// Human-readable reason when status is Error.
    /// Populated from the toolUseResult string or notification status.
    /// None when status is Running or Complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
}

impl SubAgentInfo {
    /// Mark a Running sub-agent as Error (orphaned/abandoned).
    /// Sets completed_at, clears current_activity, records the reason.
    /// No-op if the sub-agent is already Complete or Error.
    pub fn finalize_as_orphaned(&mut self, now: i64, reason: &str) {
        if self.status == SubAgentStatus::Running {
            self.status = SubAgentStatus::Error;
            self.current_activity = None;
            self.completed_at = Some(now);
            self.error_reason = Some(reason.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_info_serialization_with_activity() {
        let info = SubAgentInfo {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_id: Some("a951849".to_string()),
            agent_type: "Explore".to_string(),
            description: "Search codebase".to_string(),
            status: SubAgentStatus::Running,
            started_at: 1739700000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            model: Some("haiku".to_string()),
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: Some("Read".to_string()),
            error_reason: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"currentActivity\":\"Read\""));
    }

    #[test]
    fn test_finalize_as_orphaned_marks_running_as_error() {
        let mut info = SubAgentInfo {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_id: Some("a951849".to_string()),
            agent_type: "Explore".to_string(),
            description: "Search codebase".to_string(),
            status: SubAgentStatus::Running,
            started_at: 1739700000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            model: Some("haiku".to_string()),
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: Some("Read".to_string()),
            error_reason: None,
        };

        info.finalize_as_orphaned(1739700100, "Parent process exited");

        assert_eq!(info.status, SubAgentStatus::Error);
        assert_eq!(info.current_activity, None);
        assert_eq!(info.completed_at, Some(1739700100));
        assert_eq!(info.error_reason.as_deref(), Some("Parent process exited"));
    }

    #[test]
    fn test_finalize_as_orphaned_noop_for_complete() {
        let mut info = SubAgentInfo {
            tool_use_id: "toolu_done".to_string(),
            agent_id: Some("agent2".to_string()),
            agent_type: "Edit".to_string(),
            description: "Completed agent".to_string(),
            status: SubAgentStatus::Complete,
            started_at: 1739700000,
            completed_at: Some(1739700050),
            duration_ms: Some(50000),
            tool_use_count: Some(10),
            model: Some("haiku".to_string()),
            input_tokens: Some(500),
            output_tokens: Some(200),
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: Some(0.001),
            current_activity: None,
            error_reason: None,
        };

        info.finalize_as_orphaned(1739700200, "Parent process exited");

        // Should remain unchanged
        assert_eq!(info.status, SubAgentStatus::Complete);
        assert_eq!(info.completed_at, Some(1739700050));
        assert_eq!(info.cost_usd, Some(0.001));
        assert_eq!(info.error_reason, None);
    }

    #[test]
    fn test_finalize_as_orphaned_noop_for_error() {
        let mut info = SubAgentInfo {
            tool_use_id: "toolu_err".to_string(),
            agent_id: None,
            agent_type: "Search".to_string(),
            description: "Failed agent".to_string(),
            status: SubAgentStatus::Error,
            started_at: 1739700000,
            completed_at: Some(1739700030),
            duration_ms: None,
            tool_use_count: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: None,
            error_reason: Some("previously failed".to_string()),
        };

        info.finalize_as_orphaned(1739700200, "Parent process exited");

        // Should remain unchanged — already Error
        assert_eq!(info.status, SubAgentStatus::Error);
        assert_eq!(info.completed_at, Some(1739700030));
        assert_eq!(info.error_reason.as_deref(), Some("previously failed"));
    }

    #[test]
    fn test_subagent_info_skips_none_activity() {
        let info = SubAgentInfo {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_id: None,
            agent_type: "Explore".to_string(),
            description: "Search".to_string(),
            status: SubAgentStatus::Running,
            started_at: 1739700000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: None,
            error_reason: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("currentActivity"));
        assert!(!json.contains("errorReason"));
    }

    #[test]
    fn test_error_reason_serialization() {
        // With error_reason = Some, JSON should contain "errorReason"
        let info_with_reason = SubAgentInfo {
            tool_use_id: "toolu_err1".to_string(),
            agent_id: None,
            agent_type: "Explore".to_string(),
            description: "Failed agent".to_string(),
            status: SubAgentStatus::Error,
            started_at: 1739700000,
            completed_at: Some(1739700100),
            duration_ms: None,
            tool_use_count: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: None,
            error_reason: Some("test error reason".to_string()),
        };
        let json = serde_json::to_string(&info_with_reason).unwrap();
        assert!(
            json.contains("\"errorReason\":\"test error reason\""),
            "JSON should contain errorReason when Some, got: {}",
            json
        );

        // With error_reason = None, JSON should NOT contain "errorReason"
        let info_no_reason = SubAgentInfo {
            tool_use_id: "toolu_ok1".to_string(),
            agent_id: None,
            agent_type: "Explore".to_string(),
            description: "Running agent".to_string(),
            status: SubAgentStatus::Running,
            started_at: 1739700000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: None,
            error_reason: None,
        };
        let json = serde_json::to_string(&info_no_reason).unwrap();
        assert!(
            !json.contains("errorReason"),
            "JSON should NOT contain errorReason when None, got: {}",
            json
        );
    }
}
