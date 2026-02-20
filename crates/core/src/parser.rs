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

/// Attach common fields (timestamp, uuid, parent_uuid) to a message.
///
/// Takes references so the same Option values can be used across multiple
/// branches within the user content match arms.
fn attach_common_fields(
    mut message: Message,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
) -> Message {
    if let Some(ts) = timestamp {
        message = message.with_timestamp(ts.clone());
    }
    if let Some(u) = uuid {
        message = message.with_uuid(u.clone());
    }
    if let Some(pu) = parent_uuid {
        message = message.with_parent_uuid(pu.clone());
    }
    message
}

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
/// use claude_view_core::parse_session;
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

    // Regex for cleaning command tags from user messages (dotall for multi-line content)
    let command_name_regex = Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap();
    let command_args_regex = Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap();
    let command_message_regex = Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap();

    // Track pending thinking text from thinking-only assistant messages
    let mut pending_thinking: Option<String> = None;

    while let Some(line_result) = lines.next_line().await.map_err(|e| ParseError::io(file_path, e))? {
        line_number += 1;
        let line = line_result.trim();

        // Skip empty or whitespace-only lines
        if line.is_empty() {
            continue;
        }

        // Parse as serde_json::Value to read the top-level "type" field
        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
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

        let entry_type = match value.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => {
                debug!("Skipping line {} with missing/non-string type field", line_number);
                continue;
            }
        };

        // Extract common fields: uuid, parentUuid, timestamp
        let uuid = value.get("uuid").and_then(|v| v.as_str()).map(String::from);
        let parent_uuid = value.get("parentUuid").and_then(|v| v.as_str()).map(String::from);
        let timestamp = value.get("timestamp").and_then(|v| v.as_str()).map(String::from);

        match entry_type {
            "user" => {
                // Skip meta messages
                if value.get("isMeta").and_then(|v| v.as_bool()) == Some(true) {
                    debug!("Skipping meta message at line {}", line_number);
                    continue;
                }

                // Check content type: string (real user prompt) vs array (tool results)
                let msg_content = value.get("message").and_then(|m| m.get("content"));
                match msg_content {
                    Some(serde_json::Value::Array(arr)) => {
                        // Check if any block is a tool_result type
                        let has_tool_result = arr.iter().any(|b|
                            b.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                        );
                        if has_tool_result {
                            // Role::ToolResult - extract readable content
                            let content = extract_tool_result_content(arr);
                            if !content.trim().is_empty() {
                                let message = Message::tool_result(content);
                                let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                                messages.push(message);
                            }
                        } else {
                            // Array content without tool_result blocks - extract text
                            // Deserialize message field for existing helper
                            if let Some(msg_value) = value.get("message") {
                                if let Ok(msg) = serde_json::from_value::<JsonlMessage>(msg_value.clone()) {
                                    let content = extract_text_content(&msg.content);
                                    let cleaned_content = clean_command_tags(
                                        &content,
                                        &command_name_regex,
                                        &command_args_regex,
                                        &command_message_regex,
                                    );
                                    let cleaned_content = cleaned_content.replace("\\\n", "\n");
                                    if !cleaned_content.trim().is_empty() {
                                        let message = Message::user(cleaned_content);
                                        let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                                        messages.push(message);
                                    }
                                }
                            }
                        }
                    }
                    Some(serde_json::Value::String(s)) => {
                        // Role::User - normal user prompt, apply command tag cleaning
                        let cleaned_content = clean_command_tags(
                            s,
                            &command_name_regex,
                            &command_args_regex,
                            &command_message_regex,
                        );
                        // D5: Normalize backslash-newline sequences
                        let cleaned_content = cleaned_content.replace("\\\n", "\n");

                        if !cleaned_content.trim().is_empty() {
                            let message = Message::user(cleaned_content);
                            let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                            messages.push(message);
                        }
                    }
                    _ => {
                        // No content or unexpected format - try legacy deserialization
                        if let Some(msg_value) = value.get("message") {
                            if let Ok(msg) = serde_json::from_value::<JsonlMessage>(msg_value.clone()) {
                                let content = extract_text_content(&msg.content);
                                let cleaned_content = clean_command_tags(
                                    &content,
                                    &command_name_regex,
                                    &command_args_regex,
                                    &command_message_regex,
                                );
                                let cleaned_content = cleaned_content.replace("\\\n", "\n");
                                if !cleaned_content.trim().is_empty() {
                                    let message = Message::user(cleaned_content);
                                    let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                                    messages.push(message);
                                }
                            }
                        }
                    }
                }
            }
            "assistant" => {
                // Deserialize the message field using existing types
                if let Some(msg_value) = value.get("message") {
                    if let Ok(msg) = serde_json::from_value::<JsonlMessage>(msg_value.clone()) {
                        let (content, tool_calls, thinking_text) =
                            extract_assistant_content(&msg.content);
                        let tool_call_count = tool_calls.iter().map(|tc| tc.count).sum::<usize>();
                        total_tool_calls += tool_call_count;

                        let has_content = !content.trim().is_empty();
                        let has_tools = !tool_calls.is_empty();
                        let has_thinking = thinking_text.is_some();

                        // If this message has ONLY thinking (no content, no tools),
                        // store the thinking for the next assistant message
                        if !has_content && !has_tools && has_thinking {
                            pending_thinking = thinking_text;
                            continue;
                        }

                        // Skip completely empty assistant messages
                        if !has_content && !has_tools && !has_thinking && pending_thinking.is_none() {
                            continue;
                        }

                        // Determine role: tool-only → ToolUse, otherwise Assistant
                        let mut message = if !has_content && has_tools {
                            Message::tool_use(content)
                        } else {
                            Message::assistant(content)
                        };

                        message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                        if has_tools {
                            message = message.with_tools(tool_calls);
                        }
                        // Attach thinking: prefer pending (from previous thinking-only message),
                        // fall back to this message's own thinking
                        if let Some(thinking) = pending_thinking.take() {
                            message = message.with_thinking(thinking);
                        } else if let Some(thinking) = thinking_text {
                            message = message.with_thinking(thinking);
                        }
                        messages.push(message);
                    }
                }
            }
            "system" => {
                let subtype = value.get("subtype").and_then(|v| v.as_str()).unwrap_or("unknown");

                // Note: system lines may have isMeta=true but we still emit them as metadata events

                let duration_ms = value.get("durationMs").and_then(|v| v.as_u64());
                let content = if let Some(ms) = duration_ms {
                    format!("{}: {}ms", subtype, ms)
                } else {
                    subtype.to_string()
                };

                // Build metadata from relevant fields
                let mut meta = serde_json::Map::new();
                meta.insert("subtype".to_string(), serde_json::Value::String(subtype.to_string()));
                if let Some(ms) = duration_ms {
                    meta.insert("durationMs".to_string(), serde_json::json!(ms));
                }
                if let Some(err) = value.get("error") {
                    meta.insert("error".to_string(), err.clone());
                }

                let message = Message::system(content)
                    .with_metadata(serde_json::Value::Object(meta));
                let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                messages.push(message);
            }
            "progress" => {
                let data = value.get("data");
                let data_type = data
                    .and_then(|d| d.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");

                // Build a readable content string
                let content = if let Some(hook_name) = data.and_then(|d| d.get("hookName")).and_then(|v| v.as_str()) {
                    format!("{}: {}", data_type, hook_name)
                } else if let Some(command) = data.and_then(|d| d.get("command")).and_then(|v| v.as_str()) {
                    format!("{}: {}", data_type, command)
                } else {
                    data_type.to_string()
                };

                // Copy data object as metadata
                let metadata = data.cloned().unwrap_or(serde_json::json!({"type": data_type}));

                let message = Message::progress(content)
                    .with_metadata(metadata);
                let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                messages.push(message);
            }
            "queue-operation" => {
                let operation = value.get("operation").and_then(|v| v.as_str()).unwrap_or("unknown");
                let op_content = value.get("content").and_then(|v| v.as_str());

                let content = if let Some(c) = op_content {
                    format!("queue-{}: {}", operation, c)
                } else {
                    format!("queue-{}", operation)
                };

                let mut meta = serde_json::Map::new();
                meta.insert("type".to_string(), serde_json::Value::String("queue-operation".to_string()));
                meta.insert("operation".to_string(), serde_json::Value::String(operation.to_string()));
                if let Some(c) = op_content {
                    meta.insert("content".to_string(), serde_json::Value::String(c.to_string()));
                }

                let message = Message::system(content)
                    .with_metadata(serde_json::Value::Object(meta));
                let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                messages.push(message);
            }
            "file-history-snapshot" => {
                let is_update = value.get("isSnapshotUpdate").and_then(|v| v.as_bool()).unwrap_or(false);
                let content = if is_update {
                    "file-history-snapshot (update)".to_string()
                } else {
                    "file-history-snapshot".to_string()
                };

                let mut meta = serde_json::Map::new();
                meta.insert("type".to_string(), serde_json::Value::String("file-history-snapshot".to_string()));
                if let Some(snapshot) = value.get("snapshot") {
                    meta.insert("snapshot".to_string(), snapshot.clone());
                }
                meta.insert("isSnapshotUpdate".to_string(), serde_json::json!(is_update));

                let message = Message::system(content)
                    .with_metadata(serde_json::Value::Object(meta));
                let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                messages.push(message);
            }
            "summary" => {
                let summary_text = value.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                let leaf_uuid = value.get("leafUuid").and_then(|v| v.as_str());

                let mut meta = serde_json::Map::new();
                meta.insert("summary".to_string(), serde_json::Value::String(summary_text.to_string()));
                if let Some(lu) = leaf_uuid {
                    meta.insert("leafUuid".to_string(), serde_json::Value::String(lu.to_string()));
                }

                let message = Message::summary(summary_text.to_string())
                    .with_metadata(serde_json::Value::Object(meta));
                let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                messages.push(message);
            }
            "saved_hook_context" => {
                // Hook-injected context (e.g. claude-mem snapshots)
                let content_items = value.get("content")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();

                let preview: String = content_items.chars().take(200).collect();
                let display = if content_items.is_empty() {
                    "saved_hook_context".to_string()
                } else if preview.len() < content_items.len() {
                    format!("saved_hook_context: {preview}…")
                } else {
                    format!("saved_hook_context: {preview}")
                };

                let mut meta = serde_json::Map::new();
                meta.insert("type".to_string(), serde_json::Value::String("saved_hook_context".to_string()));
                if let Some(content) = value.get("content") {
                    meta.insert("content".to_string(), content.clone());
                }

                let message = Message::system(display)
                    .with_metadata(serde_json::Value::Object(meta));
                let message = attach_common_fields(message, &timestamp, &uuid, &parent_uuid);
                messages.push(message);
            }
            _ => {
                // Silently ignore unknown entry types for forward compatibility
                debug!("Ignoring unknown entry type '{}' at line {}", entry_type, line_number);
            }
        }
    }

    Ok(ParsedSession::new(messages, total_tool_calls))
}

