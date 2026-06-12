// crates/providers/src/parsers/piebald/message.rs
//
// One message row + its batched parts → conversation blocks and meta
// accounting (counts, first message, timestamps, token usage).

use super::db::{ChatParts, MsgRow, ToolCallRow};
use crate::model::{ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, preview, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;

/// Append the blocks for one message (if contentful) and update the meta.
pub(super) fn append_message(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    raw_id: &str,
    msg: &MsgRow,
    parts: &ChatParts,
) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut text_parts: Vec<&str> = Vec::new();
    let mut thinking_parts: Vec<&str> = Vec::new();
    // (tool_use_id, output, is_error) — attached after the call block lands.
    let mut results: Vec<(String, String, bool)> = Vec::new();

    for (part_id, part_type) in parts.by_message.get(&msg.id).into_iter().flatten() {
        match part_type.as_str() {
            "text" => {
                // Detail-row presence is load-bearing: a text part without
                // its message_part_text row is corrupt (the Go source fails
                // the whole chat; we skip + count instead).
                let Some(&is_thinking) = parts.thinking.get(part_id) else {
                    meta.malformed_lines += 1;
                    continue;
                };
                let text = parts.text.get(part_id).map_or("", String::as_str);
                if text.trim().is_empty() {
                    continue;
                }
                if is_thinking {
                    thinking_parts.push(text);
                } else {
                    text_parts.push(text);
                    segments.push(blocks::text_segment(text.to_string()));
                }
            }
            "tool_call" => {
                let Some(tc) = parts.tools.get(part_id) else {
                    meta.malformed_lines += 1;
                    continue;
                };
                if tc.tool_use_id.is_empty() {
                    continue;
                }
                segments.push(blocks::tool_segment(
                    tc.tool_name.clone(),
                    parse_tool_input(&tc.tool_input),
                    tc.tool_use_id.clone(),
                ));
                if let Some((output, is_error)) = tool_result_text(tc) {
                    results.push((tc.tool_use_id.clone(), output, is_error));
                }
            }
            _ => {}
        }
    }

    let content = text_parts.join("\n");
    let content = content.trim();
    let thinking = thinking_parts.join("\n");
    let thinking = thinking.trim();
    let has_tools = segments
        .iter()
        .any(|s| matches!(s, AssistantSegment::Tool { .. }));
    if content.is_empty() && thinking.is_empty() && !has_tools && results.is_empty() {
        return; // nothing contentful — not a message (Go parity)
    }

    let ts = time::parse_timestamp(&msg.created_at, false);
    if let Some(t) = ts {
        meta.observe_timestamp(t);
    }
    let ordinal = meta.message_count as usize;
    meta.message_count += 1;
    meta.record_model(&msg.model);
    if let Some(u) = usage_from(msg) {
        meta.usage.record(&msg.model, u);
    }

    let id = blocks::block_id(raw_id, ordinal);
    if msg.role.eq_ignore_ascii_case("assistant") {
        if !segments.is_empty() || !thinking.is_empty() {
            out.push(blocks::assistant(id, segments, non_empty(thinking), ts));
        }
    } else {
        // Any non-assistant role renders in the user lane (Go parity).
        // A user row carrying tool results is plumbing, not a real prompt.
        if results.is_empty() {
            meta.user_message_count += 1;
        }
        if !content.is_empty() {
            if meta.first_message.is_empty() {
                meta.first_message = preview(content, 200);
            }
            out.push(blocks::user(id, content.to_string(), ts));
        }
    }
    for (tool_use_id, output, is_error) in results {
        blocks::attach_tool_result(out, &tool_use_id, output, is_error);
    }
}

pub(super) fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Tool input is stored as a JSON string; non-JSON payloads pass through as
/// a plain string (the Go normalizeJSON quotes them — same information).
fn parse_tool_input(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_string()))
}

/// Result text priority (Go parity): non-NULL tool_result, else non-NULL
/// tool_error (flagged as error), else a `[state]` marker for non-completed
/// states. Nothing usable → no result attached.
fn tool_result_text(tc: &ToolCallRow) -> Option<(String, bool)> {
    if let Some(r) = &tc.tool_result {
        if !r.is_empty() {
            return Some((r.clone(), false));
        }
    } else if let Some(e) = &tc.tool_error {
        if !e.is_empty() {
            return Some((e.clone(), true));
        }
    }
    if !tc.tool_state.is_empty() && tc.tool_state != "completed" {
        return Some((format!("[{}]", tc.tool_state), false));
    }
    None
}

/// Token columns are nullable: NULL = absent (≠ zero), so usage is only
/// recorded when at least one column is present. Reasoning folds into
/// output and cache_write maps to cache_creation (Go parity). Piebald's
/// input_tokens is already Anthropic-shaped (cache reads are separate), so
/// no cache subtraction happens here.
fn usage_from(m: &MsgRow) -> Option<UsageTotals> {
    let cols = [
        m.input_tokens,
        m.output_tokens,
        m.reasoning_tokens,
        m.cache_read_tokens,
        m.cache_write_tokens,
    ];
    if cols.iter().all(Option::is_none) {
        return None;
    }
    let nn = |v: Option<i64>| v.unwrap_or(0).max(0) as u64;
    Some(UsageTotals {
        input_tokens: nn(m.input_tokens),
        output_tokens: nn(m.output_tokens) + nn(m.reasoning_tokens),
        cache_read_input_tokens: nn(m.cache_read_tokens),
        cache_creation_input_tokens: nn(m.cache_write_tokens),
    })
}
