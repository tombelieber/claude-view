// crates/providers/src/parsers/opencode/build.rs
//
// Shared session assembly for both OpenCode backends. Rows arrive
// pre-extracted (storage JSON files or SQLite JSON `data` columns) in one
// uniform shape: a session row plus message/part rows whose `data` is the
// raw OpenCode JSON document. Ported from agentsview's
// buildOpenCodeParsedSession.

use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

pub(super) struct SessionRow {
    pub id: String,
    pub title: String,
    /// Worktree (SQLite) or `directory` (storage) — the session cwd.
    pub directory: String,
    pub created_ms: i64,
    pub updated_ms: i64,
}

pub(super) struct MessageRow {
    pub id: String,
    pub data: Value,
    pub sort_time_ms: i64,
}

pub(super) struct PartRow {
    pub id: String,
    pub data: Value,
    pub sort_time_ms: i64,
}

/// Assemble one normalized session. Returns `None` when the session has no
/// contentful user/assistant messages (non-interactive noise).
pub(super) fn build_session(
    row: SessionRow,
    source_path: PathBuf,
    msgs: Vec<MessageRow>,
    mut parts: HashMap<String, Vec<PartRow>>,
    malformed: u32,
) -> Option<ForeignSession> {
    let mut meta = ForeignSessionMeta::new(ProviderKind::Opencode, &row.id, source_path);
    meta.malformed_lines = malformed;
    // Prefer OpenCode's LLM-generated title; skip the exact default
    // placeholders ("New session - <ISO8601>" / "Child session - …").
    if !row.title.is_empty() && !is_default_title(&row.title) {
        meta.title = Some(row.title.clone());
    }
    if !row.directory.is_empty() {
        meta.cwd = Some(row.directory.clone());
    }
    let project = project_from_cwd(&row.directory);
    meta.project = if project.is_empty() {
        "unknown".to_string()
    } else {
        project
    };
    if row.created_ms > 0 {
        meta.observe_timestamp(time::from_millis(row.created_ms as f64));
    }
    if row.updated_ms > 0 {
        meta.observe_timestamp(time::from_millis(row.updated_ms as f64));
    }

    let mut out_blocks: Vec<ConversationBlock> = Vec::new();
    let mut ordinal = 0usize;
    for msg in &msgs {
        let role = msg.data.get("role").and_then(Value::as_str).unwrap_or("");
        if role != "user" && role != "assistant" {
            continue; // system / foreign roles
        }
        let mut msg_parts = parts.remove(&msg.id).unwrap_or_default();
        msg_parts.sort_by(|a, b| {
            a.sort_time_ms
                .cmp(&b.sort_time_ms)
                .then_with(|| a.id.cmp(&b.id))
        });
        let ts = (msg.sort_time_ms > 0).then(|| time::from_millis(msg.sort_time_ms as f64));
        let id = blocks::block_id(&row.id, ordinal);
        let kept = if role == "user" {
            handle_user(&mut out_blocks, &mut meta, id, ts, &msg_parts)
        } else {
            handle_assistant(&mut out_blocks, &mut meta, id, ts, &msg_parts)
        };
        if !kept {
            continue;
        }
        ordinal += 1;
        if let Some(ts) = ts {
            meta.observe_timestamp(ts);
        }
        let model = extract_model(&msg.data);
        if !model.is_empty() {
            meta.record_model(&model);
        }
        if let Some(totals) = collect_usage(&msg.data, &msg_parts) {
            meta.usage.record(&model, totals);
        }
    }

    if meta.message_count == 0 {
        return None;
    }
    Some(ForeignSession {
        meta,
        blocks: out_blocks,
    })
}

