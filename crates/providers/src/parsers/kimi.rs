// crates/providers/src/parsers/kimi.rs
//
// Kimi CLI — wire-event JSONL at
// `<root>/<project-hash>/<session-uuid>/wire.jsonl` (two-level walk).
// Raw session id is `<project-hash>:<session-uuid>`; the project display
// name is the hash dir name (Go parity — the format carries nothing better).
//
// Format (ported from agentsview's kimi.go):
//   {type:"metadata", …}                                  → skipped
//   {timestamp: unix-secs float, message:{type, payload}} with types:
//     TurnBegin    payload.user_input:[{type:"text",text}]  → user block
//     StepBegin    informational, no action
//     ContentPart  payload.type "text" (field `text`) or "think" (field
//                  `think`) — accumulates into the pending assistant turn
//     ToolCall     payload.function{name, arguments: STRING-encoded JSON —
//                  double-parsed}, payload.id
//     ToolResult   payload.tool_call_id, payload.return_value{output,
//                  is_error}; output is a string OR [{type:"text",text}]
//     StatusUpdate payload.token_usage.output (summed); context_tokens is
//                  peak context size, not a usage bucket → ignored
//     TurnEnd      flush
//   The assistant turn is STATEFUL: ContentParts + ToolCalls accumulate and
//   flush on TurnBegin, ToolResult, or TurnEnd; the result then attaches to
//   the just-flushed tool segment.
// No model name exists anywhere in the format: output tokens are recorded
// under the empty model id, so has_usage=true but per_model stays empty
// (truthfully unpriceable).

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::Path;

pub struct KimiProvider;

impl Provider for KimiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Kimi
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(proj_dirs) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for proj in proj_dirs.flatten() {
            let proj_path = proj.path();
            // is_dir() follows symlinks — matches Go's isDirOrSymlink.
            if !proj_path.is_dir() {
                continue;
            }
            let Some(proj_name) = proj_path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(sess_dirs) = std::fs::read_dir(&proj_path) else {
                continue;
            };
            for sess in sess_dirs.flatten() {
                let sess_path = sess.path();
                if !sess_path.is_dir() {
                    continue;
                }
                let Some(sess_name) = sess_path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let wire = sess_path.join("wire.jsonl");
                let Some((mtime, size_bytes)) = stat_entry(&wire) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id: ProviderKind::Kimi.session_id(&format!("{proj_name}:{sess_name}")),
                    provider: ProviderKind::Kimi,
                    path: wire,
                    project_hint: Some(proj_name.to_string()),
                    mtime,
                    size_bytes,
                });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        // Raw id from the path: …/<project-hash>/<session-uuid>/wire.jsonl.
        let sess_dir = path.parent().ok_or_else(|| {
            anyhow::anyhow!("kimi wire.jsonl has no session dir: {}", path.display())
        })?;
        let session_uuid = sess_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("kimi session dir name invalid: {}", path.display()))?;
        let proj_hash = sess_dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("kimi project dir name invalid: {}", path.display()))?;
        let raw_id = format!("{proj_hash}:{session_uuid}");

        let read = jsonl::read_values(path)?;
        let mut meta = ForeignSessionMeta::new(ProviderKind::Kimi, &raw_id, path.to_path_buf());
        meta.project = proj_hash.to_string();
        meta.malformed_lines = read.malformed;

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        let mut pending = PendingTurn::default();
        let mut ordinal = 0usize;
        let mut current_ts: Option<f64> = None;
        let mut output_tokens: u64 = 0;
        let mut has_output_tokens = false;

        for line in &read.values {
            if line.get("type").and_then(Value::as_str) == Some("metadata") {
                continue;
            }
            if let Some(ts) = line.get("timestamp").and_then(Value::as_f64) {
                if ts > 0.0 {
                    meta.observe_timestamp(ts);
                    current_ts = Some(ts);
                }
            }
            let msg_type = line
                .pointer("/message/type")
                .and_then(Value::as_str)
                .unwrap_or("");
            let payload = line.pointer("/message/payload");
            match msg_type {
                "TurnBegin" => {
                    pending.flush(&mut out_blocks, &mut meta, &raw_id, &mut ordinal);
                    emit_user_turn(
                        &mut out_blocks,
                        &mut meta,
                        &raw_id,
                        &mut ordinal,
                        payload,
                        current_ts,
                    );
                }
                "ContentPart" => pending.content_part(payload, current_ts),
                "ToolCall" => pending.tool_call(payload, current_ts),
                "ToolResult" => {
                    pending.flush(&mut out_blocks, &mut meta, &raw_id, &mut ordinal);
                    attach_result(&mut out_blocks, payload);
                }
                "StatusUpdate" => {
                    if let Some(out) = payload.and_then(|p| p.pointer("/token_usage/output")) {
                        // Presence (even at 0) means the format DID report
                        // usage — mirrors Go's HasTotalOutputTokens.
                        has_output_tokens = true;
                        output_tokens += token_count(out);
                    }
                    // payload.context_tokens (peak context size) intentionally
                    // ignored: it is not an Anthropic-shape usage bucket.
                }
                "TurnEnd" => pending.flush(&mut out_blocks, &mut meta, &raw_id, &mut ordinal),
                // StepBegin and unknown types: informational, no action.
                _ => {}
            }
        }
        pending.flush(&mut out_blocks, &mut meta, &raw_id, &mut ordinal);

        // Sessions with zero contentful messages are non-interactive noise.
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        if has_output_tokens {
            meta.usage.record(
                "",
                UsageTotals {
                    output_tokens,
                    ..Default::default()
                },
            );
        }
        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

