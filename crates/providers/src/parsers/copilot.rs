// crates/providers/src/parsers/copilot.rs
//
// GitHub Copilot CLI — JSONL event streams under `<root>/session-state/`.
// Two layouts coexist: bare `<uuid>.jsonl` and directory `<uuid>/events.jsonl`
// (the directory form wins when both exist for the same uuid). Directory
// sessions carry a `workspace.yaml` sidecar whose `name:` line is the
// session title.
//
// Format (from the agentsview census, verified against fixtures): one JSON
// event per line, `{timestamp, type, data}`:
//   session.start            {sessionId, context.cwd, context.branch}
//   user.message             {content, source}
//   assistant.message        {content, reasoningText, toolRequests:[
//                              {toolCallId, name, arguments}], outputTokens}
//   tool.execution_complete  {toolCallId, result}
//   assistant.reasoning      (no text payload — stats marker only)
//   session.model_change     {newModel}
//   session.shutdown         {modelMetrics: model → {usage:{inputTokens,
//                              cacheReadTokens, cacheWriteTokens,
//                              outputTokens, reasoningTokens}}}
// Token usage arrives ONLY at session.shutdown; `inputTokens` INCLUDES both
// cache buckets, so fresh input = total − cacheRead − cacheWrite.

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

pub struct CopilotProvider;

