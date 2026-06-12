// crates/providers/src/parsers/kiro_ide/new_format.rs
//
// NEW Kiro IDE generation: `workspace-sessions/<b64-path>/<uuid>.json` with a
// history array. Assistant turns reference exec-log files by executionId
// (see exec_log.rs); when those are missing the promptLogs completions are
// the fallback transcript.

use super::exec_log;
use crate::discover::stat_entry;
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, preview, project_from_cwd};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(super) fn parse(path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
    let raw = std::fs::read_to_string(path)?;
    let doc: Value = serde_json::from_str(&raw)?;
    let Some(history) = doc.get("history").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    if history.is_empty() {
        return Ok(Vec::new());
    }

    let raw_id = doc
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow::anyhow!("kiro-ide session has no id"))?;

    let mut meta = ForeignSessionMeta::new(ProviderKind::KiroIde, &raw_id, path.to_path_buf());
    meta.title = doc
        .get("title")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .map(str::to_string);
    let ws_dir = doc
        .get("workspaceDirectory")
        .and_then(Value::as_str)
        .unwrap_or("");
    meta.cwd = (!ws_dir.is_empty()).then(|| ws_dir.to_string());
    meta.project = if ws_dir.is_empty() {
        "unknown".to_string()
    } else {
        let p = project_from_cwd(ws_dir);
        if p.is_empty() {
            "unknown".to_string()
        } else {
            p
        }
    };

    let exec_index = exec_log::dir_for(path)
        .map(exec_log::build_index)
        .unwrap_or_default();

    let mut out_blocks: Vec<ConversationBlock> = Vec::new();
    let mut has_content = false;
    for entry in history {
        let Some(msg) = entry.get("message") else {
            continue;
        };
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        let content = extract_text(msg.get("content"));
        match role {
            "user" => {
                if content.is_empty() {
                    continue;
                }
                if meta.first_message.is_empty() {
                    meta.first_message = preview(&content, 200);
                }
                let id = blocks::block_id(&raw_id, out_blocks.len());
                meta.message_count += 1;
                meta.user_message_count += 1;
                has_content = true;
                out_blocks.push(blocks::user(id, content, None));
            }
            "assistant" => {
                // Exec log first, then the (usually empty) inline content,
                // then the promptLogs completions as the last resort.
                let (mut text, tools) = resolve_assistant(entry, &exec_index);
                if text.is_empty() {
                    text = content;
                }
                if text.is_empty() {
                    text = prompt_log_text(entry);
                }
                if text.is_empty() && tools.is_empty() {
                    continue;
                }
                let mut segments: Vec<AssistantSegment> = Vec::with_capacity(tools.len() + 1);
                if !text.is_empty() {
                    has_content = true;
                    segments.push(blocks::text_segment(text));
                }
                segments.extend(tools);
                let id = blocks::block_id(&raw_id, out_blocks.len());
                meta.message_count += 1;
                out_blocks.push(blocks::assistant(id, segments, None, None));
            }
            // Tool results are consumed by the assistant exec log; standalone
            // 'tool' rows would render blank.
            _ => {}
        }
    }
    // Tool-only transcripts carry no human-readable content — skip.
    if !has_content {
        return Ok(Vec::new());
    }
    if let Some((mtime, _)) = stat_entry(path) {
        meta.observe_timestamp(mtime);
    }
    Ok(vec![ForeignSession {
        meta,
        blocks: out_blocks,
    }])
}

/// Content can be a plain string or Claude-style `[{type:'text', text}]`.
fn extract_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Array(items)) => {
            let parts: Vec<&str> = items
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .collect();
            parts.join("\n")
        }
        _ => String::new(),
    }
}

/// Concatenated promptLogs completions (fallback when exec logs are gone).
fn prompt_log_text(entry: &Value) -> String {
    let Some(logs) = entry.get("promptLogs").and_then(Value::as_array) else {
        return String::new();
    };
    let parts: Vec<&str> = logs
        .iter()
        .filter_map(|l| l.get("completion").and_then(Value::as_str))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    parts.join("\n\n")
}

/// Resolve an assistant history entry against its exec log: 'say' actions
/// become text; 'replace'/'create'/'readCode' become tool segments carrying
/// the REAL file contents (no diff synthesis — truthfulness rule).
fn resolve_assistant(
    entry: &Value,
    exec_index: &HashMap<String, PathBuf>,
) -> (String, Vec<AssistantSegment>) {
    let empty = (String::new(), Vec::new());
    let exec_id = entry
        .get("executionId")
        .and_then(Value::as_str)
        .unwrap_or("");
    if exec_id.is_empty() {
        return empty;
    }
    let Some(path) = exec_index.get(exec_id) else {
        return empty;
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return empty;
    };
    let Ok(doc) = serde_json::from_str::<Value>(&raw) else {
        return empty;
    };
    let Some(actions) = doc.get("actions").and_then(Value::as_array) else {
        return empty;
    };

    let mut text_parts: Vec<&str> = Vec::new();
    let mut tools: Vec<AssistantSegment> = Vec::new();
    for action in actions {
        let action_type = action
            .get("actionType")
            .and_then(Value::as_str)
            .unwrap_or("");
        let action_id = action.get("actionId").and_then(Value::as_str).unwrap_or("");
        let input = action.get("input");
        let file = input
            .and_then(|i| i.get("file"))
            .and_then(Value::as_str)
            .unwrap_or("");
        match action_type {
            "say" => {
                if let Some(m) = action.pointer("/output/message").and_then(Value::as_str) {
                    if !m.is_empty() {
                        text_parts.push(m);
                    }
                }
            }
            "replace" if !file.is_empty() => {
                let mut obj = serde_json::Map::new();
                obj.insert("file".to_string(), Value::String(file.to_string()));
                for key in ["originalContent", "modifiedContent"] {
                    if let Some(v) = input.and_then(|i| i.get(key)).and_then(Value::as_str) {
                        if !v.is_empty() {
                            obj.insert(key.to_string(), Value::String(v.to_string()));
                        }
                    }
                }
                tools.push(blocks::tool_segment(
                    "Edit".to_string(),
                    Value::Object(obj),
                    action_id.to_string(),
                ));
            }
            "create" if !file.is_empty() => {
                let mut obj = serde_json::Map::new();
                obj.insert("file".to_string(), Value::String(file.to_string()));
                if let Some(c) = input
                    .and_then(|i| i.get("modifiedContent"))
                    .and_then(Value::as_str)
                {
                    if !c.is_empty() {
                        obj.insert("content".to_string(), Value::String(c.to_string()));
                    }
                }
                tools.push(blocks::tool_segment(
                    "Write".to_string(),
                    Value::Object(obj),
                    action_id.to_string(),
                ));
            }
            "readCode" if !file.is_empty() => {
                tools.push(blocks::tool_segment(
                    "Read".to_string(),
                    input.cloned().unwrap_or(Value::Null),
                    action_id.to_string(),
                ));
            }
            _ => {}
        }
    }
    (text_parts.join("\n\n"), tools)
}
