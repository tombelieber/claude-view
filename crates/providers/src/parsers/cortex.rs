// crates/providers/src/parsers/cortex.rs
//
// Cortex Code (Snowflake) — single-JSON session files at
// `<root>/<uuid>.json` (root = ~/.snowflake/cortex/conversations).
//
// Format (ported from agentsview's cortex.go):
//   { session_id, title, history: [msg], connection_name,
//     working_directory, git_root, git_branch, created_at, last_updated }
//   msg    = { role, id, content: [block], user_sent_time }
//   block  = { type: text|tool_use|tool_result, text, internalOnly,
//              is_user_prompt, tool_use: {tool_use_id, name, input},
//              tool_result: {name, tool_use_id, content: [block], status} }
// Tool payloads are NESTED under a key matching the block type — not flat
// like Anthropic's shape.
//
// Split-file variant: when the .json carries no `history`, the conversation
// lives in a companion `<uuid>.history.jsonl` (one msg per line); a missing
// sidecar means an empty session, not an error. Backup files
// `<uuid>.back.<timestamp>.json` are skipped at discovery. Filters:
// internalOnly blocks, `<system-reminder>` text, and the entire first user
// turn unless it has real user text. No model ids and no token usage exist
// anywhere in the format.

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct CortexProvider;

impl Provider for CortexProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Cortex
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(entries) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(stem) = session_file_stem(name) else {
                continue;
            };
            let Some((mtime, size_bytes)) = stat_entry(&path) else {
                continue;
            };
            out.push(DiscoveredSession {
                id: ProviderKind::Cortex.session_id(stem),
                provider: ProviderKind::Cortex,
                path,
                project_hint: None,
                mtime,
                size_bytes,
            });
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let raw = std::fs::read_to_string(path)?;
        let doc: Value = serde_json::from_str(&raw)?;
        let raw_id = doc
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if raw_id.is_empty() {
            // No session id at all — non-session noise, skip (Go parity).
            return Ok(Vec::new());
        }

        let mut meta = ForeignSessionMeta::new(ProviderKind::Cortex, &raw_id, path.to_path_buf());
        meta.title = doc
            .get("title")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty() && !t.starts_with("Chat for session:"))
            .map(str::to_string);
        meta.cwd = non_empty_str(&doc, "working_directory");
        meta.git_branch = non_empty_str(&doc, "git_branch");
        let project_cwd = meta.cwd.clone().or_else(|| non_empty_str(&doc, "git_root"));
        meta.project = match project_cwd.as_deref().map(project_from_cwd) {
            Some(p) if !p.is_empty() => p,
            _ => "unknown".to_string(),
        };
        for key in ["created_at", "last_updated"] {
            if let Some(ts) = doc
                .get(key)
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false))
            {
                meta.observe_timestamp(ts);
            }
        }

        // History source: embedded array wins; otherwise the sidecar JSONL.
        let mut history: Vec<Value> = doc
            .get("history")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if history.is_empty() {
            match jsonl::read_values(&sidecar_path(path)) {
                Ok(read) => {
                    meta.malformed_lines = read.malformed;
                    history = read
                        .values
                        .into_iter()
                        .filter(|v| {
                            v.get("role")
                                .and_then(Value::as_str)
                                .is_some_and(|r| !r.is_empty())
                        })
                        .collect();
                }
                // Missing sidecar = empty conversation, not an error.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                Err(e) => return Err(e.into()),
            }
        }

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        for (ordinal, msg) in history.iter().enumerate() {
            let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
            if role != "user" && role != "assistant" {
                continue;
            }
            let content = msg.get("content").and_then(Value::as_array);
            // The first user turn is injected system-reminder boilerplate
            // unless it carries real user text — drop it wholesale.
            if role == "user" && ordinal == 0 && !has_real_user_text(content) {
                continue;
            }
            let ts = msg
                .get("user_sent_time")
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false));
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }
            let id = blocks::block_id(&raw_id, ordinal);
            if role == "user" {
                handle_user(&mut out_blocks, &mut meta, id, content, ts);
            } else {
                handle_assistant(&mut out_blocks, &mut meta, id, content, ts);
            }
        }

        // Sessions with zero contentful messages are non-interactive noise.
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

