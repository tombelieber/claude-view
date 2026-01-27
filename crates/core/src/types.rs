// crates/core/src/types.rs
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Tool usage statistics for a session
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct ToolCounts {
    pub edit: usize,
    pub read: usize,
    pub bash: usize,
    pub write: usize,
}

impl ToolCounts {
    pub fn total(&self) -> usize {
        self.edit + self.read + self.bash + self.write
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Message role in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// A tool call made by the assistant
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct ToolCall {
    pub name: String,
    pub count: usize,
}

/// A message in a conversation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            timestamp: None,
            tool_calls: None,
            thinking: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            timestamp: None,
            tool_calls: None,
            thinking: None,
        }
    }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tools);
        self
    }

    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }
}

/// Session metadata extracted from parsing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub total_messages: usize,
    pub tool_call_count: usize,
}

/// A parsed session with messages and metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct ParsedSession {
    pub messages: Vec<Message>,
    pub metadata: SessionMetadata,
}

impl ParsedSession {
    pub fn new(messages: Vec<Message>, tool_call_count: usize) -> Self {
        Self {
            metadata: SessionMetadata {
                total_messages: messages.len(),
                tool_call_count,
            },
            messages,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn turn_count(&self) -> usize {
        let user_count = self.messages.iter().filter(|m| m.role == Role::User).count();
        let assistant_count = self.messages.iter().filter(|m| m.role == Role::Assistant).count();
        user_count.min(assistant_count)
    }
}

/// Session info for listing (without full message content)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub project: String,
    pub project_path: String,
    pub file_path: String,
    pub modified_at: i64,
    pub size_bytes: u64,
    pub preview: String,
    pub last_message: String,
    pub files_touched: Vec<String>,
    pub skills_used: Vec<String>,
    pub tool_counts: ToolCounts,
    pub message_count: usize,
    pub turn_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub is_sidechain: bool,
    #[serde(default)]
    pub deep_indexed: bool,
}

/// Project info with sessions
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub display_name: String,
    pub path: String,
    pub sessions: Vec<SessionInfo>,
    pub active_count: usize,
}

impl ProjectInfo {
    pub fn total_sessions(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

// ============================================================================
// Turn-level types (Phase 2B: token & model tracking)
// ============================================================================

/// A single assistant turn extracted from JSONL, capturing model and token usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTurn {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub seq: u32,
    pub model_id: String,
    pub content_type: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub service_tier: Option<String>,
    pub timestamp: Option<i64>,
}

/// A model record for the models table (deduplicated across all sessions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecord {
    pub id: String,
    pub provider: String,
    pub family: String,
    pub first_seen: i64,
    pub last_seen: i64,
}

/// Parse a model ID string into (provider, family) slices.
///
/// Known patterns:
/// - `claude-opus-*`   → `("anthropic", "opus")`
/// - `claude-sonnet-*` → `("anthropic", "sonnet")`
/// - `claude-haiku-*`  → `("anthropic", "haiku")`
/// - `gpt-4*` / `gpt-3*` → `("openai", "gpt-4")` / `("openai", "gpt-3")`
/// - `o1*` / `o3*`     → `("openai", "o1")` / `("openai", "o3")`
/// - `gemini-*`        → `("google", "gemini")`
/// - Anything else     → `("unknown", model_str)`
pub fn parse_model_id(model_str: &str) -> (&str, &str) {
    if model_str.starts_with("claude-opus") {
        ("anthropic", "opus")
    } else if model_str.starts_with("claude-sonnet") {
        ("anthropic", "sonnet")
    } else if model_str.starts_with("claude-haiku") {
        ("anthropic", "haiku")
    } else if model_str.starts_with("gpt-4") {
        ("openai", "gpt-4")
    } else if model_str.starts_with("gpt-3") {
        ("openai", "gpt-3")
    } else if model_str.starts_with("o1") {
        ("openai", "o1")
    } else if model_str.starts_with("o3") {
        ("openai", "o3")
    } else if model_str.starts_with("gemini") {
        ("google", "gemini")
    } else {
        ("unknown", model_str)
    }
}

