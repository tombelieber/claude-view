// crates/providers/src/parsers/vscode_copilot.rs
//
// VS Code Copilot Chat + Positron Assistant — byte-identical chat-session
// format, different User dir. ONE parser parameterized by ProviderKind.
//
// Layout under <User dir> (ported from agentsview vscode_copilot.go /
// positron.go / discovery.go):
//   workspaceStorage/<hash>/chatSessions/<uuid>.{json,jsonl}
//   globalStorage/emptyWindowChatSessions/<uuid>.{json,jsonl}
//   globalStorage/transferredChatSessions/<uuid>.{json,jsonl}
// Project = workspaceStorage/<hash>/workspace.json {folder|workspace:
// "file://…"} → path basename; global dirs → "empty-window". When both
// .json and .jsonl exist for one uuid the .jsonl (newer op-log format) wins.
//
// DUAL FORMAT:
//   .json  — full snapshot {version, sessionId, creationDate (epoch ms),
//            lastMessageDate, customTitle, requests:[{requestId,
//            message:{text}, response:[items], modelId, timestamp}]}
//   .jsonl — OPERATION LOG replayed into the same snapshot shape:
//            {kind:0 Initial full-state} {kind:1 Set k,v}
//            {kind:2 Push k,v,i? splice} {kind:3 Delete k}
//            where k mixes object keys and numeric array indices.
// Response items by `kind`: text/markdown (no kind or unknown kind, value
// string or {value}); toolInvocationSerialized → tool call; the rest
// (prepareToolInvocation, inlineReference, undoStop, codeblockUri,
// textEditGroup) are display chrome and skipped. The format stores NO tool
// outputs and NO token usage (has_usage stays false); requests[].modelId is
// recorded via meta.record_model (improvement over the Go parser, which
// drops it).

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, jsonl, preview, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const EMPTY_WINDOW_PROJECT: &str = "empty-window";
const GLOBAL_SESSION_DIRS: [&str; 2] = ["emptyWindowChatSessions", "transferredChatSessions"];

pub struct VscodeChatProvider {
    kind: ProviderKind,
}

pub static VSCODE_COPILOT: VscodeChatProvider = VscodeChatProvider {
    kind: ProviderKind::VscodeCopilot,
};
pub static POSITRON: VscodeChatProvider = VscodeChatProvider {
    kind: ProviderKind::Positron,
};

