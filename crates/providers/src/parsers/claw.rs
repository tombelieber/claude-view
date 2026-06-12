// crates/providers/src/parsers/claw.rs
//
// OpenClaw + QClaw — two claw-gateway forks with byte-identical session
// formats. ONE parser parameterized by ProviderKind; only the root
// (~/.openclaw/agents vs ~/.qclaw/agents) and the id prefix differ.
//
// Layout: <root>/<agentId>/sessions/<sessionId>.jsonl — the raw session id
// is "<agentId>:<sessionId>" because the same uuid can exist under different
// agents. Archive suffixes are first-class: .jsonl.deleted.<ts>,
// .jsonl.reset.<ts> and .jsonl.full.bak all parse; discovery dedups per
// logical session id, preferring the active .jsonl, else the NEWEST archive
// (by the timestamp embedded in the filename, mtime as a last resort).
//
// JSONL entries (ported from agentsview openclaw.go / qclaw.go):
//   {type:"session", id, cwd}                              — header
//   {type:"message", message:{role, content, model, usage, toolCallId}}
//     role ∈ {user, assistant, toolResult}; content is a Claude-style block
//     array (text / thinking / tool_use / tool_result) or a plain string
//   {type:"model_change"|"thinking_level_change"|"custom"} — skipped
//   {type:"compaction", summary}                           — compaction notice
// Assistant usage rides message.usage with short keys (input, output,
// cacheRead, cacheWrite) — already Anthropic-shaped, no cache subtraction.
// message.usage.cost.total is deliberately IGNORED: claude-view re-prices
// downstream from the model id (the load-bearing field).

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ClawProvider {
    kind: ProviderKind,
}

pub static OPENCLAW: ClawProvider = ClawProvider {
    kind: ProviderKind::Openclaw,
};
pub static QCLAW: ClawProvider = ClawProvider {
    kind: ProviderKind::Qclaw,
};

