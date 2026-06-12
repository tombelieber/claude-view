// crates/providers/src/parsers/pi.rs
//
// Pi (pi-agent) — JSONL sessions at `<root>/<encoded-cwd>/<session-id>.jsonl`.
//
// Format (ported from agentsview's pi.go):
//   line 1: header { type:"session", version, id, timestamp ISO8601, cwd,
//           branchedFrom } — V1 files lack `id` (derive from filename stem).
//   then entries type ∈ { message, model_change, compaction,
//           thinking_level_change } with message.role ∈
//           { user, assistant, toolResult }. Content is a plain string OR a
//           block array: {type:text,text} | {type:thinking, thinking,
//           thinkingSignature, redacted} | {type:toolCall, id, name,
//           arguments}. toolResult entries carry message.toolCallId and
//           attach to the matching tool segment.
//
// Usage rides each assistant message as flat {input, output} plus cache
// counts in TWO transport shapes (nested cache.{read,write} or flat
// cacheRead/cacheCreation). Already Anthropic-shaped — no cache subtraction.
// The embedded usage.cost.total is deliberately ignored: we re-price from
// tokens. The encoded-cwd dir-name format is ambiguous across pi versions,
// so discovery validates candidates by their header line instead.

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub struct PiProvider;

impl Provider for PiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Pi
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(entries) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let cwd_dir = entry.path();
            // is_dir() follows symlinks — matches Go's isDirOrSymlink.
            if !cwd_dir.is_dir() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&cwd_dir) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if path.is_dir()
                    || path.extension().and_then(|e| e.to_str()) != Some("jsonl")
                    || !is_pi_session_file(&path)
                {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let id = ProviderKind::Pi.session_id(stem);
                let Some((mtime, size_bytes)) = stat_entry(&path) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id,
                    provider: ProviderKind::Pi,
                    path,
                    // Project intentionally absent: parse derives it from the
                    // header cwd (the dir name encoding is version-ambiguous).
                    project_hint: None,
                    mtime,
                    size_bytes,
                });
            }
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let read = jsonl::read_values(path)?;
        let header = read
            .values
            .first()
            .filter(|v| v.get("type").and_then(Value::as_str) == Some("session"))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "not a pi session: missing session header in {}",
                    path.display()
                )
            })?;

        // V2 headers carry the id; V1 files derive it from the filename stem.
        let raw_id = header
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
            })
            .ok_or_else(|| anyhow::anyhow!("no pi session id for {}", path.display()))?;

        let mut meta = ForeignSessionMeta::new(ProviderKind::Pi, &raw_id, path.to_path_buf());
        meta.malformed_lines = read.malformed;
        if let Some(cwd) = header
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            meta.project = project_from_cwd(cwd);
            meta.cwd = Some(cwd.to_string());
        }
        if meta.project.is_empty() {
            meta.project = "pi".to_string();
        }
        if let Some(ts) = header
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|s| time::parse_timestamp(s, false))
        {
            meta.observe_timestamp(ts);
        }
        // header.branchedFrom (fork lineage) is intentionally skipped:
        // ForeignSessionMeta carries no parent-session field.

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        let mut current_model = String::new();
        for (ordinal, entry) in read.values.iter().enumerate().skip(1) {
            let id = blocks::block_id(&raw_id, ordinal);
            match entry.get("type").and_then(Value::as_str) {
                Some("message") => {
                    let ts = entry_timestamp(entry);
                    if let Some(t) = ts {
                        meta.observe_timestamp(t);
                    }
                    match entry.pointer("/message/role").and_then(Value::as_str) {
                        Some("user") => handle_user(&mut out_blocks, &mut meta, id, entry, ts),
                        Some("assistant") => handle_assistant(
                            &mut out_blocks,
                            &mut meta,
                            id,
                            entry,
                            ts,
                            &mut current_model,
                        ),
                        Some("toolResult") => handle_tool_result(&mut out_blocks, entry),
                        _ => {}
                    }
                }
                Some("model_change") => {
                    if let Some(m) = entry
                        .get("modelId")
                        .and_then(Value::as_str)
                        .filter(|m| !m.is_empty())
                    {
                        current_model = m.to_string();
                    }
                }
                Some("compaction") => {
                    let summary = entry
                        .get("summary")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    out_blocks.push(blocks::compaction_notice(id, summary));
                }
                // thinking_level_change + unknown future entry types: skip.
                _ => {}
            }
        }

        // Header-only / bookkeeping-only files are non-interactive noise.
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