/// Accumulator for the current assistant turn. Text parts merge into one
/// segment (joined "\n", Go parity); tool calls interleave in order; think
/// parts feed the dedicated thinking field. `ts` is the timestamp of the
/// first contentful part (empty fragments must not pin it — Go parity).
#[derive(Default)]
struct PendingTurn {
    segments: Vec<AssistantSegment>,
    text_buf: Vec<String>,
    thinking: Vec<String>,
    ts: Option<f64>,
}

impl PendingTurn {
    fn touch(&mut self, ts: Option<f64>) {
        if self.ts.is_none() {
            self.ts = ts;
        }
    }

    fn content_part(&mut self, payload: Option<&Value>, ts: Option<f64>) {
        let Some(p) = payload else { return };
        match p.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = p.get("text").and_then(Value::as_str) {
                    if !t.is_empty() {
                        self.touch(ts);
                        self.text_buf.push(t.to_string());
                    }
                }
            }
            // Think parts carry their text in the `think` field.
            Some("think") => {
                if let Some(t) = p.get("think").and_then(Value::as_str) {
                    if !t.is_empty() {
                        self.touch(ts);
                        self.thinking.push(t.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    fn tool_call(&mut self, payload: Option<&Value>, ts: Option<f64>) {
        let Some(p) = payload else { return };
        self.touch(ts);
        let name = p
            .pointer("/function/name")
            .and_then(Value::as_str)
            .unwrap_or("tool");
        let args = p
            .pointer("/function/arguments")
            .and_then(Value::as_str)
            .unwrap_or("");
        let tool_id = p.get("id").and_then(Value::as_str).unwrap_or("");
        self.flush_text();
        self.segments.push(blocks::tool_segment(
            name.to_string(),
            parse_arguments(args),
            tool_id.to_string(),
        ));
    }

    /// Close the open text run into a single text segment.
    fn flush_text(&mut self) {
        if !self.text_buf.is_empty() {
            let text = std::mem::take(&mut self.text_buf).join("\n");
            self.segments.push(blocks::text_segment(text));
        }
    }

    /// Emit the pending turn as one assistant block (if contentful) and reset.
    fn flush(
        &mut self,
        out: &mut Vec<ConversationBlock>,
        meta: &mut ForeignSessionMeta,
        raw_id: &str,
        ordinal: &mut usize,
    ) {
        self.flush_text();
        let segments = std::mem::take(&mut self.segments);
        let thinking_parts = std::mem::take(&mut self.thinking);
        let ts = self.ts.take();
        // Go parity: whitespace-only text with no tools and no thinking is
        // not a turn.
        let contentful = !thinking_parts.is_empty()
            || segments.iter().any(|s| match s {
                AssistantSegment::Text { text, .. } => !text.trim().is_empty(),
                _ => true,
            });
        if !contentful {
            return;
        }
        let thinking = (!thinking_parts.is_empty()).then(|| thinking_parts.join("\n\n"));
        meta.message_count += 1;
        out.push(blocks::assistant(
            blocks::block_id(raw_id, *ordinal),
            segments,
            thinking,
            ts,
        ));
        *ordinal += 1;
    }
}

/// TurnBegin → user block from the text entries of `user_input`.
fn emit_user_turn(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    raw_id: &str,
    ordinal: &mut usize,
    payload: Option<&Value>,
    ts: Option<f64>,
) {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(items) = payload
        .and_then(|p| p.get("user_input"))
        .and_then(Value::as_array)
    {
        for item in items {
            if item.get("type").and_then(Value::as_str) != Some("text") {
                continue;
            }
            if let Some(t) = item.get("text").and_then(Value::as_str) {
                if !t.is_empty() {
                    parts.push(t);
                }
            }
        }
    }
    if parts.is_empty() {
        return;
    }
    let text = parts.join("\n");
    if meta.first_message.is_empty() {
        meta.first_message = preview(&text, 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    out.push(blocks::user(blocks::block_id(raw_id, *ordinal), text, ts));
    *ordinal += 1;
}

/// ToolResult → attach to the matching tool segment (already flushed).
/// An error result with empty output renders as "[error]" (Go parity).
fn attach_result(out: &mut [ConversationBlock], payload: Option<&Value>) {
    let Some(p) = payload else { return };
    let tool_call_id = p.get("tool_call_id").and_then(Value::as_str).unwrap_or("");
    let ret = p.get("return_value");
    let is_error = ret
        .and_then(|r| r.get("is_error"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut output = extract_output(ret.and_then(|r| r.get("output")));
    if is_error && output.is_empty() {
        output = "[error]".to_string();
    }
    blocks::attach_tool_result(out, tool_call_id, output, is_error);
}

/// Tool output is a plain string OR an array of {type:"text", text} entries;
/// anything else non-null falls back to its raw JSON (Go parity).
fn extract_output(output: Option<&Value>) -> String {
    match output {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
    }
}

/// `function.arguments` is STRING-encoded JSON — double-parse. Unparseable
/// arguments survive verbatim as a JSON string (never fabricate, never drop).
fn parse_arguments(args: &str) -> Value {
    if args.trim().is_empty() {
        return Value::Null;
    }
    serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_string()))
}

/// Token counts arrive as JSON numbers (int in practice; tolerate float).
fn token_count(v: &Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_f64().map(|f| f.max(0.0) as u64))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;
    use std::path::PathBuf;

    fn write_wire(root: &Path, proj: &str, uuid: &str, lines: &[&str]) -> PathBuf {
        let sess = root.join(proj).join(uuid);
        std::fs::create_dir_all(&sess).unwrap();
        let path = sess.join("wire.jsonl");
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        path
    }

    const TOOL_FLOW: &[&str] = &[
        r#"{"type": "metadata", "protocol_version": "1.3"}"#,
        r#"{"timestamp": 1704067200.0, "message": {"type": "TurnBegin", "payload": {"user_input": [{"type": "text", "text": "Read the file"}]}}}"#,
        r#"{"timestamp": 1704067200.5, "message": {"type": "StepBegin", "payload": {}}}"#,
        r#"{"timestamp": 1704067201.0, "message": {"type": "ContentPart", "payload": {"type": "think", "think": "Let me plan.", "encrypted": null}}}"#,
        r#"{"timestamp": 1704067202.0, "message": {"type": "ToolCall", "payload": {"type": "function", "id": "tool_1", "function": {"name": "Glob", "arguments": "{\"pattern\": \"*.go\"}"}, "extras": null}}}"#,
        r#"{"timestamp": 1704067203.0, "message": {"type": "ToolResult", "payload": {"tool_call_id": "tool_1", "return_value": {"is_error": false, "output": "main.go\nutil.go"}}}}"#,
        r#"{"timestamp": 1704067204.0, "message": {"type": "ContentPart", "payload": {"type": "text", "text": "Found the files."}}}"#,
        r#"{"timestamp": 1704067204.5, "message": {"type": "StatusUpdate", "payload": {"context_tokens": 5000, "token_usage": {"output": 42}}}}"#,
        r#"{"timestamp": 1704067205.0, "message": {"type": "TurnEnd", "payload": {}}}"#,
    ];

    fn parse_tool_flow() -> ForeignSession {
        let dir = tempfile::tempdir().unwrap();
        let path = write_wire(dir.path(), "proj1", "sess1", TOOL_FLOW);
        let mut sessions = KimiProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        sessions.remove(0)
    }

    #[test]
    fn parses_wire_into_blocks() {
        let s = parse_tool_flow();
        assert_eq!(s.meta.id, "kimi:proj1:sess1");
        assert_eq!(s.meta.project, "proj1");
        assert_eq!(s.meta.first_message, "Read the file");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.malformed_lines, 0);
        // Envelope spans every timestamped record (TurnBegin → TurnEnd).
        assert_eq!(s.meta.started_at, Some(1704067200.0));
        assert_eq!(s.meta.ended_at, Some(1704067205.0));
        // Usage: output tokens summed, but NO model name exists in the
        // format → per_model stays empty (truthfully unpriceable).
        assert!(s.meta.usage.has_usage);
        assert_eq!(s.meta.usage.totals.output_tokens, 42);
        assert!(s.meta.usage.per_model.is_empty());
        assert!(s.meta.models.is_empty());
        // user, assistant(think+tool flushed by ToolResult), assistant(text).
        assert_eq!(s.blocks.len(), 3);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block")
        };
        assert_eq!(u.text, "Read the file");
        assert_eq!(u.timestamp, 1704067200.0);
    }

    #[test]
    fn turn_accumulator_flushes_on_tool_result() {
        let s = parse_tool_flow();
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        // Timestamp = first contentful part (the think at :01, not the
        // ToolCall at :02).
        assert_eq!(a.timestamp, Some(1704067201.0));
        assert_eq!(a.thinking.as_deref(), Some("Let me plan."));
        assert_eq!(a.segments.len(), 1);
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "Glob");
        // Double-parsed string-encoded arguments.
        assert_eq!(execution.tool_input, serde_json::json!({"pattern": "*.go"}));
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "main.go\nutil.go"
        );
        // The post-result text becomes its own assistant block.
        let ConversationBlock::Assistant(b) = &s.blocks[2] else {
            panic!("expected assistant block")
        };
        assert_eq!(b.timestamp, Some(1704067204.0));
        let AssistantSegment::Text { text, .. } = &b.segments[0] else {
            panic!("expected text segment")
        };
        assert_eq!(text, "Found the files.");
    }

    #[test]
    fn error_and_array_tool_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_wire(
            dir.path(),
            "p",
            "s",
            &[
                r#"{"timestamp": 1704067200.0, "message": {"type": "TurnBegin", "payload": {"user_input": [{"type": "text", "text": "Do something"}]}}}"#,
                r#"{"timestamp": 1704067201.0, "message": {"type": "ToolCall", "payload": {"id": "tool_err", "function": {"name": "Bash", "arguments": "{\"command\": \"exit 1\"}"}}}}"#,
                r#"{"timestamp": 1704067202.0, "message": {"type": "ToolResult", "payload": {"tool_call_id": "tool_err", "return_value": {"is_error": true, "output": ""}}}}"#,
                r#"{"timestamp": 1704067203.0, "message": {"type": "ToolCall", "payload": {"id": "tool_arr", "function": {"name": "Bash", "arguments": "{\"command\": \"echo hi\"}"}}}}"#,
                r#"{"timestamp": 1704067204.0, "message": {"type": "ToolResult", "payload": {"tool_call_id": "tool_arr", "return_value": {"is_error": false, "output": [{"type": "text", "text": "line one"}, {"type": "text", "text": "line two"}]}}}}"#,
                r#"{"timestamp": 1704067205.0, "message": {"type": "TurnEnd", "payload": {}}}"#,
            ],
        );
        let s = KimiProvider.parse(&path).unwrap().remove(0);
        assert_eq!(s.blocks.len(), 3); // user + two single-tool turns
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!()
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!()
        };
        assert_eq!(execution.status, ToolStatus::Error);
        assert_eq!(execution.result.as_ref().unwrap().output, "[error]");
        assert!(execution.result.as_ref().unwrap().is_error);
        let ConversationBlock::Assistant(b) = &s.blocks[2] else {
            panic!()
        };
        let AssistantSegment::Tool { execution } = &b.segments[0] else {
            panic!()
        };
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "line one\nline two"
        );
    }

    #[test]
    fn fractional_timestamps_and_zero_usage_presence() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_wire(
            dir.path(),
            "p",
            "s",
            &[
                r#"{"timestamp": 1704067200.25, "message": {"type": "TurnBegin", "payload": {"user_input": [{"type": "text", "text": "Hello"}]}}}"#,
                // Empty fragment must NOT pin the assistant timestamp.
                r#"{"timestamp": 1704067201.0, "message": {"type": "ContentPart", "payload": {"type": "text", "text": ""}}}"#,
                r#"{"timestamp": 1704067201.5, "message": {"type": "ContentPart", "payload": {"type": "text", "text": "Hi"}}}"#,
                r#"{"timestamp": 1704067201.75, "message": {"type": "StatusUpdate", "payload": {"context_tokens": 0, "token_usage": {"output": 0}}}}"#,
                r#"{"timestamp": 1704067202.5, "message": {"type": "TurnEnd", "payload": {}}}"#,
            ],
        );
        let s = KimiProvider.parse(&path).unwrap().remove(0);
        assert_eq!(s.meta.started_at, Some(1704067200.25));
        assert_eq!(s.meta.ended_at, Some(1704067202.5));
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!()
        };
        assert_eq!(a.timestamp, Some(1704067201.5));
        // A zero-valued StatusUpdate still means "this format reports
        // usage" — presence flag true, totals zero (Go parity).
        assert!(s.meta.usage.has_usage);
        assert_eq!(s.meta.usage.totals.output_tokens, 0);
    }

    #[test]
    fn empty_sessions_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        // Metadata only.
        let p1 = write_wire(
            dir.path(),
            "p",
            "s1",
            &[r#"{"type": "metadata", "protocol_version": "1.3"}"#],
        );
        assert!(KimiProvider.parse(&p1).unwrap().is_empty());
        // Empty user input + empty fragment → still no contentful message.
        let p2 = write_wire(
            dir.path(),
            "p",
            "s2",
            &[
                r#"{"timestamp": 1704067200.0, "message": {"type": "TurnBegin", "payload": {"user_input": []}}}"#,
                r#"{"timestamp": 1704067201.0, "message": {"type": "ContentPart", "payload": {"type": "text", "text": ""}}}"#,
                r#"{"timestamp": 1704067202.0, "message": {"type": "TurnEnd", "payload": {}}}"#,
            ],
        );
        assert!(KimiProvider.parse(&p2).unwrap().is_empty());
    }

    #[test]
    fn malformed_lines_are_counted_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_wire(
            dir.path(),
            "p",
            "s",
            &[
                r#"{"timestamp": 1704067200.0, "message": {"type": "TurnBegin", "payload": {"user_input": [{"type": "text", "text": "Hello"}]}}}"#,
                "this is not json",
                r#"{"timestamp": 1704067201.0, "message": {"type": "ContentPart", "payload": {"type": "text", "text": "Hi"}}}"#,
                r#"{"timestamp": 1704067202.0, "message": {"type": "TurnEnd", "payload": {}}}"#,
            ],
        );
        let s = KimiProvider.parse(&path).unwrap().remove(0);
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.message_count, 2);
    }

    #[test]
    fn discover_walks_two_levels() {
        let dir = tempfile::tempdir().unwrap();
        write_wire(dir.path(), "abc123", "uuid-1", &[r#"{"type":"metadata"}"#]);
        write_wire(dir.path(), "abc123", "uuid-2", &[r#"{"type":"metadata"}"#]);
        // Noise: top-level file, session dir without wire.jsonl, stray file
        // inside a project dir.
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("abc123").join("uuid-3")).unwrap();
        std::fs::write(dir.path().join("abc123").join("stray.json"), "{}").unwrap();
        let found = KimiProvider.discover(dir.path());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "kimi:abc123:uuid-1");
        assert_eq!(found[1].id, "kimi:abc123:uuid-2");
        assert_eq!(found[0].project_hint.as_deref(), Some("abc123"));
        assert!(found[0].path.ends_with("abc123/uuid-1/wire.jsonl"));
        // Empty/nonexistent roots discover nothing.
        assert!(KimiProvider.discover(Path::new("/nonexistent")).is_empty());
    }
}
