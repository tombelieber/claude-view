// crates/providers/src/parsers/codex.rs
//
// Codex CLI — JSONL rollout files under `~/.codex/sessions` (dated
// `YYYY/MM/DD` subdirs) and `~/.codex/archived_sessions` (flat). Files are
// named `rollout-<ts>-<uuid>.jsonl`; the trailing UUID is the raw session id
// at discovery time, while `session_meta.payload.id` is authoritative once
// parsed (they match in real data).
//
// Line shape: `{timestamp, type, payload}` with type one of
//   session_meta   → payload.id / cwd / git.branch / originator
//   turn_context   → payload.model (last one wins as current model)
//   response_item  → payload.type: message | function_call |
//                    function_call_output
//   event_msg      → payload.type: token_count | task_* | turn_aborted | …
//
// Ported quirks (from agentsview's codex.go):
//   * token_count dedup by JSON equality of `info.last_token_usage`
//     (streaming repeats the same count), then backwards attachment to the
//     most recent usage-less assistant message, stopping at a user boundary.
//   * turn_aborted first-prompt replay dedup: after a turn_aborted signal
//     Codex re-emits the initial user prompt verbatim — drop only that
//     positively-identified replay (full-content match, never the preview).
//   * system-message filters for injected user content (# AGENTS.md,
//     <environment_context>, <INSTRUCTIONS>, <skill>, <turn_aborted>,
//     <subagent_notification>).
//   * function_call arguments arrive as an object OR string-encoded JSON —
//     double-parse; bare `*** Begin Patch` envelopes wrap as `{patch}`.
// The Go subagent spawn/wait state machine is intentionally NOT ported:
// spawn_agent / wait_agent render as ordinary tool segments and
// collab_agent_spawn_end is ignored.

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::ConversationBlock;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct CodexProvider;

impl Provider for CodexProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Codex
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let mut out = Vec::new();
        // Flat layout (archived_sessions, or stray files at the root).
        collect_session_files(root, &mut out);
        // Dated layout: <root>/YYYY/MM/DD/rollout-*.jsonl.
        for year in digit_dirs(root) {
            for month in digit_dirs(&year) {
                for day in digit_dirs(&month) {
                    collect_session_files(&day, &mut out);
                }
            }
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let read = jsonl::read_values(path)?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        let file_id = uuid_from_stem(stem).unwrap_or(stem);
        let mut b = Builder::new(file_id, path);
        b.meta.malformed_lines = read.malformed;
        for line in &read.values {
            b.process_line(line);
        }
        Ok(b.finish())
    }
}

fn collect_session_files(dir: &Path, out: &mut Vec<DiscoveredSession>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".jsonl") else {
            continue;
        };
        if !stem.starts_with("rollout-") {
            continue;
        }
        let raw = uuid_from_stem(stem).unwrap_or(stem);
        let id = ProviderKind::Codex.session_id(raw);
        let Some((mtime, size_bytes)) = stat_entry(&path) else {
            continue;
        };
        out.push(DiscoveredSession {
            id,
            provider: ProviderKind::Codex,
            path,
            project_hint: None,
            mtime,
            size_bytes,
        });
    }
}

