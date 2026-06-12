// crates/providers/src/parsers/zencoder.rs
//
// Zencoder — JSONL session files at `<root>/*.jsonl`
// (default root `~/.zencoder/sessions`).
//
// Header-line schema (ported from agentsview's zencoder.go):
//   line 1: {id, parentId, creationReason, createdAt, updatedAt}
//   lines 2+ keyed by `role`:
//     system     — content is a PLAIN STRING (environment banner); cwd is
//                  scraped from "Working directory: <path>"
//     user       — content[] of {type:text, text, tag?} | {type:skill, name,
//                  content}. tag ""/"user-input" = real user input; any other
//                  tag (instructions / system-reminder / todo-reminder) is
//                  agent-injected context → separate system-info line
//     assistant  — content[] of text | reasoning (→ thinking) |
//                  tool-call {toolCallId, toolName, input}
//     tool       — content[] of tool-result {toolCallId, content[], isError};
//                  tagged text sub-blocks are reminders → system-info line,
//                  untagged `text` fields = the tool output
//     finish     — {reason} → "[Turn finished: …]" info line
//     permission — skipped entirely
// No model ids and no token usage exist anywhere in the format, so
// `usage.has_usage` stays false (Trust over Accuracy: show nothing).
//
// Not ported (no corresponding ForeignSessionMeta fields): the
// parentId/creationReason continuation relationship and the
// `<session-id>` subagent linkage scraped from tool results.

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::Path;

pub struct ZencoderProvider;

impl Provider for ZencoderProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Zencoder
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(entries) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_type().is_ok_and(|t| t.is_file())
                || path.extension().and_then(|e| e.to_str()) != Some("jsonl")
            {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some((mtime, size_bytes)) = stat_entry(&path) else {
                continue;
            };
            out.push(DiscoveredSession {
                id: ProviderKind::Zencoder.session_id(stem),
                provider: ProviderKind::Zencoder,
                path,
                project_hint: None,
                mtime,
                size_bytes,
            });
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let read = jsonl::read_values(path)?;
        let values = read.values;
        // Line 1 is a session header (no `role` field); every message line
        // carries one. A malformed/missing header degrades to "no header"
        // exactly like the Go parser.
        let (header, messages) = match values.first() {
            Some(first) if first.get("role").and_then(Value::as_str).is_none() => {
                (Some(first), &values[1..])
            }
            _ => (None, &values[..]),
        };
        let raw_id = header
            .and_then(|h| h.get("id").and_then(Value::as_str))
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
            })
            .ok_or_else(|| anyhow::anyhow!("no zencoder session id for {}", path.display()))?;

        let mut b = SessionBuilder::new(raw_id, path);
        b.meta.malformed_lines = read.malformed;
        if let Some(h) = header {
            // parentId/creationReason exist here too — intentionally not
            // surfaced (no relationship model for foreign sessions yet).
            for key in ["createdAt", "updatedAt"] {
                if let Some(ts) = h
                    .get(key)
                    .and_then(Value::as_str)
                    .and_then(|s| time::parse_timestamp(s, false))
                {
                    b.meta.observe_timestamp(ts);
                }
            }
        }
        for msg in messages {
            b.message(msg);
        }

        // Sessions whose only content is system/environment noise (or
        // nothing at all) are not real conversations.
        if b.meta.message_count == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForeignSession {
            meta: b.meta,
            blocks: b.blocks,
        }])
    }
}

/// Accumulates blocks + metadata while scanning message lines in order.
struct SessionBuilder {
    meta: ForeignSessionMeta,
    blocks: Vec<ConversationBlock>,
    raw_id: String,
    ordinal: usize,
}

impl SessionBuilder {
    fn new(raw_id: String, path: &Path) -> Self {
        let mut meta = ForeignSessionMeta::new(ProviderKind::Zencoder, &raw_id, path.to_path_buf());
        meta.project = "zencoder".to_string();
        Self {
            meta,
            blocks: Vec::new(),
            raw_id,
            ordinal: 0,
        }
    }

    fn next_id(&mut self) -> String {
        let id = blocks::block_id(&self.raw_id, self.ordinal);
        self.ordinal += 1;
        id
    }

    fn message(&mut self, msg: &Value) {
        let ts = msg
            .get("createdAt")
            .and_then(Value::as_str)
            .and_then(|s| time::parse_timestamp(s, false));
        if let Some(t) = ts {
            // Per-message timestamps widen the envelope past stale or
            // missing header timestamps.
            self.meta.observe_timestamp(t);
        }
        match msg.get("role").and_then(Value::as_str) {
            Some("system") => self.system(msg),
            Some("user") => self.user(msg, ts),
            Some("assistant") => self.assistant(msg, ts),
            Some("tool") => self.tool(msg),
            Some("finish") => self.finish(msg),
            // "permission" and unknown roles: non-conversational, skipped.
            _ => {}
        }
    }

