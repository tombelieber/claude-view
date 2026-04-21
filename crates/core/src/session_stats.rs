//! On-demand session stats extraction from JSONL files.
//!
//! Reads a session's JSONL file and computes all metrics that v1 stored
//! in the SQLite mirror. Called by v2 route handlers on each request.
//!
//! Performance: Benchmarked at 0.28ms for p95 session (813KB). This is
//! fast enough for on-demand reads — no caching layer needed yet.
//!
//! Key design choice: assistant messages appear multiple times in JSONL
//! as content streams in. Each duplicate carries cumulative usage. We
//! take the **last** occurrence of each message ID (final token counts).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::jsonl_reader;
use crate::pricing::TokenUsage;

/// All metrics extractable from a single session JSONL file.
///
/// Fields mirror `SessionInfo` from `claude_view_types` but are computed
/// on-the-fly rather than stored in SQLite. Fields that require external
/// data (git branch, commit count, classification) are not included —
/// those are layered on by the route handler.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    // ── Token counts (summed across unique messages, last occurrence wins) ──
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_creation_5m_tokens: u64,
    pub cache_creation_1hr_tokens: u64,

    // ── Counts ──
    pub turn_count: u32,
    pub user_prompt_count: u32,
    pub line_count: u32,
    pub tool_call_count: u32,
    pub thinking_block_count: u32,
    pub api_error_count: u32,

    // ── Tool breakdown ──
    pub files_read_count: u32,
    pub files_edited_count: u32,
    pub bash_count: u32,
    pub agent_spawn_count: u32,

    // ── Timestamps ──
    pub first_message_at: Option<String>,
    pub last_message_at: Option<String>,
    pub duration_seconds: u32,

    // ── Model ──
    pub primary_model: Option<String>,

    // ── Git metadata (from JSONL first line `gitBranch` field) ──
    /// Branch recorded by Claude Code when the session started. Populated
    /// by the first JSONL line that carries a non-empty `gitBranch`
    /// attribute. Sessions with no git context return `None`, which maps
    /// to the `NO_BRANCH_SENTINEL = "~"` semantics used by the branch filter.
    pub git_branch: Option<String>,

    // ── Text content (truncated) ──
    pub preview: String,
    pub last_message: String,

    // ── Per-model token breakdown (for accurate cost calculation) ──
    pub per_model_tokens: HashMap<String, TokenUsage>,

    // ── Per-invocable invocation counts (CQRS Phase 6.2 retirement of `invocations`) ──
    //
    // Key is the raw tool_use `name` with optional `:sub` suffix:
    // - built-in / MCP tools → `"Bash"`, `"Read"`, `"mcp__plugin_X__tool_Y"`
    // - skills (`Skill` tool) → `"Skill:<input.skill>"` (e.g. `"Skill:commit"`)
    // - agents (`Task`/`Agent` tool) → `"Task:<input.subagent_type>"` /
    //   `"Agent:<input.subagent_type>"` (e.g. `"Task:general-purpose"`)
    //
    // Readers filter by prefix to emulate the legacy `invocables.kind`
    // grouping; the `invocables` registry is no longer part of the read
    // path. Writer populates this during parse via `compute_stats`.
    pub invocation_counts: HashMap<String, u64>,
}

// ── JSONL deserialization types ──
//
// These are `pub` because `claude_view_session_parser::SessionDoc` exposes
// `Vec<StatsLine>`, and downstream consumers (Phase 4 rollup writers,
// endpoint handlers in Phase 3) must introspect line.message, .timestamp,
// .usage, etc. Treat the shape as the parser contract — it mirrors the
// Claude CLI JSONL event layout exactly. Add-only evolution (new fields with
// `#[serde(default)]`); never remove or rename a field without bumping
// `PARSER_VERSION`. Richer than MinLine; still skips unknown fields.

