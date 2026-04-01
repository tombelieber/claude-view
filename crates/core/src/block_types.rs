// crates/core/src/block_types.rs
//
// ConversationBlock type hierarchy — the structured output of BlockAccumulator.
// These types replace the hand-written TS `blocks.ts` with generated types
// derived from Rust via ts-rs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::category::ActionCategory;

// ── Top-level tagged union ──────────────────────────────────────────

/// A single block in a conversation timeline.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationBlock {
    User(UserBlock),
    Assistant(AssistantBlock),
    Interaction(InteractionBlock),
    TurnBoundary(TurnBoundaryBlock),
    Notice(NoticeBlock),
    System(SystemBlock),
    Progress(ProgressBlock),
}

impl ConversationBlock {
    /// Extract the block's ID regardless of variant.
    pub fn id(&self) -> &str {
        match self {
            Self::User(b) => &b.id,
            Self::Assistant(b) => &b.id,
            Self::Interaction(b) => &b.id,
            Self::TurnBoundary(b) => &b.id,
            Self::Notice(b) => &b.id,
            Self::System(b) => &b.id,
            Self::Progress(b) => &b.id,
        }
    }
}

// ── Image content ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    pub source_type: String,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

// ── User ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct UserBlock {
    pub id: String,
    pub text: String,
    pub timestamp: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_sidechain: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub raw_json: Option<serde_json::Value>,
}

// ── Assistant ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AssistantBlock {
    pub id: String,
    pub segments: Vec<AssistantSegment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    pub streaming: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_sidechain: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub raw_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssistantSegment {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,
    },
    Tool {
        execution: ToolExecution,
    },
}

// ── Tool execution ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    pub tool_name: String,
    #[ts(type = "any")]
    pub tool_input: serde_json::Value,
    pub tool_use_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ToolResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<ToolProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub status: ToolStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<ActionCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
    pub is_replay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ToolProgress {
    pub elapsed_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Complete,
    Error,
}

// ── Interaction ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InteractionBlock {
    pub id: String,
    pub variant: InteractionVariant,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub resolved: bool,
    #[ts(type = "any")]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum InteractionVariant {
    Permission,
    Question,
    Plan,
    Elicitation,
}

// ── Turn boundary ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TurnBoundaryBlock {
    pub id: String,
    pub success: bool,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_api_ms: Option<u64>,
    #[serde(default)]
    pub usage: HashMap<String, u64>,
    #[serde(default)]
    #[ts(type = "Record<string, any>")]
    pub model_usage: HashMap<String, serde_json::Value>,
    #[serde(default)]
    #[ts(type = "any[]")]
    pub permission_denials: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub structured_output: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_mode_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TurnError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(type = "any[]")]
    pub hook_infos: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hook_errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prevented_continuation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TurnError {
    pub subtype: String,
    pub messages: Vec<String>,
}

// ── Notice ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct NoticeBlock {
    pub id: String,
    pub variant: NoticeVariant,
    #[ts(type = "any")]
    pub data: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_in_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum NoticeVariant {
    AssistantError,
    RateLimit,
    ContextCompacted,
    AuthStatus,
    SessionClosed,
    Error,
    PromptSuggestion,
    SessionResumed,
}

// ── System ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SystemBlock {
    pub id: String,
    pub variant: SystemVariant,
    #[ts(type = "any")]
    pub data: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub raw_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum SystemVariant {
    SessionInit,
    SessionStatus,
    ElicitationComplete,
    HookEvent,
    TaskStarted,
    TaskProgress,
    TaskNotification,
    FilesSaved,
    CommandOutput,
    StreamDelta,
    LocalCommand,
    QueueOperation,
    FileHistorySnapshot,
    AiTitle,
    LastPrompt,
    WorktreeState,
    PrLink,
    CustomTitle,
    PlanContent,
    Informational,
    AgentName,
    Unknown,
}