/// Companion history file: `<path minus .json>.history.jsonl` (Go parity).
fn sidecar_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let base = s.strip_suffix(".json").unwrap_or(s.as_ref());
    PathBuf::from(format!("{base}.history.jsonl"))
}

/// Primary session metadata file: `<id>.json` where the stem is
/// alphanumeric/dash/underscore (Go IsCortexSessionFile). Backup files
/// (`<uuid>.back.<ts>.json`) are excluded.
fn session_file_stem(name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".json")?;
    if stem.is_empty() || is_backup_file(name) {
        return None;
    }
    stem.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        .then_some(stem)
}

/// Matches Go's cortexBackupRe: a lowercase-hex uuid followed by `.back.`.
fn is_backup_file(name: &str) -> bool {
    if name.len() < 42 || !name.is_char_boundary(36) {
        return false;
    }
    let (uuid, rest) = name.split_at(36);
    rest.starts_with(".back.") && is_lower_uuid(uuid)
}

fn is_lower_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => matches!(c, '0'..='9' | 'a'..='f'),
        })
}

fn non_empty_str(doc: &Value, key: &str) -> Option<String> {
    doc.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// internalOnly blocks and `<system-reminder>` text are injected context,
/// never user/assistant speech.
fn is_internal_block(block: &Value) -> bool {
    if block.get("internalOnly").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    block
        .get("text")
        .and_then(Value::as_str)
        .is_some_and(|t| t.contains("<system-reminder>"))
}

fn has_real_user_text(content: Option<&Vec<Value>>) -> bool {
    content.into_iter().flatten().any(|b| {
        b.get("type").and_then(Value::as_str) == Some("text")
            && !is_internal_block(b)
            && b.get("text")
                .and_then(Value::as_str)
                .is_some_and(|t| !t.trim().is_empty())
    })
}

fn handle_user(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    content: Option<&Vec<Value>>,
    ts: Option<f64>,
) {
    let mut text_parts: Vec<String> = Vec::new();
    for block in content.into_iter().flatten() {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if is_internal_block(block) {
                    continue;
                }
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    let t = t.trim();
                    if !t.is_empty() {
                        text_parts.push(t.to_string());
                    }
                }
            }
            Some("tool_result") => attach_result(out, block),
            _ => {}
        }
    }
    if text_parts.is_empty() {
        // Tool-result-only turns are responses to prior calls, not messages.
        return;
    }
    let text = text_parts.join("\n");
    if meta.first_message.is_empty() {
        meta.first_message = preview(&text, 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    out.push(blocks::user(id, text, ts));
}

fn handle_assistant(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    content: Option<&Vec<Value>>,
    ts: Option<f64>,
) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    for block in content.into_iter().flatten() {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if is_internal_block(block) {
                    continue;
                }
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    let t = t.trim();
                    if !t.is_empty() {
                        segments.push(blocks::text_segment(t.to_string()));
                    }
                }
            }
            Some("tool_use") => {
                // Payload is nested under `tool_use`, not flat on the block.
                let Some(tu) = block.get("tool_use") else {
                    continue;
                };
                let name = tu.get("name").and_then(Value::as_str).unwrap_or("tool");
                let tool_id = tu.get("tool_use_id").and_then(Value::as_str).unwrap_or("");
                if tool_id.is_empty() {
                    continue;
                }
                segments.push(blocks::tool_segment(
                    name.to_string(),
                    tu.get("input").cloned().unwrap_or(Value::Null),
                    tool_id.to_string(),
                ));
            }
            _ => {}
        }
    }
    if !segments.is_empty() {
        meta.message_count += 1;
        out.push(blocks::assistant(id, segments, None, ts));
    }
    // Results can reference calls in this same message — attach after push.
    for block in content.into_iter().flatten() {
        if block.get("type").and_then(Value::as_str) == Some("tool_result") {
            attach_result(out, block);
        }
    }
}