/// Subdirectories whose names are all digits (year/month/day components).
fn digit_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(is_digits) && e.path().is_dir())
        .map(|e| e.path())
        .collect()
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Extract the trailing UUID from a `rollout-<ts>-<uuid>` stem. Mirrors the
/// Go regex `^rollout-.*-(8-4-4-4-12 hex)$` — the char before the UUID must
/// be a `-` that is not part of the `rollout-` prefix itself.
fn uuid_from_stem(stem: &str) -> Option<&str> {
    let head_len = stem.len().checked_sub(36)?;
    if !stem.is_char_boundary(head_len) {
        return None;
    }
    let (head, uuid) = stem.split_at(head_len);
    if head.len() < 9 || !head.starts_with("rollout-") || !head.ends_with('-') {
        return None;
    }
    is_uuid(uuid).then_some(uuid)
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Which role occupies each emitted block, for the backwards token-count
/// walk. `model` is the turn_context model current when the assistant
/// message was emitted (load-bearing for per-model pricing).
enum Slot {
    User,
    Assistant { model: String, has_usage: bool },
}

struct Builder {
    meta: ForeignSessionMeta,
    blocks: Vec<ConversationBlock>,
    slots: Vec<Slot>,
    /// Filename-derived id used for stable block ids (known before any
    /// session_meta line is seen).
    block_seed: String,
    /// Authoritative raw id from session_meta (empty until seen).
    raw_id: String,
    current_model: String,
    first_user_content: String,
    saw_user_turn_after_first: bool,
    may_replay_first_prompt: bool,
    /// Last `info.last_token_usage` object — streaming re-emits identical
    /// counts which must be deduplicated.
    last_usage: Option<Value>,
}

impl Builder {
    fn new(file_id: &str, path: &Path) -> Self {
        let mut meta = ForeignSessionMeta::new(ProviderKind::Codex, file_id, path.to_path_buf());
        meta.project = "unknown".to_string();
        Self {
            meta,
            blocks: Vec::new(),
            slots: Vec::new(),
            block_seed: file_id.to_string(),
            raw_id: String::new(),
            current_model: String::new(),
            first_user_content: String::new(),
            saw_user_turn_after_first: false,
            may_replay_first_prompt: false,
            last_usage: None,
        }
    }

    fn finish(self) -> Vec<ForeignSession> {
        // Sessions with zero contentful messages are non-interactive noise.
        if self.meta.message_count == 0 {
            return Vec::new();
        }
        // The session id stays the FILENAME-derived id from discovery —
        // overriding it with session_meta.payload.id would orphan the
        // catalog row (lookup-by-id misses) and defeat the stats cache
        // whenever the two differ. payload.id was still validated during
        // parsing; it just isn't the identity.
        vec![ForeignSession {
            meta: self.meta,
            blocks: self.blocks,
        }]
    }

    fn process_line(&mut self, line: &Value) {
        let ts = line
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|s| time::parse_timestamp(s, false));
        if let Some(t) = ts {
            self.meta.observe_timestamp(t);
        }
        let Some(payload) = line.get("payload") else {
            return;
        };
        match str_field(line, "type") {
            "session_meta" => self.handle_session_meta(payload),
            // Every turn_context overwrites the model, empty included.
            "turn_context" => self.current_model = str_field(payload, "model").to_string(),
            "response_item" => self.handle_response_item(payload, ts),
            "event_msg" => self.handle_event_msg(payload),
            _ => {}
        }
    }

    fn handle_session_meta(&mut self, payload: &Value) {
        // Later session_meta lines overwrite earlier ones (Go behavior).
        self.raw_id = str_field(payload, "id").to_string();
        let cwd = str_field(payload, "cwd");
        if cwd.is_empty() {
            return;
        }
        self.meta.cwd = Some(cwd.to_string());
        let branch = payload
            .pointer("/git/branch")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !branch.is_empty() {
            self.meta.git_branch = Some(branch.to_string());
        }
        self.meta.project = project_name(cwd);
    }

    fn handle_response_item(&mut self, payload: &Value, ts: Option<f64>) {
        match str_field(payload, "type") {
            "function_call" => self.handle_function_call(payload, ts),
            "function_call_output" => self.handle_function_call_output(payload),
            _ => self.handle_message(payload, ts),
        }
    }

    fn handle_message(&mut self, payload: &Value, ts: Option<f64>) {
        let role = str_field(payload, "role");
        if role != "user" && role != "assistant" {
            return;
        }
        let content = extract_content(payload);
        if content.trim().is_empty() {
            return;
        }
        if role == "user" {
            self.handle_user(content, ts);
        } else {
            let id = self.next_block_id();
            self.blocks.push(blocks::assistant(
                id,
                vec![blocks::text_segment(content)],
                None,
                ts,
            ));
            self.push_assistant_slot();
        }
    }

    fn handle_user(&mut self, content: String, ts: Option<f64>) {
        // The <turn_aborted> marker both arms the replay dedup AND is
        // itself filtered as system content — order matters.
        if is_turn_aborted_message(&content) {
            self.mark_replay_possible();
        }
        if is_system_message(&content) {
            return;
        }
        if self.first_user_content.is_empty() {
            self.first_user_content.clone_from(&content);
            self.meta.first_message = preview(&content, 200);
        } else if content == self.first_user_content {
            if !self.saw_user_turn_after_first && self.may_replay_first_prompt {
                // Codex re-emits the initial prompt verbatim when it
                // continues after a turn_aborted. Drop only that
                // positively-identified replay; an identical second prompt
                // without the signal is real transcript content.
                self.may_replay_first_prompt = false;
                return;
            }
            self.saw_user_turn_after_first = true;
            self.may_replay_first_prompt = false;
        } else {
            self.saw_user_turn_after_first = true;
            self.may_replay_first_prompt = false;
        }
        let id = self.next_block_id();
        self.blocks.push(blocks::user(id, content, ts));
        self.slots.push(Slot::User);
        self.meta.message_count += 1;
        self.meta.user_message_count += 1;
    }

    fn handle_function_call(&mut self, payload: &Value, ts: Option<f64>) {
        let name = str_field(payload, "name");
        if name.is_empty() {
            return;
        }
        let call_id = str_field(payload, "call_id").to_string();
        let input = function_args(name, payload);
        let id = self.next_block_id();
        self.blocks.push(blocks::assistant(
            id,
            vec![blocks::tool_segment(name.to_string(), input, call_id)],
            None,
            ts,
        ));
        self.push_assistant_slot();
    }

    fn handle_function_call_output(&mut self, payload: &Value) {
        let call_id = str_field(payload, "call_id");
        if call_id.is_empty() {
            return;
        }
        let Some(parsed) = payload.get("output").and_then(double_parse) else {
            return;
        };
        let (text, is_error) = render_output(&parsed);
        blocks::attach_tool_result(&mut self.blocks, call_id, text, is_error);
    }

    fn handle_event_msg(&mut self, payload: &Value) {
        match str_field(payload, "type") {
            "turn_aborted" => self.mark_replay_possible(),
            "token_count" => self.handle_token_count(payload),
            // task_started / task_complete only drive termination
            // classification in the Go source (not modeled here);
            // collab_agent_spawn_end belongs to the skipped subagent
            // state machine.
            _ => {}
        }
    }

    fn handle_token_count(&mut self, payload: &Value) {
        let Some(usage) = payload.pointer("/info/last_token_usage") else {
            return;
        };
        if !usage.is_object() {
            return;
        }
        // Streaming repeats the same count — dedup by JSON equality.
        if self.last_usage.as_ref() == Some(usage) {
            return;
        }
        self.last_usage = Some(usage.clone());
        // Attach backwards to the most recent usage-less assistant message
        // in the current turn; a user boundary means the count belongs to
        // no message we kept, so it is dropped (matches the Go source).
        let mut attach_model = None;
        for slot in self.slots.iter_mut().rev() {
            match slot {
                Slot::User => break,
                Slot::Assistant { has_usage, model } => {
                    if *has_usage {
                        continue;
                    }
                    *has_usage = true;
                    attach_model = Some(model.clone());
                    break;
                }
            }
        }
        let Some(model) = attach_model else {
            return;
        };
        self.meta.usage.record(&model, normalize_usage(usage));
    }

    fn mark_replay_possible(&mut self) {
        if self.first_user_content.is_empty() || self.saw_user_turn_after_first {
            return;
        }
        self.may_replay_first_prompt = true;
    }

    fn push_assistant_slot(&mut self) {
        self.meta.record_model(&self.current_model);
        self.slots.push(Slot::Assistant {
            model: self.current_model.clone(),
            has_usage: false,
        });
        self.meta.message_count += 1;
    }

    fn next_block_id(&self) -> String {
        blocks::block_id(&self.block_seed, self.blocks.len())
    }
}

fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn project_name(cwd: &str) -> String {
    let p = project_from_cwd(cwd);
    let p = p.trim();
    if p.is_empty() || p == "/" || p == "." || p == ".." {
        "unknown".to_string()
    } else {
        p.to_string()
    }
}

/// Join all text blocks (`input_text` / `output_text` / `text`) from a
/// response item's content array.
fn extract_content(payload: &Value) -> String {
    let mut texts: Vec<&str> = Vec::new();
    for block in payload
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if matches!(
            str_field(block, "type"),
            "input_text" | "output_text" | "text"
        ) {
            let t = str_field(block, "text");
            if !t.is_empty() {
                texts.push(t);
            }
        }
    }
    texts.join("\n")
}

/// Codex-injected user content that must not count as a user turn.
fn is_system_message(content: &str) -> bool {
    let trimmed = content.trim_start();
    content.starts_with("# AGENTS.md")
        || content.starts_with("<environment_context>")
        || content.starts_with("<INSTRUCTIONS>")
        || trimmed.starts_with("<skill>")
        || trimmed.starts_with("<turn_aborted>")
        || trimmed.starts_with("<subagent_notification>")
}

fn is_turn_aborted_message(content: &str) -> bool {
    content.trim_start().starts_with("<turn_aborted>")
}