/// Parse a session JSONL file and return a paginated slice of messages.
///
/// Parses the full file (necessary for correct message ordering and thinking
/// attachment), then returns the requested slice. The total message count is
/// included so the frontend can compute pagination state.
pub async fn parse_session_paginated(
    file_path: &Path,
    limit: usize,
    offset: usize,
) -> Result<PaginatedMessages, ParseError> {
    let session = parse_session(file_path).await?;
    let total = session.messages.len();
    let messages: Vec<Message> = session
        .messages
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();
    let has_more = offset + messages.len() < total;

    Ok(PaginatedMessages {
        messages,
        total,
        offset,
        limit,
        has_more,
    })
}

/// Extract readable content from tool_result array blocks.
fn extract_tool_result_content(blocks: &[serde_json::Value]) -> String {
    let mut parts: Vec<String> = Vec::new();

    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str());
        match block_type {
            Some("tool_result") => {
                let tool_use_id = block.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("unknown");
                // Try to extract text content from the tool result
                match block.get("content") {
                    Some(serde_json::Value::String(s)) => {
                        let truncated = if s.len() > 200 {
                            // Find a safe char boundary at or before byte 200
                            let mut end = 200;
                            while end > 0 && !s.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...", &s[..end])
                        } else {
                            s.clone()
                        };
                        parts.push(format!("[Tool result for {}]: {}", tool_use_id, truncated));
                    }
                    Some(serde_json::Value::Array(arr)) => {
                        // Content might be array of text blocks
                        let text: String = arr.iter()
                            .filter_map(|item| {
                                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                    item.get("text").and_then(|t| t.as_str()).map(String::from)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        if !text.is_empty() {
                            parts.push(format!("[Tool result for {}]: {}", tool_use_id, text));
                        } else {
                            parts.push(format!("[Tool result for {}]", tool_use_id));
                        }
                    }
                    _ => {
                        parts.push(format!("[Tool result for {}]", tool_use_id));
                    }
                }
            }
            Some("text") => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    parts.join("\n")
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

/// Extract text content, tool calls, and thinking text from assistant message content.
///
/// Returns `(text, tool_calls, thinking)` where thinking is the concatenated
/// content of any `Thinking` blocks in the message.
fn extract_assistant_content(content: &JsonlContent) -> (String, Vec<ToolCall>, Option<String>) {
    match content {
        JsonlContent::Text(text) => (text.clone(), vec![], None),
        JsonlContent::Blocks(blocks) => {
            let mut text_parts: Vec<&str> = Vec::new();
            let mut thinking_parts: Vec<&str> = Vec::new();
            let mut tool_counts: HashMap<String, usize> = HashMap::new();

            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        text_parts.push(text);
                    }
                    ContentBlock::Thinking { thinking } => {
                        thinking_parts.push(thinking);
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

            let thinking = if thinking_parts.is_empty() {
                None
            } else {
                Some(thinking_parts.join("\n"))
            };

            (text, tool_calls, thinking)
        }
    }
}

/// Clean command tags from user messages.
///
/// Extracts content from `<command-args>` (the actual user input for slash commands),
/// strips `<command-name>` and `<command-message>` tags. If `<command-args>` is present,
/// its inner content becomes the message; otherwise the remaining text after stripping
/// the other tags is used.
fn clean_command_tags(
    content: &str,
    name_regex: &Regex,
    args_regex: &Regex,
    message_regex: &Regex,
) -> String {
    // Try to extract command-args content first
    if let Some(caps) = args_regex.captures(content) {
        if let Some(args_content) = caps.get(1) {
            let extracted = args_content.as_str().trim();
            if !extracted.is_empty() {
                return extracted.to_string();
            }
        }
    }

    // No command-args found (or empty), strip command-name and command-message tags
    let cleaned = name_regex.replace_all(content, "");
    let cleaned = message_regex.replace_all(&cleaned, "");
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

        // Second user message should extract command-args content as the message
        let second_user = &session.messages[2];
        assert!(!second_user.content.contains("<command-args>"));
        assert!(!second_user.content.contains("</command-args>"));
        assert!(!second_user.content.contains("<command-name>"));
        assert_eq!(second_user.content, "Review this PR #123");
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
        // File has: user, future_type (ignored), assistant, telemetry (ignored),
        // user, metadata (ignored), assistant
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

        let (text, tools, thinking) = extract_assistant_content(&content);
        assert_eq!(text, "Let me help");
        assert_eq!(tools.len(), 2); // Read and Edit
        assert!(thinking.is_none());

        let read_tool = tools.iter().find(|t| t.name == "Read").unwrap();
        assert_eq!(read_tool.count, 2);

        let edit_tool = tools.iter().find(|t| t.name == "Edit").unwrap();
        assert_eq!(edit_tool.count, 1);
    }

    #[test]
    fn test_extract_assistant_content_with_thinking() {
        let content = JsonlContent::Blocks(vec![
            ContentBlock::Thinking {
                thinking: "Let me reason about this...".to_string(),
            },
            ContentBlock::Text {
                text: "Here is the answer".to_string(),
            },
        ]);

        let (text, tools, thinking) = extract_assistant_content(&content);
        assert_eq!(text, "Here is the answer");
        assert!(tools.is_empty());
        assert_eq!(thinking, Some("Let me reason about this...".to_string()));
    }

    #[test]
    fn test_extract_assistant_content_thinking_only() {
        let content = JsonlContent::Blocks(vec![
            ContentBlock::Thinking {
                thinking: "Just thinking...".to_string(),
            },
        ]);

        let (text, tools, thinking) = extract_assistant_content(&content);
        assert_eq!(text, "");
        assert!(tools.is_empty());
        assert_eq!(thinking, Some("Just thinking...".to_string()));
    }

    #[test]
    fn test_clean_command_tags_basic() {
        let name_regex = Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap();
        let message_regex = Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap();

        let input = "<command-name>/commit</command-name>\nPlease commit";
        let result = clean_command_tags(input, &name_regex, &args_regex, &message_regex);
        assert_eq!(result, "Please commit");
    }

    #[test]
    fn test_clean_command_tags_with_args() {
        let name_regex = Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap();
        let message_regex = Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap();

        // When command-args is present, its content becomes the message
        let input = "<command-name>/review</command-name>\n<command-args>123</command-args>\nReview PR";
        let result = clean_command_tags(input, &name_regex, &args_regex, &message_regex);
        assert_eq!(result, "123");
    }

    #[test]
    fn test_clean_command_tags_with_multiline_args() {
        let name_regex = Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap();
        let message_regex = Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap();

        // command-args can contain < characters and span multiple lines
        let input = "<command-name>/review</command-name>\n<command-args>Fix the <T> generic\nacross files</command-args>";
        let result = clean_command_tags(input, &name_regex, &args_regex, &message_regex);
        assert_eq!(result, "Fix the <T> generic\nacross files");
    }

    #[test]
    fn test_clean_command_tags_no_tags() {
        let name_regex = Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap();
        let message_regex = Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap();

        let input = "Normal message without tags";
        let result = clean_command_tags(input, &name_regex, &args_regex, &message_regex);
        assert_eq!(result, "Normal message without tags");
    }

    #[test]
    fn test_clean_command_message_tags() {
        let name_regex = Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap();
        let args_regex = Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap();
        let message_regex = Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap();

        let input = "<command-name>/commit</command-name>\n<command-message>System prompt text</command-message>\nPlease commit";
        let result = clean_command_tags(input, &name_regex, &args_regex, &message_regex);
        assert_eq!(result, "Please commit");
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

    // ============================================================================
    // All 8 JSONL Line Types Tests
    // ============================================================================

    /// Fixture: all_types.jsonl has 13 lines:
    ///   1. user (string content)         → Role::User
    ///   2. assistant (text+thinking+tool) → Role::Assistant
    ///   3. user (tool_result array)       → Role::ToolResult
    ///   4. assistant (tool-only)          → Role::ToolUse
    ///   5. assistant (text-only)          → Role::Assistant
    ///   6. system                         → Role::System
    ///   7. progress                       → Role::Progress
    ///   8. queue-operation (enqueue)      → Role::System
    ///   9. queue-operation (dequeue)      → Role::System
    ///  10. summary                        → Role::Summary
    ///  11. file-history-snapshot          → Role::System
    ///  12. saved_hook_context             → Role::System
    ///  13. user (isMeta=true)             → skipped
    /// = 12 messages total

    #[tokio::test]
    async fn test_parse_all_types_count() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();
        assert_eq!(session.messages.len(), 12);
    }

    #[tokio::test]
    async fn test_parse_user_string_is_user_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        let msg = &session.messages[0];
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Read and fix auth.rs");
    }

    #[tokio::test]
    async fn test_parse_user_array_is_tool_result_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        let msg = &session.messages[2];
        assert_eq!(msg.role, Role::ToolResult);
        assert!(msg.content.contains("tool_result") || msg.content.contains("Tool result"));
    }

    #[tokio::test]
    async fn test_parse_assistant_with_text_is_assistant_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 1 is assistant with text+thinking+tools → Role::Assistant
        let msg = &session.messages[1];
        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.content.contains("I'll read the file first"));
        assert!(msg.tool_calls.is_some());
        assert!(msg.thinking.is_some());
    }

    #[tokio::test]
    async fn test_parse_assistant_tool_only_is_tool_use_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 3 is assistant with only tool_use blocks → Role::ToolUse
        let msg = &session.messages[3];
        assert_eq!(msg.role, Role::ToolUse);
        assert!(msg.tool_calls.is_some());
        let tools = msg.tool_calls.as_ref().unwrap();
        let edit_tool = tools.iter().find(|t| t.name == "Edit");
        assert!(edit_tool.is_some());
    }

    #[tokio::test]
    async fn test_parse_system_role_and_metadata() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 5 is system (turn_duration)
        let msg = &session.messages[5];
        assert_eq!(msg.role, Role::System);
        assert!(msg.content.contains("turn_duration"));
        assert!(msg.content.contains("5000"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(meta.get("subtype").unwrap().as_str().unwrap(), "turn_duration");
        assert_eq!(meta.get("durationMs").unwrap().as_u64().unwrap(), 5000);
    }

    #[tokio::test]
    async fn test_parse_progress_role_and_metadata() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 6 is progress (hook_progress)
        let msg = &session.messages[6];
        assert_eq!(msg.role, Role::Progress);
        assert!(msg.content.contains("hook_progress"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(meta.get("type").unwrap().as_str().unwrap(), "hook_progress");
        assert_eq!(meta.get("hookName").unwrap().as_str().unwrap(), "lint-check");
    }

    #[tokio::test]
    async fn test_parse_queue_operation_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 7 is queue-operation (enqueue)
        let msg = &session.messages[7];
        assert_eq!(msg.role, Role::System);
        assert!(msg.content.contains("enqueue"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(meta.get("type").unwrap().as_str().unwrap(), "queue-operation");
        assert_eq!(meta.get("operation").unwrap().as_str().unwrap(), "enqueue");
        assert_eq!(meta.get("content").unwrap().as_str().unwrap(), "next task");

        // Message index 8 is queue-operation (dequeue)
        let msg2 = &session.messages[8];
        assert_eq!(msg2.role, Role::System);
        assert!(msg2.content.contains("dequeue"));
        let meta2 = msg2.metadata.as_ref().unwrap();
        assert_eq!(meta2.get("operation").unwrap().as_str().unwrap(), "dequeue");
    }

    #[tokio::test]
    async fn test_parse_file_history_snapshot_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 10 is file-history-snapshot
        let msg = &session.messages[10];
        assert_eq!(msg.role, Role::System);
        assert!(msg.content.contains("file-history-snapshot"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(meta.get("type").unwrap().as_str().unwrap(), "file-history-snapshot");
        assert_eq!(meta.get("isSnapshotUpdate").unwrap().as_bool().unwrap(), false);
        assert!(meta.get("snapshot").is_some());
    }

    #[tokio::test]
    async fn test_parse_summary_role_and_metadata() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 9 is summary
        let msg = &session.messages[9];
        assert_eq!(msg.role, Role::Summary);
        assert_eq!(msg.content, "Fixed authentication bug in auth.rs");

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(meta.get("summary").unwrap().as_str().unwrap(), "Fixed authentication bug in auth.rs");
        assert_eq!(meta.get("leafUuid").unwrap().as_str().unwrap(), "a2");
    }

    #[tokio::test]
    async fn test_parse_uuid_passthrough() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // First user message has uuid "u1"
        assert_eq!(session.messages[0].uuid, Some("u1".to_string()));
        // First assistant has uuid "a1"
        assert_eq!(session.messages[1].uuid, Some("a1".to_string()));
        // System has uuid "s1"
        assert_eq!(session.messages[5].uuid, Some("s1".to_string()));
        // Progress has uuid "p1"
        assert_eq!(session.messages[6].uuid, Some("p1".to_string()));
        // Summary has uuid "sum1"
        assert_eq!(session.messages[9].uuid, Some("sum1".to_string()));
    }

    #[tokio::test]
    async fn test_parse_parent_uuid_passthrough() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // First user has no parentUuid
        assert_eq!(session.messages[0].parent_uuid, None);
        // First assistant has parentUuid "u1"
        assert_eq!(session.messages[1].parent_uuid, Some("u1".to_string()));
        // Tool result user (index 2) has parentUuid "a1"
        assert_eq!(session.messages[2].parent_uuid, Some("a1".to_string()));
    }

    #[tokio::test]
    async fn test_parse_saved_hook_context_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 11 is saved_hook_context
        let msg = &session.messages[11];
        assert_eq!(msg.role, Role::System);
        assert!(msg.content.contains("saved_hook_context"));
        assert!(msg.content.contains("hook context line 1"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(meta.get("type").unwrap().as_str().unwrap(), "saved_hook_context");
        let content_arr = meta.get("content").unwrap().as_array().unwrap();
        assert_eq!(content_arr.len(), 2);
        assert_eq!(content_arr[0].as_str().unwrap(), "hook context line 1");
    }

    #[tokio::test]
    async fn test_parse_meta_user_still_skipped() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // The last line is a user with isMeta=true, should be skipped
        // So we should have 12 messages, not 13
        assert_eq!(session.messages.len(), 12);
        // No message should contain "System init"
        for msg in &session.messages {
            assert!(!msg.content.contains("System init"));
        }
    }

    // ============================================================================
    // Paginated Parsing Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_session_paginated_first_page() {
        let path = fixtures_path().join("large_session.jsonl");
        let result = parse_session_paginated(&path, 10, 0).await.unwrap();
        assert_eq!(result.messages.len(), 10);
        assert_eq!(result.total, 200);
        assert_eq!(result.offset, 0);
        assert_eq!(result.limit, 10);
        assert!(result.has_more);
        assert!(result.messages[0].content.contains("Question number 1"));
    }

    #[tokio::test]
    async fn test_parse_session_paginated_last_page() {
        let path = fixtures_path().join("large_session.jsonl");
        let result = parse_session_paginated(&path, 10, 195).await.unwrap();
        assert_eq!(result.messages.len(), 5); // only 5 remaining
        assert_eq!(result.total, 200);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_parse_session_paginated_beyond_end() {
        let path = fixtures_path().join("large_session.jsonl");
        let result = parse_session_paginated(&path, 10, 999).await.unwrap();
        assert_eq!(result.messages.len(), 0);
        assert_eq!(result.total, 200);
        assert!(!result.has_more);
    }
}
