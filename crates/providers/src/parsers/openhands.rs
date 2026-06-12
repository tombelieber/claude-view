// crates/providers/src/parsers/openhands.rs
//
// OpenHands CLI — one DIRECTORY per conversation under
// `~/.openhands/conversations/<id>/`, valid iff it contains an `events/`
// subdir. Layout (ported from agentsview's openhands.go):
//   base_state.json  { id, agent.llm.model, workspace.* (cwd-ish keys) }
//   TASKS.json       optional, counted into discovery size only
//   events/*.json    one JSON document per event, ordered by FILENAME:
//     MessageEvent     { llm_message{role, content}, thinking_blocks[],
//                        reasoning_content }
//     ActionEvent      { tool_name|tool_call.name, action,
//                        tool_call.arguments, tool_call_id, thought, summary }
//     ObservationEvent { observation{content|command|detail}, tool_call_id,
//                        observation.metadata.working_dir }
// No token usage exists anywhere in the format (has_usage stays false); the
// model id lives only in base_state. Timestamps are naive local wall-clock
// (same producer behavior as Hermes).

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, preview, project_from_cwd};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct OpenhandsProvider;

impl Provider for OpenhandsProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Openhands
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(entries) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !is_valid_session_id(name) || !path.join("events").is_dir() {
                continue;
            }
            let Some((mtime, size_bytes)) = snapshot(&path) else {
                continue;
            };
            out.push(DiscoveredSession {
                id: ProviderKind::Openhands.session_id(&normalize_session_id(name)),
                provider: ProviderKind::Openhands,
                path,
                project_hint: None,
                mtime,
                size_bytes,
            });
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let dir = normalize_session_path(path)?;
        let base = read_json(&dir.join("base_state.json"));
        // Session id from base_state; dir-name fallback gets 32-char bare-hex
        // names re-hyphenated to canonical UUID form.
        let raw_id = base
            .as_ref()
            .and_then(|b| b.get("id").and_then(Value::as_str))
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .map(normalize_session_id)
            })
            .ok_or_else(|| anyhow::anyhow!("openhands: no session id for {}", dir.display()))?;

        let mut meta = ForeignSessionMeta::new(ProviderKind::Openhands, &raw_id, dir.clone());
        if let Some(model) = base
            .as_ref()
            .and_then(|b| b.pointer("/agent/llm/model"))
            .and_then(Value::as_str)
        {
            meta.record_model(model);
        }
        // cwd cascade: base_state workspace keys → first event-discovered
        // value (file_editor path dirname / observation working_dir).
        let mut cwd: Option<String> = base.as_ref().and_then(base_state_cwd);

        // Events apply in FILENAME order (Go's os.ReadDir sorts; Rust's
        // read_dir does not — sort explicitly).
        let mut event_files: Vec<PathBuf> = std::fs::read_dir(dir.join("events"))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json") && p.is_file())
            .collect();
        event_files.sort();

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        let mut ordinal = 0usize;
        for file in &event_files {
            let Some(ev) = read_json(file) else {
                meta.malformed_lines += 1;
                continue;
            };
            let ts = event_timestamp(&ev);
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }
            let id = blocks::block_id(&raw_id, ordinal);
            let consumed = match ev.get("kind").and_then(Value::as_str) {
                Some("MessageEvent") => handle_message(&mut out_blocks, &mut meta, &ev, id, ts),
                Some("ActionEvent") => {
                    handle_action(&mut out_blocks, &mut meta, &ev, id, ts, &mut cwd)
                }
                Some("ObservationEvent") => {
                    handle_observation(&mut out_blocks, &mut meta, &ev, id, ts, &mut cwd)
                }
                _ => false,
            };
            if consumed {
                ordinal += 1;
            }
        }

        // Conversations with zero contentful messages are noise.
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        meta.project = cwd
            .as_deref()
            .map(project_from_cwd)
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "openhands".to_string());
        meta.cwd = cwd;
        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

/// OpenHands records naive local wall-clock timestamps — `assume_local`.
fn event_timestamp(ev: &Value) -> Option<f64> {
    ev.get("timestamp")
        .and_then(Value::as_str)
        .and_then(|s| crate::util::time::parse_timestamp(s, true))
}

