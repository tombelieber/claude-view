// crates/providers/src/parsers/gemini.rs
//
// Gemini CLI — sessions live at
// `<root>/tmp/<sha256-hex-of-abs-project-path>/chats/session-*.json|.jsonl`
// (root defaults to ~/.gemini).
//
// Two on-disk generations, sniffed per file (ported from agentsview's
// gemini.go):
//   (a) legacy single JSON document:
//       { sessionId, startTime, lastUpdated, messages: [...] }
//   (b) JSONL stream: header records carry sessionId/startTime/lastUpdated,
//       `$set.lastUpdated` records bump the end time, and message records are
//       { id, type: "user"|"gemini", content (string OR parts[] with .text),
//         thoughts: [{subject, description}], toolCalls: [{id, name, args,
//         result: [... functionResponse.response.output]}],
//         tokens: {input, output, cached, thoughts}, model, timestamp }.
//       A LATER record with the same id REPLACES the earlier one in place
//       (streaming updates) — only the final revision is rendered/counted.
//
// Tokens are per-message: thoughts bill at the output rate (output +=
// thoughts), `cached` → cache_read_input_tokens, and `input` is taken
// verbatim (Gemini keeps input and cached separate; nothing to subtract).
//
// Project display names: SHA-256-hash every path known to
// `<root>/projects.json` and `<root>/trustedFolders.json` (Gemini CLI's own
// tmp/-dir naming scheme) and match the tmp/ dir name; unmapped dirs fall
// back to the dir name itself (dashes normalized to underscores).

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

pub struct GeminiProvider;

impl Provider for GeminiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gemini
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(hash_dirs) = std::fs::read_dir(root.join("tmp")) else {
            return Vec::new();
        };
        let project_map = build_project_map(root);
        let mut out = Vec::new();
        for hd in hash_dirs.flatten() {
            let hash_path = hd.path();
            // `is_dir` follows symlinks — matches the Go walker's
            // dir-or-symlink acceptance.
            if !hash_path.is_dir() {
                continue;
            }
            let Some(dir_name) = hash_path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(entries) = std::fs::read_dir(hash_path.join("chats")) else {
                continue;
            };
            let project = resolve_project(dir_name, &project_map);
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if path.is_dir() || !is_session_filename(name) {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let Some((mtime, size_bytes)) = stat_entry(&path) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id: ProviderKind::Gemini.session_id(stem),
                    provider: ProviderKind::Gemini,
                    path,
                    project_hint: Some(project.clone()),
                    mtime,
                    size_bytes,
                });
            }
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        // Raw id = filename stem so lookup-by-id matches discovery (the
        // in-file sessionId is validated for presence but not used as id).
        let raw_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("no file stem in {}", path.display()))?
            .to_string();
        let mut meta = ForeignSessionMeta::new(ProviderKind::Gemini, &raw_id, path.to_path_buf());
        meta.project = project_for_path(path);

        let raw = std::fs::read_to_string(path)?;
        // Sniff: a whole-file JSON document with messages[]/sessionId is the
        // legacy object format; otherwise multi-line input is JSONL.
        let doc = serde_json::from_str::<Value>(&raw).ok().filter(|d| {
            d.get("messages").is_some_and(Value::is_array) || d.get("sessionId").is_some()
        });
        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        if let Some(doc) = doc {
            parse_object(&doc, &raw_id, &mut meta, &mut out_blocks)?;
        } else if raw.contains('\n') {
            parse_jsonl(&raw, path, &raw_id, &mut meta, &mut out_blocks)?;
        } else {
            anyhow::bail!("invalid Gemini session in {}", path.display());
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

/// Legacy single-document format.
fn parse_object(
    doc: &Value,
    raw_id: &str,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) -> anyhow::Result<()> {
    if doc
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        anyhow::bail!("missing sessionId in {}", meta.source_path.display());
    }
    observe_str_timestamp(meta, doc.get("startTime"));
    observe_str_timestamp(meta, doc.get("lastUpdated"));
    for msg in doc
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        append_block(msg, raw_id, meta, out);
    }
    Ok(())
}