impl Provider for ClawProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(agents) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for agent in agents.flatten() {
            let agent_name = agent.file_name();
            let Some(agent_name) = agent_name.to_str() else {
                continue;
            };
            if !is_valid_id(agent_name) {
                continue;
            }
            let sessions_dir = root.join(agent_name).join("sessions");
            let Ok(entries) = std::fs::read_dir(&sessions_dir) else {
                continue;
            };
            // Dedup by logical session id: active .jsonl wins, else newest
            // archive (deleted/reset/full.bak).
            let mut best: HashMap<String, PathBuf> = HashMap::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    continue;
                }
                let name = file_name_str(&path);
                if !is_session_file(name) {
                    continue;
                }
                match best.entry(logical_session_id(name).to_string()) {
                    Entry::Occupied(mut o) => {
                        if replaces(o.get(), &path) {
                            o.insert(path);
                        }
                    }
                    Entry::Vacant(v) => {
                        v.insert(path);
                    }
                }
            }
            for (sid, path) in best {
                let Some((mtime, size_bytes)) = stat_entry(&path) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id: self.kind.session_id(&format!("{agent_name}:{sid}")),
                    provider: self.kind,
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

        // Header pass: first session id + first cwd seen.
        let mut session_id = String::new();
        let mut cwd = String::new();
        for entry in &read.values {
            if entry.get("type").and_then(Value::as_str) != Some("session") {
                continue;
            }
            if session_id.is_empty() {
                if let Some(id) = entry.get("id").and_then(Value::as_str) {
                    session_id = id.to_string();
                }
            }
            if cwd.is_empty() {
                if let Some(c) = entry.get("cwd").and_then(Value::as_str) {
                    cwd = c.to_string();
                }
            }
            if !session_id.is_empty() && !cwd.is_empty() {
                break;
            }
        }
        if session_id.is_empty() {
            session_id = logical_session_id(file_name_str(path)).to_string();
        }
        let raw_id = format!("{}:{}", agent_id_from_path(path), session_id);

        let mut meta = ForeignSessionMeta::new(self.kind, &raw_id, path.to_path_buf());
        meta.malformed_lines = read.malformed;
        if !cwd.is_empty() {
            meta.project = project_from_cwd(&cwd);
            meta.cwd = Some(cwd);
        }
        if meta.project.is_empty() {
            meta.project = self.kind.as_str().to_string();
        }

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        for entry in &read.values {
            let entry_ts = entry
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false));
            // Session bounds widen from timestamps on ALL entry types.
            if let Some(ts) = entry_ts {
                meta.observe_timestamp(ts);
            }
            match entry.get("type").and_then(Value::as_str) {
                Some("message") => {}
                Some("compaction") => {
                    let summary = entry
                        .get("summary")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    let id = blocks::block_id(&raw_id, out_blocks.len());
                    out_blocks.push(blocks::compaction_notice(id, summary));
                    continue;
                }
                // session header, model_change, thinking_level_change,
                // custom, unknown — metadata only.
                _ => continue,
            }
            let Some(msg) = entry.get("message") else {
                continue;
            };
            let ts = msg
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false))
                .or(entry_ts);
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }
            match msg.get("role").and_then(Value::as_str) {
                Some("user") => handle_user(&mut out_blocks, &mut meta, &raw_id, msg, ts),
                Some("assistant") => handle_assistant(&mut out_blocks, &mut meta, &raw_id, msg, ts),
                Some("toolResult") => {
                    // Tool results are separate messages keyed by toolCallId.
                    let tool_call_id = msg.get("toolCallId").and_then(Value::as_str).unwrap_or("");
                    if tool_call_id.is_empty() {
                        continue; // orphan result — drop, never fabricate
                    }
                    let output = result_text(msg.get("content"));
                    let is_error = msg.get("isError").and_then(Value::as_bool).unwrap_or(false);
                    blocks::attach_tool_result(&mut out_blocks, tool_call_id, output, is_error);
                }
                _ => {}
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

fn handle_user(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    raw_id: &str,
    msg: &Value,
    ts: Option<f64>,
) {
    let mut text_parts: Vec<&str> = Vec::new();
    match msg.get("content") {
        Some(Value::String(s)) if !s.trim().is_empty() => text_parts.push(s),
        Some(Value::Array(items)) => {
            for block in items {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(Value::as_str) {
                            if !t.trim().is_empty() {
                                text_parts.push(t);
                            }
                        }
                    }
                    Some("tool_result") => {
                        // Claude-style inline result (rare in claw; the
                        // dedicated toolResult role is the usual carrier).
                        let tool_use_id = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if !tool_use_id.is_empty() {
                            blocks::attach_tool_result(
                                out,
                                tool_use_id,
                                result_text(block.get("content")),
                                false,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    let text = text_parts.join("\n").trim().to_string();
    if text.is_empty() {
        return;
    }
    if meta.first_message.is_empty() {
        meta.first_message = preview(strip_date_prefix(&text), 200);
    }
    meta.message_count += 1;
    meta.user_message_count += 1;
    let id = blocks::block_id(raw_id, out.len());
    out.push(blocks::user(id, text, ts));
}

fn handle_assistant(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    raw_id: &str,
    msg: &Value,
    ts: Option<f64>,
) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut thinking_parts: Vec<&str> = Vec::new();
    match msg.get("content") {
        Some(Value::String(s)) if !s.trim().is_empty() => {
            segments.push(blocks::text_segment(s.clone()));
        }
        Some(Value::Array(items)) => {
            for block in items {
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
                                thinking_parts.push(t);
                            }
                        }
                    }
                    Some("tool_use") => {
                        let tool_id = block.get("id").and_then(Value::as_str).unwrap_or("");
                        if tool_id.is_empty() {
                            continue;
                        }
                        let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
                        segments.push(blocks::tool_segment(
                            name.to_string(),
                            block.get("input").cloned().unwrap_or(Value::Null),
                            tool_id.to_string(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    if segments.is_empty() && thinking_parts.is_empty() {
        return;
    }
    // Model id passes through verbatim — load-bearing for pricing.
    let model = msg.get("model").and_then(Value::as_str).unwrap_or("");
    meta.record_model(model);
    if let Some(totals) = extract_usage(msg) {
        meta.usage.record(model, totals);
    }
    meta.message_count += 1;
    let thinking = (!thinking_parts.is_empty()).then(|| thinking_parts.join("\n\n"));
    let id = blocks::block_id(raw_id, out.len());
    out.push(blocks::assistant(id, segments, thinking, ts));
}

/// Map the gateway's short usage keys onto Anthropic-shape buckets. The
/// counts are already Anthropic-style (input excludes cache reads), so no
/// subtraction applies. Returns `None` when no token field exists at all —
/// `has_usage` stays truthful for usage-less older sessions.
fn extract_usage(msg: &Value) -> Option<UsageTotals> {
    let usage = msg.get("usage")?;
    let mut present = false;
    let mut field = |key: &str| -> u64 {
        let Some(v) = usage.get(key) else { return 0 };
        present = true;
        v.as_u64().unwrap_or(0)
    };
    let totals = UsageTotals {
        input_tokens: field("input"),
        output_tokens: field("output"),
        cache_read_input_tokens: field("cacheRead"),
        cache_creation_input_tokens: field("cacheWrite"),
    };
    present.then_some(totals)
}

/// Plain text from a toolResult content field (string or block array).
fn result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Strip the gateway-injected date prefix ("[Wed 2026-02-18 11:21 GMT+1] …")
/// that claw gateways prepend to messages arriving via Telegram/channels.
fn strip_date_prefix(s: &str) -> &str {
    if !s.starts_with('[') {
        return s;
    }
    match s.find("] ") {
        Some(idx) if idx <= 40 => s[idx + 2..].trim(),
        _ => s,
    }
}

/// Agent id = grandparent dir name (<root>/<agentId>/sessions/<file>).
fn agent_id_from_path(path: &Path) -> String {
    path.parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn is_valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Active session files plus the known archive suffixes.
fn is_session_file(name: &str) -> bool {
    if name.ends_with(".jsonl") {
        return true;
    }
    let Some(idx) = name.find(".jsonl.") else {
        return false;
    };
    if idx == 0 {
        return false;
    }
    let suffix = &name[idx + ".jsonl.".len()..];
    suffix.starts_with("deleted.") || suffix.starts_with("reset.") || suffix == "full.bak"
}

/// Logical session id = filename up to the first ".jsonl" (strips any
/// archive suffix): "abc.jsonl.deleted.<ts>" → "abc".
fn logical_session_id(name: &str) -> &str {
    match name.find(".jsonl") {
        Some(idx) if idx > 0 => &name[..idx],
        _ => name.strip_suffix(".jsonl").unwrap_or(name),
    }
}

/// True when `candidate` should replace `current` for the same logical
/// session id: active .jsonl always wins; among archives the newest embedded
/// timestamp wins; mtime breaks ties when no timestamp parses.
fn replaces(current: &Path, candidate: &Path) -> bool {
    let cur_name = file_name_str(current);
    let cand_name = file_name_str(candidate);
    let cur_active = cur_name.ends_with(".jsonl");
    let cand_active = cand_name.ends_with(".jsonl");
    if cur_active != cand_active {
        return cand_active;
    }
    match (archive_time(cur_name), archive_time(cand_name)) {
        (Some(cur), Some(cand)) => cand > cur,
        (Some(_), None) => false,
        (None, Some(_)) => true,
        (None, None) => match (file_mtime(current), file_mtime(candidate)) {
            (Some(cur), Some(cand)) => cand > cur,
            _ => false,
        },
    }
}

/// Timestamp embedded in an archive suffix, e.g.
/// ".jsonl.deleted.2026-02-19T08-59-24.951Z" (dashes in the time portion).
fn archive_time(name: &str) -> Option<f64> {
    let idx = name.find(".jsonl.")?;
    if idx == 0 {
        return None;
    }
    let suffix = &name[idx + ".jsonl.".len()..];
    let (_, ts) = suffix.split_once('.')?;
    let normalized = match ts.split_once('T') {
        // Time portion uses dashes (08-59-24) — restore the first two colons.
        Some((date, t)) => format!("{date}T{}", t.replacen('-', ":", 2)),
        None => ts.to_string(),
    };
    time::parse_timestamp(&normalized, false)
}

fn file_name_str(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

fn file_mtime(path: &Path) -> Option<f64> {
    stat_entry(path).map(|(mtime, _)| mtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::{NoticeVariant, ToolStatus};

    const FIXTURE_LINES: &[&str] = &[
        r#"{"type":"session","version":3,"id":"abc-123","timestamp":"2026-02-25T10:00:00Z","cwd":"/home/user/project"}"#,
        r#"{"type":"model_change","id":"mc1","timestamp":"2026-02-25T10:00:00Z","provider":"anthropic","modelId":"claude-sonnet-4-6"}"#,
        r#"{"type":"message","id":"m1","timestamp":"2026-02-25T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"[Wed 2026-02-18 11:21 GMT+1] Read the hosts file"}],"timestamp":"2026-02-25T10:00:01Z"}}"#,
        r#"{"type":"message","id":"m2","timestamp":"2026-02-25T10:00:02Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"need to read"},{"type":"tool_use","id":"tu1","name":"read","input":{"path":"/etc/hosts"}}],"timestamp":"2026-02-25T10:00:02Z","model":"claude-sonnet-4-6","usage":{"input":3,"output":91,"cacheRead":5,"cacheWrite":9612,"totalTokens":9711,"cost":{"input":0.000009,"output":0.001365,"total":0.037419}}}}"#,
        r#"{"type":"message","id":"m3","timestamp":"2026-02-25T10:00:03Z","message":{"role":"toolResult","toolCallId":"tu1","toolName":"read","content":[{"type":"text","text":"127.0.0.1 localhost"}],"isError":false,"timestamp":"2026-02-25T10:00:03Z"}}"#,
        r#"{"type":"compaction","id":"c1","timestamp":"2026-02-25T10:00:04Z","summary":"earlier work"}"#,
        r#"{"type":"message","id":"m4","timestamp":"2026-02-25T10:00:05Z","message":{"role":"assistant","content":[{"type":"text","text":"The hosts file maps localhost."}],"timestamp":"2026-02-25T10:00:05Z","model":"claude-sonnet-4-6","usage":{"input":10,"output":4}}}"#,
    ];

    /// Write lines under <root>/<agent>/sessions/<name> and return the path.
    fn write_session(root: &Path, agent: &str, name: &str, lines: &[&str]) -> PathBuf {
        let dir = root.join(agent).join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
        path
    }

    #[test]
    fn parses_session_into_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_session(dir.path(), "main", "abc-123.jsonl", FIXTURE_LINES);
        let mut sessions = OPENCLAW.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);

        assert_eq!(s.meta.id, "openclaw:main:abc-123");
        assert_eq!(s.meta.provider, ProviderKind::Openclaw);
        assert_eq!(s.meta.project, "project");
        assert_eq!(s.meta.cwd.as_deref(), Some("/home/user/project"));
        // Gateway date prefix stripped from the title preview.
        assert_eq!(s.meta.first_message, "Read the hosts file");
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.models, vec!["claude-sonnet-4-6"]);
        assert_eq!(s.meta.malformed_lines, 0);
        // Bounds span ALL entry types: header 10:00:00 … last message 10:00:05.
        assert_eq!(s.meta.started_at, Some(1772013600.0));
        assert_eq!(s.meta.ended_at, Some(1772013605.0));

        // Usage: short keys mapped 1:1; cost.total ignored.
        assert!(s.meta.usage.has_usage);
        assert_eq!(s.meta.usage.totals.input_tokens, 13);
        assert_eq!(s.meta.usage.totals.output_tokens, 95);
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 5);
        assert_eq!(s.meta.usage.totals.cache_creation_input_tokens, 9612);
        assert_eq!(
            s.meta.usage.per_model["claude-sonnet-4-6"].output_tokens,
            95
        );

        // user, assistant(tool), compaction notice, assistant(text).
        assert_eq!(s.blocks.len(), 4);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block")
        };
        // Block text keeps the raw gateway prefix — only the title strips it.
        assert!(u.text.starts_with("[Wed 2026-02-18 11:21 GMT+1] "));
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("need to read"));
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "read");
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "127.0.0.1 localhost"
        );
        let ConversationBlock::Notice(n) = &s.blocks[2] else {
            panic!("expected compaction notice")
        };
        assert_eq!(n.variant, NoticeVariant::ContextCompacted);
        assert_eq!(n.data["summary"], "earlier work");
    }

    #[test]
    fn discovery_dedups_archives_and_spans_agents() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Same session under two agents — both must surface, distinct ids.
        write_session(root, "main", "abc.jsonl", FIXTURE_LINES);
        write_session(root, "claude", "abc.jsonl", FIXTURE_LINES);
        // Archives of main:abc lose to the active file.
        write_session(
            root,
            "main",
            "abc.jsonl.deleted.2026-02-19T08-59-24.951Z",
            FIXTURE_LINES,
        );
        write_session(
            root,
            "main",
            "abc.jsonl.reset.2026-02-17T09-39-39.691Z",
            FIXTURE_LINES,
        );
        // Archive-only session: newest embedded timestamp wins even when the
        // suffix family differs (reset/March beats deleted/January).
        write_session(
            root,
            "main",
            "xyz.jsonl.deleted.2026-01-15T00-00-00.000Z",
            FIXTURE_LINES,
        );
        write_session(
            root,
            "main",
            "xyz.jsonl.reset.2026-03-01T00-00-00.000Z",
            FIXTURE_LINES,
        );
        // Noise: non-session files and an invalid agent dir name.
        write_session(root, "main", "sessions.json", &["{}"]);
        write_session(root, "main", "abc.jsonl.tmp", &["{}"]);
        write_session(root, "bad agent", "zzz.jsonl", FIXTURE_LINES);

        let found = OPENCLAW.discover(root);
        let mut ids: Vec<&str> = found.iter().map(|d| d.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            vec![
                "openclaw:claude:abc",
                "openclaw:main:abc",
                "openclaw:main:xyz"
            ]
        );
        let abc = found.iter().find(|d| d.id == "openclaw:main:abc").unwrap();
        assert!(
            abc.path.ends_with("main/sessions/abc.jsonl"),
            "active file wins"
        );
        let xyz = found.iter().find(|d| d.id == "openclaw:main:xyz").unwrap();
        assert_eq!(
            file_name_str(&xyz.path),
            "xyz.jsonl.reset.2026-03-01T00-00-00.000Z"
        );
    }

    #[test]
    fn empty_and_orphan_only_sessions_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        // Header only — no contentful messages.
        let p1 = write_session(
            dir.path(),
            "main",
            "empty.jsonl",
            &[
                r#"{"type":"session","version":3,"id":"empty","timestamp":"2026-02-25T10:00:00Z","cwd":"/tmp"}"#,
            ],
        );
        assert!(OPENCLAW.parse(&p1).unwrap().is_empty());
        // Orphan toolResult (empty toolCallId) is not contentful either.
        let p2 = write_session(
            dir.path(),
            "main",
            "orphan.jsonl",
            &[
                r#"{"type":"session","version":3,"id":"orphan","timestamp":"2026-02-25T10:00:00Z","cwd":"/tmp"}"#,
                r#"{"type":"message","id":"m1","timestamp":"2026-02-25T10:00:01Z","message":{"role":"toolResult","toolCallId":"","content":[{"type":"text","text":"orphan"}],"timestamp":"2026-02-25T10:00:01Z"}}"#,
            ],
        );
        assert!(OPENCLAW.parse(&p2).unwrap().is_empty());
    }

    #[test]
    fn qclaw_static_shares_the_parser() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_session(dir.path(), "main", "abc-123.jsonl", FIXTURE_LINES);
        let sessions = QCLAW.parse(&path).unwrap();
        assert_eq!(sessions[0].meta.id, "qclaw:main:abc-123");
        assert_eq!(sessions[0].meta.provider, ProviderKind::Qclaw);
        let found = QCLAW.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "qclaw:main:abc-123");
    }

    #[test]
    fn malformed_lines_counted_and_no_usage_stays_truthful() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_session(
            dir.path(),
            "main",
            "mixed.jsonl",
            &[
                r#"{"type":"session","version":3,"id":"mixed","timestamp":"2026-02-25T10:00:00Z"}"#,
                "this is not json",
                // String content (not a block array) is tolerated.
                r#"{"type":"message","id":"m1","timestamp":"2026-02-25T10:00:01Z","message":{"role":"user","content":"plain string prompt","timestamp":"2026-02-25T10:00:01Z"}}"#,
                r#"{"type":"message","id":"m2","timestamp":"2026-02-25T10:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"reply, no usage block"}],"timestamp":"2026-02-25T10:00:02Z"}}"#,
            ],
        );
        let sessions = OPENCLAW.parse(&path).unwrap();
        let meta = &sessions[0].meta;
        assert_eq!(meta.malformed_lines, 1);
        assert_eq!(meta.first_message, "plain string prompt");
        assert_eq!(meta.message_count, 2);
        // No cwd header → fallback project; no usage → has_usage stays false.
        assert_eq!(meta.project, "openclaw");
        assert!(
            !meta.usage.has_usage,
            "usage-less session must not claim usage"
        );
    }

    #[test]
    fn date_prefix_strip_matches_go_semantics() {
        assert_eq!(
            strip_date_prefix("[Wed 2026-02-18 11:21 GMT+1] hello"),
            "hello"
        );
        assert_eq!(strip_date_prefix("no prefix"), "no prefix");
        assert_eq!(strip_date_prefix("[short] x"), "x");
        assert_eq!(strip_date_prefix("[unclosed bracket"), "[unclosed bracket");
        // Prefix longer than 40 bytes is left alone.
        let long = format!("[{}] tail", "x".repeat(45));
        assert_eq!(strip_date_prefix(&long), long.as_str());
    }
}