impl Provider for CopilotProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Copilot
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let state_dir = root.join("session-state");
        let Ok(entries) = std::fs::read_dir(&state_dir) else {
            return Vec::new();
        };
        let entries: Vec<_> = entries.flatten().collect();
        // Directory sessions (<uuid>/events.jsonl); these shadow a bare
        // sibling <uuid>.jsonl of the same uuid.
        let dir_ids: HashSet<String> = entries
            .iter()
            .filter(|e| e.path().join("events.jsonl").is_file())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        let mut out = Vec::new();
        for entry in &entries {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let (raw_id, session_path) = if path.is_dir() {
                if !dir_ids.contains(name) {
                    continue;
                }
                (name.to_string(), path.join("events.jsonl"))
            } else {
                let Some(stem) = name.strip_suffix(".jsonl") else {
                    continue;
                };
                if dir_ids.contains(stem) {
                    continue; // directory layout wins
                }
                (stem.to_string(), path.clone())
            };
            let Some((mtime, size_bytes)) = stat_entry(&session_path) else {
                continue;
            };
            out.push(DiscoveredSession {
                id: ProviderKind::Copilot.session_id(&raw_id),
                provider: ProviderKind::Copilot,
                path: session_path,
                project_hint: None,
                mtime,
                size_bytes,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        // Session id comes from the path (dir name or file stem) so
        // lookup-by-id always matches discovery; in real Copilot data this
        // equals the session.start sessionId.
        let raw_id = raw_id_from_path(path)
            .ok_or_else(|| anyhow::anyhow!("no Copilot session id in path {}", path.display()))?;
        let read = jsonl::read_values(path)?;

        let mut meta = ForeignSessionMeta::new(ProviderKind::Copilot, &raw_id, path.to_path_buf());
        meta.malformed_lines = read.malformed;
        meta.project = "unknown".to_string();

        let mut b = Builder {
            meta,
            blocks: Vec::new(),
            raw_id,
            ordinal: 0,
            current_model: String::new(),
        };
        for event in &read.values {
            b.process(event);
        }

        // Sessions with zero contentful messages are non-interactive noise.
        if b.meta.message_count == 0 {
            return Ok(Vec::new());
        }
        b.meta.title = workspace_title(path);
        Ok(vec![ForeignSession {
            meta: b.meta,
            blocks: b.blocks,
        }])
    }
}

/// Accumulates state while scanning the event stream line by line.
struct Builder {
    meta: ForeignSessionMeta,
    blocks: Vec<ConversationBlock>,
    raw_id: String,
    ordinal: usize,
    current_model: String,
}

impl Builder {
    fn process(&mut self, event: &Value) {
        let ts = event
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|s| time::parse_timestamp(s, false));
        if let Some(t) = ts {
            self.meta.observe_timestamp(t);
        }
        let data = event.get("data").unwrap_or(&Value::Null);
        match event.get("type").and_then(Value::as_str).unwrap_or("") {
            "session.start" => self.handle_session_start(data),
            "user.message" => self.handle_user_message(data, ts),
            "assistant.message" => self.handle_assistant_message(data, ts),
            "tool.execution_complete" => self.handle_tool_complete(data),
            // assistant.reasoning carries no text payload; the Go source only
            // flips a has-thinking stat bit we have no field for — no-op.
            "session.model_change" => {
                if let Some(m) = data.get("newModel").and_then(Value::as_str) {
                    self.current_model = normalize_model(m);
                }
            }
            "session.shutdown" => self.handle_shutdown(data),
            _ => {}
        }
    }

    fn handle_session_start(&mut self, data: &Value) {
        if let Some(cwd) = data
            .pointer("/context/cwd")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
        {
            self.meta.cwd = Some(cwd.to_string());
            let project = project_from_cwd(cwd);
            if !project.is_empty() {
                self.meta.project = project;
            }
        }
        if let Some(branch) = data
            .pointer("/context/branch")
            .and_then(Value::as_str)
            .filter(|b| !b.is_empty())
        {
            self.meta.git_branch = Some(branch.to_string());
        }
    }

    fn handle_user_message(&mut self, data: &Value, ts: Option<f64>) {
        let content = data
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if content.is_empty() || is_synthetic_skill_message(data, content) {
            return;
        }
        if self.meta.first_message.is_empty() {
            self.meta.first_message = preview(content, 200);
        }
        self.meta.message_count += 1;
        self.meta.user_message_count += 1;
        let id = blocks::block_id(&self.raw_id, self.ordinal);
        self.ordinal += 1;
        self.blocks.push(blocks::user(id, content.to_string(), ts));
    }

    fn handle_assistant_message(&mut self, data: &Value, ts: Option<f64>) {
        let content = data
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let reasoning = data
            .get("reasoningText")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();

        let mut segments: Vec<AssistantSegment> = Vec::new();
        if !content.is_empty() {
            segments.push(blocks::text_segment(content.to_string()));
        }
        for req in data
            .get("toolRequests")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(name) = req
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.is_empty())
            else {
                continue;
            };
            let tool_id = req.get("toolCallId").and_then(Value::as_str).unwrap_or("");
            segments.push(blocks::tool_segment(
                name.to_string(),
                tool_arguments(req.get("arguments")),
                tool_id.to_string(),
            ));
        }

        if segments.is_empty() && reasoning.is_empty() {
            return;
        }
        self.meta.message_count += 1;
        self.meta.record_model(&self.current_model);
        let thinking = (!reasoning.is_empty()).then(|| reasoning.to_string());
        let id = blocks::block_id(&self.raw_id, self.ordinal);
        self.ordinal += 1;
        self.blocks
            .push(blocks::assistant(id, segments, thinking, ts));
    }

    fn handle_tool_complete(&mut self, data: &Value) {
        let tool_call_id = data.get("toolCallId").and_then(Value::as_str).unwrap_or("");
        if tool_call_id.is_empty() {
            return;
        }
        let output = match data.get("result") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        blocks::attach_tool_result(&mut self.blocks, tool_call_id, output, false);
    }

    /// Per-model token usage from session.shutdown's modelMetrics. The
    /// `inputTokens` total INCLUDES both cache buckets — fresh input is
    /// total minus cacheRead minus cacheWrite, floored at zero.
    fn handle_shutdown(&mut self, data: &Value) {
        let Some(metrics_map) = data.get("modelMetrics").and_then(Value::as_object) else {
            return;
        };
        for (model_key, metrics) in metrics_map {
            let usage = metrics.get("usage");
            let total_input = token(usage, "inputTokens");
            let cache_read = token(usage, "cacheReadTokens");
            let cache_write = token(usage, "cacheWriteTokens");
            let output = token(usage, "outputTokens");
            let reasoning = token(usage, "reasoningTokens");
            let fresh = total_input
                .saturating_sub(cache_read)
                .saturating_sub(cache_write);
            if fresh == 0 && output == 0 && cache_read == 0 && cache_write == 0 && reasoning == 0 {
                continue;
            }
            let model = normalize_model(model_key);
            self.meta.record_model(&model);
            // reasoningTokens has no Anthropic-shape bucket; it only
            // participates in the all-zero skip above (mirrors Go).
            self.meta.usage.record(
                &model,
                UsageTotals {
                    input_tokens: fresh,
                    output_tokens: output,
                    cache_read_input_tokens: cache_read,
                    cache_creation_input_tokens: cache_write,
                },
            );
        }
    }
}