    /// System role: content is a plain string. Scrape cwd/project, then keep
    /// the banner as a low-emphasis info line.
    fn system(&mut self, msg: &Value) {
        let Some(content) = msg
            .get("content")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
        else {
            return;
        };
        if let Some(cwd) = extract_cwd(content) {
            let project = project_from_cwd(&cwd);
            if !project.is_empty() {
                self.meta.project = project;
            }
            if self.meta.cwd.is_none() {
                self.meta.cwd = Some(cwd);
            }
        }
        let id = self.next_id();
        self.blocks
            .push(blocks::system_info(id, content.to_string()));
    }

    /// User role: text blocks split by tag — ""/"user-input" is the human;
    /// everything else (instructions, reminders, skills) is agent-injected
    /// and rendered as a separate system-info line, never merged.
    fn user(&mut self, msg: &Value, ts: Option<f64>) {
        let Some(content) = msg.get("content").and_then(Value::as_array) else {
            return;
        };
        let mut user_parts: Vec<&str> = Vec::new();
        let mut system_parts: Vec<String> = Vec::new();
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    let Some(text) = non_empty_str(block, "text") else {
                        continue;
                    };
                    let tag = block.get("tag").and_then(Value::as_str).unwrap_or("");
                    if tag.is_empty() || tag == "user-input" {
                        user_parts.push(text);
                    } else {
                        system_parts.push(text.to_string());
                    }
                }
                Some("skill") => {
                    let Some(name) = non_empty_str(block, "name") else {
                        continue;
                    };
                    let body = block.get("content").and_then(Value::as_str).unwrap_or("");
                    system_parts.push(format!("[Skill: {name}]\n{body}\n[/Skill]"));
                }
                _ => {}
            }
        }
        let user_text = user_parts.join("\n");
        if !user_text.trim().is_empty() {
            if self.meta.first_message.is_empty() {
                self.meta.first_message = preview(&user_text, 200);
            }
            self.meta.message_count += 1;
            self.meta.user_message_count += 1;
            let id = self.next_id();
            self.blocks.push(blocks::user(id, user_text, ts));
        }
        let system_text = system_parts.join("\n");
        if !system_text.trim().is_empty() {
            let id = self.next_id();
            self.blocks.push(blocks::system_info(id, system_text));
        }
    }

    /// Assistant role: text → segments, reasoning → thinking, tool-call →
    /// tool segment (results attach later from `role:"tool"` lines).
    fn assistant(&mut self, msg: &Value, ts: Option<f64>) {
        let Some(content) = msg.get("content").and_then(Value::as_array) else {
            return;
        };
        let mut segments: Vec<AssistantSegment> = Vec::new();
        let mut thinking_parts: Vec<&str> = Vec::new();
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = non_empty_str(block, "text") {
                        segments.push(blocks::text_segment(text.to_string()));
                    }
                }
                Some("reasoning") => {
                    if let Some(text) = non_empty_str(block, "text") {
                        thinking_parts.push(text);
                    }
                }
                Some("tool-call") => {
                    let Some(name) = non_empty_str(block, "toolName") else {
                        continue;
                    };
                    let tool_id = block
                        .get("toolCallId")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    segments.push(blocks::tool_segment(
                        name.to_string(),
                        block.get("input").cloned().unwrap_or(Value::Null),
                        tool_id,
                    ));
                }
                _ => {}
            }
        }
        if segments.is_empty() && thinking_parts.is_empty() {
            return;
        }
        self.meta.message_count += 1;
        let thinking = (!thinking_parts.is_empty()).then(|| thinking_parts.join("\n\n"));
        let id = self.next_id();
        self.blocks
            .push(blocks::assistant(id, segments, thinking, ts));
    }

    /// Tool role: attach each tool-result to its call. Tagged text
    /// sub-blocks are system reminders riding inside the result — they go to
    /// a separate info line so the tool output stays truthful.
    fn tool(&mut self, msg: &Value) {
        let Some(content) = msg.get("content").and_then(Value::as_array) else {
            return;
        };
        let mut system_parts: Vec<String> = Vec::new();
        for block in content {
            if block.get("type").and_then(Value::as_str) != Some("tool-result") {
                continue;
            }
            let Some(tool_call_id) = non_empty_str(block, "toolCallId") else {
                continue;
            };
            let is_error = block
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut output_parts: Vec<&str> = Vec::new();
            for cb in block
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let text = cb.get("text").and_then(Value::as_str).unwrap_or("");
                let tagged = cb.get("type").and_then(Value::as_str) == Some("text")
                    && non_empty_str(cb, "tag").is_some();
                if tagged {
                    if !text.is_empty() {
                        system_parts.push(text.to_string());
                    }
                } else if !text.is_empty() {
                    // text / shell-result / text-file-chunk all carry `text`.
                    output_parts.push(text);
                }
            }
            blocks::attach_tool_result(
                &mut self.blocks,
                tool_call_id,
                output_parts.join("\n"),
                is_error,
            );
        }
        if !system_parts.is_empty() {
            let id = self.next_id();
            self.blocks
                .push(blocks::system_info(id, system_parts.join("\n")));
        }
    }

    fn finish(&mut self, msg: &Value) {
        let reason = msg
            .get("reason")
            .and_then(Value::as_str)
            .filter(|r| !r.is_empty())
            .unwrap_or("unknown");
        let text = format!("[Turn finished: {reason}]");
        let id = self.next_id();
        self.blocks.push(blocks::system_info(id, text));
    }
}