#[derive(Deserialize, Clone)]
pub struct StatsLine {
    #[serde(rename = "type")]
    pub line_type: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub message: Option<StatsMessage>,
    #[serde(default, rename = "gitBranch")]
    pub git_branch: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct StatsMessage {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub usage: Option<StatsUsage>,
    #[serde(default)]
    pub content: serde_json::Value,
    #[serde(default)]
    pub stop_reason: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct StatsUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_creation: Option<CacheCreationSplit>,
}

#[derive(Deserialize, Clone)]
pub struct CacheCreationSplit {
    #[serde(default)]
    pub ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u64,
}

/// Snapshot of one assistant message (last occurrence wins).
struct AssistantSnapshot {
    model: Option<String>,
    usage: Option<StatsUsage>,
    tool_names: Vec<String>,
    thinking_count: u32,
}

/// Extract all session stats from a JSONL file.
pub fn extract_stats(path: &Path, is_compressed: bool) -> std::io::Result<SessionStats> {
    let lines: Vec<StatsLine> = jsonl_reader::read_all(path, is_compressed)?;
    Ok(compute_stats(&lines))
}

pub fn compute_stats(lines: &[StatsLine]) -> SessionStats {
    let mut stats = SessionStats {
        line_count: lines.len() as u32,
        ..Default::default()
    };

    // ── Pass 1: collect assistant message snapshots (last occurrence wins) ──
    let mut assistant_msgs: HashMap<String, AssistantSnapshot> = HashMap::new();
    // For messages without an ID, use a synthetic key
    let mut anon_counter: u32 = 0;

    let mut first_ts: Option<&str> = None;
    let mut last_ts: Option<&str> = None;
    let mut model_counts: HashMap<String, u32> = HashMap::new();
    let mut first_user_text: Option<String> = None;
    let mut last_assistant_text: Option<String> = None;

    for line in lines {
        // Track timestamps
        if let Some(ref ts) = line.timestamp {
            if first_ts.is_none() {
                first_ts = Some(ts);
            }
            last_ts = Some(ts);
        }

        // First non-empty gitBranch wins. JSONL writes this on every line but
        // it only changes across resumes; the first occurrence is authoritative
        // for the session's branch at start.
        if stats.git_branch.is_none() {
            if let Some(ref b) = line.git_branch {
                if !b.is_empty() {
                    stats.git_branch = Some(b.clone());
                }
            }
        }

        let Some(ref msg) = line.message else {
            continue;
        };

        match msg.role.as_deref() {
            Some("assistant") => {
                let key = msg.id.clone().unwrap_or_else(|| {
                    anon_counter += 1;
                    format!("__anon_{anon_counter}")
                });

                // Extract content block info
                let mut tool_names = Vec::new();
                let mut thinking_count: u32 = 0;
                let mut text_content: Option<String> = None;

                if let Some(arr) = msg.content.as_array() {
                    for block in arr {
                        match block.get("type").and_then(|t| t.as_str()) {
                            Some("tool_use") => {
                                if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                                    tool_names.push(invocation_key(name, block.get("input")));
                                }
                            }
                            Some("thinking") => {
                                thinking_count += 1;
                            }
                            Some("text") => {
                                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                                    if !t.is_empty() {
                                        text_content = Some(truncate(t, 200));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                if let Some(ref t) = text_content {
                    last_assistant_text = Some(t.clone());
                }

                if let Some(ref model) = msg.model {
                    *model_counts.entry(model.clone()).or_default() += 1;
                }

                // Overwrite on duplicate ID → last occurrence wins
                assistant_msgs.insert(
                    key,
                    AssistantSnapshot {
                        model: msg.model.clone(),
                        usage: msg.usage.clone(),
                        tool_names,
                        thinking_count,
                    },
                );
            }
            Some("user") => {
                // Count user prompts (messages with at least one text block, not just tool results)
                let has_user_text = match msg.content {
                    serde_json::Value::String(ref s) => !s.is_empty(),
                    serde_json::Value::Array(ref arr) => arr
                        .iter()
                        .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")),
                    _ => false,
                };
                if has_user_text {
                    stats.user_prompt_count += 1;
                    if first_user_text.is_none() {
                        let text = extract_user_text(&msg.content);
                        if !text.is_empty() {
                            first_user_text = Some(truncate(&text, 200));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ── Pass 2: aggregate from deduped assistant messages ──
    for snap in assistant_msgs.values() {
        if snap.model.is_some() {
            stats.turn_count += 1;
        }

        if let Some(ref usage) = snap.usage {
            stats.total_input_tokens += usage.input_tokens;
            stats.total_output_tokens += usage.output_tokens;
            stats.cache_read_tokens += usage.cache_read_input_tokens;
            stats.cache_creation_tokens += usage.cache_creation_input_tokens;

            if let Some(ref split) = usage.cache_creation {
                stats.cache_creation_5m_tokens += split.ephemeral_5m_input_tokens;
                stats.cache_creation_1hr_tokens += split.ephemeral_1h_input_tokens;
            }

            // Per-model token tracking for cost calculation
            if let Some(ref model) = snap.model {
                let entry = stats.per_model_tokens.entry(model.clone()).or_default();
                entry.input_tokens += usage.input_tokens;
                entry.output_tokens += usage.output_tokens;
                entry.cache_read_tokens += usage.cache_read_input_tokens;
                entry.cache_creation_tokens += usage.cache_creation_input_tokens;
                if let Some(ref split) = usage.cache_creation {
                    entry.cache_creation_5m_tokens += split.ephemeral_5m_input_tokens;
                    entry.cache_creation_1hr_tokens += split.ephemeral_1h_input_tokens;
                }
            }
        }

        stats.thinking_block_count += snap.thinking_count;

        for name in &snap.tool_names {
            stats.tool_call_count += 1;
            *stats.invocation_counts.entry(name.clone()).or_default() += 1;
            // Tool-breakdown counts match the **base** name (before `:sub`),
            // so skill/agent variants still roll up under their parent tool.
            let base = name.split(':').next().unwrap_or(name);
            match base {
                "Read" | "NotebookRead" => stats.files_read_count += 1,
                "Edit" | "Write" | "NotebookEdit" => stats.files_edited_count += 1,
                "Bash" => stats.bash_count += 1,
                "Agent" | "Task" => stats.agent_spawn_count += 1,
                _ => {}
            }
        }
    }

    // ── Timestamps & duration ──
    stats.first_message_at = first_ts.map(|s| s.to_string());
    stats.last_message_at = last_ts.map(|s| s.to_string());
    if let (Some(first), Some(last)) = (first_ts, last_ts) {
        stats.duration_seconds = parse_duration_seconds(first, last);
    }

    // ── Primary model (most used; ties broken by lexicographic name) ──
    //
    // `HashMap::into_iter` is non-deterministic, so a bare
    // `max_by_key(|(_, count)| *count)` picks an arbitrary winner when
    // two models tie on count. That non-determinism leaks into every
    // consumer (CQRS Phase 2 parity test caught it on 2/2000 prod
    // sessions whose JSONL had model-count ties). Including the model
    // string in the key — sorted ascending so the lexicographically-
    // smallest name wins on a tie — makes the result deterministic
    // for any given input bytes.
    stats.primary_model = model_counts
        .into_iter()
        .max_by(|(am, ac), (bm, bc)| ac.cmp(bc).then_with(|| bm.cmp(am)))
        .map(|(model, _)| model);

    // ── Text content ──
    stats.preview = first_user_text.unwrap_or_default();
    stats.last_message = last_assistant_text.unwrap_or_default();

    stats
}

/// Build the `invocation_counts` map key for a single `tool_use` block.
///
/// Base tools return their raw name (e.g. `"Bash"`, `"mcp__plugin_X__tool_Y"`).
/// `Skill` / `Task` / `Agent` calls get a `:sub` suffix so readers can emulate
/// the retired `invocables.kind` grouping without the registry join —
/// e.g. `"Skill:commit"`, `"Task:general-purpose"`. Returns the raw tool
/// name unchanged when the expected `input` field is absent or not a string.
fn invocation_key(tool_name: &str, input: Option<&serde_json::Value>) -> String {
    let sub_field = match tool_name {
        "Skill" => "skill",
        "Task" | "Agent" => "subagent_type",
        _ => return tool_name.to_string(),
    };
    input
        .and_then(|v| v.get(sub_field))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|sub| format!("{tool_name}:{sub}"))
        .unwrap_or_else(|| tool_name.to_string())
}

/// Extract text from a user message content field.
fn extract_user_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            for block in arr {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        return t.to_string();
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        truncated
    }
}

fn parse_duration_seconds(first: &str, last: &str) -> u32 {
    let parse = |s: &str| -> Option<i64> {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.timestamp())
    };
    match (parse(first), parse(last)) {
        (Some(a), Some(b)) => (b - a).max(0) as u32,
        _ => 0,
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write as IoWrite;
    use tempfile::tempdir;

    fn make_jsonl(path: &Path, lines: &[&str]) {
        let mut f = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
    }

    #[test]
    fn extracts_token_counts_with_dedup() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                // msg_1 streaming: first chunk (incomplete)
                r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5-20250514","role":"assistant","content":[{"type":"thinking","thinking":"..."}],"usage":{"input_tokens":1000,"output_tokens":10,"cache_read_input_tokens":200,"cache_creation_input_tokens":100}}}"#,
                // msg_1 streaming: final chunk (stop_reason set, higher output)
                r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5-20250514","role":"assistant","content":[{"type":"thinking","thinking":"..."},{"type":"tool_use","id":"t1","name":"Read","input":{}}],"usage":{"input_tokens":1000,"output_tokens":200,"cache_read_input_tokens":200,"cache_creation_input_tokens":100},"stop_reason":"tool_use"}}"#,
                // msg_2: separate message
                r#"{"type":"assistant","message":{"id":"msg_2","model":"claude-sonnet-4-5-20250514","role":"assistant","content":[{"type":"text","text":"Done"}],"usage":{"input_tokens":1500,"output_tokens":50,"cache_read_input_tokens":500,"cache_creation_input_tokens":0},"stop_reason":"end_turn"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();

        // Last occurrence of msg_1: in=1000, out=200, cr=200, cc=100
        // msg_2: in=1500, out=50, cr=500, cc=0
        assert_eq!(stats.total_input_tokens, 2500);
        assert_eq!(stats.total_output_tokens, 250);
        assert_eq!(stats.cache_read_tokens, 700);
        assert_eq!(stats.cache_creation_tokens, 100);
    }

    #[test]
    fn counts_turns_tools_thinking() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                // User prompt
                r#"{"type":"user","timestamp":"2026-04-01T10:00:00Z","message":{"role":"user","content":[{"type":"text","text":"Fix the bug"}]}}"#,
                // Assistant with thinking + Read
                r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-opus-4-6","role":"assistant","content":[{"type":"thinking","thinking":"..."},{"type":"tool_use","id":"t1","name":"Read","input":{}}],"usage":{"input_tokens":100,"output_tokens":50},"stop_reason":"tool_use"}}"#,
                // Tool result (user role, but no text block)
                r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"file contents"}]}}"#,
                // Assistant with Edit + Write
                r#"{"type":"assistant","message":{"id":"msg_2","model":"claude-opus-4-6","role":"assistant","content":[{"type":"tool_use","id":"t2","name":"Edit","input":{}},{"type":"tool_use","id":"t3","name":"Bash","input":{}}],"usage":{"input_tokens":200,"output_tokens":100},"stop_reason":"tool_use"}}"#,
                // Tool results
                r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t2","content":"ok"},{"type":"tool_result","tool_use_id":"t3","content":"ok"}]}}"#,
                // Final text response
                r#"{"type":"assistant","timestamp":"2026-04-01T10:00:15Z","message":{"id":"msg_3","model":"claude-opus-4-6","role":"assistant","content":[{"type":"text","text":"All fixed"}],"usage":{"input_tokens":300,"output_tokens":30},"stop_reason":"end_turn"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();

        assert_eq!(stats.turn_count, 3, "3 unique assistant messages");
        assert_eq!(stats.user_prompt_count, 1, "only first user msg has text");
        assert_eq!(stats.tool_call_count, 3, "Read + Edit + Bash");
        assert_eq!(stats.thinking_block_count, 1, "1 thinking block");
        assert_eq!(stats.files_read_count, 1);
        assert_eq!(stats.files_edited_count, 1);
        assert_eq!(stats.bash_count, 1);
        assert_eq!(stats.line_count, 6);
    }

    #[test]
    fn extracts_timestamps_and_duration() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"system","timestamp":"2026-04-01T10:00:00Z"}"#,
                r#"{"type":"user","timestamp":"2026-04-01T10:00:01Z","message":{"role":"user","content":"hello"}}"#,
                r#"{"type":"assistant","timestamp":"2026-04-01T10:05:30Z","message":{"id":"m1","model":"claude-opus-4-6","role":"assistant","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":10,"output_tokens":5},"stop_reason":"end_turn"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();

        assert_eq!(
            stats.first_message_at.as_deref(),
            Some("2026-04-01T10:00:00Z")
        );
        assert_eq!(
            stats.last_message_at.as_deref(),
            Some("2026-04-01T10:05:30Z")
        );
        assert_eq!(stats.duration_seconds, 330); // 5 min 30 sec
    }

    #[test]
    fn identifies_primary_model() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-6","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
                r#"{"type":"assistant","message":{"id":"m2","model":"claude-sonnet-4-5-20250514","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
                r#"{"type":"assistant","message":{"id":"m3","model":"claude-sonnet-4-5-20250514","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        assert_eq!(
            stats.primary_model.as_deref(),
            Some("claude-sonnet-4-5-20250514")
        );
    }

    /// Regression test for the non-deterministic `primary_model`
    /// selection bug caught by the CQRS Phase 2 parity gate
    /// (2/2000 prod sessions had model-count ties; HashMap iteration
    /// order leaked through `max_by_key` and the same input bytes
    /// produced different `primary_model` values across two parses).
    ///
    /// Repeats the same input 100 times to exercise HashMap order
    /// stochasticity. With the deterministic tie-break, the result
    /// must be identical every iteration. With the bug, this test
    /// fails on > 50 % of runs.
    #[test]
    fn primary_model_is_deterministic_under_ties() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("ties.jsonl");
        // Three models, each appearing exactly twice — fully tied.
        make_jsonl(
            &path,
            &[
                r#"{"type":"assistant","message":{"id":"a1","model":"claude-opus-4-7","role":"assistant","content":[],"usage":{"input_tokens":1,"output_tokens":1}}}"#,
                r#"{"type":"assistant","message":{"id":"a2","model":"claude-opus-4-7","role":"assistant","content":[],"usage":{"input_tokens":1,"output_tokens":1}}}"#,
                r#"{"type":"assistant","message":{"id":"b1","model":"claude-sonnet-4-6","role":"assistant","content":[],"usage":{"input_tokens":1,"output_tokens":1}}}"#,
                r#"{"type":"assistant","message":{"id":"b2","model":"claude-sonnet-4-6","role":"assistant","content":[],"usage":{"input_tokens":1,"output_tokens":1}}}"#,
                r#"{"type":"assistant","message":{"id":"c1","model":"claude-haiku-4-5","role":"assistant","content":[],"usage":{"input_tokens":1,"output_tokens":1}}}"#,
                r#"{"type":"assistant","message":{"id":"c2","model":"claude-haiku-4-5","role":"assistant","content":[],"usage":{"input_tokens":1,"output_tokens":1}}}"#,
            ],
        );

        let first = extract_stats(&path, false).unwrap().primary_model;
        for _ in 0..99 {
            let next = extract_stats(&path, false).unwrap().primary_model;
            assert_eq!(
                next, first,
                "primary_model must be deterministic across repeated parses; \
                 if this test flakes, the HashMap tie-break regressed"
            );
        }

        // Tie-break rule: lex-smallest model name wins.
        // Sorted ascending: claude-haiku-4-5 < claude-opus-4-7 < claude-sonnet-4-6.
        assert_eq!(
            first.as_deref(),
            Some("claude-haiku-4-5"),
            "lex-smallest model must win on a count tie"
        );
    }

    #[test]
    fn extracts_preview_and_last_message() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Fix the login bug in auth.rs please"}]}}"#,
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-6","role":"assistant","content":[{"type":"text","text":"I found and fixed the issue"}],"usage":{"input_tokens":10,"output_tokens":5},"stop_reason":"end_turn"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        assert_eq!(stats.preview, "Fix the login bug in auth.rs please");
        assert_eq!(stats.last_message, "I found and fixed the issue");
    }

    #[test]
    fn handles_string_content_user_messages() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                // String content (not array)
                r#"{"type":"user","message":{"role":"user","content":"hello world"}}"#,
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-6","role":"assistant","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":10,"output_tokens":5},"stop_reason":"end_turn"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        assert_eq!(stats.user_prompt_count, 1);
        assert_eq!(stats.preview, "hello world");
    }

    #[test]
    fn per_model_token_tracking() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-6","role":"assistant","content":[],"usage":{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":100},"stop_reason":"end_turn"}}"#,
                r#"{"type":"assistant","message":{"id":"m2","model":"claude-haiku-3-5-20241022","role":"assistant","content":[],"usage":{"input_tokens":500,"output_tokens":200},"stop_reason":"end_turn"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        let opus = stats.per_model_tokens.get("claude-opus-4-6").unwrap();
        assert_eq!(opus.input_tokens, 1000);
        assert_eq!(opus.output_tokens, 500);

        let haiku = stats
            .per_model_tokens
            .get("claude-haiku-3-5-20241022")
            .unwrap();
        assert_eq!(haiku.input_tokens, 500);
    }

    #[test]
    fn counts_agent_spawns() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-6","role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Agent","input":{}},{"type":"tool_use","id":"t2","name":"Agent","input":{}}],"usage":{"input_tokens":10,"output_tokens":5},"stop_reason":"tool_use"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        assert_eq!(stats.agent_spawn_count, 2);
        assert_eq!(stats.tool_call_count, 2);
    }

    #[test]
    fn handles_empty_file() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("empty.jsonl");
        make_jsonl(&path, &[]);

        let stats = extract_stats(&path, false).unwrap();
        assert_eq!(stats.line_count, 0);
        assert_eq!(stats.turn_count, 0);
        assert_eq!(stats.total_input_tokens, 0);
        assert!(stats.primary_model.is_none());
        assert!(stats.first_message_at.is_none());
    }

    #[test]
    fn cache_creation_split_tracking() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-6","role":"assistant","content":[],"usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":5000,"cache_creation":{"ephemeral_5m_input_tokens":2000,"ephemeral_1h_input_tokens":3000}},"stop_reason":"end_turn"}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        assert_eq!(stats.cache_creation_tokens, 5000);
        assert_eq!(stats.cache_creation_5m_tokens, 2000);
        assert_eq!(stats.cache_creation_1hr_tokens, 3000);

        let opus = stats.per_model_tokens.get("claude-opus-4-6").unwrap();
        assert_eq!(opus.cache_creation_5m_tokens, 2000);
        assert_eq!(opus.cache_creation_1hr_tokens, 3000);
    }

    #[test]
    fn extracts_git_branch_from_first_occurrence() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        // Real JSONL puts gitBranch at the top of every line; session_stats
        // must take the first non-empty value.
        make_jsonl(
            &path,
            &[
                r#"{"type":"user","timestamp":"2026-04-17T00:00:00Z","gitBranch":"","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#,
                r#"{"type":"assistant","timestamp":"2026-04-17T00:00:01Z","gitBranch":"feature/hardcut","message":{"id":"m1","model":"claude-opus-4-7","role":"assistant","content":[{"type":"text","text":"hello"}],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
                r#"{"type":"user","timestamp":"2026-04-17T00:00:02Z","gitBranch":"main","message":{"role":"user","content":[{"type":"text","text":"later branch, should be ignored"}]}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        assert_eq!(stats.git_branch.as_deref(), Some("feature/hardcut"));
    }

    #[test]
    fn git_branch_missing_stays_none() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"user","timestamp":"2026-04-17T00:00:00Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#,
            ],
        );

        let stats = extract_stats(&path, false).unwrap();
        assert!(stats.git_branch.is_none());
    }
}