impl Provider for VscodeChatProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let mut out = Vec::new();
        // 1. workspaceStorage/<hash>/chatSessions/*.{json,jsonl}
        if let Ok(hashes) = std::fs::read_dir(root.join("workspaceStorage")) {
            for hash in hashes.flatten() {
                let hash_path = hash.path();
                if !hash_path.is_dir() {
                    continue;
                }
                let project =
                    read_workspace_manifest(&hash_path).unwrap_or_else(|| "unknown".to_string());
                self.collect_session_files(&hash_path.join("chatSessions"), &project, &mut out);
            }
        }
        // 2. globalStorage empty-window + transferred sessions.
        for subdir in GLOBAL_SESSION_DIRS {
            let dir = root.join("globalStorage").join(subdir);
            self.collect_session_files(&dir, EMPTY_WINDOW_PROJECT, &mut out);
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let is_jsonl = path.extension().and_then(|e| e.to_str()) == Some("jsonl");
        let (doc, malformed) = if is_jsonl {
            let read = jsonl::read_values(path)?;
            let (state, op_malformed) = replay_ops(&read.values);
            let Some(doc) = state else {
                // No Initial snapshot ever landed — nothing truthful to show.
                return Ok(Vec::new());
            };
            (doc, read.malformed + op_malformed)
        } else {
            let raw = std::fs::read_to_string(path)?;
            (serde_json::from_str(&raw)?, 0)
        };

        // Raw id = filename stem (uuid) so lookup-by-id matches discovery;
        // the JSON sessionId field is only a fallback.
        let raw_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .or_else(|| {
                doc.get("sessionId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .ok_or_else(|| anyhow::anyhow!("no usable session id"))?;

        let mut meta = ForeignSessionMeta::new(self.kind, &raw_id, path.to_path_buf());
        meta.malformed_lines = malformed;
        meta.project = derive_project(path);
        meta.title = doc
            .get("customTitle")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string);
        for key in ["creationDate", "lastMessageDate"] {
            if let Some(ms) = doc.get(key).and_then(Value::as_f64) {
                meta.observe_timestamp(time::from_millis(ms));
            }
        }

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        for req in doc
            .get("requests")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let ts = req
                .get("timestamp")
                .and_then(Value::as_f64)
                .map(time::from_millis)
                .filter(|t| *t > 0.0);
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }
            if let Some(model) = req.get("modelId").and_then(Value::as_str) {
                meta.record_model(model);
            }

            let text = req
                .pointer("/message/text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if !text.is_empty() {
                if meta.first_message.is_empty() {
                    meta.first_message = preview(text, 200);
                }
                meta.message_count += 1;
                meta.user_message_count += 1;
                let id = blocks::block_id(&raw_id, out_blocks.len());
                out_blocks.push(blocks::user(id, text.to_string(), ts));
            }

            let segments = parse_response(req.get("response").and_then(Value::as_array));
            if !segments.is_empty() {
                meta.message_count += 1;
                let id = blocks::block_id(&raw_id, out_blocks.len());
                out_blocks.push(blocks::assistant(id, segments, None, ts));
            }
        }

        // No user text anywhere → customTitle is the only honest preview.
        if meta.first_message.is_empty() {
            if let Some(title) = meta.title.as_deref() {
                meta.first_message = preview(title, 200);
            }
        }
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

impl VscodeChatProvider {
    /// Collect *.json / *.jsonl session files from one directory; when both
    /// exist for the same uuid the .jsonl wins (newer op-log format).
    fn collect_session_files(&self, dir: &Path, project: &str, out: &mut Vec<DiscoveredSession>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut candidates: Vec<(String, bool, PathBuf)> = Vec::new();
        let mut has_jsonl: HashSet<String> = HashSet::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match ext {
                "jsonl" => {
                    has_jsonl.insert(stem.to_string());
                    candidates.push((stem.to_string(), true, path));
                }
                "json" => candidates.push((stem.to_string(), false, path)),
                _ => {}
            }
        }
        for (stem, is_jsonl, path) in candidates {
            if !is_jsonl && has_jsonl.contains(&stem) {
                continue;
            }
            let Some((mtime, size_bytes)) = stat_entry(&path) else {
                continue;
            };
            out.push(DiscoveredSession {
                id: self.kind.session_id(&stem),
                provider: self.kind,
                path,
                project_hint: Some(project.to_string()),
                mtime,
                size_bytes,
            });
        }
    }
}

/// Turn one request's response-item array into ordered assistant segments.
/// Consecutive text values coalesce into one segment (they are markdown
/// stream chunks — joined with NO separator, exactly like the Go parser).
fn parse_response(items: Option<&Vec<Value>>) -> Vec<AssistantSegment> {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut text_buf = String::new();
    for item in items.into_iter().flatten() {
        match item.get("kind").and_then(Value::as_str).unwrap_or("") {
            "toolInvocationSerialized" => {
                let Some(tool_id) = item
                    .get("toolId")
                    .and_then(Value::as_str)
                    .filter(|t| !t.is_empty())
                else {
                    continue;
                };
                flush_text(&mut text_buf, &mut segments);
                let call_id = item.get("toolCallId").and_then(Value::as_str).unwrap_or("");
                segments.push(blocks::tool_segment(
                    tool_id.to_string(),
                    tool_input(item),
                    call_id.to_string(),
                ));
            }
            // Pre-invocation placeholder + display chrome: not transcript.
            "prepareToolInvocation"
            | "inlineReference"
            | "undoStop"
            | "codeblockUri"
            | "textEditGroup" => {}
            // No kind (markdown chunk) or unknown kind: extract value text.
            _ => {
                if let Some(v) = item_text(item) {
                    text_buf.push_str(v);
                }
            }
        }
    }
    flush_text(&mut text_buf, &mut segments);
    segments
}

fn flush_text(buf: &mut String, segments: &mut Vec<AssistantSegment>) {
    let text = buf.trim();
    if !text.is_empty() {
        segments.push(blocks::text_segment(text.to_string()));
    }
    buf.clear();
}