fn non_empty_str<'a>(block: &'a Value, key: &str) -> Option<&'a str> {
    block
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// Port of the Go scrape `Working directory:\s+(.+)` — fixed label, at least
/// one whitespace char, then the rest of that line, trimmed.
fn extract_cwd(content: &str) -> Option<String> {
    const LABEL: &str = "Working directory:";
    let start = content.find(LABEL)? + LABEL.len();
    let rest = &content[start..];
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let line = rest.trim_start().lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::{SystemVariant, ToolStatus};

    const FIXTURE: &str = r#"{"id":"abc-123","parentId":"","creationReason":"newChat","createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:01:00Z"}
{"role":"system","content":"You are an AI assistant.\n\n# Environment\n\nWorking directory: /home/user/myproject\n\nOS: linux"}
{"role":"user","content":[{"type":"text","text":"Fix the bug.","tag":"user-input"}],"createdAt":"2024-01-01T00:00:02Z"}
{"role":"assistant","content":[{"type":"reasoning","text":"Let me think.","provider":"anthropic","subtype":"thinking"},{"type":"text","text":"I will read it now."},{"type":"tool-call","toolCallId":"tc1","toolName":"Read","input":{"file_path":"main.go"}}],"createdAt":"2024-01-01T00:10:00Z"}
{"role":"tool","content":[{"type":"tool-result","toolCallId":"tc1","toolName":"Read","content":[{"type":"text","text":"package main"}],"isError":false}]}
{"role":"permission","data":{"allowed":true}}
{"role":"finish","reason":"endTurn"}
"#;

    fn parse_str(name: &str, content: &str) -> Vec<ForeignSession> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        ZencoderProvider.parse(&path).unwrap()
    }

    fn parse_fixture() -> ForeignSession {
        let mut sessions = parse_str("abc-123.jsonl", FIXTURE);
        assert_eq!(sessions.len(), 1);
        sessions.remove(0)
    }

    fn system_content(block: &ConversationBlock) -> &str {
        let ConversationBlock::System(sys) = block else {
            panic!("expected system block, got {block:?}")
        };
        assert_eq!(sys.variant, SystemVariant::Informational);
        sys.data["content"].as_str().unwrap()
    }

    #[test]
    fn parses_session_into_blocks() {
        let s = parse_fixture();
        assert_eq!(s.meta.id, "zencoder:abc-123");
        assert_eq!(s.meta.project, "myproject");
        assert_eq!(s.meta.cwd.as_deref(), Some("/home/user/myproject"));
        assert_eq!(s.meta.first_message, "Fix the bug.");
        // Contentful only: 1 user + 1 assistant. System/finish lines render
        // but do not count.
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        assert!(
            !s.meta.usage.has_usage,
            "zencoder carries no usage — must stay false"
        );
        assert!(s.meta.models.is_empty());
        assert_eq!(s.meta.malformed_lines, 0);
        // started = header createdAt; ended = assistant createdAt, which is
        // LATER than the stale header updatedAt.
        assert_eq!(s.meta.started_at, Some(1704067200.0));
        assert_eq!(s.meta.ended_at, Some(1704067800.0));
        // system banner + user + assistant + finish (permission skipped,
        // tool line attaches to the assistant call instead).
        assert_eq!(s.blocks.len(), 4);
        assert!(system_content(&s.blocks[0]).contains("Working directory"));
        let ConversationBlock::User(u) = &s.blocks[1] else {
            panic!("expected user block")
        };
        assert_eq!(u.text, "Fix the bug.");
        assert_eq!(u.timestamp, 1704067202.0);
        assert_eq!(system_content(&s.blocks[3]), "[Turn finished: endTurn]");
    }

    #[test]
    fn reasoning_and_tool_result_attach() {
        let s = parse_fixture();
        let ConversationBlock::Assistant(a) = &s.blocks[2] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("Let me think."));
        assert_eq!(a.timestamp, Some(1704067800.0));
        assert_eq!(a.segments.len(), 2);
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "Read");
        assert_eq!(execution.tool_use_id, "tc1");
        assert_eq!(execution.tool_input["file_path"], "main.go");
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "package main");
    }

    #[test]
    fn tag_filtering_splits_user_from_injected_context() {
        let fixture = r#"{"id":"tag-123","createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:01:00Z"}
{"role":"user","content":[{"type":"text","text":"system instructions","tag":"instructions"},{"type":"text","text":"actual user input","tag":"user-input"},{"type":"text","text":"todo reminder","tag":"todo-reminder"},{"type":"skill","name":"init","content":"skill body"}]}
{"role":"assistant","content":[{"type":"text","text":"Got it."}]}
"#;
        let s = parse_str("tag-123.jsonl", fixture).remove(0);
        assert_eq!(s.meta.first_message, "actual user input");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.blocks.len(), 3);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block first")
        };
        assert_eq!(u.text, "actual user input");
        let sys = system_content(&s.blocks[1]);
        assert!(sys.contains("system instructions"));
        assert!(sys.contains("todo reminder"));
        assert!(sys.contains("[Skill: init]\nskill body\n[/Skill]"));
        assert!(!u.text.contains("system instructions"));
    }

    #[test]
    fn tool_result_reminders_split_from_output() {
        let fixture = r#"{"id":"trsys-1","createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:01:00Z"}
{"role":"user","content":[{"type":"text","text":"Run it."}]}
{"role":"assistant","content":[{"type":"tool-call","toolCallId":"tc1","toolName":"Bash","input":{"command":"ls"}}]}
{"role":"tool","content":[{"type":"tool-result","toolCallId":"tc1","content":[{"type":"shell-result","text":"file1.go\nfile2.go"},{"type":"text","tag":"system-reminder","text":"Remember your tasks"}],"isError":true}]}
"#;
        let s = parse_str("trsys-1.jsonl", fixture).remove(0);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        let result = execution.result.as_ref().unwrap();
        assert_eq!(result.output, "file1.go\nfile2.go");
        assert!(result.is_error);
        assert_eq!(execution.status, ToolStatus::Error);
        // Reminder rides as a separate info line, never in the tool output.
        assert_eq!(system_content(&s.blocks[2]), "Remember your tasks");
    }

    #[test]
    fn system_only_sessions_are_skipped() {
        let fixture = r#"{"id":"sysonly-1","createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:01:00Z"}
{"role":"system","content":"You are an AI assistant.\n\nWorking directory: /home/user/proj"}
{"role":"finish","reason":"endTurn"}
"#;
        assert!(parse_str("sysonly-1.jsonl", fixture).is_empty());
        // Header-only file is skipped too.
        assert!(parse_str("empty-1.jsonl", "{\"id\":\"empty-1\"}\n").is_empty());
    }

    #[test]
    fn malformed_lines_counted_and_id_falls_back_to_filename() {
        let fixture = r#"{"createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:01:00Z"}
this line is not json
{"role":"user","content":[{"type":"text","text":"hello"}]}
"#;
        let s = parse_str("no-id.jsonl", fixture).remove(0);
        assert_eq!(s.meta.id, "zencoder:no-id");
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.message_count, 1);
        assert_eq!(s.meta.first_message, "hello");
        // No cwd ever seen → provider-name fallback.
        assert_eq!(s.meta.project, "zencoder");
        assert_eq!(s.meta.cwd, None);
    }

    #[test]
    fn discover_finds_jsonl_files_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("abc-123.jsonl"), FIXTURE).unwrap();
        std::fs::write(dir.path().join("notes.txt"), "nope").unwrap();
        std::fs::create_dir(dir.path().join("subdir.jsonl")).unwrap();
        let found = ZencoderProvider.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "zencoder:abc-123");
        assert_eq!(found[0].provider, ProviderKind::Zencoder);
        assert!(found[0].size_bytes > 0);
    }
}