// ============================================================================
// JSONL Parsing Types (internal, for deserializing Claude Code format)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JsonlEntry {
    User {
        message: Option<JsonlMessage>,
        timestamp: Option<String>,
        #[serde(rename = "isMeta")]
        is_meta: Option<bool>,
    },
    Assistant {
        message: Option<JsonlMessage>,
        timestamp: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonlMessage {
    pub role: Option<String>,
    pub content: JsonlContent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum JsonlContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        name: String,
        #[serde(default)]
        input: Option<serde_json::Value>,
    },
    ToolResult {
        #[serde(default)]
        content: Option<serde_json::Value>,
    },
    #[serde(other)]
    Other,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_counts_total() {
        let counts = ToolCounts {
            edit: 5,
            read: 10,
            bash: 3,
            write: 2,
        };
        assert_eq!(counts.total(), 20);
    }

    #[test]
    fn test_tool_counts_empty() {
        let counts = ToolCounts::default();
        assert!(counts.is_empty());

        let counts = ToolCounts { edit: 1, ..Default::default() };
        assert!(!counts.is_empty());
    }

    #[test]
    fn test_message_builders() {
        let msg = Message::user("Hello")
            .with_timestamp("2026-01-27T10:00:00Z");

        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.timestamp, Some("2026-01-27T10:00:00Z".to_string()));
    }

    #[test]
    fn test_message_with_tools() {
        let msg = Message::assistant("Let me help")
            .with_tools(vec![ToolCall { name: "Read".to_string(), count: 2 }]);

        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.unwrap()[0].count, 2);
    }

    #[test]
    fn test_parsed_session_turn_count() {
        let session = ParsedSession::new(
            vec![
                Message::user("Q1"),
                Message::assistant("A1"),
                Message::user("Q2"),
                Message::assistant("A2"),
                Message::user("Q3"), // No response yet
            ],
            0,
        );

        assert_eq!(session.turn_count(), 2); // min(3 users, 2 assistants)
    }

    #[test]
    fn test_parsed_session_empty() {
        let session = ParsedSession::new(vec![], 0);
        assert!(session.is_empty());
        assert_eq!(session.turn_count(), 0);
    }

    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(serde_json::to_string(&Role::Assistant).unwrap(), "\"assistant\"");
    }

    #[test]
    fn test_role_deserialization() {
        let user: Role = serde_json::from_str("\"user\"").unwrap();
        let assistant: Role = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(user, Role::User);
        assert_eq!(assistant, Role::Assistant);
    }

    #[test]
    fn test_jsonl_entry_deserialization() {
        let json = r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-27T10:00:00Z"}"#;
        let entry: JsonlEntry = serde_json::from_str(json).unwrap();

        match entry {
            JsonlEntry::User { message, timestamp, .. } => {
                assert!(message.is_some());
                assert_eq!(timestamp, Some("2026-01-27T10:00:00Z".to_string()));
            }
            _ => panic!("Expected User entry"),
        }
    }

    #[test]
    fn test_jsonl_content_text() {
        let json = r#""Hello world""#;
        let content: JsonlContent = serde_json::from_str(json).unwrap();

        match content {
            JsonlContent::Text(text) => assert_eq!(text, "Hello world"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_jsonl_content_blocks() {
        let json = r#"[{"type":"text","text":"Hello"},{"type":"tool_use","name":"Read"}]"#;
        let content: JsonlContent = serde_json::from_str(json).unwrap();

        match content {
            JsonlContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                match &blocks[0] {
                    ContentBlock::Text { text } => assert_eq!(text, "Hello"),
                    _ => panic!("Expected Text block"),
                }
                match &blocks[1] {
                    ContentBlock::ToolUse { name, .. } => assert_eq!(name, "Read"),
                    _ => panic!("Expected ToolUse block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn test_jsonl_entry_unknown_type() {
        let json = r#"{"type":"unknown_future_type","data":"something"}"#;
        let entry: JsonlEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(entry, JsonlEntry::Other));
    }

    #[test]
    fn test_content_block_unknown_type() {
        let json = r#"{"type":"future_block_type","data":"something"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(matches!(block, ContentBlock::Other));
    }

    #[test]
    fn test_project_info_methods() {
        let project = ProjectInfo {
            name: "test-project".to_string(),
            display_name: "Test Project".to_string(),
            path: "/path/to/project".to_string(),
            sessions: vec![],
            active_count: 0,
        };

        assert!(project.is_empty());
        assert_eq!(project.total_sessions(), 0);
    }

    #[test]
    fn test_message_serialization_omits_none() {
        let msg = Message::user("Hello");
        let json = serde_json::to_string(&msg).unwrap();

        // Should not contain timestamp, tool_calls, or thinking when None
        assert!(!json.contains("timestamp"));
        assert!(!json.contains("tool_calls"));
        assert!(!json.contains("thinking"));
    }

    #[test]
    fn test_message_with_thinking() {
        let msg = Message::assistant("Here's the answer")
            .with_thinking("Let me reason about this...");

        assert_eq!(msg.thinking, Some("Let me reason about this...".to_string()));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"thinking\":\"Let me reason about this...\""));
    }

    // ============================================================================
    // modifiedAt serialization tests
    // ============================================================================

    #[test]
    fn test_session_info_modified_at_serializes_as_number() {
        let session = SessionInfo {
            id: "test-123".to_string(),
            project: "test-project".to_string(),
            project_path: "/path/to/project".to_string(),
            file_path: "/path/to/session.jsonl".to_string(),
            modified_at: 1769482232, // Unix timestamp
            size_bytes: 1024,
            preview: "Test".to_string(),
            last_message: "Test".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 1,
            turn_count: 1,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
        };
        let json = serde_json::to_string(&session).unwrap();

        // Should serialize as number
        assert!(
            json.contains("\"modifiedAt\":1769482232"),
            "modifiedAt should be a number, got: {}",
            json
        );
    }

    // ============================================================================
    // parse_model_id tests
    // ============================================================================

    #[test]
    fn test_parse_model_id_anthropic_opus() {
        assert_eq!(parse_model_id("claude-opus-4-5-20251101"), ("anthropic", "opus"));
        assert_eq!(parse_model_id("claude-opus-4-20250514"), ("anthropic", "opus"));
    }

    #[test]
    fn test_parse_model_id_anthropic_sonnet() {
        assert_eq!(parse_model_id("claude-sonnet-4-20250514"), ("anthropic", "sonnet"));
        assert_eq!(parse_model_id("claude-sonnet-4-5-20260130"), ("anthropic", "sonnet"));
    }

    #[test]
    fn test_parse_model_id_anthropic_haiku() {
        assert_eq!(parse_model_id("claude-haiku-4-20250514"), ("anthropic", "haiku"));
    }

    #[test]
    fn test_parse_model_id_openai_gpt() {
        assert_eq!(parse_model_id("gpt-4o-2024-08-06"), ("openai", "gpt-4"));
        assert_eq!(parse_model_id("gpt-4-turbo"), ("openai", "gpt-4"));
        assert_eq!(parse_model_id("gpt-3.5-turbo"), ("openai", "gpt-3"));
    }

    #[test]
    fn test_parse_model_id_openai_reasoning() {
        assert_eq!(parse_model_id("o1-preview"), ("openai", "o1"));
        assert_eq!(parse_model_id("o1-mini"), ("openai", "o1"));
        assert_eq!(parse_model_id("o3-mini"), ("openai", "o3"));
    }

    #[test]
    fn test_parse_model_id_google_gemini() {
        assert_eq!(parse_model_id("gemini-1.5-pro"), ("google", "gemini"));
        assert_eq!(parse_model_id("gemini-2.0-flash"), ("google", "gemini"));
    }

    #[test]
    fn test_parse_model_id_unknown() {
        assert_eq!(parse_model_id("llama-3-70b"), ("unknown", "llama-3-70b"));
        assert_eq!(parse_model_id("mistral-large"), ("unknown", "mistral-large"));
        assert_eq!(parse_model_id(""), ("unknown", ""));
    }
}
