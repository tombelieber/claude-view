// crates/providers/src/parsers/commandcode.rs
//
// Command Code — JSONL transcripts at
// `<root>/<slugified-cwd>/<session-id>.jsonl` (root: ~/.commandcode/projects).
// Sibling `<id>.checkpoints.jsonl` / `<id>.prompts.jsonl` files are internal
// state, excluded from discovery. A sidecar `<id>.meta.json`
// ({title, userRenamed, projectPath, cwd}) supplies the session title and a
// fallback cwd.
//
// Line shape: { sessionId, timestamp: ISO8601, role: user|assistant|tool,
// content, gitBranch, metadata{cwd|projectPath|context.cwd} }. `content` is
// a plain string OR a block array in TWO dialects (both occur in real data —
// accept both):
//   { type: "reasoning"|"thinking",      text|thinking }
//   { type: "tool-call"|"tool_use",      toolName|name, toolCallId|id, input }
//   { type: "tool-result"|"tool_result", toolCallId|tool_use_id,
//     output|content (string / {value} / array) or text|error|value fallbacks }
// role:"tool" lines are pure tool-result carriers attaching to prior
// assistant tool segments. No model names and no token usage exist anywhere
// in the format (usage.has_usage stays false).

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::Path;

pub struct CommandcodeProvider;

