// crates/core/src/types.rs
use serde::{Deserialize, Serialize};

/// Custom serializer to convert Unix timestamp (seconds) to ISO 8601 string.
/// This matches the old Node.js behavior where Date objects serialize to ISO strings.
mod unix_to_iso {
    use chrono::{DateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(timestamp: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let dt = DateTime::<Utc>::from_timestamp(*timestamp, 0)
            .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
        serializer.serialize_str(&dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Accept either ISO string or number for flexibility
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrNumber {
            String(String),
            Number(i64),
        }

        match StringOrNumber::deserialize(deserializer)? {
            StringOrNumber::Number(n) => Ok(n),
            StringOrNumber::String(s) => {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.timestamp())
                    .map_err(serde::de::Error::custom)
            }
        }
    }
}

/// Tool usage statistics for a session
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// A tool call made by the assistant
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub count: usize,
}

/// A message in a conversation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            timestamp: None,
            tool_calls: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            timestamp: None,
            tool_calls: None,
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
}

/// Session metadata extracted from parsing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub total_messages: usize,
    pub tool_call_count: usize,
}

/// A parsed session with messages and metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub project: String,
    pub project_path: String,
    pub file_path: String,
    #[serde(with = "unix_to_iso")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

        // Should not contain timestamp or tool_calls when None
        assert!(!json.contains("timestamp"));
        assert!(!json.contains("tool_calls"));
    }

    // ============================================================================
    // Issue 1: modifiedAt should serialize as ISO 8601 string
    // ============================================================================

    #[test]
    fn test_session_info_modified_at_serializes_as_iso_string() {
        let session = SessionInfo {
            id: "test-123".to_string(),
            project: "test-project".to_string(),
            project_path: "/path/to/project".to_string(),
            file_path: "/path/to/session.jsonl".to_string(),
            modified_at: 1769482232, // 2026-01-27T02:50:32Z
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

        // Should serialize as ISO string, not number
        assert!(
            json.contains("\"modifiedAt\":\"2026-"),
            "modifiedAt should be ISO string, got: {}",
            json
        );
        assert!(
            !json.contains("\"modifiedAt\":1769"),
            "modifiedAt should NOT be a number"
        );
    }

    #[test]
    fn test_session_info_iso_format_with_utc_timezone() {
        let session = SessionInfo {
            id: "test-123".to_string(),
            project: "test-project".to_string(),
            project_path: "/path/to/project".to_string(),
            file_path: "/path/to/session.jsonl".to_string(),
            modified_at: 1769482232,
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

        // Should contain T separator and Z suffix for UTC
        assert!(json.contains("T"), "ISO string should have T separator");
        assert!(json.contains("Z"), "ISO string should end with Z for UTC");
    }

    #[test]
    fn test_session_info_modified_at_correct_date() {
        let session = SessionInfo {
            id: "test-123".to_string(),
            project: "test-project".to_string(),
            project_path: "/path/to/project".to_string(),
            file_path: "/path/to/session.jsonl".to_string(),
            modified_at: 1769482232, // 2026-01-27T02:50:32Z
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

        // Should contain correct date components
        assert!(
            json.contains("2026-01-27"),
            "Should contain correct date: {}",
            json
        );
    }
}
