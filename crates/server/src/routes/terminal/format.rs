//! JSONL line formatting for terminal WebSocket display modes.
//!
//! Handles raw, rich, and block mode formatting of JSONL lines before
//! sending them over the WebSocket connection.

use claude_view_core::category::{categorize_progress, categorize_tool};

use super::types::RichModeFinders;

/// Strip Claude Code internal command tags from content.
///
/// Removes matched pairs of tags like `<command-name>...</command-name>` that
/// Claude Code injects for internal command routing. These are noise in the
/// terminal monitor and should never reach the WebSocket stream.
pub(super) fn strip_command_tags(content: &str) -> String {
    let mut result = content.to_string();
    let tags = [
        "command-name",
        "command-message",
        "command-args",
        "local-command-stdout",
        "system-reminder",
    ];

    for tag in &tags {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");

        // Loop until no more opening tags are found
        while let Some(start) = result.find(&open) {
            // Search for closing tag AFTER the opening tag position
            match result[start..].find(&close) {
                Some(offset) => {
                    let end = start + offset + close.len();
                    result.replace_range(start..end, "");
                }
                None => {
                    // No closing tag found — break to avoid infinite loop
                    break;
                }
            }
        }
    }
    result.trim().to_string()
}

/// Format a JSONL line for sending over WebSocket.
///
/// - In "raw" mode: wraps the line in `{ "type": "line", "data": "..." }`.
/// - In "rich" mode: SIMD pre-filters the line, parses as JSON, and extracts
///   structured fields (type/role/content/tool names/timestamp). Returns an
///   empty vec for lines that shouldn't be displayed (progress events, metadata,
///   empty messages). Returns multiple messages when a single JSONL line contains
///   multiple content blocks (e.g., thinking + text, or multiple tool_use calls).
pub(super) fn format_line_for_mode(
    line: &str,
    mode: &str,
    finders: &RichModeFinders,
) -> Vec<String> {
    if mode == "block" {
        return format_block_mode(line);
    }

    if mode != "rich" {
        // Raw mode: send as-is
        let msg = serde_json::json!({
            "type": "line",
            "data": line,
        });
        return vec![msg.to_string()];
    }

    format_rich_mode(line, finders)
}

/// Block mode: parse via BlockAccumulator, return ConversationBlock JSON.
/// Per-line accumulator works for independent lines (user, progress, system).
/// Multi-line constructs (AssistantBlock spanning assistant + tool_result)
/// produce separate blocks per line — the frontend stream accumulator handles assembly.
fn format_block_mode(line: &str) -> Vec<String> {
    if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
        let mut acc = claude_view_core::block_accumulator::BlockAccumulator::new();
        acc.process_line(&entry);
        let blocks = acc.finalize();
        return blocks
            .into_iter()
            .filter_map(|block| serde_json::to_string(&block).ok())
            .collect();
    }
    vec![]
}

/// Rich mode: SIMD pre-filter before JSON parse, then extract structured fields.
fn format_rich_mode(line: &str, finders: &RichModeFinders) -> Vec<String> {
    let line_bytes = line.as_bytes();

    // Quick check: does the line even look like it has a "type" key?
    if finders.type_key.find(line_bytes).is_none() {
        return vec![]; // No "type" key — not a structured message
    }

    // Parse as JSON
    let parsed: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return vec![], // JSON parse failed — skip in rich mode
    };

    // Extract the top-level "type" field (e.g., "assistant", "user", "system")
    let line_type = parsed
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Extract timestamp early so all match arms can use it
    let timestamp = if finders.timestamp_key.find(line_bytes).is_some() {
        parsed.get("timestamp").and_then(|v| v.as_str())
    } else {
        None
    };

    // Handle structured types with categories
    match line_type {
        "progress" => return format_progress(&parsed, timestamp),
        "file-history-snapshot" => return format_file_history_snapshot(timestamp),
        "system" => return format_system(&parsed, timestamp),
        "queue-operation" => return format_queue_operation(&parsed, timestamp),
        "summary" => return format_summary(&parsed, timestamp),
        _ => {}
    }

    // Skip meta messages (internal system prompts, skill loading, etc.)
    if parsed.get("isMeta").and_then(|v| v.as_bool()) == Some(true) {
        return vec![];
    }

    format_message_content(&parsed, line_type, line_bytes, finders, timestamp)
}

