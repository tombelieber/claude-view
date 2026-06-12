// crates/providers/src/parsers/zed/doc.rs
//
// Thread doc → ForeignSession: decode the (optionally zstd-compressed)
// JSON payload, extract model/usage/messages, and build blocks.

use super::db::{is_valid_session_id, virtual_path, ThreadRow};
use super::walk::{
    extract_text, extract_thinking, extract_tool_calls, extract_tool_results, has_content,
    message_content,
};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::Path;

pub(super) fn build_session(row: ThreadRow, db_path: &Path) -> Option<ForeignSession> {
    if !is_valid_session_id(&row.id) {
        return None;
    }
    let payload = decode_thread_data(&row.data_type, &row.data).ok()?;
    let doc: Value = serde_json::from_slice(&payload).ok()?;
    doc.as_object()?;

    let mut meta =
        ForeignSessionMeta::new(ProviderKind::Zed, &row.id, virtual_path(db_path, &row.id));
    meta.title = Some(row.summary.clone()).filter(|s| !s.is_empty());
    let cwd = first_folder_path(&row.folder_paths);
    if !cwd.is_empty() {
        let project = project_from_cwd(&cwd);
        if !project.is_empty() {
            meta.project = project;
        }
        meta.cwd = Some(cwd);
    }
    if meta.project.is_empty() {
        meta.project = "zed".to_string();
    }
    if let Some(ts) = time::parse_timestamp(&row.created_at, false) {
        meta.observe_timestamp(ts);
    }
    if let Some(ts) = time::parse_timestamp(&row.updated_at, false) {
        meta.observe_timestamp(ts);
    }

    // One model applies to the whole thread; request_token_usage is summed
    // into a single per-model record (input/output only — Zed tracks no
    // cache buckets).
    let model = doc
        .pointer("/model/model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !model.is_empty() {
        meta.record_model(&model);
    }
    let (totals, has_usage) = sum_request_usage(&doc);
    if has_usage {
        meta.usage.record(&model, totals);
    }

    let mut out_blocks: Vec<ConversationBlock> = Vec::new();
    let messages = doc.get("messages").and_then(Value::as_array);
    for (ordinal, item) in messages.into_iter().flatten().enumerate() {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let id = blocks::block_id(&row.id, ordinal);
        if let Some(user) = obj.get("User") {
            handle_user(&mut out_blocks, &mut meta, id, user);
        } else if let Some(agent) = obj.get("Agent") {
            handle_agent(&mut out_blocks, &mut meta, id, agent);
        }
    }
    if meta.first_message.is_empty() {
        meta.first_message = preview(&row.summary, 200);
    }

    // Threads with zero contentful messages are non-interactive noise.
    if meta.message_count == 0 {
        return None;
    }
    Some(ForeignSession {
        meta,
        blocks: out_blocks,
    })
}

fn decode_thread_data(data_type: &str, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    match data_type.to_ascii_lowercase().as_str() {
        "" | "json" => Ok(data.to_vec()),
        "zstd" => Ok(zstd::decode_all(data)?),
        other => anyhow::bail!("unsupported zed data_type {other:?}"),
    }
}

fn sum_request_usage(doc: &Value) -> (UsageTotals, bool) {
    let mut totals = UsageTotals::default();
    let mut has_usage = false;
    let Some(req_usage) = doc.get("request_token_usage").and_then(Value::as_object) else {
        return (totals, false);
    };
    for entry in req_usage.values() {
        let Some(entry) = entry.as_object() else {
            continue;
        };
        has_usage = true;
        if let Some(input) = entry.get("input_tokens").and_then(Value::as_f64) {
            totals.input_tokens += input.max(0.0) as u64;
        }
        if let Some(output) = entry.get("output_tokens").and_then(Value::as_f64) {
            totals.output_tokens += output.max(0.0) as u64;
        }
    }
    (totals, has_usage)
}

/// First path from the newline/NUL-separated folder_paths column
/// (ported from zedFirstFolderPath).
fn first_folder_path(paths: &str) -> String {
    let paths = paths.trim();
    for sep in ['\n', '\0'] {
        if let Some((before, _)) = paths.split_once(sep) {
            return before.trim().to_string();
        }
    }
    paths.to_string()
}

fn handle_user(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    user: &Value,
) {
    let content = message_content(user);
    let text = extract_text(content).trim().to_string();
    // Drop only when there is truly no content at all; messages with
    // structured blocks (attachments, images) but no Text leaf are kept
    // with empty text so conversation continuity is preserved.
    if text.is_empty() && !has_content(content) {
        return;
    }
    if !text.is_empty() && meta.first_message.is_empty() {
        meta.first_message = preview(&text, 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    out.push(blocks::user(id, text, None));
}

fn handle_agent(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    agent: &Value,
) {
    let text = extract_text(message_content(agent)).trim().to_string();
    let thinking = extract_thinking(agent).trim().to_string();
    let tool_calls = extract_tool_calls(agent);
    let tool_results = extract_tool_results(agent);
    if text.is_empty() && thinking.is_empty() && tool_calls.is_empty() && tool_results.is_empty() {
        return;
    }
    meta.message_count += 1;

    let mut segments: Vec<AssistantSegment> = Vec::new();
    if !text.is_empty() {
        segments.push(blocks::text_segment(text));
    }
    for (tool_id, name, input) in tool_calls {
        segments.push(blocks::tool_segment(name, input, tool_id));
    }
    if !segments.is_empty() || !thinking.is_empty() {
        out.push(blocks::assistant(
            id,
            segments,
            Some(thinking).filter(|t| !t.is_empty()),
            None,
        ));
    }
    // Results live alongside their calls on the same Agent message; attach
    // searches backwards so calls from earlier messages also resolve.
    for (tool_use_id, output) in tool_results {
        blocks::attach_tool_result(out, &tool_use_id, output, false);
    }
}