/// Attach a nested tool_result payload to its matching tool call.
fn attach_result(out: &mut [ConversationBlock], block: &Value) {
    let Some(tr) = block.get("tool_result") else {
        return;
    };
    let tool_use_id = tr.get("tool_use_id").and_then(Value::as_str).unwrap_or("");
    if tool_use_id.is_empty() {
        return;
    }
    let is_error = tr.get("status").and_then(Value::as_str) == Some("error");
    blocks::attach_tool_result(out, tool_use_id, result_text(tr.get("content")), is_error);
}

/// Display text for a tool_result content array (nested cortex blocks):
/// text blocks joined by newlines; non-text payloads fall back to the raw
/// JSON serialization (Go stores the raw marshal — never fabricate).
fn result_text(content: Option<&Value>) -> String {
    let Some(items) = content.and_then(Value::as_array) else {
        return String::new();
    };
    let texts: Vec<&str> = items
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .filter(|t| !t.trim().is_empty())
        .collect();
    if !texts.is_empty() {
        return texts.join("\n");
    }
    if items.is_empty() {
        return String::new();
    }
    serde_json::to_string(items).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;

    const UUID: &str = "11111111-2222-3333-4444-555555555555";

    fn fixture() -> String {
        format!(
            r#"{{
              "session_id": "{UUID}",
              "title": "Fix the schema",
              "connection_name": "prod",
              "working_directory": "/home/user/my-project",
              "git_root": "/home/user/my-project",
              "git_branch": "main",
              "created_at": "2024-06-01T10:00:00Z",
              "last_updated": "2024-06-01T10:05:00Z",
              "history": [
                {{
                  "role": "user", "id": "m0",
                  "content": [
                    {{"type": "text", "text": "<system-reminder>injected env context</system-reminder>"}},
                    {{"type": "text", "text": "ignore me", "internalOnly": true}}
                  ]
                }},
                {{
                  "role": "user", "id": "m1",
                  "user_sent_time": "2024-06-01T10:01:00Z",
                  "content": [{{"type": "text", "text": "Read main.go please"}}]
                }},
                {{
                  "role": "assistant", "id": "m2",
                  "content": [
                    {{"type": "text", "text": "Sure.", "internalOnly": false}},
                    {{"type": "text", "text": "secret scratch", "internalOnly": true}},
                    {{"type": "tool_use", "tool_use": {{
                      "tool_use_id": "tu1", "name": "read",
                      "input": {{"file_path": "/tmp/main.go"}}
                    }}}}
                  ]
                }},
                {{
                  "role": "user", "id": "m3",
                  "content": [{{"type": "tool_result", "tool_result": {{
                    "name": "read", "tool_use_id": "tu1",
                    "content": [{{"type": "text", "text": "package main"}}],
                    "status": "success"
                  }}}}]
                }},
                {{
                  "role": "assistant", "id": "m4",
                  "content": [{{"type": "text", "text": "Done."}}]
                }}
              ]
            }}"#
        )
    }

    fn parse_at(dir: &Path, name: &str, body: &str) -> Vec<ForeignSession> {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        CortexProvider.parse(&path).unwrap()
    }

    fn parse_fixture() -> ForeignSession {
        let dir = tempfile::tempdir().unwrap();
        let mut sessions = parse_at(dir.path(), &format!("{UUID}.json"), &fixture());
        assert_eq!(sessions.len(), 1);
        sessions.remove(0)
    }

    #[test]
    fn parses_embedded_history() {
        let s = parse_fixture();
        assert_eq!(s.meta.id, format!("cortex:{UUID}"));
        assert_eq!(s.meta.project, "my-project");
        assert_eq!(s.meta.cwd.as_deref(), Some("/home/user/my-project"));
        assert_eq!(s.meta.git_branch.as_deref(), Some("main"));
        assert_eq!(s.meta.title.as_deref(), Some("Fix the schema"));
        assert_eq!(s.meta.first_message, "Read main.go please");
        // First user turn (system reminders only) dropped; tool-result-only
        // user turn attaches but does not count.
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.blocks.len(), 3);
        assert!(!s.meta.usage.has_usage, "cortex has no token accounting");
        assert!(s.meta.models.is_empty());
        assert_eq!(s.meta.started_at, Some(1717236000.0));
        assert_eq!(s.meta.ended_at, Some(1717236300.0));
        // user_sent_time flows onto the user block.
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block")
        };
        assert_eq!(u.text, "Read main.go please");
        assert_eq!(u.timestamp, 1717236060.0);
    }

    #[test]
    fn nested_tool_use_and_result_attach() {
        let s = parse_fixture();
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        // internalOnly text filtered, real text + tool segment kept.
        assert_eq!(a.segments.len(), 2);
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "read");
        assert_eq!(execution.tool_use_id, "tu1");
        assert_eq!(execution.tool_input["file_path"], "/tmp/main.go");
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "package main");
        assert!(!execution.result.as_ref().unwrap().is_error);
    }

    #[test]
    fn split_history_sidecar_and_missing_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let meta = format!(
            r#"{{"session_id": "{UUID}", "title": "Chat for session: {UUID}",
                "working_directory": "/tmp/proj",
                "created_at": "2024-06-01T10:00:00Z",
                "last_updated": "2024-06-01T10:05:00Z"}}"#
        );
        // No sidecar yet → empty conversation, not an error.
        assert!(parse_at(dir.path(), &format!("{UUID}.json"), &meta).is_empty());

        // Sidecar with one malformed line: parsed tolerantly, counted.
        let lines = concat!(
            r#"{"role":"user","id":"m1","content":[{"type":"text","text":"Hello from JSONL"}]}"#,
            "\n",
            "not json\n",
            r#"{"role":"assistant","id":"m2","content":[{"type":"text","text":"Got it"}]}"#,
            "\n",
        );
        std::fs::write(dir.path().join(format!("{UUID}.history.jsonl")), lines).unwrap();
        let sessions = CortexProvider
            .parse(&dir.path().join(format!("{UUID}.json")))
            .unwrap();
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.first_message, "Hello from JSONL");
        // Auto-generated "Chat for session:" titles are suppressed.
        assert!(s.meta.title.is_none());
        // Discovery fingerprint = the .json metadata file, not the sidecar.
        assert!(s.meta.source_path.to_string_lossy().ends_with(".json"));
    }

    #[test]
    fn skips_empty_and_idless_sessions() {
        let dir = tempfile::tempdir().unwrap();
        // Empty session_id → skip.
        assert!(parse_at(dir.path(), "a.json", r#"{"session_id":"","history":[]}"#).is_empty());
        // Only internal first-turn content → zero messages → skip.
        let only_internal = format!(
            r#"{{"session_id": "{UUID}", "history": [
                {{"role": "user", "id": "m0",
                  "content": [{{"type": "text", "text": "<system-reminder>x</system-reminder>"}}]}}
            ]}}"#
        );
        assert!(parse_at(dir.path(), &format!("{UUID}.json"), &only_internal).is_empty());
    }

    #[test]
    fn discover_skips_backups_and_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(format!("{UUID}.json")), fixture()).unwrap();
        std::fs::write(
            dir.path().join(format!("{UUID}.back.1717236000.json")),
            "{}",
        )
        .unwrap();
        std::fs::write(dir.path().join(format!("{UUID}.history.jsonl")), "").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        let found = CortexProvider.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, format!("cortex:{UUID}"));
        assert!(found[0]
            .path
            .to_string_lossy()
            .ends_with(&format!("{UUID}.json")));
    }

    #[test]
    fn backup_matcher_is_exact() {
        assert!(is_backup_file(&format!("{UUID}.back.1717236000.json")));
        assert!(!is_backup_file(&format!("{UUID}.json")));
        assert!(!is_backup_file(
            "UPPERCASE-2222-3333-4444-555555555555.back.1.json"
        ));
        assert!(!is_backup_file("short.back.json"));
        // Non-uuid stems with ".back." are not backups (Go regex parity).
        assert!(!is_backup_file("notes.back.1.json"));
    }
}