fn token(usage: Option<&Value>, key: &str) -> u64 {
    usage
        .and_then(|u| u.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// Synthetic skill-context prompts injected by the CLI, not typed by the
/// user: `data.source` prefixed `skill-` or content prefixed `<skill-context`.
fn is_synthetic_skill_message(data: &Value, content: &str) -> bool {
    let source = data
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    source.starts_with("skill-") || content.starts_with("<skill-context")
}

/// toolRequests[].arguments is either a JSON-encoded string or a native JSON
/// value. Decode strings so the UI gets structured toolInput; keep
/// undecodable strings verbatim (truthful, never fabricated).
fn tool_arguments(args: Option<&Value>) -> Value {
    match args {
        Some(Value::String(s)) => {
            serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
        }
        Some(other) => other.clone(),
        None => Value::Null,
    }
}

/// Copilot writes Claude model ids with dots in version numbers
/// ("claude-sonnet-4.6") but the pricing catalog uses hyphens
/// ("claude-sonnet-4-6"). Other families (GPT etc.) keep dots — verbatim.
fn normalize_model(model: &str) -> String {
    if model.starts_with("claude-") {
        model.replace('.', "-")
    } else {
        model.to_string()
    }
}

/// Raw session id from the backing path: `<uuid>/events.jsonl` → dir name,
/// bare `<uuid>.jsonl` → file stem.
fn raw_id_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if name == "events.jsonl" {
        path.parent()?
            .file_name()?
            .to_str()
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    } else {
        Some(name.strip_suffix(".jsonl").unwrap_or(name).to_string()).filter(|s| !s.is_empty())
    }
}

/// Session title from the workspace.yaml sidecar (directory layout only).
/// Plain line scan for `name: ` — no YAML dependency.
fn workspace_title(events_path: &Path) -> Option<String> {
    if events_path.file_name().and_then(|n| n.to_str()) != Some("events.jsonl") {
        return None;
    }
    let yaml = events_path.parent()?.join("workspace.yaml");
    let data = crate::util::read_to_string_capped(yaml).ok()?;
    data.lines()
        .filter_map(|line| line.strip_prefix("name: "))
        .map(str::trim)
        .find(|name| !name.is_empty())
        .map(|name| preview(name, 200))
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;

    const FIXTURE: &str = concat!(
        r#"{"type":"session.start","data":{"sessionId":"abc-123","context":{"cwd":"/home/alice/code/myproject","branch":"main"}},"timestamp":"2026-01-15T10:00:00Z"}"#,
        "\n",
        r#"{"type":"session.model_change","data":{"newModel":"claude-sonnet-4.6"},"timestamp":"2026-01-15T10:00:01Z"}"#,
        "\n",
        r#"{"type":"user.message","data":{"content":"Fix the login bug"},"timestamp":"2026-01-15T10:00:02Z"}"#,
        "\n",
        r#"{"type":"assistant.message","data":{"content":"","reasoningText":"check auth first","toolRequests":[{"toolCallId":"tc-1","name":"view","arguments":"{\"path\":\"config.json\"}"}],"outputTokens":120},"timestamp":"2026-01-15T10:00:03Z"}"#,
        "\n",
        r#"{"type":"tool.execution_complete","data":{"toolCallId":"tc-1","success":true,"result":"file contents here"},"timestamp":"2026-01-15T10:00:04Z"}"#,
        "\n",
        r#"{"type":"assistant.reasoning","data":{},"timestamp":"2026-01-15T10:00:05Z"}"#,
        "\n",
        r#"{"type":"assistant.message","data":{"content":"Fixed."},"timestamp":"2026-01-15T10:00:06Z"}"#,
        "\n",
        r#"{"type":"session.shutdown","data":{"modelMetrics":{"claude-sonnet-4.6":{"usage":{"inputTokens":931647,"outputTokens":7150,"cacheReadTokens":873267,"cacheWriteTokens":51438,"reasoningTokens":432}}}},"timestamp":"2026-01-15T10:01:00Z"}"#,
        "\n",
    );

    fn parse_fixture(jsonl: &str) -> Vec<ForeignSession> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abc-123.jsonl");
        std::fs::write(&path, jsonl).unwrap();
        CopilotProvider.parse(&path).unwrap()
    }

    #[test]
    fn parses_session_into_blocks() {
        let mut sessions = parse_fixture(FIXTURE);
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "copilot:abc-123");
        assert_eq!(s.meta.project, "myproject");
        assert_eq!(s.meta.cwd.as_deref(), Some("/home/alice/code/myproject"));
        assert_eq!(s.meta.git_branch.as_deref(), Some("main"));
        assert_eq!(s.meta.first_message, "Fix the login bug");
        assert_eq!(s.meta.user_message_count, 1);
        // user + 2 assistant; tool.execution_complete attaches, never counts.
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.blocks.len(), 3);
        assert_eq!(s.meta.models, vec!["claude-sonnet-4-6".to_string()]);
        assert_eq!(
            s.meta.started_at,
            time::parse_timestamp("2026-01-15T10:00:00Z", false)
        );
        assert_eq!(
            s.meta.ended_at,
            time::parse_timestamp("2026-01-15T10:01:00Z", false)
        );
        assert_eq!(s.meta.malformed_lines, 0);
    }

    #[test]
    fn tool_call_gets_result_and_thinking_is_structured() {
        let s = parse_fixture(FIXTURE).remove(0);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("check auth first"));
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "view");
        // String-encoded arguments decode to structured JSON.
        assert_eq!(
            execution.tool_input,
            serde_json::json!({"path": "config.json"})
        );
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "file contents here"
        );
    }

    #[test]
    fn shutdown_usage_subtracts_caches_and_normalizes_model() {
        let s = parse_fixture(FIXTURE).remove(0);
        assert!(s.meta.usage.has_usage);
        // fresh = 931647 − 873267 − 51438 = 6942
        assert_eq!(s.meta.usage.totals.input_tokens, 6942);
        assert_eq!(s.meta.usage.totals.output_tokens, 7150);
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 873267);
        assert_eq!(s.meta.usage.totals.cache_creation_input_tokens, 51438);
        let per = &s.meta.usage.per_model["claude-sonnet-4-6"];
        assert_eq!(per.input_tokens, 6942);
    }

    #[test]
    fn multi_model_shutdown_skips_zero_usage_entries() {
        let jsonl = concat!(
            r#"{"type":"user.message","data":{"content":"hi"},"timestamp":"2026-01-15T10:00:00Z"}"#,
            "\n",
            r#"{"type":"assistant.message","data":{"content":"hello"},"timestamp":"2026-01-15T10:00:01Z"}"#,
            "\n",
            r#"{"type":"session.shutdown","data":{"modelMetrics":{"claude-haiku-4.5":{"usage":{"inputTokens":200,"outputTokens":80,"cacheReadTokens":120,"cacheWriteTokens":20}},"gpt-5.4":{"usage":{"inputTokens":100,"outputTokens":50,"cacheReadTokens":60,"cacheWriteTokens":10}},"idle-model":{"usage":{"inputTokens":0,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0,"reasoningTokens":0}}}},"timestamp":"2026-01-15T10:01:00Z"}"#,
            "\n",
        );
        let s = parse_fixture(jsonl).remove(0);
        assert_eq!(s.meta.usage.per_model.len(), 2, "zero-usage entry skipped");
        assert_eq!(s.meta.usage.per_model["claude-haiku-4-5"].input_tokens, 60);
        // Non-claude ids stay verbatim (dots preserved).
        assert_eq!(s.meta.usage.per_model["gpt-5.4"].input_tokens, 30);
        assert_eq!(s.meta.usage.totals.input_tokens, 90);
    }

    #[test]
    fn synthetic_skill_prompts_are_filtered() {
        let jsonl = concat!(
            r#"{"type":"user.message","data":{"content":"<skill-context name=\"gh-cli\">body</skill-context>","source":"skill-gh-cli"},"timestamp":"2026-01-15T10:00:00Z"}"#,
            "\n",
            r#"{"type":"user.message","data":{"content":"skill payload without wrapper","source":"skill-prd"},"timestamp":"2026-01-15T10:00:01Z"}"#,
            "\n",
            r#"{"type":"user.message","data":{"content":"<skill-context name=\"daily\">body</skill-context>"},"timestamp":"2026-01-15T10:00:02Z"}"#,
            "\n",
            r#"{"type":"user.message","data":{"content":"Fix the parser"},"timestamp":"2026-01-15T10:00:03Z"}"#,
            "\n",
            r#"{"type":"assistant.message","data":{"content":"Working on it."},"timestamp":"2026-01-15T10:00:04Z"}"#,
            "\n",
        );
        let s = parse_fixture(jsonl).remove(0);
        assert_eq!(s.meta.first_message, "Fix the parser");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.blocks.len(), 2);
    }

    #[test]
    fn sessions_without_contentful_messages_are_skipped() {
        // Only lifecycle events — even with real usage, no transcript exists.
        let jsonl = concat!(
            r#"{"type":"session.start","data":{"sessionId":"empty"},"timestamp":"2026-01-15T10:00:00Z"}"#,
            "\n",
            r#"{"type":"session.shutdown","data":{"modelMetrics":{"gpt-5.4":{"usage":{"inputTokens":10,"outputTokens":5,"cacheReadTokens":0,"cacheWriteTokens":0}}}},"timestamp":"2026-01-15T10:01:00Z"}"#,
            "\n",
        );
        assert!(parse_fixture(jsonl).is_empty());
    }

    #[test]
    fn malformed_lines_are_counted_not_hidden() {
        let jsonl = concat!(
            r#"{"type":"user.message","data":{"content":"hi"},"timestamp":"2026-01-15T10:00:00Z"}"#,
            "\n",
            "this is not json\n",
            r#"{"type":"assistant.message","data":{"content":"hello"},"timestamp":"2026-01-15T10:00:01Z"}"#,
            "\n",
        );
        let s = parse_fixture(jsonl).remove(0);
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.message_count, 2);
    }

    #[test]
    fn directory_layout_uses_workspace_yaml_title() {
        let dir = tempfile::tempdir().unwrap();
        let sess_dir = dir.path().join("abc-456");
        std::fs::create_dir_all(&sess_dir).unwrap();
        let events = sess_dir.join("events.jsonl");
        std::fs::write(
            &events,
            concat!(
                r#"{"type":"user.message","data":{"content":"hello"},"timestamp":"2026-01-15T10:00:01Z"}"#,
                "\n",
                r#"{"type":"assistant.message","data":{"content":"hi"},"timestamp":"2026-01-15T10:00:02Z"}"#,
                "\n",
            ),
        )
        .unwrap();
        std::fs::write(
            sess_dir.join("workspace.yaml"),
            "id: abc-456\nname: Fix Login Authentication Bug\nuser_named: false\n",
        )
        .unwrap();
        let s = CopilotProvider.parse(&events).unwrap().remove(0);
        // Id from the directory name (no session.start in this file).
        assert_eq!(s.meta.id, "copilot:abc-456");
        assert_eq!(
            s.meta.title.as_deref(),
            Some("Fix Login Authentication Bug")
        );
        // Title never overwrites the real first user message.
        assert_eq!(s.meta.first_message, "hello");

        // Whitespace-only name → no title (silence over wrong).
        std::fs::write(sess_dir.join("workspace.yaml"), "id: abc-456\nname:   \n").unwrap();
        let s = CopilotProvider.parse(&events).unwrap().remove(0);
        assert_eq!(s.meta.title, None);
    }

    #[test]
    fn discover_prefers_directory_layout_over_bare_file() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("session-state");
        // Directory session with a duplicate bare sibling — dir wins.
        let abc = state.join("abc-1");
        std::fs::create_dir_all(&abc).unwrap();
        std::fs::write(abc.join("events.jsonl"), FIXTURE).unwrap();
        std::fs::write(state.join("abc-1.jsonl"), FIXTURE).unwrap();
        // Bare-only session.
        std::fs::write(state.join("def-2.jsonl"), FIXTURE).unwrap();
        // Noise: dir without events.jsonl, non-jsonl file.
        std::fs::create_dir_all(state.join("not-a-session")).unwrap();
        std::fs::write(state.join("notes.txt"), "nope").unwrap();

        let found = CopilotProvider.discover(dir.path());
        let mut ids: Vec<_> = found.iter().map(|d| d.id.clone()).collect();
        ids.sort();
        assert_eq!(ids, vec!["copilot:abc-1", "copilot:def-2"]);
        let abc_found = found.iter().find(|d| d.id == "copilot:abc-1").unwrap();
        assert!(abc_found.path.ends_with("abc-1/events.jsonl"));
    }

    #[test]
    fn model_normalization_rules() {
        assert_eq!(normalize_model("claude-sonnet-4.6"), "claude-sonnet-4-6");
        assert_eq!(normalize_model("claude-haiku-4.5"), "claude-haiku-4-5");
        assert_eq!(normalize_model("gpt-5.4"), "gpt-5.4");
        assert_eq!(normalize_model("o3-mini"), "o3-mini");
        assert_eq!(normalize_model(""), "");
    }

    #[test]
    fn object_arguments_pass_through_natively() {
        let jsonl = concat!(
            r#"{"type":"user.message","data":{"content":"list"},"timestamp":"2026-01-15T10:00:01Z"}"#,
            "\n",
            r#"{"type":"assistant.message","data":{"content":"","toolRequests":[{"toolCallId":"tc-5","name":"glob","arguments":{"pattern":"*.go"}}]},"timestamp":"2026-01-15T10:00:02Z"}"#,
            "\n",
        );
        let s = parse_fixture(jsonl).remove(0);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_input, serde_json::json!({"pattern": "*.go"}));
    }
}