fn handle_user(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    ts: Option<f64>,
    parts: &[PartRow],
) -> bool {
    let texts: Vec<&str> = parts
        .iter()
        .filter(|p| part_type(&p.data) == "text")
        .map(|p| extract_text(&p.data))
        .filter(|t| !t.trim().is_empty())
        .collect();
    if texts.is_empty() {
        return false;
    }
    let text = texts.join("\n");
    if meta.first_message.is_empty() {
        meta.first_message = preview(&text, 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    out.push(blocks::user(id, text, ts));
    true
}

fn handle_assistant(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    ts: Option<f64>,
    parts: &[PartRow],
) -> bool {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut thinking_parts: Vec<&str> = Vec::new();
    let mut results: Vec<(String, String, bool)> = Vec::new();
    let mut has_tool_use = false;
    for p in parts {
        match part_type(&p.data) {
            "text" => {
                let t = extract_text(&p.data);
                if !t.trim().is_empty() {
                    segments.push(blocks::text_segment(t.to_string()));
                }
            }
            "reasoning" => {
                let t = extract_text(&p.data);
                if !t.trim().is_empty() {
                    thinking_parts.push(t);
                }
            }
            "tool" => {
                has_tool_use = true;
                let name = p.data.get("tool").and_then(Value::as_str).unwrap_or("");
                if name.is_empty() {
                    continue;
                }
                let call_id = p
                    .data
                    .get("callID")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let state = p.data.get("state");
                let input = state
                    .and_then(|s| s.get("input"))
                    .cloned()
                    .unwrap_or(Value::Null);
                segments.push(blocks::tool_segment(
                    name.to_string(),
                    input,
                    call_id.clone(),
                ));
                if !call_id.is_empty() {
                    if let Some((output, is_error)) = tool_state_result(state) {
                        results.push((call_id, output, is_error));
                    }
                }
            }
            // step-start, step-finish, patch, file, snapshot…
            _ => {}
        }
    }
    if segments.is_empty() && thinking_parts.is_empty() && !has_tool_use {
        return false;
    }
    let thinking = (!thinking_parts.is_empty()).then(|| thinking_parts.join("\n\n"));
    meta.message_count += 1;
    out.push(blocks::assistant(id, segments, thinking, ts));
    for (call_id, output, is_error) in results {
        blocks::attach_tool_result(out, &call_id, output, is_error);
    }
    true
}

fn part_type(data: &Value) -> &str {
    data.get("type").and_then(Value::as_str).unwrap_or("")
}

/// Text of a text/reasoning part: `content` preferred, `text` fallback
/// (mirrors agentsview's extractOpenCodeText).
fn extract_text(data: &Value) -> &str {
    match data.get("content").and_then(Value::as_str) {
        Some(c) if !c.is_empty() => c,
        _ => data.get("text").and_then(Value::as_str).unwrap_or(""),
    }
}

/// Result text for a tool part. OpenCode stores the rendered output on the
/// same part (`state.output`); errors carry `state.error`. Non-string
/// payloads yield no result (never fabricated).
fn tool_state_result(state: Option<&Value>) -> Option<(String, bool)> {
    let state = state?;
    let status = state.get("status").and_then(Value::as_str).unwrap_or("");
    if status == "error" {
        let msg = state
            .get("error")
            .and_then(Value::as_str)
            .or_else(|| state.get("output").and_then(Value::as_str))
            .unwrap_or("error");
        return Some((msg.to_string(), true));
    }
    let output = state.get("output").and_then(Value::as_str)?;
    if output.is_empty() {
        return None;
    }
    Some((output.to_string(), false))
}

/// Model id: top-level `modelID`, falling back to `model.modelID`.
fn extract_model(data: &Value) -> String {
    data.get("modelID")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
        .or_else(|| {
            data.pointer("/model/modelID")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty())
        })
        .unwrap_or("")
        .to_string()
}

#[derive(Default)]
struct TokenFields {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    any: bool,
}

/// Per-message token usage: the message `tokens{}` plus any `step-finish`
/// part tokens — later sources override per-field (mirrors agentsview's
/// collectOpenCodeTokenFields). Returns `None` when no recognized field is
/// present anywhere (empty `{}` / foreign schema ⇒ truthfully no usage);
/// explicit zeros are preserved as known-zero usage.
fn collect_usage(data: &Value, parts: &[PartRow]) -> Option<UsageTotals> {
    let mut f = TokenFields::default();
    collect_token_fields(&mut f, data);
    for p in parts.iter().filter(|p| part_type(&p.data) == "step-finish") {
        collect_token_fields(&mut f, &p.data);
    }
    f.any.then_some(UsageTotals {
        input_tokens: f.input,
        output_tokens: f.output,
        cache_read_input_tokens: f.cache_read,
        cache_creation_input_tokens: f.cache_write,
    })
}

fn collect_token_fields(fields: &mut TokenFields, raw: &Value) {
    let Some(tokens) = raw.get("tokens").and_then(Value::as_object) else {
        return;
    };
    if let Some(v) = tokens.get("input") {
        fields.input = token_value(v);
        fields.any = true;
    }
    if let Some(v) = tokens.get("output") {
        fields.output = token_value(v);
        fields.any = true;
    }
    let cache = tokens.get("cache");
    if let Some(v) = cache.and_then(|c| c.get("read")) {
        fields.cache_read = token_value(v);
        fields.any = true;
    }
    if let Some(v) = cache.and_then(|c| c.get("write")) {
        fields.cache_write = token_value(v);
        fields.any = true;
    }
}

fn token_value(v: &Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_f64().map(|f| f.max(0.0) as u64))
        .unwrap_or(0)
}

/// Matches OpenCode's auto-generated placeholder titles exactly:
/// `New session - 2026-03-22T10:00:00.000Z` (also "Child session").
fn is_default_title(title: &str) -> bool {
    let Some(ts) = title
        .strip_prefix("New session - ")
        .or_else(|| title.strip_prefix("Child session - "))
    else {
        return false;
    };
    is_placeholder_timestamp(ts)
}

/// `\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z` — checked without a regex
/// dependency.
fn is_placeholder_timestamp(ts: &str) -> bool {
    let b = ts.as_bytes();
    if b.len() != 24 {
        return false;
    }
    b.iter().enumerate().all(|(i, &c)| match i {
        4 | 7 => c == b'-',
        10 => c == b'T',
        13 | 16 => c == b':',
        19 => c == b'.',
        23 => c == b'Z',
        _ => c.is_ascii_digit(),
    })
}

/// Numeric epoch-ms JSON field (int or float) → ms, 0 when absent.
pub(super) fn as_ms(v: Option<&Value>) -> i64 {
    v.and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}

/// Message ordering time: created → start → end → updated.
pub(super) fn message_sort_time(data: &Value) -> i64 {
    first_nonzero_time(data, &["created", "start", "end", "updated"])
}

/// Part ordering time: start → created → end → updated (streamed parts get
/// `start` before `created` lands).
pub(super) fn part_sort_time(data: &Value) -> i64 {
    first_nonzero_time(data, &["start", "created", "end", "updated"])
}

fn first_nonzero_time(data: &Value, keys: &[&str]) -> i64 {
    let Some(t) = data.get("time") else {
        return 0;
    };
    keys.iter()
        .map(|k| as_ms(t.get(*k)))
        .find(|&v| v != 0)
        .unwrap_or(0)
}