/// Cheap header probe: the first non-blank line must be JSON with
/// `"type":"session"` (mirrors Go's IsPiSessionFile). Capped at 256 KiB —
/// real headers are one short line.
fn is_pi_session_file(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut reader = BufReader::new(file.take(256 * 1024));
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return false,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                return serde_json::from_str::<Value>(trimmed)
                    .ok()
                    .map(|v| v.get("type").and_then(Value::as_str) == Some("session"))
                    .unwrap_or(false);
            }
        }
    }
}

/// Entry timestamp: top-level ISO8601 `timestamp`, falling back to the inner
/// `message.timestamp` unix-millis field (Go: piTimestamp).
fn entry_timestamp(entry: &Value) -> Option<f64> {
    if let Some(ts) = entry
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|s| time::parse_timestamp(s, false))
    {
        return Some(ts);
    }
    entry
        .pointer("/message/timestamp")
        .and_then(Value::as_f64)
        .filter(|ms| *ms > 0.0)
        .map(time::from_millis)
}

/// User entries always count — image-only or empty payloads are real turns
/// (Go counts them too); only text content is rendered, never fabricated.
fn handle_user(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    id: String,
    entry: &Value,
    ts: Option<f64>,
) {
    let text = match entry.pointer("/message/content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    if meta.first_message.is_empty() && !text.is_empty() {
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
    entry: &Value,
    ts: Option<f64>,
    current_model: &mut String,
) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut thinking_parts: Vec<String> = Vec::new();
    match entry.pointer("/message/content") {
        // Plain string content (back-compat format variation).
        Some(Value::String(s)) => {
            if !s.is_empty() {
                segments.push(blocks::text_segment(s.clone()));
            }
        }
        Some(Value::Array(items)) => {
            for block in items {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(Value::as_str) {
                            if !t.is_empty() {
                                segments.push(blocks::text_segment(t.to_string()));
                            }
                        }
                    }
                    Some("thinking") => {
                        // Redacted blocks carry an empty `thinking` field —
                        // emit nothing for them (no fabrication).
                        if let Some(t) = block.get("thinking").and_then(Value::as_str) {
                            if !t.is_empty() {
                                thinking_parts.push(t.to_string());
                            }
                        }
                    }
                    Some("toolCall") => {
                        let tool_id = block.get("id").and_then(Value::as_str).unwrap_or("");
                        if tool_id.is_empty() {
                            continue;
                        }
                        let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
                        let args = normalize_intent(
                            block.get("arguments").cloned().unwrap_or(Value::Null),
                        );
                        segments.push(blocks::tool_segment(
                            name.to_string(),
                            args,
                            tool_id.to_string(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    // Model: inline message.model wins and becomes the new running model;
    // otherwise inherit the most recent model_change / assistant model.
    if let Some(m) = entry
        .pointer("/message/model")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
    {
        *current_model = m.to_string();
    }
    meta.record_model(current_model);
    if let Some(totals) = extract_usage(entry.pointer("/message/usage")) {
        meta.usage.record(current_model, totals);
    }

    meta.message_count += 1;
    let thinking = (!thinking_parts.is_empty()).then(|| thinking_parts.join("\n\n"));
    out.push(blocks::assistant(id, segments, thinking, ts));
}

/// toolResult entries attach to the matching tool segment; they are not
/// separate blocks and do not count as messages (crate convention — the Go
/// source counts them as RoleUser rows).
fn handle_tool_result(out: &mut [ConversationBlock], entry: &Value) {
    let Some(tool_call_id) = entry
        .pointer("/message/toolCallId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let output = match entry.pointer("/message/content") {
        Some(Value::String(s)) => s.clone(),
        // Go's DecodeContent joins text-block parts with "".
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    };
    let is_error = entry
        .pointer("/message/isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    blocks::attach_tool_result(out, tool_call_id, output, is_error);
}

/// Pi usage is already Anthropic-shaped (input excludes cache reads — the Go
/// parser maps fields 1:1 with no subtraction). Cache counts arrive in two
/// transport shapes: nested cache.{read,write} (OpenCode-style) or flat
/// cacheRead/cacheCreation (Anthropic-style) — try both. A usage object with
/// none of the known keys (e.g. only totalTokens) yields None so we never
/// fabricate a zero record; explicit zeros are preserved as "known zero".
fn extract_usage(usage: Option<&Value>) -> Option<UsageTotals> {
    let usage = usage?;
    let input = usage.get("input");
    let output = usage.get("output");
    let cache_read = usage
        .pointer("/cache/read")
        .or_else(|| usage.get("cacheRead"));
    let cache_write = usage
        .pointer("/cache/write")
        .or_else(|| usage.get("cacheCreation"));
    if input.is_none() && output.is_none() && cache_read.is_none() && cache_write.is_none() {
        return None;
    }
    Some(UsageTotals {
        input_tokens: tokens(input),
        output_tokens: tokens(output),
        cache_read_input_tokens: tokens(cache_read),
        cache_creation_input_tokens: tokens(cache_write),
    })
}

fn tokens(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_f64)
        .map(|f| f.max(0.0) as u64)
        .unwrap_or(0)
}

/// Rename Pi's tool-arg intent field (`agent__intent`, fallback `_i`) to
/// `description` so the UI's uniform params.description check works. An
/// existing `description` is never overwritten (intent keys then stay put);
/// when one is promoted, both intent keys are removed (Go: normalizePiIntent).
fn normalize_intent(args: Value) -> Value {
    match args {
        Value::Object(mut map) => {
            if !map.contains_key("description") {
                let primary = map.remove("agent__intent");
                let fallback = map.remove("_i");
                if let Some(v) = primary.or(fallback) {
                    map.insert("description".to_string(), v);
                }
            }
            Value::Object(map)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::{NoticeVariant, ToolStatus};

    // Mirrors agentsview testdata/pi/session.jsonl: header with branchedFrom,
    // thinking + toolCall + usage(+cost), toolResult, model_change,
    // compaction, redacted thinking, thinking_level_change, a malformed line,
    // and an unknown future entry type.
    const FIXTURE: &str = concat!(
        r#"{"type":"session","version":3,"id":"pi-test-session-uuid","timestamp":"2025-01-01T10:00:00Z","cwd":"/Users/alice/code/my-project","branchedFrom":"/Users/alice/.pi/agent/sessions/--path--/2025-01-01T09-00-00-000Z_parent-uuid.jsonl"}"#,
        "\n",
        r#"{"type":"message","id":"entry-1","timestamp":"2025-01-01T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"Fix the login bug"}],"timestamp":1735725601000}}"#,
        "\n",
        r#"{"type":"message","id":"entry-2","timestamp":"2025-01-01T10:00:02Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me analyze this carefully.","thinkingSignature":"sig-abc","redacted":false},{"type":"text","text":"Looking at the auth module."},{"type":"toolCall","id":"toolu_01","name":"read","arguments":{"file_path":"auth.go","agent__intent":"Read auth"}}],"model":"claude-opus-4-5","usage":{"input":100,"output":50,"totalTokens":150,"cost":{"total":0.01}},"timestamp":1735725602000}}"#,
        "\n",
        r#"{"type":"message","id":"entry-3","timestamp":"2025-01-01T10:00:03Z","message":{"role":"toolResult","toolCallId":"toolu_01","toolName":"read","content":[{"type":"text","text":"package auth\nfunc Login() {}"}],"isError":false,"timestamp":1735725603000}}"#,
        "\n",
        r#"{"type":"model_change","id":"entry-4","timestamp":"2025-01-01T10:00:04Z","provider":"anthropic","modelId":"claude-opus-4-5"}"#,
        "\n",
        r##"{"type":"compaction","id":"entry-5","timestamp":"2025-01-01T10:00:05Z","summary":"# Context Checkpoint","firstKeptEntryIndex":0,"tokensBefore":5000}"##,
        "\n",
        r#"{"type":"message","id":"entry-6","timestamp":"2025-01-01T10:00:06Z","message":{"role":"user","content":[{"type":"text","text":"Look good to you?"}],"timestamp":1735725606000}}"#,
        "\n",
        r#"{"type":"message","id":"entry-7","timestamp":"2025-01-01T10:00:07Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"","thinkingSignature":"redacted-sig","redacted":true},{"type":"text","text":"Looks good!"}],"model":"claude-opus-4-5","usage":{"input":200,"output":10,"cache":{"read":30,"write":7},"cost":{"total":0.005}},"timestamp":1735725607000}}"#,
        "\n",
        r#"{"type":"thinking_level_change","id":"entry-8","timestamp":"2025-01-01T10:00:08Z","thinkingLevel":"high"}"#,
        "\n",
        "not valid json -- malformed line that should be counted\n",
        r#"{"type":"unknown_future_entry_type","id":"entry-9","timestamp":"2025-01-01T10:00:09Z"}"#,
        "\n",
    );

    fn parse_str(name: &str, content: &str) -> Vec<ForeignSession> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        PiProvider.parse(&path).unwrap()
    }

    fn parse_fixture() -> ForeignSession {
        let mut sessions = parse_str("pi-test-session-uuid.jsonl", FIXTURE);
        assert_eq!(sessions.len(), 1);
        sessions.remove(0)
    }

    #[test]
    fn parses_full_session() {
        let s = parse_fixture();
        assert_eq!(s.meta.id, "pi:pi-test-session-uuid");
        assert_eq!(s.meta.project, "my-project");
        assert_eq!(s.meta.cwd.as_deref(), Some("/Users/alice/code/my-project"));
        assert_eq!(s.meta.first_message, "Fix the login bug");
        assert_eq!(s.meta.user_message_count, 2);
        assert_eq!(s.meta.message_count, 4, "toolResult entries do not count");
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.models, vec!["claude-opus-4-5".to_string()]);
        // started from header ts, ended from the last message entry.
        assert_eq!(s.meta.started_at, Some(1735725600.0));
        assert_eq!(s.meta.ended_at, Some(1735725607.0));
        // user, assistant, compaction notice, user, assistant.
        assert_eq!(s.blocks.len(), 5);
        let ConversationBlock::Notice(n) = &s.blocks[2] else {
            panic!("expected compaction notice")
        };
        assert_eq!(n.variant, NoticeVariant::ContextCompacted);
        assert_eq!(n.data["summary"], "# Context Checkpoint");
        // Usage: both shapes summed; cost.total ignored entirely.
        assert!(s.meta.usage.has_usage);
        let t = s.meta.usage.totals;
        assert_eq!(t.input_tokens, 300);
        assert_eq!(t.output_tokens, 60);
        assert_eq!(t.cache_read_input_tokens, 30);
        assert_eq!(t.cache_creation_input_tokens, 7);
        assert_eq!(s.meta.usage.per_model["claude-opus-4-5"].input_tokens, 300);
    }

    #[test]
    fn tool_call_attaches_result_and_renames_intent() {
        let s = parse_fixture();
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(
            a.thinking.as_deref(),
            Some("Let me analyze this carefully.")
        );
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "read");
        assert_eq!(execution.tool_use_id, "toolu_01");
        // agent__intent renamed to description; original key removed.
        assert_eq!(execution.tool_input["description"], "Read auth");
        assert_eq!(execution.tool_input["file_path"], "auth.go");
        assert!(execution.tool_input.get("agent__intent").is_none());
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "package auth\nfunc Login() {}"
        );
        // Redacted thinking on the second assistant: no fabricated text.
        let ConversationBlock::Assistant(a2) = &s.blocks[4] else {
            panic!("expected assistant block")
        };
        assert_eq!(a2.thinking, None);
    }

    #[test]
    fn model_fallback_and_usage_shape_gates() {
        let content = concat!(
            r#"{"type":"session","id":"mc-sess","timestamp":"2025-01-01T10:00:00Z","cwd":"/tmp/proj"}"#,
            "\n",
            r#"{"type":"model_change","id":"mc1","timestamp":"2025-01-01T10:00:00.5Z","provider":"openai","modelId":"gpt-5.4"}"#,
            "\n",
            // No inline model → inherits gpt-5.4 from model_change.
            r#"{"type":"message","id":"a1","timestamp":"2025-01-01T10:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"ok"}],"usage":{"input":10,"output":5,"cacheRead":3,"cacheCreation":2}}}"#,
            "\n",
            // Unknown usage shape → no record fabricated.
            r#"{"type":"message","id":"a2","timestamp":"2025-01-01T10:00:03Z","message":{"role":"assistant","content":"plain string response","usage":{"totalTokens":42,"promptCount":3}}}"#,
            "\n",
            // Explicit zero usage → "known zero", still recorded.
            r#"{"type":"message","id":"a3","timestamp":"2025-01-01T10:00:04Z","message":{"role":"assistant","content":[{"type":"text","text":"zero"}],"usage":{"input":0,"output":0}}}"#,
            "\n",
        );
        let s = parse_str("mc-sess.jsonl", content).remove(0);
        assert_eq!(s.meta.models, vec!["gpt-5.4".to_string()]);
        assert!(s.meta.usage.has_usage);
        let t = s.meta.usage.per_model["gpt-5.4"];
        assert_eq!(t.input_tokens, 10);
        assert_eq!(t.output_tokens, 5);
        assert_eq!(t.cache_read_input_tokens, 3);
        assert_eq!(t.cache_creation_input_tokens, 2);
        // Plain-string assistant content becomes a text segment.
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        let AssistantSegment::Text { text, .. } = &a.segments[0] else {
            panic!("expected text segment")
        };
        assert_eq!(text, "plain string response");
        assert_eq!(s.meta.message_count, 3);
    }

    #[test]
    fn v1_id_from_filename_and_image_only_user_counts() {
        let content = concat!(
            r#"{"type":"session","timestamp":"2025-01-01T10:00:00Z","cwd":"/Users/alice/code/v1-project"}"#,
            "\n",
            r#"{"type":"message","timestamp":"2025-01-01T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"hello"}]}}"#,
            "\n",
            r#"{"type":"message","timestamp":"2025-01-01T10:00:02Z","message":{"role":"user","content":[{"type":"image","source":{"data":"abc"}}]}}"#,
            "\n",
        );
        let s = parse_str("v1-session.jsonl", content).remove(0);
        assert_eq!(
            s.meta.id, "pi:v1-session",
            "V1 id derives from filename stem"
        );
        // Image-only user message still counts as a real turn (Go parity)
        // but its text stays empty — nothing fabricated.
        assert_eq!(s.meta.user_message_count, 2);
        let ConversationBlock::User(u) = &s.blocks[1] else {
            panic!("expected user block")
        };
        assert!(u.text.is_empty());
        assert_eq!(s.meta.first_message, "hello");
    }

    #[test]
    fn non_interactive_and_invalid_files() {
        // Header + bookkeeping only → zero contentful messages → skipped.
        let content = concat!(
            r#"{"type":"session","id":"empty","timestamp":"2025-01-01T10:00:00Z","cwd":"/tmp"}"#,
            "\n",
            r#"{"type":"model_change","id":"mc1","modelId":"gpt-5.4"}"#,
            "\n",
            r#"{"type":"compaction","id":"c1","summary":"checkpoint"}"#,
            "\n",
        );
        assert!(parse_str("empty.jsonl", content).is_empty());
        // Missing session header → error, not a pi session.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-pi.jsonl");
        std::fs::write(
            &path,
            r#"{"type":"message","message":{"role":"user","content":"hi"}}"#,
        )
        .unwrap();
        assert!(PiProvider.parse(&path).is_err());
        let empty = dir.path().join("zero.jsonl");
        std::fs::write(&empty, "").unwrap();
        assert!(PiProvider.parse(&empty).is_err());
    }

    #[test]
    fn discover_validates_header_line() {
        let dir = tempfile::tempdir().unwrap();
        let cwd_dir = dir.path().join("--Users-alice-code-my-project--");
        std::fs::create_dir_all(&cwd_dir).unwrap();
        std::fs::write(cwd_dir.join("sess-1.jsonl"), FIXTURE).unwrap();
        // Valid JSON but not a session header → rejected.
        std::fs::write(cwd_dir.join("not-pi.jsonl"), "{\"type\":\"message\"}\n").unwrap();
        // Wrong extension → rejected.
        std::fs::write(cwd_dir.join("notes.txt"), "{\"type\":\"session\"}\n").unwrap();
        // Files directly under the root (not in a cwd dir) → ignored.
        std::fs::write(dir.path().join("stray.jsonl"), FIXTURE).unwrap();
        let found = PiProvider.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "pi:sess-1");
        assert_eq!(found[0].provider, ProviderKind::Pi);
        assert!(found[0].project_hint.is_none());
    }

    #[test]
    fn normalize_intent_rules() {
        // _i fallback when agent__intent absent.
        let v = normalize_intent(serde_json::json!({"command":"pwd","_i":"Show dir"}));
        assert_eq!(
            v,
            serde_json::json!({"command":"pwd","description":"Show dir"})
        );
        // agent__intent preferred over _i; both removed.
        let v = normalize_intent(
            serde_json::json!({"command":"ls","agent__intent":"Primary","_i":"Fallback"}),
        );
        assert_eq!(
            v,
            serde_json::json!({"command":"ls","description":"Primary"})
        );
        // Existing description never overwritten; intent keys kept.
        let v = normalize_intent(
            serde_json::json!({"description":"Already set","agent__intent":"Ignored"}),
        );
        assert_eq!(
            v,
            serde_json::json!({"description":"Already set","agent__intent":"Ignored"})
        );
        // Non-object arguments pass through untouched.
        assert_eq!(normalize_intent(Value::Null), Value::Null);
    }
}
