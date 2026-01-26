// crates/core/src/parser.rs
//! Async JSONL parser for Claude Code session files.
//!
//! This module provides battle-tested parsing of Claude Code's JSONL session format,
//! handling malformed lines gracefully, aggregating tool calls, and cleaning command tags.

use crate::error::ParseError;
use crate::types::*;
use regex_lite::Regex;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

/// Parse a Claude Code session JSONL file into a structured `ParsedSession`.
///
/// # Features
/// - Async streaming with tokio BufReader for memory efficiency
/// - Skips malformed JSON lines (logs at debug level, doesn't fail)
/// - Skips meta messages (isMeta: true)
/// - Cleans command tags from user messages
/// - Aggregates tool calls and attaches them to assistant messages
/// - Forward-compatible with unknown entry types (silently ignores)
///
/// # Errors
/// - `ParseError::NotFound` if the file doesn't exist
/// - `ParseError::PermissionDenied` if the file can't be read
/// - `ParseError::Io` for other I/O errors
///
/// # Example
/// ```ignore
/// use std::path::Path;
/// use vibe_recall_core::parse_session;
///
/// let session = parse_session(Path::new("session.jsonl")).await?;
/// println!("Parsed {} messages", session.messages.len());
/// ```
pub async fn parse_session(file_path: &Path) -> Result<ParsedSession, ParseError> {
    let file = File::open(file_path)
        .await
        .map_err(|e| ParseError::io(file_path, e))?;

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut messages: Vec<Message> = Vec::new();
    let mut total_tool_calls: usize = 0;
    let mut line_number: usize = 0;

    // Regex for cleaning command tags from user messages
    let command_tag_regex = Regex::new(r"<command-name>[^<]*</command-name>\s*").unwrap();
    let command_args_regex = Regex::new(r"<command-args>[^<]*</command-args>\s*").unwrap();

    while let Some(line_result) = lines.next_line().await.map_err(|e| ParseError::io(file_path, e))? {
        line_number += 1;
        let line = line_result.trim();

        // Skip empty or whitespace-only lines
        if line.is_empty() {
            continue;
        }

        // Try to parse as JSON, skip malformed lines
        let entry: JsonlEntry = match serde_json::from_str(line) {
            Ok(entry) => entry,
            Err(e) => {
                debug!(
                    "Skipping malformed JSON at line {} in {:?}: {}",
                    line_number,
                    file_path,
                    e
                );
                continue;
            }
        };

        match entry {
            JsonlEntry::User {
                message,
                timestamp,
                is_meta,
            } => {
                // Skip meta messages
                if is_meta == Some(true) {
                    debug!("Skipping meta message at line {}", line_number);
                    continue;
                }

                if let Some(msg) = message {
                    let content = extract_text_content(&msg.content);
                    // Clean command tags from user messages
                    let cleaned_content = clean_command_tags(&content, &command_tag_regex, &command_args_regex);

                    // Only add non-empty messages
                    if !cleaned_content.trim().is_empty() {
                        let mut message = Message::user(cleaned_content);
                        if let Some(ts) = timestamp {
                            message = message.with_timestamp(ts);
                        }
                        messages.push(message);
                    }
                }
            }
            JsonlEntry::Assistant { message, timestamp } => {
                if let Some(msg) = message {
                    let (content, tool_calls) = extract_assistant_content(&msg.content);
                    let tool_call_count = tool_calls.iter().map(|tc| tc.count).sum::<usize>();
                    total_tool_calls += tool_call_count;

                    let mut message = Message::assistant(content);
                    if let Some(ts) = timestamp {
                        message = message.with_timestamp(ts);
                    }
                    if !tool_calls.is_empty() {
                        message = message.with_tools(tool_calls);
                    }
                    messages.push(message);
                }
            }
            JsonlEntry::Other => {
                // Silently ignore unknown entry types for forward compatibility
                debug!("Ignoring unknown entry type at line {}", line_number);
            }
        }
    }

    Ok(ParsedSession::new(messages, total_tool_calls))
}

