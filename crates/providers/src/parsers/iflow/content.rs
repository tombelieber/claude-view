// crates/providers/src/parsers/iflow/content.rs
//
// Entry → ConversationBlock extraction. Message content is either a plain
// string or an Anthropic-style block array (text/thinking/tool_use in
// assistant turns; text/tool_result in user turns). Filtering order mirrors
// the Go parser: meta drop → command normalization → emptiness → system
// patterns; a dropped user entry discards its tool results too.

use super::command::{extract_command_text, is_command_envelope, is_system_message};
use super::{is_meta_entry, Entry};
use crate::model::ForeignSessionMeta;
use crate::util::{blocks, preview};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;

pub(super) fn handle_user(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    entry: &Entry,
) {
    if is_meta_entry(&entry.value) {
        return;
    }
    let mut text_parts: Vec<String> = Vec::new();
    let mut results: Vec<(String, String, bool)> = Vec::new();
    match entry.value.pointer("/message/content") {
        Some(Value::String(s)) => {
            if !s.trim().is_empty() {
                text_parts.push(s.clone());
            }
        }
        Some(Value::Array(items)) => {
            for block in items {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(Value::as_str) {
                            if !t.trim().is_empty() {
                                text_parts.push(t.to_string());
                            }
                        }
                    }
                    Some("tool_result") => {
                        let tuid = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if tuid.is_empty() {
                            continue;
                        }
                        let is_error = block
                            .get("is_error")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        results.push((
                            tuid.to_string(),
                            decode_result_content(block.get("content")),
                            is_error,
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    let mut text = text_parts.join("\n");
    // Command envelopes normalize to "/cmd args"; un-normalizable envelopes
    // and system-injected messages drop the whole entry (results included),
    // mirroring the Go parser's filtering order.
    if let Some(cmd) = extract_command_text(&text) {
        text = cmd;
    } else if is_command_envelope(&text) {
        return;
    }
    if text.trim().is_empty() && results.is_empty() {
        return;
    }
    if is_system_message(&text) {
        return;
    }
    for (tuid, output, is_error) in results {
        blocks::attach_tool_result(out, &tuid, output, is_error);
    }
    if text.trim().is_empty() {
        return;
    }
    if meta.first_message.is_empty() {
        meta.first_message = preview(&text, 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    out.push(blocks::user(id, text, entry.timestamp));
}

pub(super) fn handle_assistant(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    entry: &Entry,
) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut thinking_parts: Vec<String> = Vec::new();
    match entry.value.pointer("/message/content") {
        Some(Value::String(s)) => {
            if !s.trim().is_empty() {
                segments.push(blocks::text_segment(s.clone()));
            }
        }
        Some(Value::Array(items)) => {
            for block in items {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(Value::as_str) {
                            if !t.trim().is_empty() {
                                segments.push(blocks::text_segment(t.to_string()));
                            }
                        }
                    }
                    Some("thinking") => {
                        if let Some(t) = block.get("thinking").and_then(Value::as_str) {
                            if !t.trim().is_empty() {
                                thinking_parts.push(t.to_string());
                            }
                        }
                    }
                    Some("tool_use") => {
                        let tool_id = block.get("id").and_then(Value::as_str).unwrap_or("");
                        if tool_id.is_empty() {
                            continue;
                        }
                        let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
                        segments.push(blocks::tool_segment(
                            name.to_string(),
                            block.get("input").cloned().unwrap_or(Value::Null),
                            tool_id.to_string(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    if segments.is_empty() && thinking_parts.is_empty() {
        return;
    }
    meta.message_count += 1;
    let thinking = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join("\n\n"))
    };
    out.push(blocks::assistant(id, segments, thinking, entry.timestamp));
}

/// Decode a tool_result `content` payload: plain string, array of text
/// blocks, or iFlow's nested functionResponse envelope.
fn decode_result_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .concat(),
        Some(obj @ Value::Object(_)) => obj
            .pointer("/responseParts/functionResponse/response/output")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}