/// Function-call arguments arrive as an object OR a string-encoded JSON
/// document — double-parse strings. `apply_patch` envelopes that arrive as
/// a bare `*** Begin Patch` string wrap as `{patch}` so tool input stays
/// structured. Checks `arguments` then `input`, skipping empty containers.
fn function_args(name: &str, payload: &Value) -> Value {
    let mut args = Value::Null;
    for key in ["arguments", "input"] {
        let Some(arg) = payload.get(key) else {
            continue;
        };
        match arg {
            Value::String(s) => {
                let t = s.trim();
                if t.is_empty() {
                    continue;
                }
                args = match serde_json::from_str::<Value>(t) {
                    Ok(v) if is_empty_container(&v) => continue,
                    Ok(v) => v,
                    // Non-JSON string (e.g. a raw patch envelope).
                    Err(_) => Value::String(t.to_string()),
                };
            }
            Value::Null => continue,
            other => {
                if is_empty_container(other) {
                    continue;
                }
                args = other.clone();
            }
        }
        break;
    }
    if name == "apply_patch" {
        if let Value::String(s) = &args {
            if s.contains("*** Begin Patch") {
                return serde_json::json!({ "patch": s });
            }
        }
    }
    args
}

fn is_empty_container(v: &Value) -> bool {
    match v {
        Value::Object(o) => o.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// Tool outputs are frequently string-encoded JSON — parse one level.
/// Returns `None` for absent/empty output (nothing to attach).
fn double_parse(v: &Value) -> Option<Value> {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                return None;
            }
            Some(serde_json::from_str(t).unwrap_or_else(|_| Value::String(t.to_string())))
        }
        Value::Null => None,
        other => Some(other.clone()),
    }
}

/// Codex wraps shell output as `{"output": "...", "metadata":
/// {"exit_code": N, …}}`. Surface the inner text and map a non-zero exit
/// code to an error result; everything else passes through as text /
/// compact JSON.
fn render_output(v: &Value) -> (String, bool) {
    match v {
        Value::String(s) => (s.clone(), false),
        Value::Object(map) => {
            if let Some(text) = map.get("output").and_then(Value::as_str) {
                let is_error = map
                    .get("metadata")
                    .and_then(|m| m.get("exit_code"))
                    .and_then(Value::as_i64)
                    .is_some_and(|c| c != 0);
                return (text.to_string(), is_error);
            }
            (v.to_string(), false)
        }
        other => (other.to_string(), false),
    }
}

