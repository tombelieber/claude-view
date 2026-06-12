// crates/providers/src/parsers/kiro_ide/chat_format.rs
//
// OLD Kiro IDE generation: `<root>/<ws-hash>/<exec-hash>.chat` single-JSON
// with a flat chat array (roles human/bot/tool) and modelId + epoch-ms
// start/end in metadata. System-prompt rows ride the 'human' role and are
// filtered by prefix heuristics; real prompts may be wrapped in
// `<kiro-ide-message>` envelopes.

use crate::discover::stat_entry;
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, preview, project_from_cwd, time};
use claude_view_types::block_types::ConversationBlock;
use serde_json::Value;
use std::path::Path;

pub(super) fn parse(path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
    let raw = std::fs::read_to_string(path)?;
    let doc: Value = serde_json::from_str(&raw)?;
    let Some(chat) = doc.get("chat").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    if chat.is_empty() {
        return Ok(Vec::new());
    }

    let ws_hash = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let file_hash = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let raw_id = format!("{ws_hash}:{file_hash}");
    let mut meta = ForeignSessionMeta::new(ProviderKind::KiroIde, &raw_id, path.to_path_buf());
    meta.project = project_for_ws_hash(path, ws_hash);
    let model = doc
        .pointer("/metadata/modelId")
        .and_then(Value::as_str)
        .unwrap_or("");

    let mut out_blocks: Vec<ConversationBlock> = Vec::new();
    for msg in chat {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        let content = msg
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        match role {
            "human" => {
                if content.is_empty() || is_system_message(content) {
                    continue;
                }
                let text = strip_wrapper(content);
                if text.is_empty() {
                    continue;
                }
                if meta.first_message.is_empty() {
                    meta.first_message = preview(text, 200);
                }
                let id = blocks::block_id(&raw_id, out_blocks.len());
                meta.message_count += 1;
                meta.user_message_count += 1;
                out_blocks.push(blocks::user(id, text.to_string(), None));
            }
            "bot" => {
                if content.is_empty() || content == "I will follow these instructions." {
                    continue;
                }
                let id = blocks::block_id(&raw_id, out_blocks.len());
                meta.message_count += 1;
                meta.record_model(model);
                out_blocks.push(blocks::assistant(
                    id,
                    vec![blocks::text_segment(content.to_string())],
                    None,
                    None,
                ));
            }
            // Tool results are folded into the bot text by Kiro itself;
            // skipping them avoids blank transcript rows.
            _ => {}
        }
    }
    if meta.message_count == 0 {
        return Ok(Vec::new());
    }

    let mtime = stat_entry(path).map(|(m, _)| m).unwrap_or(0.0);
    let start_ms = doc
        .pointer("/metadata/startTime")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let end_ms = doc
        .pointer("/metadata/endTime")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    meta.observe_timestamp(if start_ms > 0.0 {
        time::from_millis(start_ms)
    } else {
        mtime
    });
    meta.observe_timestamp(if end_ms > 0.0 {
        time::from_millis(end_ms)
    } else {
        mtime
    });

    Ok(vec![ForeignSession {
        meta,
        blocks: out_blocks,
    }])
}

/// System-prompt / rules payloads injected as 'human' turns — not user text.
fn is_system_message(content: &str) -> bool {
    [
        "# System Prompt",
        "# Identity",
        "<identity>",
        "## Included Rules",
        "You are operating in a workspace",
    ]
    .iter()
    .any(|p| content.starts_with(p))
}

/// Strip one `<kiro-ide-message>…</kiro-ide-message>` envelope.
fn strip_wrapper(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("<kiro-ide-message>") else {
        return content;
    };
    rest.strip_suffix("</kiro-ide-message>")
        .unwrap_or(rest)
        .trim()
}

/// Reverse-lookup the project for an old-format chat: scan
/// workspace-sessions/*/sessions.json for a workspaceDirectory whose
/// sha256[:32] matches the chat's parent dir name.
fn project_for_ws_hash(chat_path: &Path, ws_hash: &str) -> String {
    let Some(root) = chat_path.parent().and_then(Path::parent) else {
        return "unknown".to_string();
    };
    let Ok(entries) = std::fs::read_dir(root.join("workspace-sessions")) else {
        return "unknown".to_string();
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(ws_dir) = super::first_workspace_dir(&dir.join("sessions.json")) else {
            continue;
        };
        if super::hash32(&ws_dir) == ws_hash {
            let p = project_from_cwd(&ws_dir);
            if !p.is_empty() {
                return p;
            }
        }
    }
    "unknown".to_string()
}
