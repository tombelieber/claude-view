// crates/core/src/parser/session.rs
//! Core session parsing logic: reads JSONL files and produces `ParsedSession`.
//!
//! This is the main entry point for parsing Claude Code session JSONL files.
//! It handles the line-by-line dispatch (user, assistant, system, progress,
//! queue-operation, file-history-snapshot) and delegates content extraction
//! to sibling modules.

use crate::category::{categorize_progress, categorize_tool};
use crate::error::ParseError;
use crate::types::*;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

use super::content::{
    extract_assistant_content, extract_text_content, extract_tool_result_content,
};
use super::helpers::{attach_common_fields, maybe_attach_raw};
use super::tags::{clean_command_tags, TagRegexes};

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
    parse_session_inner(file_path, false).await
}

/// Like [`parse_session`], but attaches the full raw JSONL value to each message
/// as `Message.raw_json`. Used by the debug UI to inspect the original data.
pub async fn parse_session_with_raw(file_path: &Path) -> Result<ParsedSession, ParseError> {
    parse_session_inner(file_path, true).await
}

async fn parse_session_inner(
    file_path: &Path,
    include_raw: bool,
) -> Result<ParsedSession, ParseError> {
    let file = File::open(file_path)
        .await
        .map_err(|e| ParseError::io(file_path, e))?;

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut messages: Vec<Message> = Vec::new();
    let mut total_tool_calls: usize = 0;
    let mut line_number: usize = 0;

    let tag_regexes = TagRegexes::new();

    // Track pending thinking text from thinking-only assistant messages
    let mut pending_thinking: Option<String> = None;

    while let Some(line_result) = lines
        .next_line()
        .await
        .map_err(|e| ParseError::io(file_path, e))?
    {
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
                    line_number, file_path, e
                );
                continue;
            }
        };

        let entry_type = match value.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => {
                debug!(
                    "Skipping line {} with missing/non-string type field",
                    line_number
                );
                continue;
            }
        };

        // Extract common fields: uuid, parentUuid, timestamp
        let uuid = value.get("uuid").and_then(|v| v.as_str()).map(String::from);
        let parent_uuid = value
            .get("parentUuid")
            .and_then(|v| v.as_str())
            .map(String::from);
        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from);

        match entry_type {
            "user" => {
                handle_user_entry(
                    &value,
                    &timestamp,
                    &uuid,
                    &parent_uuid,
                    &tag_regexes,
                    include_raw,
                    line_number,
                    &mut messages,
                );
            }
            "assistant" => {
                handle_assistant_entry(
                    &value,
                    &timestamp,
                    &uuid,
                    &parent_uuid,
                    include_raw,
                    &mut pending_thinking,
                    &mut total_tool_calls,
                    &mut messages,
                );
            }
            "system" => {
                handle_system_entry(
                    &value,
                    &timestamp,
                    &uuid,
                    &parent_uuid,
                    include_raw,
                    &mut messages,
                );
            }
            "progress" => {
                handle_progress_entry(
                    &value,
                    &timestamp,
                    &uuid,
                    &parent_uuid,
                    include_raw,
                    &mut messages,
                );
            }
            "queue-operation" => {
                handle_queue_operation_entry(
                    &value,
                    &timestamp,
                    &uuid,
                    &parent_uuid,
                    include_raw,
                    &mut messages,
                );
            }
            "file-history-snapshot" => {
                handle_file_history_snapshot_entry(
                    &value,
                    &timestamp,
                    &uuid,
                    &parent_uuid,
                    include_raw,
                    &mut messages,
                );
            }
            _ => {
                // Silently ignore unknown entry types for forward compatibility
                debug!(
                    "Ignoring unknown entry type '{}' at line {}",
                    entry_type, line_number
                );
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
    parse_session_paginated_inner(file_path, limit, offset, false).await
}

/// Like [`parse_session_paginated`], but includes raw JSONL on each message.
pub async fn parse_session_paginated_with_raw(
    file_path: &Path,
    limit: usize,
    offset: usize,
) -> Result<PaginatedMessages, ParseError> {
    parse_session_paginated_inner(file_path, limit, offset, true).await
}

async fn parse_session_paginated_inner(
    file_path: &Path,
    limit: usize,
    offset: usize,
    include_raw: bool,
) -> Result<PaginatedMessages, ParseError> {
    let session = parse_session_inner(file_path, include_raw).await?;
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

// ---------------------------------------------------------------------------
// Entry type handlers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn handle_user_entry(
    value: &serde_json::Value,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
    tag_regexes: &TagRegexes,
    include_raw: bool,
    line_number: usize,
    messages: &mut Vec<Message>,
) {
    // Skip meta messages
    if value.get("isMeta").and_then(|v| v.as_bool()) == Some(true) {
        debug!("Skipping meta message at line {}", line_number);
        return;
    }

    // Check content type: string (real user prompt) vs array (tool results)
    let msg_content = value.get("message").and_then(|m| m.get("content"));
    match msg_content {
        Some(serde_json::Value::Array(arr)) => {
            // Check if any block is a tool_result type
            let has_tool_result = arr
                .iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_result"));
            if has_tool_result {
                // Role::ToolResult - extract readable content
                let content = extract_tool_result_content(arr);
                if !content.trim().is_empty() {
                    let message = Message::tool_result(content);
                    let message = attach_common_fields(message, timestamp, uuid, parent_uuid);
                    let message = maybe_attach_raw(message, value, include_raw);
                    messages.push(message);
                }
            } else {
                // Array content without tool_result blocks - extract text
                push_user_text_message(
                    value,
                    timestamp,
                    uuid,
                    parent_uuid,
                    tag_regexes,
                    include_raw,
                    messages,
                );
            }
        }
        Some(serde_json::Value::String(s)) => {
            // Role::User - normal user prompt, apply command tag cleaning
            let cleaned_content = clean_command_tags(s, tag_regexes);
            // Normalize backslash-newline sequences
            let cleaned_content = cleaned_content.replace("\\\n", "\n");

            if !cleaned_content.trim().is_empty() {
                let message = Message::user(cleaned_content);
                let message = attach_common_fields(message, timestamp, uuid, parent_uuid);
                let message = maybe_attach_raw(message, value, include_raw);
                messages.push(message);
            }
        }
        _ => {
            // No content or unexpected format - try legacy deserialization
            push_user_text_message(
                value,
                timestamp,
                uuid,
                parent_uuid,
                tag_regexes,
                include_raw,
                messages,
            );
        }
    }
}

/// Shared helper for user messages that need JsonlMessage deserialization + tag cleaning.
fn push_user_text_message(
    value: &serde_json::Value,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
    tag_regexes: &TagRegexes,
    include_raw: bool,
    messages: &mut Vec<Message>,
) {
    if let Some(msg_value) = value.get("message") {
        if let Ok(msg) = serde_json::from_value::<JsonlMessage>(msg_value.clone()) {
            let content = extract_text_content(&msg.content);
            let cleaned_content = clean_command_tags(&content, tag_regexes);
            let cleaned_content = cleaned_content.replace("\\\n", "\n");
            if !cleaned_content.trim().is_empty() {
                let message = Message::user(cleaned_content);
                let message = attach_common_fields(message, timestamp, uuid, parent_uuid);
                let message = maybe_attach_raw(message, value, include_raw);
                messages.push(message);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_assistant_entry(
    value: &serde_json::Value,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
    include_raw: bool,
    pending_thinking: &mut Option<String>,
    total_tool_calls: &mut usize,
    messages: &mut Vec<Message>,
) {
    // Deserialize the message field using existing types
    if let Some(msg_value) = value.get("message") {
        if let Ok(msg) = serde_json::from_value::<JsonlMessage>(msg_value.clone()) {
            let (content, tool_calls, thinking_text) = extract_assistant_content(&msg.content);
            let tool_call_count = tool_calls.iter().map(|tc| tc.count).sum::<usize>();
            *total_tool_calls += tool_call_count;

            let has_content = !content.trim().is_empty();
            let has_tools = !tool_calls.is_empty();
            let has_thinking = thinking_text.is_some();

            // If this message has ONLY thinking (no content, no tools),
            // store the thinking for the next assistant message
            if !has_content && !has_tools && has_thinking {
                *pending_thinking = thinking_text;
                return;
            }

            // Skip completely empty assistant messages
            if !has_content && !has_tools && !has_thinking && pending_thinking.is_none() {
                return;
            }

            // Determine role: tool-only -> ToolUse, otherwise Assistant
            let mut message = if !has_content && has_tools {
                Message::tool_use(content)
            } else {
                Message::assistant(content)
            };

            message = attach_common_fields(message, timestamp, uuid, parent_uuid);
            if has_tools {
                if let Some(first_tool) = tool_calls.first() {
                    message = message.with_category(categorize_tool(&first_tool.name));
                }
                message = message.with_tools(tool_calls);
            }
            // Attach thinking: prefer pending (from previous thinking-only message),
            // fall back to this message's own thinking
            if let Some(thinking) = pending_thinking.take() {
                message = message.with_thinking(thinking);
            } else if let Some(thinking) = thinking_text {
                message = message.with_thinking(thinking);
            }
            let message = maybe_attach_raw(message, value, include_raw);
            messages.push(message);
        }
    }
}

fn handle_system_entry(
    value: &serde_json::Value,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
    include_raw: bool,
    messages: &mut Vec<Message>,
) {
    let subtype = value
        .get("subtype")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let duration_ms = value.get("durationMs").and_then(|v| v.as_u64());
    let content = if let Some(ms) = duration_ms {
        format!("{}: {}ms", subtype, ms)
    } else {
        subtype.to_string()
    };

    // Build metadata from relevant fields
    let mut meta = serde_json::Map::new();
    meta.insert(
        "subtype".to_string(),
        serde_json::Value::String(subtype.to_string()),
    );
    if let Some(ms) = duration_ms {
        meta.insert("durationMs".to_string(), serde_json::json!(ms));
    }
    if let Some(err) = value.get("error") {
        meta.insert("error".to_string(), err.clone());
    }

    let message = Message::system(content).with_metadata(serde_json::Value::Object(meta));
    let message = attach_common_fields(message, timestamp, uuid, parent_uuid);
    let message = message.with_category("system");
    let message = maybe_attach_raw(message, value, include_raw);
    messages.push(message);
}

fn handle_progress_entry(
    value: &serde_json::Value,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
    include_raw: bool,
    messages: &mut Vec<Message>,
) {
    let data = value.get("data");
    let data_type = data
        .and_then(|d| d.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");

    // Build a readable content string
    let content = if let Some(hook_name) = data
        .and_then(|d| d.get("hookName"))
        .and_then(|v| v.as_str())
    {
        format!("{}: {}", data_type, hook_name)
    } else if let Some(command) = data.and_then(|d| d.get("command")).and_then(|v| v.as_str()) {
        format!("{}: {}", data_type, command)
    } else {
        data_type.to_string()
    };

    // Copy data object as metadata
    let metadata = data
        .cloned()
        .unwrap_or(serde_json::json!({"type": data_type}));

    let message = Message::progress(content).with_metadata(metadata);
    let message = attach_common_fields(message, timestamp, uuid, parent_uuid);
    let message = if let Some(cat) = categorize_progress(data_type) {
        message.with_category(cat)
    } else {
        message
    };
    let message = maybe_attach_raw(message, value, include_raw);
    messages.push(message);
}

fn handle_queue_operation_entry(
    value: &serde_json::Value,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
    include_raw: bool,
    messages: &mut Vec<Message>,
) {
    let operation = value
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let op_content = value.get("content").and_then(|v| v.as_str());

    let content = if let Some(c) = op_content {
        format!("queue-{}: {}", operation, c)
    } else {
        format!("queue-{}", operation)
    };

    let mut meta = serde_json::Map::new();
    meta.insert(
        "type".to_string(),
        serde_json::Value::String("queue-operation".to_string()),
    );
    meta.insert(
        "operation".to_string(),
        serde_json::Value::String(operation.to_string()),
    );
    if let Some(c) = op_content {
        meta.insert(
            "content".to_string(),
            serde_json::Value::String(c.to_string()),
        );
    }

    let message = Message::system(content).with_metadata(serde_json::Value::Object(meta));
    let message = attach_common_fields(message, timestamp, uuid, parent_uuid);
    let message = message.with_category("queue");
    let message = maybe_attach_raw(message, value, include_raw);
    messages.push(message);
}

fn handle_file_history_snapshot_entry(
    value: &serde_json::Value,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
    include_raw: bool,
    messages: &mut Vec<Message>,
) {
    let is_update = value
        .get("isSnapshotUpdate")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let content = if is_update {
        "file-history-snapshot (update)".to_string()
    } else {
        "file-history-snapshot".to_string()
    };

    let mut meta = serde_json::Map::new();
    meta.insert(
        "type".to_string(),
        serde_json::Value::String("file-history-snapshot".to_string()),
    );
    if let Some(snapshot) = value.get("snapshot") {
        meta.insert("snapshot".to_string(), snapshot.clone());
    }
    meta.insert("isSnapshotUpdate".to_string(), serde_json::json!(is_update));

    let message = Message::system(content).with_metadata(serde_json::Value::Object(meta));
    let message = attach_common_fields(message, timestamp, uuid, parent_uuid);
    let message = message.with_category("snapshot");
    let message = maybe_attach_raw(message, value, include_raw);
    messages.push(message);
}