/// `value` is a plain string or a `{value: "…"}` wrapper.
fn item_text(item: &Value) -> Option<&str> {
    match item.get("value")? {
        Value::String(s) => Some(s),
        Value::Object(o) => o.get("value").and_then(Value::as_str),
        _ => None,
    }
}

/// Build the opaque toolInput JSON the UI renders: human message (past-tense
/// preferred over in-progress) plus the terminal command when present.
fn tool_input(item: &Value) -> Value {
    let mut obj = serde_json::Map::new();
    let msg = invocation_text(item.get("pastTenseMessage"))
        .filter(|m| !m.is_empty())
        .or_else(|| invocation_text(item.get("invocationMessage")).filter(|m| !m.is_empty()));
    if let Some(m) = msg {
        obj.insert("message".into(), Value::String(m));
    }
    if let Some(cmd) = item
        .pointer("/toolSpecificData/command")
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
    {
        obj.insert("command".into(), Value::String(cmd.to_string()));
    }
    if obj.is_empty() {
        Value::Null
    } else {
        Value::Object(obj)
    }
}

/// invocationMessage / pastTenseMessage: plain string OR `{value: "…"}`.
fn invocation_text(raw: Option<&Value>) -> Option<String> {
    match raw? {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("value").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// JSONL operation-log replay (ports reconstructJSONL + jsonlSet/Push/Delete).
// Paths in `k` mix object keys and numeric array indices; splice indices are
// clamped to [0, len]. Wrong index handling here silently drops requests, so
// the Go replay tests are ported below.
// ---------------------------------------------------------------------------

/// Replay parsed op lines into the final snapshot. Returns the state (None
/// when no Initial op ever landed) plus the count of unusable ops.
fn replay_ops(values: &[Value]) -> (Option<Value>, u32) {
    let mut state: Option<Value> = None;
    let mut malformed: u32 = 0;
    for op in values {
        let kind = op.get("kind").and_then(Value::as_i64).unwrap_or(-1);
        match kind {
            0 => match op.get("v") {
                Some(v) => state = Some(v.clone()),
                None => malformed += 1,
            },
            1..=3 => {
                let Some(st) = state.as_mut() else {
                    continue;
                };
                let Some(keys) = decode_keys(op.get("k")) else {
                    continue;
                };
                match kind {
                    1 => {
                        if let Some(v) = op.get("v") {
                            op_set(st, &keys, v.clone());
                        }
                    }
                    2 => {
                        if let Some(items) = op.get("v").and_then(Value::as_array) {
                            let splice = op.get("i").and_then(Value::as_i64);
                            op_push(st, &keys, items, splice);
                        }
                    }
                    _ => op_delete(st, &keys),
                }
            }
            // Unknown op kinds are ignored, matching the Go replay.
            _ => {}
        }
    }
    (state, malformed)
}

/// Path elements are strings (object keys) or numbers (array indices);
/// numbers normalize to their decimal string.
fn decode_keys(k: Option<&Value>) -> Option<Vec<String>> {
    let arr = k?.as_array()?;
    if arr.is_empty() {
        return None;
    }
    Some(
        arr.iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect(),
    )
}

fn navigate_mut<'a>(state: &'a mut Value, keys: &[String]) -> Option<&'a mut Value> {
    let mut cur = state;
    for k in keys {
        cur = child_mut(cur, k)?;
    }
    Some(cur)
}

fn child_mut<'a>(node: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    match node {
        Value::Object(m) => m.get_mut(key),
        Value::Array(a) => a.get_mut(key.parse::<usize>().ok()?),
        _ => None,
    }
}

fn op_set(state: &mut Value, keys: &[String], val: Value) {
    let Some((last, parents)) = keys.split_last() else {
        return;
    };
    match navigate_mut(state, parents) {
        Some(Value::Object(m)) => {
            m.insert(last.clone(), val);
        }
        Some(Value::Array(a)) => {
            if let Some(slot) = last.parse::<usize>().ok().and_then(|i| a.get_mut(i)) {
                *slot = val;
            }
        }
        _ => {}
    }
}