/// MessageEvent → user/assistant block. Returns true when the event carried
/// any content (advances the block ordinal, mirrors Go's `ok`).
fn handle_message(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    ev: &Value,
    id: String,
    ts: Option<f64>,
) -> bool {
    let role = ev
        .pointer("/llm_message/role")
        .and_then(Value::as_str)
        .unwrap_or("");
    if role != "user" && role != "assistant" {
        return false;
    }
    let mut ex = extract_content(ev.pointer("/llm_message/content"));
    if let Some(t) = event_thinking(ev) {
        ex.thinking_parts.push(t);
    }
    if ex.is_empty() {
        return false;
    }
    for (tool_use_id, output) in ex.results {
        blocks::attach_tool_result(out, &tool_use_id, output, false);
    }
    if role == "user" {
        let text = ex.text_parts.join("\n");
        if text.trim().is_empty() {
            return true; // tool-result-only event: consumed, no block
        }
        if meta.first_message.is_empty() {
            meta.first_message = preview(&text, 200);
        }
        meta.message_count += 1;
        meta.user_message_count += 1;
        out.push(blocks::user(id, text, ts));
    } else {
        let thinking = join_nonempty(&ex.thinking_parts, "\n\n");
        if ex.segments.is_empty() && thinking.is_none() {
            return true; // consumed (results attached) but nothing renderable
        }
        meta.message_count += 1;
        out.push(blocks::assistant(id, ex.segments, thinking, ts));
    }
    true
}

/// ActionEvent → assistant block with one tool segment; `thought` plus
/// thinking_blocks/reasoning_content land in the thinking field.
fn handle_action(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    ev: &Value,
    id: String,
    ts: Option<f64>,
    cwd: &mut Option<String>,
) -> bool {
    let tool_name = ev
        .get("tool_name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| ev.pointer("/tool_call/name").and_then(Value::as_str))
        .unwrap_or("");
    if tool_name.is_empty() {
        return false;
    }
    let action = ev.get("action");
    // Prefer the verbatim tool_call.arguments JSON; fall back to the action
    // object itself (Go: inputJSON = arguments || action.Raw).
    let args = ev
        .pointer("/tool_call/arguments")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let input = if args.is_empty() {
        action.cloned().unwrap_or(Value::Null)
    } else {
        serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_string()))
    };
    let tool_id = ev
        .get("tool_call_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut seg = blocks::tool_segment(tool_name.to_string(), input, tool_id);
    if let AssistantSegment::Tool { execution } = &mut seg {
        execution.summary = ev
            .get("summary")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
    }
    let mut thinking_parts = Vec::new();
    let thought = text_of(ev.get("thought"));
    if !thought.is_empty() {
        thinking_parts.push(thought);
    }
    if let Some(t) = event_thinking(ev) {
        thinking_parts.push(t);
    }
    meta.message_count += 1;
    out.push(blocks::assistant(
        id,
        vec![seg],
        join_nonempty(&thinking_parts, "\n\n"),
        ts,
    ));
    if cwd.is_none() {
        *cwd = action_cwd(tool_name, action);
    }
    true
}