impl Provider for CommandcodeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Commandcode
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(projects) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for project in projects.flatten() {
            // `is_dir` follows symlinks — mirrors Go's isDirOrSymlink.
            let project_dir = project.path();
            if !project_dir.is_dir() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&project_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    continue;
                }
                let Some(id) = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(transcript_session_id)
                else {
                    continue;
                };
                let id = ProviderKind::Commandcode.session_id(id);
                let Some((mtime, size_bytes)) = stat_entry(&path) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id,
                    provider: ProviderKind::Commandcode,
                    path,
                    project_hint: None,
                    mtime,
                    size_bytes,
                });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let read = jsonl::read_values(path)?;

        // Session id: filename stem when valid (keeps lookup-by-id consistent
        // with discovery), else the first in-file sessionId, else raw stem.
        let raw_id = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(transcript_session_id)
            .map(str::to_string)
            .or_else(|| {
                read.values.iter().find_map(|v| {
                    v.get("sessionId")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
            })
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
            })
            .ok_or_else(|| anyhow::anyhow!("no Command Code session id for {}", path.display()))?;

        let mut meta =
            ForeignSessionMeta::new(ProviderKind::Commandcode, &raw_id, path.to_path_buf());
        meta.malformed_lines = read.malformed;

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        let mut ordinal = 0usize;
        for line in &read.values {
            if meta.cwd.is_none() {
                meta.cwd = line_cwd(line);
            }
            if meta.git_branch.is_none() {
                meta.git_branch = line
                    .get("gitBranch")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
            }
            let ts = line
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false));
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }

            let ex = extract_content(line.get("content"));
            match line.get("role").and_then(Value::as_str) {
                Some("user") => {
                    if ex.text.is_empty() && ex.tool_results.is_empty() {
                        continue;
                    }
                    attach_results(&mut out_blocks, &ex.tool_results);
                    if !ex.text.is_empty() {
                        if meta.first_message.is_empty() {
                            meta.first_message = preview(&ex.text, 200);
                        }
                        meta.user_message_count += 1;
                        out_blocks.push(blocks::user(
                            blocks::block_id(&raw_id, ordinal),
                            ex.text,
                            ts,
                        ));
                    }
                    meta.message_count += 1;
                    ordinal += 1;
                }
                Some("assistant") => {
                    if ex.segments.is_empty() && ex.thinking.is_none() && ex.tool_results.is_empty()
                    {
                        continue;
                    }
                    // Push first so same-line results can attach to this
                    // message's own tool calls.
                    if !ex.segments.is_empty() || ex.thinking.is_some() {
                        out_blocks.push(blocks::assistant(
                            blocks::block_id(&raw_id, ordinal),
                            ex.segments,
                            ex.thinking,
                            ts,
                        ));
                    }
                    attach_results(&mut out_blocks, &ex.tool_results);
                    meta.message_count += 1;
                    ordinal += 1;
                }
                Some("tool") => {
                    if ex.tool_results.is_empty() {
                        continue;
                    }
                    attach_results(&mut out_blocks, &ex.tool_results);
                    meta.message_count += 1;
                    ordinal += 1;
                }
                _ => {}
            }
        }

        // Sessions with zero contentful messages are non-interactive noise.
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }

        apply_sidecar_meta(path, &mut meta);
        let project = meta
            .cwd
            .as_deref()
            .map(project_from_cwd)
            .filter(|p| !p.is_empty())
            .or_else(|| {
                // Fallback: the slugified-cwd project directory name.
                path.parent()
                    .and_then(|d| d.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "commandcode".to_string());
        meta.project = project;

        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

/// Map a directory entry name to its session id: must end `.jsonl`, must NOT
/// be a `.checkpoints.jsonl` / `.prompts.jsonl` sibling, and the stem must be
/// a valid session id (alphanumeric, dash, underscore).
fn transcript_session_id(name: &str) -> Option<&str> {
    if name.ends_with(".checkpoints.jsonl") || name.ends_with(".prompts.jsonl") {
        return None;
    }
    let id = name.strip_suffix(".jsonl")?;
    is_valid_session_id(id).then_some(id)
}

fn is_valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// cwd candidates in agentsview priority order.
fn line_cwd(line: &Value) -> Option<String> {
    [
        "/metadata/cwd",
        "/metadata/projectPath",
        "/metadata/context/cwd",
        "/cwd",
    ]
    .iter()
    .find_map(|ptr| {
        line.pointer(ptr)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}

/// Sidecar `<id>.meta.json` → title + fallback cwd.
fn apply_sidecar_meta(path: &Path, meta: &mut ForeignSessionMeta) {
    let meta_path = path.with_extension("meta.json");
    let Ok(raw) = crate::util::read_to_string_capped(&meta_path) else {
        return;
    };
    let Ok(doc) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    if let Some(title) = doc
        .get("title")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
    {
        meta.title = Some(title.to_string());
        if meta.first_message.is_empty() {
            meta.first_message = preview(title, 200);
        }
    }
    if meta.cwd.is_none() {
        meta.cwd = ["cwd", "projectPath"].iter().find_map(|k| {
            doc.get(*k)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        });
    }
}

/// One pending tool result extracted from a content block.
struct ToolResultRef {
    tool_use_id: String,
    output: String,
    is_error: bool,
}

/// Normalized content of one transcript line.
#[derive(Default)]
struct Extracted {
    /// Interleaved text + tool-call segments in source order (assistant).
    segments: Vec<AssistantSegment>,
    /// Plain text joined with `\n` and trimmed (user content / emptiness).
    text: String,
    thinking: Option<String>,
    tool_results: Vec<ToolResultRef>,
}

fn extract_content(content: Option<&Value>) -> Extracted {
    let mut ex = Extracted::default();
    let mut text_parts: Vec<&str> = Vec::new();
    let mut thinking_parts: Vec<&str> = Vec::new();
    match content {
        Some(Value::String(s)) => {
            if !s.trim().is_empty() {
                ex.segments.push(blocks::text_segment(s.clone()));
                text_parts.push(s);
            }
        }
        Some(Value::Array(items)) => {
            for block in items {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(Value::as_str) {
                            if !t.trim().is_empty() {
                                ex.segments.push(blocks::text_segment(t.to_string()));
                                text_parts.push(t);
                            }
                        }
                    }
                    // Dual dialect: `reasoning` carries `text`, `thinking`
                    // carries `thinking` — accept either field on both.
                    Some("reasoning" | "thinking") => {
                        if let Some(t) = first_str(block, &["text", "thinking"]) {
                            thinking_parts.push(t);
                        }
                    }
                    Some("tool-call" | "tool_use") => {
                        let Some(name) = first_str(block, &["toolName", "name"]) else {
                            continue;
                        };
                        let id = first_str(block, &["toolCallId", "id"]).unwrap_or("");
                        ex.segments.push(blocks::tool_segment(
                            name.to_string(),
                            block
                                .get("input")
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!({})),
                            id.to_string(),
                        ));
                    }
                    Some("tool-result" | "tool_result") => {
                        let Some(id) = first_str(block, &["toolCallId", "tool_use_id"]) else {
                            continue;
                        };
                        if let Some((output, is_error)) = tool_result_output(block) {
                            ex.tool_results.push(ToolResultRef {
                                tool_use_id: id.to_string(),
                                output,
                                is_error,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    ex.text = text_parts.join("\n").trim().to_string();
    if !thinking_parts.is_empty() {
        ex.thinking = Some(thinking_parts.join("\n\n"));
    }
    ex
}

fn attach_results(out: &mut [ConversationBlock], results: &[ToolResultRef]) {
    for r in results {
        // Unmatched results (call never seen) are dropped, like amp.
        blocks::attach_tool_result(out, &r.tool_use_id, r.output.clone(), r.is_error);
    }
}

/// Tool-result payload → display text. Priority ported from agentsview's
/// commandCodeToolResultContent + DecodeContent: `output` (else `content`)
/// wins — object with string `value` unwraps, string passes through, array
/// joins its `text` fields; without either key, the first non-empty of
/// `text`/`error`/`value` string fields applies (`error` marks is_error).
fn tool_result_output(block: &Value) -> Option<(String, bool)> {
    let output = block
        .get("output")
        .or_else(|| block.get("content"))
        .filter(|v| !v.is_null());
    if let Some(v) = output {
        if let Some(s) = v.get("value").and_then(Value::as_str) {
            return Some((s.to_string(), false));
        }
        return Some((decode_value(v), false));
    }
    for key in ["text", "error", "value"] {
        if let Some(s) = block
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return Some((s.to_string(), key == "error"));
        }
    }
    None
}

fn decode_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let joined: String = items
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect();
            if joined.is_empty() {
                // No text fields → keep the raw JSON (zero data loss).
                v.to_string()
            } else {
                joined
            }
        }
        other => other.to_string(),
    }
}

fn first_str<'a>(block: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| {
        block
            .get(*k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;
    use serde_json::json;

    // Dialect A (mirrors the agentsview Go test fixture): reasoning/text,
    // tool-call/toolCallId/toolName, role:"tool" carrier, output:{value}.
    const FIXTURE_A: &str = r#"{"id":"m1","timestamp":"2026-06-01T10:00:00Z","sessionId":"sess_123","role":"user","content":[{"type":"text","text":"Inspect server logs"}],"gitBranch":"feature/command-code","metadata":{"version":2,"cwd":"/Users/alice/code/sample-project"}}
{"id":"m2","timestamp":"2026-06-01T10:00:01Z","sessionId":"sess_123","role":"assistant","content":[{"type":"reasoning","text":"I should read the logs first."},{"type":"tool-call","toolCallId":"tc1","toolName":"Read","input":{"file_path":"server.log"}}],"metadata":{"version":2}}
{"id":"m3","timestamp":"2026-06-01T10:00:02Z","sessionId":"sess_123","role":"tool","content":[{"type":"tool-result","toolCallId":"tc1","toolName":"Read","output":{"type":"text","value":"error: boom"}}],"metadata":{"version":2}}
{"id":"m4","timestamp":"2026-06-01T10:00:03Z","sessionId":"sess_123","role":"assistant","content":[{"type":"text","text":"The error is in the startup path."}],"metadata":{"version":2}}"#;

    fn parse_one(file_name: &str, content: &str, sidecar: Option<&str>) -> Vec<ForeignSession> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(file_name);
        std::fs::write(&path, content).unwrap();
        if let Some(m) = sidecar {
            std::fs::write(path.with_extension("meta.json"), m).unwrap();
        }
        CommandcodeProvider.parse(&path).unwrap()
    }

    #[test]
    fn parses_dialect_a_with_tool_carrier_and_sidecar() {
        let sessions = parse_one(
            "sess_123.jsonl",
            FIXTURE_A,
            Some(r#"{"title":"Startup investigation","userRenamed":true}"#),
        );
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.meta.id, "commandcode:sess_123");
        assert_eq!(s.meta.project, "sample-project");
        assert_eq!(
            s.meta.cwd.as_deref(),
            Some("/Users/alice/code/sample-project")
        );
        assert_eq!(s.meta.git_branch.as_deref(), Some("feature/command-code"));
        assert_eq!(s.meta.title.as_deref(), Some("Startup investigation"));
        assert_eq!(s.meta.first_message, "Inspect server logs");
        assert_eq!(
            s.meta.message_count, 4,
            "tool carrier line counts as a message"
        );
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.malformed_lines, 0);
        assert!(
            !s.meta.usage.has_usage,
            "format carries no tokens — must stay false"
        );
        assert!(s.meta.models.is_empty());
        assert_eq!(
            s.meta.started_at,
            time::parse_timestamp("2026-06-01T10:00:00Z", false)
        );
        assert_eq!(
            s.meta.ended_at,
            time::parse_timestamp("2026-06-01T10:00:03Z", false)
        );
        // user, assistant, assistant — the role:"tool" line emits no block.
        assert_eq!(s.blocks.len(), 3);

        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("I should read the logs first."));
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "Read");
        assert_eq!(execution.tool_use_id, "tc1");
        assert_eq!(execution.tool_input, json!({"file_path": "server.log"}));
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "error: boom");
    }

    #[test]
    fn parses_dialect_b_and_string_content() {
        let fixture = r#"{"timestamp":"2026-06-01T11:00:00Z","sessionId":"sess_b","role":"user","content":"plain string prompt","metadata":{"projectPath":"/w/proj"}}
{"timestamp":"2026-06-01T11:00:01Z","sessionId":"sess_b","role":"assistant","content":[{"type":"thinking","thinking":"hmm"},{"type":"tool_use","id":"tu9","name":"bash","input":{"cmd":"ls"}},{"type":"text","text":"running"}]}
{"timestamp":"2026-06-01T11:00:02Z","sessionId":"sess_b","role":"user","content":[{"type":"tool_result","tool_use_id":"tu9","output":"file1\nfile2"}]}"#;
        let sessions = parse_one("sess_b.jsonl", fixture, None);
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.meta.id, "commandcode:sess_b");
        assert_eq!(s.meta.cwd.as_deref(), Some("/w/proj"));
        assert_eq!(s.meta.project, "proj");
        assert_eq!(s.meta.first_message, "plain string prompt");
        // user, assistant, result-only user line (counted, no block).
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.blocks.len(), 2);

        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("hmm"));
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("tool segment must precede the text segment (source order)")
        };
        assert_eq!(execution.tool_name, "bash");
        assert_eq!(execution.result.as_ref().unwrap().output, "file1\nfile2");
        let AssistantSegment::Text { text, .. } = &a.segments[1] else {
            panic!("expected trailing text segment")
        };
        assert_eq!(text, "running");
    }

    #[test]
    fn tool_result_output_fallbacks() {
        let v = |j| tool_result_output(&j);
        assert_eq!(
            v(json!({"output": {"type": "text", "value": "v"}})),
            Some(("v".to_string(), false))
        );
        assert_eq!(
            v(json!({"content": "raw string"})),
            Some(("raw string".to_string(), false))
        );
        assert_eq!(
            v(json!({"output": [{"type":"text","text":"a"}, {"type":"text","text":"b"}]})),
            Some(("ab".to_string(), false))
        );
        assert_eq!(v(json!({"text": "t"})), Some(("t".to_string(), false)));
        assert_eq!(
            v(json!({"error": "boom"})),
            Some(("boom".to_string(), true)),
            "error-only results must mark is_error"
        );
        assert_eq!(v(json!({})), None);
    }

    #[test]
    fn empty_and_noise_sessions_are_skipped() {
        // Only unknown roles / contentless lines → zero messages → omitted.
        let fixture = r#"{"timestamp":"2026-06-01T10:00:00Z","sessionId":"s1","role":"system","content":"boot"}
{"timestamp":"2026-06-01T10:00:01Z","sessionId":"s1","role":"user","content":[]}
{"timestamp":"2026-06-01T10:00:02Z","sessionId":"s1","role":"assistant","content":[{"type":"text","text":"   "}]}"#;
        assert!(parse_one("s1.jsonl", fixture, None).is_empty());
        assert!(parse_one("s2.jsonl", "", None).is_empty());
    }

    #[test]
    fn malformed_lines_are_counted_not_fatal() {
        let fixture =
            "{\"sessionId\":\"s3\",\"role\":\"user\",\"content\":\"hello\"}\nnot json at all\n";
        let sessions = parse_one("s3.jsonl", fixture, None);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].meta.malformed_lines, 1);
        assert_eq!(sessions[0].meta.message_count, 1);
        assert_eq!(sessions[0].meta.first_message, "hello");
    }

    #[test]
    fn sidecar_supplies_title_cwd_and_first_message_fallback() {
        let fixture = r#"{"timestamp":"2026-06-01T12:00:00Z","sessionId":"s4","role":"assistant","content":[{"type":"text","text":"unsolicited report"}]}"#;
        let sessions = parse_one(
            "s4.jsonl",
            fixture,
            Some(r#"{"title":"My title","projectPath":"/x/y/myproj"}"#),
        );
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.meta.title.as_deref(), Some("My title"));
        assert_eq!(s.meta.first_message, "My title");
        assert_eq!(s.meta.cwd.as_deref(), Some("/x/y/myproj"));
        assert_eq!(s.meta.project, "myproj");
        assert_eq!(s.meta.user_message_count, 0);
    }

    #[test]
    fn discover_excludes_sidecars_and_internal_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("users-alice-code-sample-project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("sess_a.jsonl"), "{}\n").unwrap();
        std::fs::write(project.join("sess_a.meta.json"), "{}").unwrap();
        std::fs::write(project.join("sess_a.checkpoints.jsonl"), "{}\n").unwrap();
        std::fs::write(project.join("sess_a.prompts.jsonl"), "{}\n").unwrap();
        std::fs::write(project.join("not a session!.jsonl"), "{}\n").unwrap();
        std::fs::write(project.join("notes.txt"), "ignore").unwrap();
        // Stray transcript directly at root level: not in a project dir → skip.
        std::fs::write(dir.path().join("sess_b.jsonl"), "{}\n").unwrap();

        let found = CommandcodeProvider.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "commandcode:sess_a");
        assert_eq!(found[0].provider, ProviderKind::Commandcode);
        assert!(found[0]
            .path
            .ends_with("users-alice-code-sample-project/sess_a.jsonl"));
    }
}