fn op_push(state: &mut Value, keys: &[String], items: &[Value], splice: Option<i64>) {
    let Some((last, parents)) = keys.split_last() else {
        return;
    };
    let Some(parent) = navigate_mut(state, parents) else {
        return;
    };
    let Some(arr) = child_mut(parent, last).and_then(Value::as_array_mut) else {
        return;
    };
    // Splice index clamps to [0, len]; absent index = append.
    let idx = match splice {
        Some(i) => (i.max(0) as usize).min(arr.len()),
        None => arr.len(),
    };
    arr.splice(idx..idx, items.iter().cloned());
}

fn op_delete(state: &mut Value, keys: &[String]) {
    let Some((last, parents)) = keys.split_last() else {
        return;
    };
    if let Some(Value::Object(m)) = navigate_mut(state, parents) {
        m.remove(last);
    }
}

// ---------------------------------------------------------------------------
// Project derivation
// ---------------------------------------------------------------------------

/// Sessions live at workspaceStorage/<hash>/chatSessions/<uuid>.* — the
/// manifest is <hash>/workspace.json. Global-storage sessions have no
/// workspace and label as "empty-window".
fn derive_project(path: &Path) -> String {
    let parent = path.parent();
    let parent_name = parent
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if GLOBAL_SESSION_DIRS.contains(&parent_name) {
        return EMPTY_WINDOW_PROJECT.to_string();
    }
    parent
        .and_then(Path::parent)
        .and_then(read_workspace_manifest)
        .unwrap_or_else(|| "unknown".to_string())
}

fn read_workspace_manifest(hash_dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(hash_dir.join("workspace.json")).ok()?;
    let doc: Value = serde_json::from_str(&raw).ok()?;
    let uri = doc
        .get("folder")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            doc.get("workspace")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })?;
    Some(project_from_uri(uri))
}

/// "file:///Users/dev/projects/myapp" → "myapp" (Windows "/C:/…" handled);
/// non-file URIs fall back to their final path component.
fn project_from_uri(uri: &str) -> String {
    let Some(mut path) = uri.strip_prefix("file://") else {
        return base_name(uri);
    };
    let bytes = path.as_bytes();
    if bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':' {
        path = &path[1..];
    }
    base_name(path)
}