/// ObservationEvent: with a tool_call_id it attaches to the matching tool
/// segment; without one it surfaces as a user block (environment output).
fn handle_observation(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    ev: &Value,
    id: String,
    ts: Option<f64>,
    cwd: &mut Option<String>,
) -> bool {
    let observation = ev.get("observation");
    let display = observation_display(observation);
    let working_dir = observation
        .and_then(|o| o.pointer("/metadata/working_dir"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let tool_id = ev.get("tool_call_id").and_then(Value::as_str).unwrap_or("");

    if tool_id.is_empty() {
        if display.is_empty() {
            return false;
        }
        meta.message_count += 1;
        out.push(blocks::user(id, display, ts));
    } else {
        let is_error = observation
            .and_then(|o| o.get("is_error"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        blocks::attach_tool_result(out, tool_id, display, is_error);
    }
    if cwd.is_none() {
        *cwd = working_dir.map(str::to_string);
    }
    true
}

/// Ordered extraction of Claude-style content blocks.
#[derive(Default)]
struct Extracted {
    /// In-order segments (text + tool calls) for assistant blocks.
    segments: Vec<AssistantSegment>,
    /// Plain text parts (user blocks join these with newlines).
    text_parts: Vec<String>,
    thinking_parts: Vec<String>,
    /// `(tool_use_id, output)` results to attach to earlier calls.
    results: Vec<(String, String)>,
}

impl Extracted {
    fn is_empty(&self) -> bool {
        self.segments.is_empty()
            && self.text_parts.is_empty()
            && self.thinking_parts.is_empty()
            && self.results.is_empty()
    }
}

/// Claude-style content (plain string or block array) → segments/parts.
fn extract_content(content: Option<&Value>) -> Extracted {
    let mut ex = Extracted::default();
    match content {
        Some(Value::String(s)) => {
            if !s.trim().is_empty() {
                ex.text_parts.push(s.clone());
                ex.segments.push(blocks::text_segment(s.clone()));
            }
        }
        Some(Value::Array(items)) => {
            for block in items {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        let Some(t) = block.get("text").and_then(Value::as_str) else {
                            continue;
                        };
                        if !t.trim().is_empty() {
                            ex.text_parts.push(t.to_string());
                            ex.segments.push(blocks::text_segment(t.to_string()));
                        }
                    }
                    Some("thinking") => {
                        if let Some(t) = block.get("thinking").and_then(Value::as_str) {
                            if !t.trim().is_empty() {
                                ex.thinking_parts.push(t.to_string());
                            }
                        }
                    }
                    Some("tool_use") => {
                        let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                        if name.is_empty() {
                            continue;
                        }
                        let tool_id = block.get("id").and_then(Value::as_str).unwrap_or("");
                        ex.segments.push(blocks::tool_segment(
                            name.to_string(),
                            block.get("input").cloned().unwrap_or(Value::Null),
                            tool_id.to_string(),
                        ));
                    }
                    Some("tool_result") => {
                        let tuid = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if tuid.is_empty() {
                            continue;
                        }
                        let output = block.get("content").map(decode_content).unwrap_or_default();
                        ex.results.push((tuid.to_string(), output));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    ex
}

/// Event-level thinking: thinking_blocks[].thinking joined, else
/// reasoning_content (Go's openHandsThinkingText).
fn event_thinking(ev: &Value) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(items) = ev.get("thinking_blocks").and_then(Value::as_array) {
        for block in items {
            if let Some(t) = block.get("thinking").and_then(Value::as_str) {
                let t = t.trim();
                if !t.is_empty() {
                    parts.push(t);
                }
            }
        }
    }
    if !parts.is_empty() {
        return Some(parts.join("\n\n"));
    }
    ev.get("reasoning_content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Observation display text: `content` (string or text-block array), else
/// command+detail, else the raw observation JSON (never invented).
fn observation_display(observation: Option<&Value>) -> String {
    let Some(obs) = observation else {
        return String::new();
    };
    if obs.is_null() {
        return String::new();
    }
    if let Some(content) = obs.get("content") {
        return decode_content(content).trim().to_string();
    }
    let mut parts: Vec<&str> = Vec::new();
    for key in ["command", "detail"] {
        if let Some(s) = obs.get(key).and_then(Value::as_str) {
            if !s.is_empty() {
                parts.push(s);
            }
        }
    }
    let display = parts.join("\n").trim().to_string();
    if !display.is_empty() {
        return display;
    }
    obs.to_string()
}

/// Tool-result content → text: plain string, or array of `{text}` blocks
/// concatenated (Go's DecodeContent).
fn decode_content(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<String>(),
        _ => String::new(),
    }
}

/// Text of a string-or-content-array value (Go's openHandsText for thought).
fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn join_nonempty(parts: &[String], sep: &str) -> Option<String> {
    let joined = parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(sep);
    (!joined.is_empty()).then_some(joined)
}

/// cwd from base_state workspace, probing the known key spellings in order.
fn base_state_cwd(base: &Value) -> Option<String> {
    let ws = base.get("workspace")?;
    [
        "cwd",
        "path",
        "mount_path",
        "root",
        "repo_path",
        "repo_root",
        "working_dir",
    ]
    .into_iter()
    .find_map(|k| {
        ws.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}

/// file_editor actions carry an absolute file path whose parent dir is the
/// best cwd signal available (Go's openHandsActionCwd).
fn action_cwd(tool_name: &str, action: Option<&Value>) -> Option<String> {
    if tool_name != "file_editor" {
        return None;
    }
    let path = action?.get("path")?.as_str()?.trim();
    let p = Path::new(path);
    if path.is_empty() || !p.is_absolute() {
        return None;
    }
    p.parent().map(|d| d.to_string_lossy().into_owned())
}

/// 32-char bare-hex directory names are UUIDs with the hyphens stripped —
/// restore the canonical 8-4-4-4-12 form.
fn normalize_session_id(id: &str) -> String {
    let id = id.trim();
    if id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit()) {
        format!(
            "{}-{}-{}-{}-{}",
            &id[0..8],
            &id[8..12],
            &id[12..16],
            &id[16..20],
            &id[20..32]
        )
    } else {
        id.to_string()
    }
}

fn is_valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Synthetic (mtime, size) for a conversation dir: max mtime / summed size
/// over base_state.json, TASKS.json and events/*.json, floored at the dir's
/// own mtime (mirrors Go's OpenHandsSnapshot).
fn snapshot(dir: &Path) -> Option<(f64, u64)> {
    let (mut mtime, _) = stat_entry(dir)?;
    let mut size: u64 = 0;
    let mut add = |p: &Path| {
        if !p.is_file() {
            return;
        }
        if let Some((m, s)) = stat_entry(p) {
            if m > mtime {
                mtime = m;
            }
            size += s;
        }
    };
    add(&dir.join("base_state.json"));
    add(&dir.join("TASKS.json"));
    if let Ok(entries) = std::fs::read_dir(dir.join("events")) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                add(&p);
            }
        }
    }
    Some((mtime, size))
}

/// Read one JSON document; None for unreadable/invalid files.
fn read_json(path: &Path) -> Option<Value> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Accept the conversation dir itself or any well-known file inside it
/// (Go's normalizeOpenHandsSessionPath).
fn normalize_session_path(path: &Path) -> anyhow::Result<PathBuf> {
    if path.is_dir() {
        return Ok(path.to_path_buf());
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if matches!(name, "base_state.json" | "TASKS.json") {
        if let Some(dir) = path.parent() {
            return Ok(dir.to_path_buf());
        }
    } else if path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some("events")
    {
        if let Some(dir) = path.parent().and_then(Path::parent) {
            return Ok(dir.to_path_buf());
        }
    }
    anyhow::bail!("openhands: not a conversation dir: {}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;

    fn write(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    const BASE_STATE: &str = r#"{
      "id": "086c7ecf-6cb7-46b6-9fbc-b900358d1247",
      "agent": { "llm": { "model": "litellm_proxy/claude-sonnet-4-6" } }
    }"#;

    fn write_happy_session(root: &Path) -> PathBuf {
        let dir = root.join("086c7ecf6cb746b69fbcb900358d1247");
        write(&dir, "base_state.json", BASE_STATE);
        write(
            &dir,
            "events/event-00000-user.json",
            r#"{
              "timestamp": "2026-04-02T15:25:40.706887",
              "kind": "MessageEvent",
              "llm_message": { "role": "user",
                "content": [ { "type": "text", "text": "Help me debug the server" } ] }
            }"#,
        );
        write(
            &dir,
            "events/event-00001-action.json",
            r#"{
              "timestamp": "2026-04-02T15:25:41.706887",
              "kind": "ActionEvent",
              "thought": [ { "type": "text", "text": "I'll inspect the logs first." } ],
              "thinking_blocks": [ { "type": "thinking", "thinking": "Start with the failing process." } ],
              "action": { "command": "tail -40 /tmp/server.log", "kind": "TerminalAction" },
              "tool_name": "terminal",
              "tool_call_id": "toolu_123",
              "tool_call": { "id": "toolu_123", "name": "terminal",
                "arguments": "{\"command\":\"tail -40 /tmp/server.log\"}" },
              "summary": "Inspect latest server logs"
            }"#,
        );
        write(
            &dir,
            "events/event-00002-observation.json",
            r#"{
              "timestamp": "2026-04-02T15:25:42.706887",
              "kind": "ObservationEvent",
              "tool_call_id": "toolu_123",
              "observation": {
                "content": [ { "type": "text", "text": "panic: boom" } ],
                "is_error": false,
                "metadata": { "working_dir": "/work/demo-repo" }
              }
            }"#,
        );
        write(
            &dir,
            "events/event-00003-assistant.json",
            r#"{
              "timestamp": "2026-04-02T15:25:43.706887",
              "kind": "MessageEvent",
              "llm_message": { "role": "assistant",
                "content": [ { "type": "text", "text": "The panic happens during startup." } ] },
              "thinking_blocks": [ { "type": "thinking", "thinking": "Nil config path." } ]
            }"#,
        );
        dir
    }

    fn parse_happy(root: &Path) -> ForeignSession {
        let dir = write_happy_session(root);
        let mut sessions = OpenhandsProvider.parse(&dir).unwrap();
        assert_eq!(sessions.len(), 1);
        sessions.remove(0)
    }

    #[test]
    fn parses_conversation_dir() {
        let root = tempfile::tempdir().unwrap();
        let s = parse_happy(root.path());
        assert_eq!(s.meta.id, "openhands:086c7ecf-6cb7-46b6-9fbc-b900358d1247");
        assert_eq!(s.meta.models, vec!["litellm_proxy/claude-sonnet-4-6"]);
        assert!(!s.meta.usage.has_usage, "openhands carries no token usage");
        assert_eq!(s.meta.cwd.as_deref(), Some("/work/demo-repo"));
        assert_eq!(s.meta.project, "demo-repo");
        assert_eq!(s.meta.first_message, "Help me debug the server");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.malformed_lines, 0);
        assert_eq!(s.blocks.len(), 3);
        let (Some(start), Some(end)) = (s.meta.started_at, s.meta.ended_at) else {
            panic!("expected a timestamp envelope");
        };
        assert!(end > start);
    }

    #[test]
    fn action_gets_tool_segment_thinking_and_result() {
        let root = tempfile::tempdir().unwrap();
        let s = parse_happy(root.path());
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block for the ActionEvent");
        };
        assert_eq!(
            a.thinking.as_deref(),
            Some("I'll inspect the logs first.\n\nStart with the failing process.")
        );
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment");
        };
        assert_eq!(execution.tool_name, "terminal");
        assert_eq!(execution.tool_use_id, "toolu_123");
        assert_eq!(
            execution.summary.as_deref(),
            Some("Inspect latest server logs")
        );
        assert_eq!(
            execution.tool_input,
            serde_json::json!({ "command": "tail -40 /tmp/server.log" })
        );
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "panic: boom");
    }

    #[test]
    fn hex_dir_name_rehyphenated_and_malformed_counted() {
        let root = tempfile::tempdir().unwrap();
        // No base_state.json at all — id falls back to the dir name.
        let dir = root.path().join("aabbccddeeff00112233445566778899");
        write(
            &dir,
            "events/e0.json",
            r#"{ "kind": "MessageEvent", "llm_message": { "role": "user", "content": "hi" } }"#,
        );
        write(&dir, "events/e1.json", "{ not json");
        // Observation without tool_call_id surfaces as a user block built
        // from the command/detail fallback.
        write(
            &dir,
            "events/e2.json",
            r#"{ "kind": "ObservationEvent",
                 "observation": { "command": "git status", "detail": "clean" } }"#,
        );
        let mut sessions = OpenhandsProvider.parse(&dir).unwrap();
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "openhands:aabbccdd-eeff-0011-2233-445566778899");
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.project, "openhands");
        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::User(u) = &s.blocks[1] else {
            panic!("expected observation user block");
        };
        assert_eq!(u.text, "git status\nclean");
        // Environment output is not a real prompt.
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.message_count, 2);
    }

    #[test]
    fn file_editor_action_derives_cwd_without_arguments() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("session_a");
        write(&dir, "base_state.json", r#"{"id":"session_a"}"#);
        write(
            &dir,
            "events/e0.json",
            r#"{ "kind": "ActionEvent", "tool_name": "file_editor", "tool_call_id": "tc1",
                 "action": { "command": "view", "path": "/work/proj/src/main.rs" } }"#,
        );
        let mut sessions = OpenhandsProvider.parse(&dir).unwrap();
        let s = sessions.remove(0);
        assert_eq!(s.meta.cwd.as_deref(), Some("/work/proj/src"));
        assert_eq!(s.meta.project, "src");
        let ConversationBlock::Assistant(a) = &s.blocks[0] else {
            panic!("expected assistant block");
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment");
        };
        // No tool_call.arguments → input falls back to the action object.
        assert_eq!(
            execution.tool_input,
            serde_json::json!({ "command": "view", "path": "/work/proj/src/main.rs" })
        );
    }

    #[test]
    fn sessions_without_contentful_events_are_skipped() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("emptyconvo");
        write(&dir, "base_state.json", r#"{"id":"emptyconvo"}"#);
        write(
            &dir,
            "events/e0.json",
            r#"{ "kind": "AgentStateEvent", "state": "done" }"#,
        );
        assert!(OpenhandsProvider.parse(&dir).unwrap().is_empty());
    }

    #[test]
    fn discover_finds_conversation_dirs() {
        let root = tempfile::tempdir().unwrap();
        let dir = write_happy_session(root.path());
        // Dir without events/ is not a conversation.
        std::fs::create_dir_all(root.path().join("noeventshere")).unwrap();
        // Invalid name (dot) is skipped even with an events/ subdir.
        std::fs::create_dir_all(root.path().join("bad.name/events")).unwrap();
        // Plain-file noise with a hex-ish name.
        std::fs::write(root.path().join("aabbccddeeff00112233445566778800"), "x").unwrap();
        let found = OpenhandsProvider.discover(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].id,
            "openhands:086c7ecf-6cb7-46b6-9fbc-b900358d1247"
        );
        assert_eq!(found[0].path, dir);
        assert!(found[0].size_bytes > 0);
        assert!(found[0].mtime > 0.0);
    }
}