/// JSONL stream format, with in-place replacement of same-id records.
fn parse_jsonl(
    raw: &str,
    path: &Path,
    raw_id: &str,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) -> anyhow::Result<()> {
    let read = jsonl::read_values_from(std::io::BufReader::new(raw.as_bytes()))?;
    meta.malformed_lines = read.malformed;

    let mut saw_session_id = false;
    let mut records: Vec<Value> = Vec::new();
    let mut index_by_id: HashMap<String, usize> = HashMap::new();
    for rec in read.values {
        if rec
            .get("sessionId")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
        {
            saw_session_id = true;
            observe_str_timestamp(meta, rec.get("startTime"));
            observe_str_timestamp(meta, rec.get("lastUpdated"));
        }
        observe_str_timestamp(meta, rec.pointer("/$set/lastUpdated"));

        let msg_type = rec.get("type").and_then(Value::as_str).unwrap_or("");
        if msg_type != "user" && msg_type != "gemini" {
            continue;
        }
        let msg_id = rec
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !msg_id.is_empty() {
            // Streaming update: a later record with the same id replaces the
            // earlier one in place (keeps its chronological slot).
            if let Some(&idx) = index_by_id.get(&msg_id) {
                records[idx] = rec;
                continue;
            }
            index_by_id.insert(msg_id, records.len());
        }
        records.push(rec);
    }
    if !saw_session_id {
        anyhow::bail!("missing sessionId in {}", path.display());
    }
    for rec in &records {
        append_block(rec, raw_id, meta, out);
    }
    Ok(())
}

/// Convert one message record into a block. Non-contentful messages are
/// dropped, matching the Go parser's empty-content skip.
fn append_block(
    msg: &Value,
    raw_id: &str,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) {
    let timestamp = msg
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|s| time::parse_timestamp(s, false));
    let text = extract_text(msg.get("content"));
    let id = blocks::block_id(raw_id, out.len());
    match msg.get("type").and_then(Value::as_str) {
        Some("user") => {
            if text.trim().is_empty() {
                return;
            }
            if let Some(ts) = timestamp {
                meta.observe_timestamp(ts);
            }
            if meta.first_message.is_empty() {
                meta.first_message = preview(&text, 200);
            }
            meta.message_count += 1;
            meta.user_message_count += 1;
            out.push(blocks::user(id, text, timestamp));
        }
        Some("gemini") => append_assistant(msg, text, id, timestamp, meta, out),
        _ => {}
    }
}

fn append_assistant(
    msg: &Value,
    text: String,
    id: String,
    timestamp: Option<f64>,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) {
    let thinking = extract_thinking(msg.get("thoughts"));
    let mut segments: Vec<AssistantSegment> = Vec::new();
    if !text.trim().is_empty() {
        segments.push(blocks::text_segment(text));
    }
    // (tool_use_id, rendered output) — attached after the block is pushed.
    let mut results: Vec<(String, String)> = Vec::new();
    for tc in msg
        .get("toolCalls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = tc.get("name").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            // Matches Go: empty-name calls carry no structured tool data.
            continue;
        }
        let tc_id = tc.get("id").and_then(Value::as_str).unwrap_or("");
        segments.push(blocks::tool_segment(
            name.to_string(),
            tc.get("args").cloned().unwrap_or(Value::Null),
            tc_id.to_string(),
        ));
        // Inline results: result[].functionResponse.response.output.
        for r in tc
            .get("result")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(output) = r.pointer("/functionResponse/response/output") else {
                continue;
            };
            let rid = r
                .pointer("/functionResponse/id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(tc_id);
            results.push((rid.to_string(), render_output(output)));
        }
    }
    if segments.is_empty() && thinking.is_none() {
        return;
    }
    if let Some(ts) = timestamp {
        meta.observe_timestamp(ts);
    }
    meta.message_count += 1;
    let model = msg.get("model").and_then(Value::as_str).unwrap_or("");
    meta.record_model(model);
    if let Some(tok) = msg.get("tokens").filter(|t| t.is_object()) {
        meta.usage.record(
            model,
            UsageTotals {
                input_tokens: token_count(tok, "input"),
                // Thoughts tokens bill at the output rate.
                output_tokens: token_count(tok, "output") + token_count(tok, "thoughts"),
                cache_read_input_tokens: token_count(tok, "cached"),
                cache_creation_input_tokens: 0,
            },
        );
    }
    out.push(blocks::assistant(id, segments, thinking, timestamp));
    for (rid, output) in results {
        blocks::attach_tool_result(out, &rid, output, false);
    }
}