/// Extract text content from JSONL content, handling both string and block formats.
fn extract_text_content(content: &JsonlContent) -> String {
    match content {
        JsonlContent::Text(text) => text.clone(),
        JsonlContent::Blocks(blocks) => {
            blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

/// Extract text content and tool calls from assistant message content.
fn extract_assistant_content(content: &JsonlContent) -> (String, Vec<ToolCall>) {
    match content {
        JsonlContent::Text(text) => (text.clone(), vec![]),
        JsonlContent::Blocks(blocks) => {
            let mut text_parts: Vec<&str> = Vec::new();
            let mut tool_counts: HashMap<String, usize> = HashMap::new();

            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        text_parts.push(text);
                    }
                    ContentBlock::ToolUse { name, .. } => {
                        *tool_counts.entry(name.clone()).or_insert(0) += 1;
                    }
                    _ => {} // Ignore tool_result and other blocks
                }
            }

            let text = text_parts.join("\n");
            let tool_calls: Vec<ToolCall> = tool_counts
                .into_iter()
                .map(|(name, count)| ToolCall { name, count })
                .collect();

            (text, tool_calls)
        }
    }
}

/// Clean command tags from user messages.
fn clean_command_tags(content: &str, tag_regex: &Regex, args_regex: &Regex) -> String {
    let cleaned = tag_regex.replace_all(content, "");
    let cleaned = args_regex.replace_all(&cleaned, "");
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    // ============================================================================
    // Happy Path Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_simple_session() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages.len(), 4);
        assert_eq!(session.metadata.total_messages, 4);
        assert_eq!(session.turn_count(), 2);
    }

    #[tokio::test]
    async fn test_parse_simple_session_message_content() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[0].content, "Hello, Claude!");
        assert_eq!(session.messages[1].role, Role::Assistant);
        assert_eq!(session.messages[1].content, "Hello! How can I help you today?");
    }

    #[tokio::test]
    async fn test_parse_session_preserves_timestamps() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(
            session.messages[0].timestamp,
            Some("2026-01-27T10:00:00Z".to_string())
        );
        assert_eq!(
            session.messages[1].timestamp,
            Some("2026-01-27T10:00:01Z".to_string())
        );
    }

    // ============================================================================
    // Tool Calls Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_session_with_tool_calls() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages.len(), 6);
        assert!(session.metadata.tool_call_count > 0);
    }

    #[tokio::test]
    async fn test_tool_calls_aggregation() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Second message (first assistant) should have Read tool calls
        let assistant_msg = &session.messages[1];
        assert_eq!(assistant_msg.role, Role::Assistant);
        assert!(assistant_msg.tool_calls.is_some());

        let tools = assistant_msg.tool_calls.as_ref().unwrap();
        let read_tool = tools.iter().find(|t| t.name == "Read");
        assert!(read_tool.is_some());
        assert_eq!(read_tool.unwrap().count, 2); // Two Read calls
    }

    #[tokio::test]
    async fn test_tool_calls_count_in_metadata() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Count all tool uses: 2 Read + 1 Edit + 2 Bash + 1 Write = 6
        assert_eq!(session.metadata.tool_call_count, 6);
    }

    #[tokio::test]
    async fn test_assistant_text_extracted_with_tools() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        let assistant_msg = &session.messages[1];
        assert!(assistant_msg.content.contains("Let me read that file"));
    }

    // ============================================================================
    // Meta Message Skipping Tests
    // ============================================================================

    #[tokio::test]
    async fn test_skip_meta_messages() {
        let path = fixtures_path().join("with_meta.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Original has 7 lines: 3 meta (should be skipped), 2 user, 2 assistant
        assert_eq!(session.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_meta_messages_not_in_content() {
        let path = fixtures_path().join("with_meta.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Verify no meta content appears
        for msg in &session.messages {
            assert!(!msg.content.contains("System initialization"));
            assert!(!msg.content.contains("Meta command"));
            assert!(!msg.content.contains("Another meta"));
        }
    }

    // ============================================================================
    // Command Tag Cleaning Tests
    // ============================================================================

    #[tokio::test]
    async fn test_clean_command_tags() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // First user message should have command tag cleaned
        let first_user = &session.messages[0];
        assert!(!first_user.content.contains("<command-name>"));
        assert!(!first_user.content.contains("</command-name>"));
        assert!(first_user.content.contains("Please commit my changes"));
    }

    #[tokio::test]
    async fn test_clean_command_args_tags() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Second user message should have both command-name and command-args cleaned
        let second_user = &session.messages[2];
        assert!(!second_user.content.contains("<command-args>"));
        assert!(!second_user.content.contains("</command-args>"));
        assert!(second_user.content.contains("Review this PR"));
    }

    #[tokio::test]
    async fn test_command_only_message_removed() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // The "/help" message should be effectively empty after cleaning
        // and should result in fewer messages
        // Original: 4 user + 4 assistant = 8, but one becomes empty = 7
        assert_eq!(session.messages.len(), 7);
    }

    #[tokio::test]
    async fn test_normal_message_unchanged() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Find the normal message without commands
        let normal_msg = session.messages.iter().find(|m| {
            m.role == Role::User && m.content == "Just a normal message without commands"
        });
        assert!(normal_msg.is_some());
    }

    // ============================================================================
    // Large Session Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_large_session() {
        let path = fixtures_path().join("large_session.jsonl");
        let session = parse_session(&path).await.unwrap();

        // 100 Q&A pairs = 200 messages
        assert_eq!(session.messages.len(), 200);
        assert_eq!(session.turn_count(), 100);
    }

    #[tokio::test]
    async fn test_large_session_first_and_last() {
        let path = fixtures_path().join("large_session.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert!(session.messages[0].content.contains("Question number 1"));
        assert!(session.messages[199].content.contains("200"));
    }

    // ============================================================================
    // Error Cases Tests
    // ============================================================================

    #[tokio::test]
    async fn test_file_not_found() {
        let path = fixtures_path().join("nonexistent.jsonl");
        let result = parse_session(&path).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ParseError::NotFound { path: p } => {
                assert!(p.to_string_lossy().contains("nonexistent.jsonl"));
            }
            other => panic!("Expected NotFound error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_empty_file() {
        let path = fixtures_path().join("empty.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert!(session.is_empty());
        assert_eq!(session.messages.len(), 0);
        assert_eq!(session.turn_count(), 0);
    }

    #[tokio::test]
    async fn test_malformed_lines_skipped() {
        let path = fixtures_path().join("malformed.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Should parse valid lines only: 2 user + 2 assistant = 4
        assert_eq!(session.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_malformed_lines_content_correct() {
        let path = fixtures_path().join("malformed.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages[0].content, "Valid first message");
        assert_eq!(session.messages[1].content, "Valid response");
        assert_eq!(session.messages[2].content, "Another valid message");
        assert_eq!(session.messages[3].content, "Final valid response");
    }

    // ============================================================================
    // Edge Cases Tests
    // ============================================================================

    #[tokio::test]
    async fn test_whitespace_only_lines_skipped() {
        let path = fixtures_path().join("whitespace.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Should skip blank lines and parse 4 valid messages
        assert_eq!(session.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_user_only_session() {
        let path = fixtures_path().join("user_only.jsonl");
        let session = parse_session(&path).await.unwrap();

        // 3 user messages, 0 assistant
        assert_eq!(session.messages.len(), 3);
        assert_eq!(session.turn_count(), 0); // min(3, 0)
    }

    #[tokio::test]
    async fn test_unknown_entry_types_ignored() {
        let path = fixtures_path().join("unknown_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Should parse user/assistant only, ignore unknown types
        // File has: user, future_type (Other), assistant, system (malformed - message is string),
        // user, metadata (Other), assistant
        // = 2 user + 2 assistant = 4 valid messages
        assert_eq!(session.messages.len(), 4);

        // Verify correct messages were parsed
        assert_eq!(session.messages[0].content, "Hello");
        assert_eq!(session.messages[1].content, "Hi there!");
        assert_eq!(session.messages[2].content, "Goodbye");
        assert_eq!(session.messages[3].content, "Goodbye!");
    }

    // ============================================================================
    // Unit Tests for Helper Functions
    // ============================================================================

    #[test]
    fn test_extract_text_content_string() {
        let content = JsonlContent::Text("Hello world".to_string());
        assert_eq!(extract_text_content(&content), "Hello world");
    }

    #[test]
    fn test_extract_text_content_blocks() {
        let content = JsonlContent::Blocks(vec![
            ContentBlock::Text {
                text: "First".to_string(),
            },
            ContentBlock::ToolUse {
                name: "Read".to_string(),
                input: None,
            },
            ContentBlock::Text {
                text: "Second".to_string(),
            },
        ]);
        assert_eq!(extract_text_content(&content), "First\nSecond");
    }

    #[test]
    fn test_extract_assistant_content_with_tools() {
        let content = JsonlContent::Blocks(vec![
            ContentBlock::Text {
                text: "Let me help".to_string(),
            },
            ContentBlock::ToolUse {
                name: "Read".to_string(),
                input: None,
            },
            ContentBlock::ToolUse {
                name: "Read".to_string(),
                input: None,
            },
            ContentBlock::ToolUse {
                name: "Edit".to_string(),
                input: None,
            },
        ]);

        let (text, tools) = extract_assistant_content(&content);
        assert_eq!(text, "Let me help");
        assert_eq!(tools.len(), 2); // Read and Edit

        let read_tool = tools.iter().find(|t| t.name == "Read").unwrap();
        assert_eq!(read_tool.count, 2);

        let edit_tool = tools.iter().find(|t| t.name == "Edit").unwrap();
        assert_eq!(edit_tool.count, 1);
    }

    #[test]
    fn test_clean_command_tags_basic() {
        let tag_regex = Regex::new(r"<command-name>[^<]*</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"<command-args>[^<]*</command-args>\s*").unwrap();

        let input = "<command-name>/commit</command-name>\nPlease commit";
        let result = clean_command_tags(input, &tag_regex, &args_regex);
        assert_eq!(result, "Please commit");
    }

    #[test]
    fn test_clean_command_tags_with_args() {
        let tag_regex = Regex::new(r"<command-name>[^<]*</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"<command-args>[^<]*</command-args>\s*").unwrap();

        let input = "<command-name>/review</command-name>\n<command-args>123</command-args>\nReview PR";
        let result = clean_command_tags(input, &tag_regex, &args_regex);
        assert_eq!(result, "Review PR");
    }

    #[test]
    fn test_clean_command_tags_no_tags() {
        let tag_regex = Regex::new(r"<command-name>[^<]*</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"<command-args>[^<]*</command-args>\s*").unwrap();

        let input = "Normal message without tags";
        let result = clean_command_tags(input, &tag_regex, &args_regex);
        assert_eq!(result, "Normal message without tags");
    }

    // ============================================================================
    // Integration Tests with Temporary Files
    // ============================================================================

    #[tokio::test]
    async fn test_parse_session_with_temp_file() {
        use tempfile::NamedTempFile;
        use tokio::io::AsyncWriteExt;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let content = r#"{"type":"user","message":{"content":"Test question"},"timestamp":"2026-01-27T12:00:00Z"}
{"type":"assistant","message":{"content":"Test answer"},"timestamp":"2026-01-27T12:00:01Z"}"#;

        let mut file = tokio::fs::File::create(&path).await.unwrap();
        file.write_all(content.as_bytes()).await.unwrap();
        file.flush().await.unwrap();

        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].content, "Test question");
        assert_eq!(session.messages[1].content, "Test answer");
    }

    #[tokio::test]
    async fn test_parse_session_empty_temp_file() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let session = parse_session(&path).await.unwrap();
        assert!(session.is_empty());
    }
}
