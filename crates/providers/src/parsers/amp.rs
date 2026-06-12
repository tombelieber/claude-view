// crates/providers/src/parsers/amp.rs
//
// Amp (ampcode.com) — single-JSON thread files at
// `<root>/threads/T-*.json` (or `<root>/T-*.json` when the env override
// points directly at the threads dir).
//
// Format (from the agentsview census, verified against fixtures):
//   { id: "T-…", created: epoch-ms, title?,
//     env.initial.trees[0].displayName → project,
//     meta.traces[].endTime (RFC3339)  → session end,
//     messages: [{ role: user|assistant, content: [blocks] }] }
// Content blocks are Claude-style (text/thinking/tool_use) plus an
// Amp-specific tool_result keyed by camelCase `toolUseID` carrying
// `run.result` / `run.status` / `run.error.message`.
// No model name and no token usage exist anywhere in the format.

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, preview};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::Path;

pub struct AmpProvider;

impl Provider for AmpProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Amp
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let threads = root.join("threads");
        let dir = if threads.is_dir() { threads } else { root.to_path_buf() };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if path.extension().and_then(|e| e.to_str()) != Some("json")
                || !is_thread_id(stem)
            {
                continue;
            }
            let Some((mtime, size_bytes)) = stat_entry(&path) else {
                continue;
            };
            out.push(DiscoveredSession {
                id: ProviderKind::Amp.session_id(stem),
                provider: ProviderKind::Amp,
                path,
                project_hint: None,
                mtime,
                size_bytes,
            });
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let raw = crate::util::read_to_string_capped(path)?;
        let doc: Value = serde_json::from_str(&raw)?;
        // Thread id comes from the filename stem so lookup-by-id matches
        // discovery; the JSON `id` field is only a fallback.
        let raw_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| is_thread_id(s))
            .map(str::to_string)
            .or_else(|| {
                doc.get("id")
                    .and_then(Value::as_str)
                    .filter(|s| is_thread_id(s))
                    .map(str::to_string)
            })
            .ok_or_else(|| anyhow::anyhow!("no valid Amp thread id"))?;

        let mut meta = ForeignSessionMeta::new(ProviderKind::Amp, &raw_id, path.to_path_buf());
        meta.title = doc
            .get("title")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string);
        meta.project = doc
            .pointer("/env/initial/trees/0/displayName")
            .and_then(Value::as_str)
            .unwrap_or("amp")
            .to_string();
        if let Some(created_ms) = doc.get("created").and_then(Value::as_f64) {
            meta.observe_timestamp(crate::util::time::from_millis(created_ms));
        }
        // End time = last trace with a non-empty endTime, scanned backwards.
        if let Some(traces) = doc.pointer("/meta/traces").and_then(Value::as_array) {
            for trace in traces.iter().rev() {
                let Some(end) = trace.get("endTime").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(ts) = crate::util::time::parse_timestamp(end, false) {
                    meta.observe_timestamp(ts);
                    break;
                }
            }
        }

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        let messages = doc
            .get("messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (ordinal, msg) in messages.iter().enumerate() {
            let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
            let content = msg.get("content").and_then(Value::as_array);
            let id = blocks::block_id(&raw_id, ordinal);
            match role {
                "user" => handle_user(&mut out_blocks, &mut meta, id, content),
                "assistant" => handle_assistant(&mut out_blocks, &mut meta, id, content),
                _ => {}
            }
        }

        // Empty threads (no contentful messages) are non-interactive noise.
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

fn is_thread_id(s: &str) -> bool {
    s.strip_prefix("T-")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

fn handle_user(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    content: Option<&Vec<Value>>,
) {
    let mut text_parts: Vec<String> = Vec::new();
    for block in content.into_iter().flatten() {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    if !t.trim().is_empty() {
                        text_parts.push(t.to_string());
                    }
                }
            }
            Some("tool_result") => {
                // Amp variant: camelCase toolUseID + run payload.
                let tool_use_id = block
                    .get("toolUseID")
                    .or_else(|| block.get("tool_use_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if tool_use_id.is_empty() {
                    continue;
                }
                if let Some((output, is_error)) = serialize_result(block.get("run")) {
                    blocks::attach_tool_result(out, tool_use_id, output, is_error);
                }
            }
            _ => {}
        }
    }
    if text_parts.is_empty() {
        return;
    }
    let text = text_parts.join("\n");
    if meta.first_message.is_empty() {
        meta.first_message = preview(&text, 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    out.push(blocks::user(id, text, None));
}

fn handle_assistant(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    content: Option<&Vec<Value>>,
) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut thinking_parts: Vec<String> = Vec::new();
    for block in content.into_iter().flatten() {
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
                let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
                let tool_id = block.get("id").and_then(Value::as_str).unwrap_or("");
                if tool_id.is_empty() {
                    continue;
                }
                segments.push(blocks::tool_segment(
                    name.to_string(),
                    block.get("input").cloned().unwrap_or(Value::Null),
                    tool_id.to_string(),
                ));
            }
            _ => {}
        }
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
    out.push(blocks::assistant(id, segments, thinking, None));
}

