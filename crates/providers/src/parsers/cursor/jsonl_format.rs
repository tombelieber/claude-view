// crates/providers/src/parsers/cursor/jsonl_format.rs
//
// JSONL Cursor transcripts: one Anthropic-style message object per line —
// { role, message.content: string | [text|thinking|tool_use|tool_result] }
// (port of isCursorJSONL / parseCursorJSONL / extractJSONLUserContent).

use super::text_format::extract_user_query;
use crate::model::ForeignSessionMeta;
use crate::util::{blocks, preview};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;

/// True when the data looks like JSONL rather than plain text. Scans up to
/// 4 KB to locate the first non-whitespace byte, then validates the FULL
/// first line from the original data (exact port of isCursorJSONL).
pub(super) fn is_cursor_jsonl(data: &str) -> bool {
    const MAX_SCAN: usize = 4096;
    let bytes = data.as_bytes();
    let limit = bytes.len().min(MAX_SCAN);
    let Some(start) = bytes[..limit]
        .iter()
        .position(|&b| b != b'\n' && b != b'\r' && b != b' ' && b != b'\t')
    else {
        return false;
    };
    // `start` is a char boundary: everything before it is ASCII whitespace.
    let rest = &data[start..];
    let line = rest.find('\n').map_or(rest, |e| &rest[..e]);
    serde_json::from_str::<Value>(line.trim()).is_ok()
}

/// Convert one parsed JSONL line into blocks. Unknown roles and missing
/// `message.content` are skipped silently (valid JSON, just not messages).
pub(super) fn handle_message(
    v: &Value,
    raw_id: &str,
    ordinal: &mut usize,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) {
    let role = v.get("role").and_then(Value::as_str).unwrap_or("");
    if role != "user" && role != "assistant" {
        return;
    }
    let Some(content) = v.pointer("/message/content") else {
        return;
    };
    if role == "user" {
        handle_user(content, raw_id, ordinal, meta, out);
    } else {
        handle_assistant(content, raw_id, ordinal, meta, out);
    }
}

fn handle_user(
    content: &Value,
    raw_id: &str,
    ordinal: &mut usize,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) {
    // tool_result blocks ride user messages in the Anthropic shape; attach
    // them to the matching earlier call.
    if let Some(arr) = content.as_array() {
        for b in arr {
            if b.get("type").and_then(Value::as_str) == Some("tool_result") {
                apply_tool_result(out, b);
            }
        }
    }
    let text = extract_jsonl_user_content(content);
    if text.is_empty() {
        return;
    }
    let id = blocks::block_id(raw_id, *ordinal);
    *ordinal += 1;
    if meta.first_message.is_empty() {
        meta.first_message = preview(&text, 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    out.push(blocks::user(id, text, None));
}

fn handle_assistant(
    content: &Value,
    raw_id: &str,
    ordinal: &mut usize,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut thinking_parts: Vec<String> = Vec::new();
    let mut results: Vec<&Value> = Vec::new();
    if let Some(s) = content.as_str() {
        let t = s.trim();
        if !t.is_empty() {
            segments.push(blocks::text_segment(t.to_string()));
        }
    } else if let Some(arr) = content.as_array() {
        for b in arr {
            match b.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = b.get("text").and_then(Value::as_str) {
                        if !t.trim().is_empty() {
                            segments.push(blocks::text_segment(t.to_string()));
                        }
                    }
                }
                Some("thinking") => {
                    if let Some(t) = b.get("thinking").and_then(Value::as_str) {
                        if !t.trim().is_empty() {
                            thinking_parts.push(t.to_string());
                        }
                    }
                }
                Some("tool_use") => {
                    let name = b.get("name").and_then(Value::as_str).unwrap_or("");
                    if name.is_empty() {
                        continue;
                    }
                    let tool_id = b.get("id").and_then(Value::as_str).unwrap_or("");
                    segments.push(blocks::tool_segment(
                        name.to_string(),
                        b.get("input").cloned().unwrap_or(Value::Null),
                        tool_id.to_string(),
                    ));
                }
                Some("tool_result") => results.push(b),
                _ => {}
            }
        }
    }
    if !segments.is_empty() || !thinking_parts.is_empty() {
        let id = blocks::block_id(raw_id, *ordinal);
        *ordinal += 1;
        meta.message_count += 1;
        let thinking = (!thinking_parts.is_empty()).then(|| thinking_parts.join("\n\n"));
        out.push(blocks::assistant(id, segments, thinking, None));
    }
    // Attach after the block lands so same-message call+result pairs work.
    for b in results {
        apply_tool_result(out, b);
    }
}

fn apply_tool_result(out: &mut [ConversationBlock], block: &Value) {
    let tool_use_id = block
        .get("tool_use_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if tool_use_id.is_empty() {
        return;
    }
    let output = serialize_result_content(block.get("content"));
    let is_error = block
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    blocks::attach_tool_result(out, tool_use_id, output, is_error);
}

/// Tool-result content → display text: plain string, or the text fields of
/// an array of blocks; other shapes yield nothing (never raw-JSON noise).
fn serialize_result_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// User message content → text. Strings and text-block arrays both pass
/// through <user_query> extraction (port of extractJSONLUserContent).
fn extract_jsonl_user_content(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return extract_user_query(s);
    }
    let Some(arr) = content.as_array() else {
        return String::new();
    };
    let parts: Vec<&str> = arr
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .filter(|t| !t.is_empty())
        .collect();
    if parts.is_empty() {
        return String::new();
    }
    extract_user_query(&parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_detection_scan_window() {
        assert!(is_cursor_jsonl(
            r#"{"role":"user","message":{"content":"hi"}}"#
        ));
        assert!(is_cursor_jsonl("\n\n{\"role\":\"user\"}"));
        assert!(!is_cursor_jsonl("user:\nhello\nassistant:\nworld"));
        assert!(!is_cursor_jsonl(""));
        assert!(!is_cursor_jsonl(&"\n".repeat(100)));
        // First line longer than the 4 KB window is still JSONL — the
        // window only locates the line START; the full line is validated.
        let long = format!(
            r#"{{"role":"user","message":{{"content":"{}"}}}}"#,
            "x".repeat(5000)
        );
        assert!(is_cursor_jsonl(&long));
        // First non-empty line starting beyond 4 KB of blanks → plain text.
        let blanks = format!("{}{}", "\n".repeat(5000), r#"{"role":"user"}"#);
        assert!(!is_cursor_jsonl(&blanks));
    }

    #[test]
    fn user_content_string_and_array_forms() {
        let s = serde_json::json!("<user_query>What is Go?</user_query>");
        assert_eq!(extract_jsonl_user_content(&s), "What is Go?");
        let arr = serde_json::json!([
            { "type": "text", "text": "<user_query>hi" },
            { "type": "text", "text": "there</user_query>" }
        ]);
        assert_eq!(extract_jsonl_user_content(&arr), "hi\nthere");
        assert_eq!(extract_jsonl_user_content(&serde_json::json!(null)), "");
    }
}