/// Format a progress-type JSONL line.
fn format_progress(parsed: &serde_json::Value, timestamp: Option<&str>) -> Vec<String> {
    let data = parsed.get("data");
    let data_type = data
        .and_then(|d| d.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");

    let category = categorize_progress(data_type);

    let hook_name = data
        .and_then(|d| d.get("hookName"))
        .and_then(|v| v.as_str());
    let command = data.and_then(|d| d.get("command")).and_then(|v| v.as_str());
    let content = if let Some(hn) = hook_name {
        format!("{}: {}", data_type, hn)
    } else if let Some(cmd) = command {
        format!("{}: {}", data_type, cmd)
    } else {
        data_type.to_string()
    };

    let mut result = serde_json::json!({
        "type": "progress",
        "content": content,
        "metadata": data,
    });
    if let Some(cat) = category {
        result["category"] = serde_json::Value::String(cat.to_string());
    }
    if let Some(ts) = timestamp {
        result["ts"] = serde_json::Value::String(ts.to_string());
    }
    vec![result.to_string()]
}

/// Format a file-history-snapshot JSONL line.
fn format_file_history_snapshot(timestamp: Option<&str>) -> Vec<String> {
    let mut result = serde_json::json!({
        "type": "system",
        "content": "file-history-snapshot",
        "category": "snapshot",
    });
    if let Some(ts) = timestamp {
        result["ts"] = serde_json::Value::String(ts.to_string());
    }
    vec![result.to_string()]
}

/// Format a system-type JSONL line.
fn format_system(parsed: &serde_json::Value, timestamp: Option<&str>) -> Vec<String> {
    let subtype = parsed
        .get("subtype")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let duration_ms = parsed.get("durationMs").and_then(|v| v.as_u64());
    let content = if let Some(ms) = duration_ms {
        format!("{}: {}ms", subtype, ms)
    } else {
        subtype.to_string()
    };
    let mut result = serde_json::json!({
        "type": "system",
        "content": content,
        "category": "system",
    });
    if let Some(ts) = timestamp {
        result["ts"] = serde_json::Value::String(ts.to_string());
    }
    vec![result.to_string()]
}

/// Format a queue-operation JSONL line.
fn format_queue_operation(parsed: &serde_json::Value, timestamp: Option<&str>) -> Vec<String> {
    let operation = parsed
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let op_content = parsed.get("content").and_then(|v| v.as_str());

    let content = if let Some(c) = op_content {
        format!("queue-{}: {}", operation, c)
    } else {
        format!("queue-{}", operation)
    };

    let mut metadata = serde_json::json!({
        "type": "queue-operation",
        "operation": operation,
    });
    if let Some(c) = op_content {
        metadata["content"] = serde_json::Value::String(c.to_string());
    }

    let mut result = serde_json::json!({
        "type": "system",
        "content": content,
        "category": "queue",
        "metadata": metadata,
    });
    if let Some(ts) = timestamp {
        result["ts"] = serde_json::Value::String(ts.to_string());
    }
    vec![result.to_string()]
}

/// Format a summary-type JSONL line.
fn format_summary(parsed: &serde_json::Value, timestamp: Option<&str>) -> Vec<String> {
    let summary_text = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("");
    let mut result = serde_json::json!({
        "type": "summary",
        "content": summary_text,
    });
    if let Some(ts) = timestamp {
        result["ts"] = serde_json::Value::String(ts.to_string());
    }
    vec![result.to_string()]
}

/// Extract and format message content (role-bearing messages with string or array content).
fn format_message_content(
    parsed: &serde_json::Value,
    line_type: &str,
    line_bytes: &[u8],
    finders: &RichModeFinders,
    timestamp: Option<&str>,
) -> Vec<String> {
    // The nested "message" object (Claude Code JSONL wraps fields under "message")
    let msg_obj = parsed.get("message");

    // Extract role from top-level or nested message
    let role = if finders.role_key.find(line_bytes).is_some() {
        parsed
            .get("role")
            .or_else(|| msg_obj.and_then(|m| m.get("role")))
            .and_then(|v| v.as_str())
    } else {
        None
    };

    // Resolve content source (top-level or nested message)
    let content_source = if parsed.get("content").is_some() {
        Some(parsed)
    } else {
        msg_obj
    };

    // For plain string content, return a single message
    if let Some(src) = content_source {
        if let Some(serde_json::Value::String(s)) = src.get("content") {
            let stripped = strip_command_tags(s);
            if stripped.is_empty() {
                return vec![];
            }
            let mut result = serde_json::json!({
                "type": "message",
                "role": role.unwrap_or(line_type),
                "content": stripped,
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            return vec![result.to_string()];
        }
    }

    // For array content, extract ALL blocks — no truncation, no dropping.
    let blocks = content_source
        .and_then(|src| src.get("content"))
        .and_then(|c| c.as_array());

    let blocks = match blocks {
        Some(b) => b,
        None => return vec![],
    };

    format_content_blocks(blocks, role, line_type, timestamp)
}

/// Extract structured messages from an array of content blocks.
fn format_content_blocks(
    blocks: &[serde_json::Value],
    role: Option<&str>,
    line_type: &str,
    timestamp: Option<&str>,
) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();

    // Collect ALL thinking text (concatenated)
    let mut thinking_parts: Vec<&str> = Vec::new();
    // Collect ALL text blocks (concatenated)
    let mut text_parts: Vec<&str> = Vec::new();
    // Track the last tool category so tool_result can inherit it
    let mut last_tool_category: Option<&str> = None;

    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match block_type {
            "thinking" => {
                if let Some(thinking) = block.get("thinking").and_then(|t| t.as_str()) {
                    thinking_parts.push(thinking);
                }
            }
            "text" => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text);
                }
            }
            "tool_use" => {
                let tool_name = block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                let category = categorize_tool(tool_name);
                last_tool_category = Some(category);
                let input = block
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let mut result = serde_json::json!({
                    "type": "tool_use",
                    "name": tool_name,
                    "input": input,
                    "category": category,
                });
                if let Some(ts) = timestamp {
                    result["ts"] = serde_json::Value::String(ts.to_string());
                }
                results.push(result.to_string());
            }
            "tool_result" => {
                let content = block
                    .get("content")
                    .map(|c| match c {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let mut result = serde_json::json!({
                    "type": "tool_result",
                    "content": content,
                });
                if let Some(cat) = last_tool_category {
                    result["category"] = serde_json::Value::String(cat.to_string());
                }
                if let Some(ts) = timestamp {
                    result["ts"] = serde_json::Value::String(ts.to_string());
                }
                results.push(result.to_string());
            }
            _ => {}
        }
    }

    // Emit concatenated thinking (all thinking blocks joined)
    if !thinking_parts.is_empty() {
        let full_thinking = thinking_parts.join("\n");
        let mut result = serde_json::json!({
            "type": "thinking",
            "content": full_thinking,
        });
        if let Some(ts) = timestamp {
            result["ts"] = serde_json::Value::String(ts.to_string());
        }
        results.push(result.to_string());
    }

    // Emit concatenated text (all text blocks joined), with command tags stripped
    if !text_parts.is_empty() {
        let full_text = text_parts.join("\n");
        let stripped = strip_command_tags(&full_text);
        if !stripped.is_empty() {
            let mut result = serde_json::json!({
                "type": "message",
                "role": role.unwrap_or(line_type),
                "content": stripped,
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            results.push(result.to_string());
        }
    }

    results
}