/// Serialize an Amp `run` payload into display text. Priority order ported
/// from agentsview's serializeAmpResult; blocks with neither result nor
/// status are dropped (returns None).
fn serialize_result(run: Option<&Value>) -> Option<(String, bool)> {
    let run = run?;
    let status = run.get("status").and_then(Value::as_str).unwrap_or("");
    if status == "error" {
        let msg = run
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("error")
            .to_string();
        return Some((msg, true));
    }
    if status == "cancelled" {
        return Some(("[cancelled]".to_string(), false));
    }
    if let Some(result) = run.get("result") {
        return Some((serialize_value(result), false));
    }
    if status.is_empty() {
        return None;
    }
    Some((String::new(), false))
}

fn serialize_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(obj) => {
            for key in ["output", "content", "diff"] {
                if let Some(field) = obj.get(key) {
                    if let Some(s) = field.as_str() {
                        return s.to_string();
                    }
                    // Known field present but not a plain string → suppress
                    // raw JSON noise.
                    return String::new();
                }
            }
            match obj.get("success").and_then(Value::as_bool) {
                Some(true) => "success".to_string(),
                Some(false) => "failed".to_string(),
                None => v.to_string(),
            }
        }
        Value::Array(items) => {
            if items.iter().all(Value::is_string) {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n")
            } else if items
                .first()
                .and_then(|f| f.get("type"))
                .and_then(Value::as_str)
                == Some("image")
            {
                "[binary content]".to_string()
            } else {
                v.to_string()
            }
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;

    const FIXTURE: &str = r#"{
      "id": "T-abc123",
      "created": 1767323045000,
      "title": "Fix the login bug",
      "env": { "initial": { "trees": [ { "displayName": "my-app" } ] } },
      "meta": { "traces": [
        { "endTime": "" },
        { "endTime": "2026-01-02T04:00:00Z" }
      ] },
      "messages": [
        { "role": "user", "content": [ { "type": "text", "text": "fix the login bug" } ] },
        { "role": "assistant", "content": [
          { "type": "thinking", "thinking": "look at auth first" },
          { "type": "text", "text": "Let me check." },
          { "type": "tool_use", "id": "tu_1", "name": "Read", "input": { "path": "auth.ts" } }
        ] },
        { "role": "user", "content": [
          { "type": "tool_result", "toolUseID": "tu_1",
            "run": { "status": "done", "result": { "output": "file contents here" } } }
        ] },
        { "role": "assistant", "content": [ { "type": "text", "text": "Fixed." } ] }
      ]
    }"#;

    fn parse_fixture() -> ForeignSession {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("T-abc123.json");
        std::fs::write(&path, FIXTURE).unwrap();
        let mut sessions = AmpProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        sessions.remove(0)
    }

    #[test]
    fn parses_thread_into_blocks() {
        let s = parse_fixture();
        assert_eq!(s.meta.id, "amp:T-abc123");
        assert_eq!(s.meta.project, "my-app");
        assert_eq!(s.meta.title.as_deref(), Some("Fix the login bug"));
        assert_eq!(s.meta.first_message, "fix the login bug");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.message_count, 3);
        assert!(!s.meta.usage.has_usage, "amp carries no usage — must stay false");
        // started from created-ms, ended from last trace endTime.
        assert_eq!(s.meta.started_at, Some(1767323045.0));
        assert_eq!(s.meta.ended_at, Some(1767326400.0));
        assert_eq!(s.blocks.len(), 3);
    }

    #[test]
    fn tool_result_attaches_to_call() {
        let s = parse_fixture();
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("look at auth first"));
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "Read");
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "file contents here");
    }

    #[test]
    fn empty_threads_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("T-empty.json");
        std::fs::write(&path, r#"{"id":"T-empty","messages":[]}"#).unwrap();
        assert!(AmpProvider.parse(&path).unwrap().is_empty());
    }

    #[test]
    fn discover_finds_threads_dir() {
        let dir = tempfile::tempdir().unwrap();
        let threads = dir.path().join("threads");
        std::fs::create_dir_all(&threads).unwrap();
        std::fs::write(threads.join("T-x1.json"), FIXTURE).unwrap();
        std::fs::write(threads.join("notes.json"), "{}").unwrap();
        std::fs::write(threads.join("T-x2.txt"), "nope").unwrap();
        let found = AmpProvider.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "amp:T-x1");
    }

    #[test]
    fn error_and_cancelled_results() {
        let err = serialize_result(Some(&serde_json::json!({
            "status": "error", "error": { "message": "boom" }
        })));
        assert_eq!(err, Some(("boom".to_string(), true)));
        let cancelled = serialize_result(Some(&serde_json::json!({ "status": "cancelled" })));
        assert_eq!(cancelled, Some(("[cancelled]".to_string(), false)));
        assert_eq!(serialize_result(Some(&serde_json::json!({}))), None);
    }
}