fn base_name(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SNAPSHOT: &str = r#"{
      "version": 3,
      "sessionId": "sess-1",
      "creationDate": 1755347684754,
      "lastMessageDate": 1755347728048,
      "customTitle": "Terminal session",
      "requests": [{
        "requestId": "req1",
        "message": { "text": "Run the tests", "parts": [] },
        "response": [
          { "value": "Running tests" },
          { "value": "... " },
          { "kind": "prepareToolInvocation", "toolName": "copilot_runInTerminal" },
          { "kind": "toolInvocationSerialized", "toolId": "copilot_runInTerminal",
            "toolCallId": "tc1", "isConfirmed": true, "isComplete": true,
            "invocationMessage": "Using \"Run In Terminal\"",
            "pastTenseMessage": { "value": "Ran command in terminal" },
            "toolSpecificData": { "kind": "terminal", "language": "sh", "command": "npm test" } },
          { "kind": "inlineReference", "inlineReference": {} },
          { "kind": "undoStop" },
          { "value": "All green." }
        ],
        "modelId": "copilot/gpt-5",
        "timestamp": 1755347728047
      }]
    }"#;

    fn parse_file(name: &str, contents: &str) -> Vec<ForeignSession> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, contents).unwrap();
        VSCODE_COPILOT.parse(&path).unwrap()
    }

    #[test]
    fn parses_json_snapshot() {
        let mut sessions = parse_file("sess-1.json", SNAPSHOT);
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "vscode-copilot:sess-1");
        assert_eq!(s.meta.title.as_deref(), Some("Terminal session"));
        assert_eq!(s.meta.first_message, "Run the tests");
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.models, vec!["copilot/gpt-5"]);
        assert!(!s.meta.usage.has_usage, "format carries no token counts");
        assert!((s.meta.started_at.unwrap() - 1755347684.754).abs() < 1e-6);
        assert!((s.meta.ended_at.unwrap() - 1755347728.048).abs() < 1e-6);

        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block")
        };
        assert_eq!(u.text, "Run the tests");
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        // Stream chunks coalesce around the tool call, in response order.
        assert_eq!(a.segments.len(), 3);
        let AssistantSegment::Text { text, .. } = &a.segments[0] else {
            panic!("expected text segment")
        };
        assert_eq!(text, "Running tests...");
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "copilot_runInTerminal");
        assert_eq!(execution.tool_use_id, "tc1");
        assert_eq!(
            execution.tool_input,
            json!({ "message": "Ran command in terminal", "command": "npm test" })
        );
        let AssistantSegment::Text { text, .. } = &a.segments[2] else {
            panic!("expected trailing text segment")
        };
        assert_eq!(text, "All green.");
    }

    #[test]
    fn jsonl_oplog_replays_to_snapshot() {
        // Splice-to-front, numeric path Set, and Delete — the replay hazards.
        let lines = [
            r#"{"kind":0,"v":{"version":3,"sessionId":"jsonl-1","creationDate":1770650022790,"customTitle":"","requests":[]}}"#,
            r#"{"kind":1,"k":["customTitle"],"v":"Replayed Title"}"#,
            r#"{"kind":2,"k":["requests"],"v":[{"requestId":"r2","timestamp":1770650041889,"message":{"text":"Second"},"response":[{"value":"Answer 2"}],"modelId":"copilot/gpt-4o"}]}"#,
            r#"{"kind":2,"k":["requests"],"v":[{"requestId":"r1","timestamp":1770650031889,"message":{"text":"First"},"response":[{"value":"partial"}]}],"i":0}"#,
            r#"{"kind":1,"k":["requests",0,"response",0],"v":{"value":"Answer 1"}}"#,
            r#"{"kind":3,"k":["requests",1,"modelId"]}"#,
        ]
        .join("\n");
        let mut sessions = parse_file("jsonl-1.jsonl", &lines);
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "vscode-copilot:jsonl-1");
        assert_eq!(s.meta.title.as_deref(), Some("Replayed Title"));
        assert_eq!(s.meta.first_message, "First");
        assert_eq!(s.meta.message_count, 4);
        // r2's modelId was deleted by the op log → no model recorded at all.
        assert!(s.meta.models.is_empty());
        assert_eq!(s.meta.malformed_lines, 0);

        let texts: Vec<&str> = s
            .blocks
            .iter()
            .map(|b| match b {
                ConversationBlock::User(u) => u.text.as_str(),
                ConversationBlock::Assistant(a) => match &a.segments[0] {
                    AssistantSegment::Text { text, .. } => text.as_str(),
                    _ => panic!("expected text segment"),
                },
                _ => panic!("unexpected block"),
            })
            .collect();
        assert_eq!(texts, ["First", "Answer 1", "Second", "Answer 2"]);
    }

    #[test]
    fn replay_op_semantics_match_go() {
        // Ported from TestReconstructJSONL: splice, clamp, indexed set, delete.
        let ops = vec![
            json!({"kind": 0, "v": {"items": ["a", "c"], "arr": ["x", "y", "z"], "a": {"b": "old"}, "drop": 1}}),
            json!({"kind": 2, "k": ["items"], "v": ["b"], "i": 1}),
            json!({"kind": 2, "k": ["items"], "v": ["z0"], "i": -1}),
            json!({"kind": 2, "k": ["items"], "v": ["tail"], "i": 99}),
            json!({"kind": 1, "k": ["arr", 1], "v": "Y"}),
            json!({"kind": 1, "k": ["a", "b"], "v": "new"}),
            json!({"kind": 3, "k": ["drop"]}),
            json!({"kind": 7, "k": ["arr"], "v": "ignored-unknown-kind"}),
        ];
        let (state, malformed) = replay_ops(&ops);
        let state = state.unwrap();
        assert_eq!(malformed, 0);
        assert_eq!(state["items"], json!(["z0", "a", "b", "c", "tail"]));
        assert_eq!(state["arr"], json!(["x", "Y", "z"]));
        assert_eq!(state["a"]["b"], json!("new"));
        assert!(state.get("drop").is_none());

        // Ops before any Initial snapshot are no-ops; state stays None.
        let (none_state, _) = replay_ops(&[json!({"kind": 1, "k": ["x"], "v": 1})]);
        assert!(none_state.is_none());
    }

    #[test]
    fn empty_and_noninteractive_sessions_skip() {
        // No requests at all.
        assert!(
            parse_file("e1.json", r#"{"version":3,"sessionId":"e1","requests":[]}"#).is_empty()
        );
        // Requests with no user text and no renderable response.
        let hollow = r#"{"version":3,"sessionId":"e2","requests":[
          {"requestId":"r1","message":{"text":"  "},"response":[{"kind":"undoStop"}],"timestamp":0}
        ]}"#;
        assert!(parse_file("e2.json", hollow).is_empty());
        // Op log that never received its Initial snapshot.
        assert!(parse_file("e3.jsonl", r#"{"kind":1,"k":["customTitle"],"v":"x"}"#).is_empty());
    }

    #[test]
    fn custom_title_fallback_and_malformed_count() {
        let lines = [
            r#"{"kind":0,"v":{"version":3,"sessionId":"t1","creationDate":1770650022790,"customTitle":"Fallback Title","requests":[]}}"#,
            "this line is garbage, not json",
            r#"{"kind":2,"k":["requests"],"v":[{"requestId":"r1","timestamp":1770650031889,"message":{"text":""},"response":[{"value":"Some response"}]}]}"#,
        ]
        .join("\n");
        let mut sessions = parse_file("t1.jsonl", &(lines + "\n"));
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.first_message, "Fallback Title");
        assert_eq!(s.meta.message_count, 1);
        assert_eq!(s.meta.user_message_count, 0);
        assert_eq!(s.meta.malformed_lines, 1);
    }

    #[test]
    fn discover_prefers_jsonl_and_reads_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let hash_dir = root.join("workspaceStorage").join("abc123def456");
        let chat_dir = hash_dir.join("chatSessions");
        std::fs::create_dir_all(&chat_dir).unwrap();
        std::fs::write(
            hash_dir.join("workspace.json"),
            r#"{"folder":"file:///Users/dev/projects/myproject"}"#,
        )
        .unwrap();
        let session = r#"{"version":3,"sessionId":"dup1","requests":[{"requestId":"r1","message":{"text":"hi"},"response":[{"value":"hello"}],"timestamp":1755340000000}]}"#;
        std::fs::write(chat_dir.join("dup1.json"), session).unwrap();
        std::fs::write(
            chat_dir.join("dup1.jsonl"),
            r#"{"kind":0,"v":{"version":3,"sessionId":"dup1","requests":[]}}"#,
        )
        .unwrap();
        std::fs::write(chat_dir.join("only.json"), session).unwrap();
        std::fs::write(chat_dir.join("notes.txt"), "skip me").unwrap();
        let global_dir = root.join("globalStorage").join("emptyWindowChatSessions");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(global_dir.join("g1.json"), session).unwrap();

        let found = VSCODE_COPILOT.discover(root);
        let names: Vec<&str> = found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_str().unwrap())
            .collect();
        // dup1.json excluded because dup1.jsonl exists; txt file skipped.
        assert_eq!(names, ["g1.json", "dup1.jsonl", "only.json"]);
        assert_eq!(found[1].id, "vscode-copilot:dup1");
        assert_eq!(found[1].project_hint.as_deref(), Some("myproject"));
        assert_eq!(found[0].project_hint.as_deref(), Some("empty-window"));

        // Positron shares the parser; only the id prefix differs.
        let positron = POSITRON.discover(root);
        assert_eq!(positron.len(), 3);
        assert!(positron.iter().all(|f| f.id.starts_with("positron:")));

        // parse() re-derives project from the on-disk layout.
        let parsed = VSCODE_COPILOT.parse(&chat_dir.join("only.json")).unwrap();
        assert_eq!(parsed[0].meta.project, "myproject");
        let global = VSCODE_COPILOT.parse(&global_dir.join("g1.json")).unwrap();
        assert_eq!(global[0].meta.project, "empty-window");
    }

    #[test]
    fn project_uri_extraction() {
        assert_eq!(
            project_from_uri("file:///Users/dev/projects/myapp"),
            "myapp"
        );
        assert_eq!(project_from_uri("file:///home/user/code/repo"), "repo");
        assert_eq!(project_from_uri("file:///C:/Users/dev/projects/app"), "app");
        assert_eq!(project_from_uri("some-name"), "some-name");
    }
}