// ── Progress ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProgressBlock {
    pub id: String,
    pub variant: ProgressVariant,
    pub category: ActionCategory,
    pub data: ProgressData,
    pub ts: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ProgressVariant {
    Bash,
    Agent,
    Mcp,
    Hook,
    TaskQueue,
    Search,
    Query,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressData {
    Bash(BashProgress),
    Agent(AgentProgress),
    Mcp(McpProgress),
    Hook(HookProgress),
    TaskQueue(TaskQueueProgress),
    Search(SearchProgress),
    Query(QueryProgress),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BashProgress {
    pub output: String,
    pub full_output: String,
    pub elapsed_time_seconds: f64,
    pub total_lines: u32,
    pub total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AgentProgress {
    pub prompt: String,
    pub agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub message: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct McpProgress {
    pub status: String,
    pub server_name: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct HookProgress {
    pub hook_event: String,
    pub hook_name: String,
    pub command: String,
    pub status_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TaskQueueProgress {
    pub task_description: String,
    pub task_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SearchProgress {
    pub result_count: u32,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct QueryProgress {
    pub query: String,
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tagged_enum_discriminator() {
        let user = ConversationBlock::User(UserBlock {
            id: "u1".into(),
            text: "hello".into(),
            timestamp: 1000.0,
            status: None,
            local_id: None,
            pending: None,
            permission_mode: None,
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            images: Vec::new(),
            raw_json: None,
        });
        let json = serde_json::to_value(&user).unwrap();
        assert_eq!(json["type"], "user");

        let notice = ConversationBlock::Notice(NoticeBlock {
            id: "n1".into(),
            variant: NoticeVariant::Error,
            data: json!({}),
            retry_in_ms: None,
            retry_attempt: None,
            max_retries: None,
        });
        let json = serde_json::to_value(&notice).unwrap();
        assert_eq!(json["type"], "notice");
    }

    #[test]
    fn user_block_round_trip() {
        let block = UserBlock {
            id: "u1".into(),
            text: "hello world".into(),
            timestamp: 1234.5,
            status: Some("done".into()),
            local_id: Some("loc1".into()),
            pending: Some(false),
            permission_mode: Some("auto".into()),
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            images: Vec::new(),
            raw_json: None,
        };
        let json_str = serde_json::to_string(&block).unwrap();
        let deserialized: UserBlock = serde_json::from_str(&json_str).unwrap();
        assert_eq!(block.id, deserialized.id);
        assert_eq!(block.text, deserialized.text);
        assert_eq!(block.timestamp, deserialized.timestamp);
        assert_eq!(block.status, deserialized.status);
        assert_eq!(block.local_id, deserialized.local_id);
        assert_eq!(block.pending, deserialized.pending);
        assert_eq!(block.permission_mode, deserialized.permission_mode);
    }

    #[test]
    fn user_block_parent_uuid_serializes_as_camel_case() {
        let block = UserBlock {
            id: "u1".into(),
            text: "hello".into(),
            timestamp: 1000.0,
            status: None,
            local_id: None,
            pending: None,
            permission_mode: None,
            parent_uuid: Some("parent-msg-123".into()),
            is_sidechain: None,
            agent_id: None,
            images: Vec::new(),
            raw_json: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["parentUuid"], "parent-msg-123");

        let deserialized: UserBlock = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.parent_uuid, Some("parent-msg-123".into()));
    }

    #[test]
    fn user_block_parent_uuid_omitted_when_none() {
        let block = UserBlock {
            id: "u1".into(),
            text: "hello".into(),
            timestamp: 1000.0,
            status: None,
            local_id: None,
            pending: None,
            permission_mode: None,
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            images: Vec::new(),
            raw_json: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert!(json.get("parentUuid").is_none());
    }

    #[test]
    fn assistant_block_parent_uuid_round_trips() {
        let block = AssistantBlock {
            id: "a1".into(),
            segments: vec![AssistantSegment::Text {
                text: "Hello".into(),
                parent_tool_use_id: None,
            }],
            thinking: None,
            streaming: false,
            timestamp: None,
            parent_uuid: Some("parent-msg-456".into()),
            is_sidechain: None,
            agent_id: None,
            raw_json: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["parentUuid"], "parent-msg-456");

        let deserialized: AssistantBlock = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.parent_uuid, Some("parent-msg-456".into()));
    }

    #[test]
    fn assistant_block_with_segments_serializes() {
        let block = AssistantBlock {
            id: "a1".into(),
            segments: vec![
                AssistantSegment::Text {
                    text: "Hello".into(),
                    parent_tool_use_id: None,
                },
                AssistantSegment::Tool {
                    execution: ToolExecution {
                        tool_name: "Read".into(),
                        tool_input: json!({"path": "/tmp/a.txt"}),
                        tool_use_id: "tu1".into(),
                        parent_tool_use_id: None,
                        result: Some(ToolResult {
                            output: "contents".into(),
                            is_error: false,
                            is_replay: false,
                        }),
                        progress: None,
                        summary: None,
                        status: ToolStatus::Complete,
                        category: None,
                        live_output: None,
                        duration: Some(0.5),
                    },
                },
            ],
            thinking: Some("let me think".into()),
            streaming: false,
            timestamp: Some(100.0),
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            raw_json: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["id"], "a1");
        assert_eq!(json["segments"].as_array().unwrap().len(), 2);
        assert_eq!(json["segments"][0]["kind"], "text");
        assert_eq!(json["segments"][1]["kind"], "tool");
    }

    #[test]
    fn progress_block_bash_serializes() {
        let block = ProgressBlock {
            id: "p1".into(),
            variant: ProgressVariant::Bash,
            category: ActionCategory::Builtin,
            data: ProgressData::Bash(BashProgress {
                output: "line1".into(),
                full_output: "line1\nline2".into(),
                elapsed_time_seconds: 1.5,
                total_lines: 2,
                total_bytes: 20,
                task_id: None,
            }),
            ts: 999.0,
            parent_tool_use_id: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["id"], "p1");
        assert_eq!(json["variant"], "bash");
        assert_eq!(json["category"], "builtin");
        assert_eq!(json["data"]["type"], "bash");
        assert_eq!(json["data"]["output"], "line1");
    }

    #[test]
    fn turn_boundary_block_serializes() {
        let block = TurnBoundaryBlock {
            id: "tb1".into(),
            success: true,
            total_cost_usd: 0.05,
            num_turns: 3,
            duration_ms: 5000,
            duration_api_ms: Some(4000),
            usage: HashMap::from([("input_tokens".into(), 100)]),
            model_usage: HashMap::new(),
            permission_denials: vec![],
            result: Some("success".into()),
            structured_output: None,
            stop_reason: Some("end_turn".into()),
            fast_mode_state: None,
            error: None,
            hook_infos: Vec::new(),
            hook_errors: Vec::new(),
            hook_count: None,
            prevented_continuation: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["id"], "tb1");
        assert_eq!(json["success"], true);
        assert_eq!(json["totalCostUsd"], 0.05);
        assert_eq!(json["numTurns"], 3);
        assert_eq!(json["durationMs"], 5000);
    }

    #[test]
    fn system_block_all_variants_serialize() {
        let variants = vec![
            SystemVariant::SessionInit,
            SystemVariant::SessionStatus,
            SystemVariant::ElicitationComplete,
            SystemVariant::HookEvent,
            SystemVariant::TaskStarted,
            SystemVariant::TaskProgress,
            SystemVariant::TaskNotification,
            SystemVariant::FilesSaved,
            SystemVariant::CommandOutput,
            SystemVariant::StreamDelta,
            SystemVariant::LocalCommand,
            SystemVariant::QueueOperation,
            SystemVariant::FileHistorySnapshot,
            SystemVariant::AiTitle,
            SystemVariant::LastPrompt,
            SystemVariant::WorktreeState,
            SystemVariant::PrLink,
            SystemVariant::CustomTitle,
            SystemVariant::PlanContent,
            SystemVariant::Informational,
            SystemVariant::AgentName,
            SystemVariant::Unknown,
        ];
        assert_eq!(variants.len(), 22);
        for variant in &variants {
            let block = SystemBlock {
                id: "s1".into(),
                variant: *variant,
                data: json!({}),
                raw_json: None,
            };
            let json_str = serde_json::to_string(&block).unwrap();
            let deserialized: SystemBlock = serde_json::from_str(&json_str).unwrap();
            assert_eq!(deserialized.variant, *variant);
        }
    }

    #[test]
    fn interaction_block_serializes() {
        let block = InteractionBlock {
            id: "i1".into(),
            variant: InteractionVariant::Permission,
            request_id: Some("req1".into()),
            resolved: false,
            data: json!({"tool": "Bash", "command": "rm -rf /"}),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["id"], "i1");
        assert_eq!(json["variant"], "permission");
        assert_eq!(json["requestId"], "req1");
        assert_eq!(json["resolved"], false);
    }
}