/// Normalize OpenAI-style usage into Anthropic-shape buckets:
/// `input_tokens` INCLUDES the cached portion, so the uncached remainder is
/// `max(total - cached, 0)`; `cached_input_tokens` maps to
/// `cache_read_input_tokens`. No cache-creation accounting exists.
fn normalize_usage(usage: &Value) -> UsageTotals {
    let n = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let total_input = n("input_tokens");
    let cached = n("cached_input_tokens");
    UsageTotals {
        input_tokens: total_input.saturating_sub(cached),
        output_tokens: n("output_tokens"),
        cache_read_input_tokens: cached,
        cache_creation_input_tokens: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::{AssistantSegment, ToolStatus};

    const HAPPY: &str = r#"{"timestamp":"2026-01-02T03:04:05Z","type":"session_meta","payload":{"id":"abc-123","cwd":"/Users/alice/code/my-api","originator":"user","git":{"branch":"main"}}}
{"timestamp":"2026-01-02T03:04:06Z","type":"turn_context","payload":{"model":"gpt-5.2-codex"}}
{"timestamp":"2026-01-02T03:04:07Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Add rate limiting"}]}}
{"timestamp":"2026-01-02T03:04:08Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"I'll add rate limiting."}]}}
{"timestamp":"2026-01-02T03:04:09Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"call_1","arguments":"{\"cmd\":\"rg --files\"}"}}
{"timestamp":"2026-01-02T03:04:10Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_1","output":"{\"output\":\"a.rs\\nb.rs\",\"metadata\":{\"exit_code\":0}}"}}
{"timestamp":"2026-01-02T03:04:11Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10000,"cached_input_tokens":6000,"output_tokens":500}}}}
"#;

    fn parse_str(name: &str, content: &str) -> Vec<ForeignSession> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        CodexProvider.parse(&path).unwrap()
    }

    fn tool_execution(block: &ConversationBlock) -> &claude_view_types::block_types::ToolExecution {
        let ConversationBlock::Assistant(a) = block else {
            panic!("expected assistant block")
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        execution
    }

    #[test]
    fn parses_session_with_usage_and_tools() {
        let mut sessions = parse_str(
            "rollout-2026-01-02T03-04-05-0196fdb4-1234-4abc-8def-0123456789ab.jsonl",
            HAPPY,
        );
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        // Identity stays the FILENAME-derived id — discovery and parse must
        // agree or the catalog row is orphaned (session hidden, cache
        // defeated). session_meta payload.id is metadata, not identity.
        assert_eq!(s.meta.id, "codex:0196fdb4-1234-4abc-8def-0123456789ab");
        assert_eq!(s.meta.project, "my-api");
        assert_eq!(s.meta.cwd.as_deref(), Some("/Users/alice/code/my-api"));
        assert_eq!(s.meta.git_branch.as_deref(), Some("main"));
        assert_eq!(s.meta.first_message, "Add rate limiting");
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.models, vec!["gpt-5.2-codex".to_string()]);
        assert_eq!(s.meta.started_at, Some(1767323045.0));
        assert_eq!(s.meta.ended_at, Some(1767323051.0));
        // OpenAI-style totals include cached reads: input = 10000-6000.
        assert!(s.meta.usage.has_usage);
        assert_eq!(s.meta.usage.totals.input_tokens, 4000);
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 6000);
        assert_eq!(s.meta.usage.totals.output_tokens, 500);
        assert_eq!(s.meta.usage.per_model["gpt-5.2-codex"].input_tokens, 4000);
        // Blocks: user, assistant text, function_call w/ attached result.
        assert_eq!(s.blocks.len(), 3);
        let exec = tool_execution(&s.blocks[2]);
        assert_eq!(exec.tool_name, "exec_command");
        // String-encoded arguments were double-parsed into an object.
        assert_eq!(exec.tool_input, serde_json::json!({"cmd": "rg --files"}));
        assert_eq!(exec.status, ToolStatus::Complete);
        assert_eq!(exec.result.as_ref().unwrap().output, "a.rs\nb.rs");
    }

    #[test]
    fn token_count_streaming_duplicates_deduplicated() {
        let content = r#"{"timestamp":"2026-01-02T03:04:05Z","type":"session_meta","payload":{"id":"tu-2","cwd":"/tmp"}}
{"timestamp":"2026-01-02T03:04:06Z","type":"turn_context","payload":{"model":"gpt-5.4"}}
{"timestamp":"2026-01-02T03:04:07Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}
{"timestamp":"2026-01-02T03:04:08Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}}
{"timestamp":"2026-01-02T03:04:09Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10000,"cached_input_tokens":6000,"output_tokens":500}}}}
{"timestamp":"2026-01-02T03:04:09Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10000,"cached_input_tokens":6000,"output_tokens":500}}}}
{"timestamp":"2026-01-02T03:04:09Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10000,"cached_input_tokens":6000,"output_tokens":500}}}}
{"timestamp":"2026-01-02T03:05:00Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"think more"}]}}
{"timestamp":"2026-01-02T03:05:05Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"deep thought"}]}}
{"timestamp":"2026-01-02T03:05:06Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":20000,"cached_input_tokens":12000,"output_tokens":800}}}}
"#;
        let s = &parse_str("rollout-x.jsonl", content)[0];
        // 3 identical counts collapse to one; the second turn adds its own:
        // input (10000-6000) + (20000-12000), cached 6000+12000, out 500+800.
        assert_eq!(s.meta.usage.totals.input_tokens, 12000);
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 18000);
        assert_eq!(s.meta.usage.totals.output_tokens, 1300);
        assert_eq!(s.meta.usage.per_model["gpt-5.4"].output_tokens, 1300);
    }

    #[test]
    fn turn_aborted_replay_of_first_prompt_is_dropped() {
        let prompt = "You are a code reviewer. Review the changes below.";
        let content = format!(
            concat!(
                r#"{{"timestamp":"2026-01-02T03:04:05Z","type":"session_meta","payload":{{"id":"rev","cwd":"/tmp"}}}}"#,
                "\n",
                r#"{{"timestamp":"2026-01-02T03:04:06Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"{p}"}}]}}}}"#,
                "\n",
                r#"{{"timestamp":"2026-01-02T03:04:07Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"<turn_aborted>\ninterrupted"}}]}}}}"#,
                "\n",
                r#"{{"timestamp":"2026-01-02T03:04:08Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"{p}"}}]}}}}"#,
                "\n",
                r#"{{"timestamp":"2026-01-02T03:04:09Z","type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"No issues found."}}]}}}}"#,
                "\n",
            ),
            p = prompt,
        );
        let s = &parse_str("rollout-x.jsonl", &content)[0];
        assert_eq!(
            s.meta.user_message_count, 1,
            "re-emitted prompt after turn_aborted must not count as a second user turn"
        );
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.blocks.len(), 2);
        assert!(
            !s.meta.usage.has_usage,
            "no token_count → has_usage stays false"
        );
    }

    #[test]
    fn repeated_prompt_without_replay_signal_is_kept() {
        let content = r#"{"timestamp":"2026-01-02T03:04:05Z","type":"session_meta","payload":{"id":"rev2","cwd":"/tmp"}}
{"timestamp":"2026-01-02T03:04:06Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"same prompt"}]}}
{"timestamp":"2026-01-02T03:04:07Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"looking"}]}}
{"timestamp":"2026-01-02T03:04:08Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"same prompt"}]}}
{"timestamp":"2026-01-02T03:04:09Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}
"#;
        let s = &parse_str("rollout-x.jsonl", content)[0];
        assert_eq!(
            s.meta.user_message_count, 2,
            "an identical second prompt without a turn_aborted signal is real content"
        );
        assert_eq!(s.meta.message_count, 4);
    }

    #[test]
    fn system_injected_user_messages_are_filtered() {
        let content = r##"{"timestamp":"2026-01-02T03:04:05Z","type":"session_meta","payload":{"id":"sys","cwd":"/tmp"}}
{"timestamp":"2026-01-02T03:04:06Z","type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"# AGENTS.md\nsome instructions"}]}}
{"timestamp":"2026-01-02T03:04:07Z","type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"<environment_context>stuff</environment_context>"}]}}
{"timestamp":"2026-01-02T03:04:08Z","type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"<INSTRUCTIONS>ignore</INSTRUCTIONS>"}]}}
{"timestamp":"2026-01-02T03:04:09Z","type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"<skill>\n  <name>roborev:fix</name>"}]}}
{"timestamp":"2026-01-02T03:04:10Z","type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"<subagent_notification>{\"agent_id\":\"a1\"}</subagent_notification>"}]}}
not json at all
{"timestamp":"2026-01-02T03:04:11Z","type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"Actual user message"}]}}
"##;
        let s = &parse_str("rollout-x.jsonl", content)[0];
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.first_message, "Actual user message");
        assert_eq!(s.meta.malformed_lines, 1);
    }

    #[test]
    fn sessions_without_contentful_messages_are_skipped() {
        let content = r##"{"timestamp":"2026-01-02T03:04:05Z","type":"session_meta","payload":{"id":"empty","cwd":"/tmp"}}
{"timestamp":"2026-01-02T03:04:06Z","type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"# AGENTS.md\nonly injected content"}]}}
"##;
        assert!(parse_str("rollout-x.jsonl", content).is_empty());
    }

    #[test]
    fn apply_patch_envelopes_become_patch_input() {
        let content = r#"{"timestamp":"2026-01-02T03:04:05Z","type":"session_meta","payload":{"id":"patch","cwd":"/tmp"}}
{"timestamp":"2026-01-02T03:04:06Z","type":"response_item","payload":{"type":"function_call","name":"apply_patch","call_id":"p1","arguments":"{\"patch\":\"*** Begin Patch\\n*** Update File: a.rs\\n*** End Patch\"}"}}
{"timestamp":"2026-01-02T03:04:07Z","type":"response_item","payload":{"type":"function_call","name":"apply_patch","call_id":"p2","arguments":"*** Begin Patch\n*** End Patch"}}
{"timestamp":"2026-01-02T03:04:08Z","type":"response_item","payload":{"type":"function_call_output","call_id":"p2","output":"{\"output\":\"patch failed\",\"metadata\":{\"exit_code\":1}}"}}
"#;
        let s = &parse_str("rollout-x.jsonl", content)[0];
        assert_eq!(s.blocks.len(), 2);
        // String-encoded JSON arguments double-parse into the object.
        let first = tool_execution(&s.blocks[0]);
        assert_eq!(
            first.tool_input,
            serde_json::json!({"patch": "*** Begin Patch\n*** Update File: a.rs\n*** End Patch"})
        );
        // A bare patch-envelope string wraps as {patch: <string>}.
        let second = tool_execution(&s.blocks[1]);
        assert_eq!(
            second.tool_input,
            serde_json::json!({"patch": "*** Begin Patch\n*** End Patch"})
        );
        // Non-zero exit_code in the wrapped output maps to an error result.
        assert_eq!(second.status, ToolStatus::Error);
        assert_eq!(second.result.as_ref().unwrap().output, "patch failed");
    }

    #[test]
    fn discover_handles_dated_and_flat_layouts() {
        let dir = tempfile::tempdir().unwrap();
        let day = dir.path().join("2026").join("01").join("02");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(
            day.join("rollout-2026-01-02T03-04-05-0196fdb4-1234-4abc-8def-0123456789ab.jsonl"),
            HAPPY,
        )
        .unwrap();
        // Flat (archived_sessions-style) file directly under the root.
        std::fs::write(
            dir.path()
                .join("rollout-2026-01-03T00-00-00-aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeffff.jsonl"),
            HAPPY,
        )
        .unwrap();
        // Non-UUID rollout file falls back to the full stem.
        std::fs::write(dir.path().join("rollout-notes.jsonl"), HAPPY).unwrap();
        // Ignored: wrong prefix / extension.
        std::fs::write(dir.path().join("notes.jsonl"), "{}").unwrap();
        std::fs::write(day.join("rollout-x.txt"), "nope").unwrap();

        let mut ids: Vec<String> = CodexProvider
            .discover(dir.path())
            .into_iter()
            .map(|d| d.id)
            .collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "codex:0196fdb4-1234-4abc-8def-0123456789ab".to_string(),
                "codex:aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeffff".to_string(),
                "codex:rollout-notes".to_string(),
            ]
        );
    }
}