/// Message content is either a plain string or a parts[] array with `.text`.
fn extract_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

/// thoughts[] → one thinking string ("subject\ndescription" per entry).
fn extract_thinking(thoughts: Option<&Value>) -> Option<String> {
    let parts: Vec<String> = thoughts?
        .as_array()?
        .iter()
        .filter_map(|t| {
            let desc = t.get("description").and_then(Value::as_str).unwrap_or("");
            if desc.is_empty() {
                return None;
            }
            let subj = t.get("subject").and_then(Value::as_str).unwrap_or("");
            Some(if subj.is_empty() {
                desc.to_string()
            } else {
                format!("{subj}\n{desc}")
            })
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// Tool output is normally a string; anything else serializes compactly.
fn render_output(output: &Value) -> String {
    match output {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn token_count(tok: &Value, key: &str) -> u64 {
    tok.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn observe_str_timestamp(meta: &mut ForeignSessionMeta, v: Option<&Value>) {
    if let Some(ts) = v
        .and_then(Value::as_str)
        .and_then(|s| time::parse_timestamp(s, false))
    {
        meta.observe_timestamp(ts);
    }
}

fn is_session_filename(name: &str) -> bool {
    name.starts_with("session-") && (name.ends_with(".json") || name.ends_with(".jsonl"))
}

/// hash-or-name → project display name, from Gemini CLI's own config files.
fn build_project_map(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let projects_doc = read_json(&root.join("projects.json"));
    if let Some(projects) = projects_doc
        .as_ref()
        .and_then(|v| v.get("projects"))
        .and_then(Value::as_object)
    {
        add_project_paths(
            &mut map,
            projects
                .iter()
                .map(|(p, n)| (p.as_str(), n.as_str().unwrap_or("")))
                .collect(),
        );
    }
    let trusted_doc = read_json(&root.join("trustedFolders.json"));
    if let Some(folders) = trusted_doc
        .as_ref()
        .and_then(|v| v.get("trustedFolders"))
        .and_then(Value::as_array)
    {
        add_project_paths(
            &mut map,
            folders
                .iter()
                .filter_map(Value::as_str)
                .map(|p| (p, ""))
                .collect(),
        );
    }
    map
}

/// Register `(abs path, optional name)` pairs: SHA-256(path) → project and
/// name → project, first writer wins (paths sorted for determinism).
fn add_project_paths(map: &mut HashMap<String, String>, mut entries: Vec<(&str, &str)>) {
    entries.sort_by_key(|(p, _)| *p);
    for (abs_path, name) in entries {
        let mut project = project_from_cwd(abs_path);
        if project.is_empty() {
            project = "unknown".to_string();
        }
        map.entry(path_hash(abs_path))
            .or_insert_with(|| project.clone());
        if !name.is_empty() {
            map.entry(name.to_string()).or_insert(project);
        }
    }
}

/// SHA-256 hex of the absolute project path — Gemini CLI's tmp/-dir scheme.
fn path_hash(path: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(64);
    for byte in Sha256::digest(path.as_bytes()) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn resolve_project(dir_name: &str, map: &HashMap<String, String>) -> String {
    match map.get(dir_name) {
        Some(p) if !p.is_empty() => p.clone(),
        // Unmapped: the dir name itself — a 64-hex hash stays as-is
        // (truthful, never invented); human-named dirs normalize dashes.
        _ => dir_name.replace('-', "_"),
    }
}

/// Derive the project for one session path
/// (`<root>/tmp/<dir>/chats/session-*.json`).
fn project_for_path(path: &Path) -> String {
    let Some(hash_dir) = path.parent().and_then(Path::parent) else {
        return "gemini".to_string();
    };
    let Some(dir_name) = hash_dir.file_name().and_then(|n| n.to_str()) else {
        return "gemini".to_string();
    };
    let map = hash_dir
        .parent()
        .and_then(Path::parent)
        .map(build_project_map)
        .unwrap_or_default();
    resolve_project(dir_name, &map)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;
    use std::path::PathBuf;

    const LEGACY: &str = r#"{
      "sessionId": "sess-uuid-1",
      "projectHash": "abc123def456",
      "startTime": "2024-01-01T10:00:00Z",
      "lastUpdated": "2024-01-01T10:05:05Z",
      "messages": [
        { "id": "u1", "type": "user", "timestamp": "2024-01-01T10:00:00Z",
          "content": "Fix the login bug" },
        { "id": "a1", "type": "gemini", "timestamp": "2024-01-01T10:00:05Z",
          "model": "gemini-2.5-pro", "content": "Looking at the auth module...",
          "tokens": { "input": 1500, "output": 200, "cached": 100, "thoughts": 50, "tool": 0, "total": 1850 } },
        { "id": "u2", "type": "user", "timestamp": "2024-01-01T10:05:00Z",
          "content": "That looks right" },
        { "id": "a2", "type": "gemini", "timestamp": "2024-01-01T10:05:05Z",
          "model": "gemini-2.5-pro", "content": "Applied the fix.",
          "tokens": { "input": 2000, "output": 300, "cached": 50, "thoughts": 100, "tool": 0, "total": 2450 } }
      ]
    }"#;

    const JSONL_STREAM: &str = concat!(
        r#"{"sessionId":"sess-jsonl-1","projectHash":"hash","startTime":"2026-04-23T16:12:42.783Z","lastUpdated":"2026-04-23T16:12:42.783Z","kind":"main"}"#,
        "\n",
        r#"{"id":"u1","timestamp":"2026-04-23T16:12:43.085Z","type":"user","content":[{"text":"Fix the import path"}]}"#,
        "\n",
        r#"{"$set":{"lastUpdated":"2026-04-23T16:12:43.085Z"}}"#,
        "\n",
        r#"{"id":"a1","timestamp":"2026-04-23T16:12:50.158Z","type":"gemini","content":"","thoughts":[{"subject":"Planning","description":"Looking for the failure.","timestamp":"2026-04-23T16:12:46.795Z"}],"tokens":{"input":9184,"output":26,"cached":0},"model":"gemini-3.1-pro-preview"}"#,
        "\n",
        r#"{"id":"a1","timestamp":"2026-04-23T16:12:50.158Z","type":"gemini","content":"I found the issue.","thoughts":[{"subject":"Planning","description":"Looking for the failure.","timestamp":"2026-04-23T16:12:46.795Z"}],"tokens":{"input":9184,"output":26,"cached":0},"model":"gemini-3.1-pro-preview","toolCalls":[{"id":"read_file_1","name":"read_file","args":{"file_path":"main.go"},"result":[{"functionResponse":{"id":"read_file_1","name":"read_file","response":{"output":"package main"}}}],"displayName":"ReadFile"}]}"#,
        "\n",
        r#"{"$set":{"lastUpdated":"2026-04-23T16:12:50.158Z"}}"#,
        "\n",
    );

    /// Create `<root>/tmp/<hash(project_path)>/chats/<file>` with `content`.
    fn write_session(root: &Path, project_path: &str, file: &str, content: &str) -> PathBuf {
        let chats = root.join("tmp").join(path_hash(project_path)).join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        let p = chats.join(file);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parses_legacy_object_session() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("projects.json"),
            r#"{"projects":{"/Users/alice/dev/login-app":""}}"#,
        )
        .unwrap();
        let path = write_session(
            root,
            "/Users/alice/dev/login-app",
            "session-legacy.json",
            LEGACY,
        );
        let mut sessions = GeminiProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "gemini:session-legacy");
        assert_eq!(s.meta.project, "login-app");
        assert_eq!(s.meta.first_message, "Fix the login bug");
        assert_eq!(s.meta.message_count, 4);
        assert_eq!(s.meta.user_message_count, 2);
        assert_eq!(s.meta.started_at, Some(1704103200.0));
        assert_eq!(s.meta.ended_at, Some(1704103505.0));
        assert_eq!(s.meta.models, vec!["gemini-2.5-pro".to_string()]);
        assert!(s.meta.usage.has_usage);
        assert_eq!(s.meta.usage.totals.input_tokens, 3500);
        // Thoughts bill at the output rate: (200+50) + (300+100).
        assert_eq!(s.meta.usage.totals.output_tokens, 650);
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 150);
        assert_eq!(s.meta.usage.per_model["gemini-2.5-pro"].input_tokens, 3500);
        assert_eq!(s.blocks.len(), 4);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block")
        };
        assert_eq!(u.text, "Fix the login bug");
    }

    #[test]
    fn jsonl_stream_replaces_same_id_records_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session-stream.jsonl");
        std::fs::write(&path, JSONL_STREAM).unwrap();
        let mut sessions = GeminiProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "gemini:session-stream");
        assert_eq!(s.meta.malformed_lines, 0);
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.first_message, "Fix the import path");
        // Replacement: the a1 update supersedes the first a1 revision, so
        // usage is counted ONCE (doubling here = replacement regression).
        assert_eq!(s.meta.usage.totals.input_tokens, 9184);
        assert_eq!(s.meta.usage.totals.output_tokens, 26);
        assert_eq!(
            s.meta.usage.per_model["gemini-3.1-pro-preview"].input_tokens,
            9184
        );
        assert_eq!(
            s.meta.started_at,
            time::parse_timestamp("2026-04-23T16:12:42.783Z", false)
        );
        assert_eq!(
            s.meta.ended_at,
            time::parse_timestamp("2026-04-23T16:12:50.158Z", false)
        );
        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(
            a.thinking.as_deref(),
            Some("Planning\nLooking for the failure.")
        );
        assert_eq!(a.segments.len(), 2);
        let AssistantSegment::Text { text, .. } = &a.segments[0] else {
            panic!("expected text segment")
        };
        assert_eq!(text, "I found the issue.");
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "read_file");
        assert_eq!(execution.tool_use_id, "read_file_1");
        assert_eq!(
            execution.tool_input,
            serde_json::json!({"file_path": "main.go"})
        );
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "package main");
    }

    #[test]
    fn malformed_lines_counted_and_partial_writes_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        // Mid-stream corrupt line: skipped but COUNTED.
        let mid = dir.path().join("session-mid.jsonl");
        std::fs::write(
            &mid,
            concat!(
                r#"{"sessionId":"sess-mid","startTime":"2026-04-23T16:12:42.783Z"}"#,
                "\n",
                r#"{"id":"u1","type":"user","content":[{"text":"first"}]}"#,
                "\n",
                "{not valid json\n",
                r#"{"id":"a1","type":"gemini","content":"reply"}"#,
                "\n",
            ),
        )
        .unwrap();
        let sessions = GeminiProvider.parse(&mid).unwrap();
        assert_eq!(sessions[0].meta.malformed_lines, 1);
        assert_eq!(sessions[0].meta.message_count, 2);
        // A trailing newline-less partial line = live write, NOT malformed.
        let partial = dir.path().join("session-partial.jsonl");
        std::fs::write(
            &partial,
            concat!(
                r#"{"sessionId":"sess-partial","startTime":"2026-04-23T16:12:42.783Z"}"#,
                "\n",
                r#"{"id":"u1","type":"user","content":[{"text":"first"}]}"#,
                "\n",
                r#"{"id":"a1","type":"gemini","content":"reply"#,
            ),
        )
        .unwrap();
        let sessions = GeminiProvider.parse(&partial).unwrap();
        assert_eq!(sessions[0].meta.malformed_lines, 0);
        assert_eq!(sessions[0].meta.message_count, 1);
    }

    #[test]
    fn non_interactive_and_invalid_sessions() {
        let dir = tempfile::tempdir().unwrap();
        // info/error/warning-only sessions carry no conversation → skipped.
        let sys = dir.path().join("session-sys.json");
        std::fs::write(
            &sys,
            r#"{"sessionId":"sess-3","startTime":"2024-01-01T10:00:00Z","lastUpdated":"2024-01-01T10:00:05Z","messages":[{"id":"i1","type":"info","content":"Starting session"},{"id":"e1","type":"error","content":"Some error"}]}"#,
        )
        .unwrap();
        assert!(GeminiProvider.parse(&sys).unwrap().is_empty());
        // Whitespace-only user content is non-contentful.
        let empty = dir.path().join("session-empty.json");
        std::fs::write(
            &empty,
            r#"{"sessionId":"sess-4","messages":[{"id":"u1","type":"user","content":"   "}]}"#,
        )
        .unwrap();
        assert!(GeminiProvider.parse(&empty).unwrap().is_empty());
        // Missing sessionId is a format error.
        let bad = dir.path().join("session-bad.json");
        std::fs::write(
            &bad,
            r#"{"messages":[{"id":"u1","type":"user","content":"hi"}]}"#,
        )
        .unwrap();
        assert!(GeminiProvider.parse(&bad).is_err());
        // Single-line garbage is neither format.
        let garbage = dir.path().join("session-garbage.json");
        std::fs::write(&garbage, "not valid json {{{").unwrap();
        assert!(GeminiProvider.parse(&garbage).is_err());
    }

    #[test]
    fn discover_walks_tmp_hash_dirs_with_project_map() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("projects.json"),
            r#"{"projects":{"/home/u/dev/my-app":"my-app"}}"#,
        )
        .unwrap();
        let mapped = write_session(
            root,
            "/home/u/dev/my-app",
            "session-2026-01-01-abc123.json",
            LEGACY,
        );
        // Non-session files and nested dirs are ignored.
        std::fs::write(mapped.parent().unwrap().join("notes.txt"), "x").unwrap();
        std::fs::create_dir_all(mapped.parent().unwrap().join("sub")).unwrap();
        // Unmapped human-named dir falls back to the normalized dir name.
        let other_chats = root.join("tmp").join("my-cool-dir").join("chats");
        std::fs::create_dir_all(&other_chats).unwrap();
        std::fs::write(other_chats.join("session-zz.jsonl"), "{}").unwrap();
        // Loose file directly under tmp/ is ignored.
        std::fs::write(root.join("tmp").join("stray.json"), "{}").unwrap();

        let mut found = GeminiProvider.discover(root);
        found.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "gemini:session-2026-01-01-abc123");
        assert_eq!(found[0].project_hint.as_deref(), Some("my-app"));
        assert_eq!(found[1].id, "gemini:session-zz");
        assert_eq!(found[1].project_hint.as_deref(), Some("my_cool_dir"));
    }

    #[test]
    fn project_hash_scheme_matches_gemini_cli() {
        // SHA-256 fixed vector proves the dir-name hashing scheme.
        assert_eq!(
            path_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // trustedFolders.json paths map hash → path basename.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("trustedFolders.json"),
            r#"{"trustedFolders":["/x/y/proj-z"]}"#,
        )
        .unwrap();
        let map = build_project_map(dir.path());
        assert_eq!(
            map.get(&path_hash("/x/y/proj-z")).map(String::as_str),
            Some("proj-z")
        );
        // Unmapped 64-hex dirs fall back to the hash itself, never invented.
        let unmapped = "a".repeat(64);
        assert_eq!(resolve_project(&unmapped, &map), unmapped);
    }
}
