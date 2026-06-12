// crates/providers/src/parsers/workbuddy.rs
//
// WorkBuddy — JSONL event transcripts under `~/.workbuddy/projects`:
//   <root>/<project>/<sessionID>.jsonl                 (main sessions)
//   <root>/<project>/<sessionID>/subagents/<id>.jsonl  (subagent transcripts —
//     separate sessions with raw id `<parent>:subagent:<sub>`)
// Project dirs may be symlinks — discovery follows them.
//
// Events by `type` (ported from agentsview's workbuddy.go):
//   message              { role: user|assistant, content: string |
//                          [{text|input_text|output_text}], cwd,
//                          timestamp: epoch-ms number, providerData }
//   function_call        { name, callId, arguments: string-or-object,
//                          providerData }
//   function_call_result { callId, output: string | {type:"text",text} |
//                          arbitrary JSON }
// providerData.usage (fallback providerData.rawUsage) carries token counts
// under a triple key-alias cascade; the OpenAI-style `prompt_tokens` total
// INCLUDES cached reads and is corrected here (input = total - cached).

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview, time};
use claude_view_types::block_types::ConversationBlock;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct WorkbuddyProvider;

impl Provider for WorkbuddyProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Workbuddy
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(projects) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for proj in projects.flatten() {
            let project_dir = proj.path();
            // `is_dir()` follows symlinks — project dirs may be links.
            if !project_dir.is_dir() {
                continue;
            }
            let project_name = proj.file_name();
            let Some(project) = project_name.to_str() else {
                continue;
            };
            let Ok(entries) = std::fs::read_dir(&project_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let entry_name = entry.file_name();
                let Some(name) = entry_name.to_str() else {
                    continue;
                };
                if path.is_file() {
                    if let Some(stem) = name.strip_suffix(".jsonl") {
                        if is_valid_session_id(stem) {
                            push_discovered(&mut out, stem.to_string(), path, project);
                        }
                    }
                    continue;
                }
                // `<sessionID>/subagents/*.jsonl` — subagent transcripts.
                if !path.is_dir() || !is_valid_session_id(name) {
                    continue;
                }
                let Ok(subs) = std::fs::read_dir(path.join("subagents")) else {
                    continue;
                };
                for sub in subs.flatten() {
                    let sub_path = sub.path();
                    let sub_name = sub.file_name();
                    let Some(sub_stem) = sub_name.to_str().and_then(|n| n.strip_suffix(".jsonl"))
                    else {
                        continue;
                    };
                    if !sub_path.is_file() {
                        continue;
                    }
                    push_discovered(
                        &mut out,
                        format!("{name}:subagent:{sub_stem}"),
                        sub_path,
                        project,
                    );
                }
            }
        }
        // Bytewise path order mirrors the Go source's string sort.
        out.sort_by(|a, b| a.path.as_os_str().cmp(b.path.as_os_str()));
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let read = jsonl::read_values(path)?;
        let (raw_id, dir_project) = identity_from_path(path);
        let mut meta =
            ForeignSessionMeta::new(ProviderKind::Workbuddy, &raw_id, path.to_path_buf());
        meta.project = dir_project;
        meta.malformed_lines = read.malformed;

        let mut out: Vec<ConversationBlock> = Vec::new();
        for event in &read.values {
            // First non-empty cwd wins: session cwd + project override.
            if meta.cwd.is_none() {
                if let Some(c) = event
                    .get("cwd")
                    .and_then(Value::as_str)
                    .filter(|c| !c.is_empty())
                {
                    meta.cwd = Some(c.to_string());
                    if let Some(p) = project_from_cwd_str(c) {
                        meta.project = p;
                    }
                }
            }
            // Timestamps are epoch-ms JSON numbers; anything else is ignored.
            let ts = event
                .get("timestamp")
                .and_then(Value::as_f64)
                .map(time::from_millis);
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }
            match event.get("type").and_then(Value::as_str) {
                Some("message") => handle_message(&mut out, &mut meta, &raw_id, event, ts),
                Some("function_call") => {
                    handle_function_call(&mut out, &mut meta, &raw_id, event, ts)
                }
                Some("function_call_result") => handle_result(&mut out, event),
                _ => {}
            }
        }

        // Sessions with zero contentful turns are non-interactive noise.
        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForeignSession { meta, blocks: out }])
    }
}

fn push_discovered(out: &mut Vec<DiscoveredSession>, raw_id: String, path: PathBuf, project: &str) {
    let Some((mtime, size_bytes)) = stat_entry(&path) else {
        return;
    };
    out.push(DiscoveredSession {
        id: ProviderKind::Workbuddy.session_id(&raw_id),
        provider: ProviderKind::Workbuddy,
        path,
        project_hint: Some(project.to_string()),
        mtime,
        size_bytes,
    });
}

/// Session ids / session dir names: alphanumeric + dash + underscore.
fn is_valid_session_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn dir_name(p: Option<&Path>) -> &str {
    p.and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .unwrap_or("")
}

/// Derive (raw session id, dir-derived project) from the transcript path.
/// `<project>/<parent>/subagents/<sub>.jsonl` → `<parent>:subagent:<sub>`,
/// but only when the parent dir is a plausible session id (mirrors the Go
/// source's guard against project dirs literally named "subagents").
fn identity_from_path(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let parent = path.parent();
    if dir_name(parent) == "subagents" {
        let session_dir = parent.and_then(Path::parent);
        let parent_stem = dir_name(session_dir);
        let project = dir_name(session_dir.and_then(Path::parent)).to_string();
        if is_valid_session_id(parent_stem) {
            return (format!("{parent_stem}:subagent:{stem}"), project);
        }
        return (stem, project);
    }
    (stem, dir_name(parent).to_string())
}

/// Last path component of a cwd, as the project override. Handles
/// Windows-style paths (the Go source normalizes backslashes) and returns
/// `None` for root-like cwds so the dir-derived project survives (mirrors
/// ExtractProjectFromCwd's empty-result fallback).
fn project_from_cwd_str(cwd: &str) -> Option<String> {
    let norm = cwd.replace('\\', "/");
    let name = norm.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if name.is_empty() || name.ends_with(':') {
        return None;
    }
    Some(name.to_string())
}

/// Extract display text from `content`: plain string passes through; arrays
/// take the first non-empty of `text` / `input_text` / `output_text` per
/// part, joined with newlines. Anything else yields "" (line skipped).
fn content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => {
            let texts: Vec<&str> = parts
                .iter()
                .filter_map(|part| {
                    ["text", "input_text", "output_text"]
                        .iter()
                        .find_map(|k| part.get(k).and_then(Value::as_str))
                        .filter(|t| !t.is_empty())
                })
                .collect();
            texts.join("\n").trim().to_string()
        }
        _ => String::new(),
    }
}

fn handle_message(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    raw_id: &str,
    event: &Value,
    ts: Option<f64>,
) {
    let role = event.get("role").and_then(Value::as_str).unwrap_or("");
    let text = content_text(event.get("content"));
    if text.trim().is_empty() {
        return;
    }
    let id = blocks::block_id(raw_id, out.len());
    match role {
        "user" => {
            if meta.first_message.is_empty() {
                meta.first_message = preview(&text, 200);
            }
            meta.message_count += 1;
            meta.user_message_count += 1;
            out.push(blocks::user(id, text, ts));
        }
        "assistant" => {
            observe_usage(meta, event);
            meta.message_count += 1;
            out.push(blocks::assistant(
                id,
                vec![blocks::text_segment(text)],
                None,
                ts,
            ));
        }
        _ => {}
    }
}

fn handle_function_call(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    raw_id: &str,
    event: &Value,
    ts: Option<f64>,
) {
    let name = event.get("name").and_then(Value::as_str).unwrap_or("");
    let call_id = event.get("callId").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() || call_id.is_empty() {
        return;
    }
    observe_usage(meta, event);
    let seg = blocks::tool_segment(
        name.to_string(),
        tool_input(event.get("arguments")),
        call_id.to_string(),
    );
    // Tool calls join the open assistant turn; a fresh turn is opened (and
    // counted) only when the transcript isn't already inside one.
    if let Some(ConversationBlock::Assistant(a)) = out.last_mut() {
        a.segments.push(seg);
        return;
    }
    let id = blocks::block_id(raw_id, out.len());
    meta.message_count += 1;
    out.push(blocks::assistant(id, vec![seg], None, ts));
}

fn handle_result(out: &mut [ConversationBlock], event: &Value) {
    let Some(call_id) = event
        .get("callId")
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
    else {
        return;
    };
    blocks::attach_tool_result(out, call_id, result_text(event.get("output")), false);
}

/// `arguments` arrives either as an object or as stringified JSON; parse the
/// string form so the UI gets structured toolInput (unparseable strings stay
/// strings — never fabricate).
fn tool_input(arguments: Option<&Value>) -> Value {
    match arguments {
        Some(Value::String(s)) => {
            serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
        }
        Some(v) => v.clone(),
        None => Value::Null,
    }
}

/// `output` decoding: `{type:"text", text:"…"}` wrappers (any object with a
/// non-empty `text` string) unwrap to the plain text; strings pass through;
/// other JSON serializes as a string.
fn result_text(output: Option<&Value>) -> String {
    let Some(output) = output else {
        return String::new();
    };
    if let Some(t) = output.get("text").and_then(Value::as_str) {
        if !t.is_empty() {
            return t.to_string();
        }
    }
    match output {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

const INPUT_KEYS: &[&str] = &["inputTokens", "input_tokens", "prompt_tokens"];
const OUTPUT_KEYS: &[&str] = &["outputTokens", "output_tokens", "completion_tokens"];
const CACHE_READ_KEYS: &[&str] = &[
    "cacheReadInputTokens",
    "cache_read_input_tokens",
    "prompt_tokens_details.cached_tokens",
];
const CACHE_CREATE_KEYS: &[&str] = &["cacheCreationInputTokens", "cache_creation_input_tokens"];

/// Port of agentsview's applyWorkBuddyUsage: model from providerData.model,
/// usage from providerData.usage (fallback rawUsage — only when the `usage`
/// key is absent entirely), each field resolved through its key-alias
/// cascade by FIRST EXISTING key (a present-but-non-numeric key still wins
/// and coerces to 0). When the OpenAI-style `prompt_tokens` key is present,
/// the total includes cached reads — subtract (input = max(total-cached, 0)).
fn observe_usage(meta: &mut ForeignSessionMeta, event: &Value) {
    let Some(pd) = event.get("providerData") else {
        return;
    };
    let model = pd.get("model").and_then(Value::as_str).unwrap_or("");
    meta.record_model(model);
    let Some(usage) = pd.get("usage").or_else(|| pd.get("rawUsage")) else {
        return;
    };

    let input = first_existing(usage, INPUT_KEYS);
    let output = first_existing(usage, OUTPUT_KEYS);
    let cache_read = first_existing(usage, CACHE_READ_KEYS);
    let cache_create = first_existing(usage, CACHE_CREATE_KEYS);
    if input.is_none() && output.is_none() && cache_read.is_none() && cache_create.is_none() {
        return;
    }

    let cache_read_v = to_tokens(cache_read);
    let mut input_v = to_tokens(input);
    if usage.get("prompt_tokens").is_some() {
        input_v = input_v.saturating_sub(cache_read_v);
    }
    meta.usage.record(
        model,
        UsageTotals {
            input_tokens: input_v,
            output_tokens: to_tokens(output),
            cache_read_input_tokens: cache_read_v,
            cache_creation_input_tokens: to_tokens(cache_create),
        },
    );
}

/// First key (dotted paths descend into nested objects) that EXISTS in
/// `usage`, regardless of value type — mirrors gjson's firstExisting.
fn first_existing<'a>(usage: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| lookup(usage, k))
}

fn lookup<'a>(v: &'a Value, dotted: &str) -> Option<&'a Value> {
    dotted.split('.').try_fold(v, |acc, key| acc.get(key))
}

/// Numeric coercion matching gjson `.Int()` semantics closely enough:
/// non-numeric / negative / missing → 0.
fn to_tokens(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_f64)
        .filter(|f| f.is_finite() && *f > 0.0)
        .map_or(0, |f| f as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::{AssistantSegment, ToolStatus};

    const SESSION_ID: &str = "11111111-1111-4111-8111-111111111111";

    const MAIN_FIXTURE: &str = r#"{"id":"u1","timestamp":1778749186168,"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}],"cwd":"/tmp/cwd-project"}
{"id":"a1","timestamp":1778749187168,"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}],"providerData":{"model":"gpt-5.5","usage":{"inputTokens":20,"outputTokens":4,"cacheReadInputTokens":5}}}
{"id":"fc1","timestamp":1778749188168,"type":"function_call","name":"Bash","callId":"call_1","arguments":"{\"command\":\"pwd\"}","providerData":{"model":"gpt-5.5","usage":{"inputTokens":10,"outputTokens":3,"cacheReadInputTokens":2}}}
{"id":"fr1","timestamp":1778749189168,"type":"function_call_result","name":"Bash","callId":"call_1","output":{"type":"text","text":"/tmp/cwd-project"}}
"#;

    fn write_file(root: &Path, rel: &str, content: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parses_main_session_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_file(
            dir.path(),
            &format!("proj/{SESSION_ID}.jsonl"),
            MAIN_FIXTURE,
        );
        let mut sessions = WorkbuddyProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, format!("workbuddy:{SESSION_ID}"));
        assert_eq!(
            s.meta.project, "cwd-project",
            "first cwd overrides the dir-derived project"
        );
        assert_eq!(s.meta.cwd.as_deref(), Some("/tmp/cwd-project"));
        assert_eq!(s.meta.first_message, "hello");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.models, vec!["gpt-5.5"]);
        assert_eq!(s.meta.malformed_lines, 0);
        assert!(s.meta.usage.has_usage);
        assert_eq!(s.meta.usage.totals.input_tokens, 30);
        assert_eq!(s.meta.usage.totals.output_tokens, 7);
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 7);
        assert_eq!(s.meta.usage.per_model["gpt-5.5"].output_tokens, 7);
        assert!((s.meta.started_at.unwrap() - 1_778_749_186.168).abs() < 1e-6);
        assert!((s.meta.ended_at.unwrap() - 1_778_749_189.168).abs() < 1e-6);

        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block");
        };
        assert_eq!(
            a.segments.len(),
            2,
            "function_call joins the open assistant turn"
        );
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment");
        };
        assert_eq!(execution.tool_name, "Bash");
        assert_eq!(execution.tool_input, serde_json::json!({"command": "pwd"}));
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "/tmp/cwd-project"
        );
    }

    #[test]
    fn openai_prompt_tokens_exclude_cached_reads() {
        let dir = tempfile::tempdir().unwrap();
        let line = r#"{"timestamp":1778749187168,"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}],"providerData":{"model":"gpt-5.5","rawUsage":{"prompt_tokens":20,"completion_tokens":4,"prompt_tokens_details":{"cached_tokens":5}}}}
"#;
        let path = write_file(dir.path(), &format!("proj/{SESSION_ID}.jsonl"), line);
        let s = &WorkbuddyProvider.parse(&path).unwrap()[0];
        assert!(s.meta.usage.has_usage);
        assert_eq!(
            s.meta.usage.totals.input_tokens, 15,
            "prompt_tokens includes cached reads — must subtract"
        );
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 5);
        assert_eq!(s.meta.usage.totals.output_tokens, 4);
        assert_eq!(s.meta.usage.per_model["gpt-5.5"].input_tokens, 15);
    }

    #[test]
    fn malformed_lines_are_counted_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let content =
            "{\"type\":\"message\",\"role\":\"user\",\"content\":\"hello there\"}\nnot json at all\n";
        let path = write_file(dir.path(), "proj/s_1.jsonl", content);
        let s = &WorkbuddyProvider.parse(&path).unwrap()[0];
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.message_count, 1);
        assert_eq!(s.meta.first_message, "hello there");
        assert_eq!(s.meta.project, "proj", "no cwd — dir-derived project stays");
        assert!(!s.meta.usage.has_usage, "no token counts in this fixture");
    }

    #[test]
    fn sessions_without_contentful_messages_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"type":"message","role":"user","content":[]}
{"type":"message","role":"tool","content":"ignored role"}
{"type":"function_call_result","callId":"x","output":"orphan"}
{"type":"unknown"}
"#;
        let path = write_file(dir.path(), &format!("proj/{SESSION_ID}.jsonl"), content);
        assert!(WorkbuddyProvider.parse(&path).unwrap().is_empty());
    }

    #[test]
    fn subagent_file_gets_composite_id() {
        let dir = tempfile::tempdir().unwrap();
        let line = r#"{"timestamp":1778749186168,"type":"message","role":"user","content":[{"text":"sub task"}]}
"#;
        let path = write_file(
            dir.path(),
            &format!("proj/{SESSION_ID}/subagents/agent-123.jsonl"),
            line,
        );
        let s = &WorkbuddyProvider.parse(&path).unwrap()[0];
        assert_eq!(
            s.meta.id,
            format!("workbuddy:{SESSION_ID}:subagent:agent-123")
        );
        assert_eq!(s.meta.project, "proj");
        assert_eq!(s.meta.first_message, "sub task");

        // Parent dir that is not a plausible session id → plain session.
        let path2 = write_file(
            dir.path(),
            "proj2/not a session id!/subagents/agent-9.jsonl",
            line,
        );
        let s2 = &WorkbuddyProvider.parse(&path2).unwrap()[0];
        assert_eq!(
            s2.meta.id, "workbuddy:agent-9",
            "invalid parent dir — not a subagent"
        );
        assert_eq!(s2.meta.project, "proj2");
    }

    #[test]
    fn discover_walks_projects_and_subagents() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, &format!("proj/{SESSION_ID}.jsonl"), MAIN_FIXTURE);
        write_file(
            root,
            &format!("proj/{SESSION_ID}/subagents/agent-1.jsonl"),
            "{}\n",
        );
        // Noise that must be ignored:
        write_file(
            root,
            &format!("proj/{SESSION_ID}/tool-results/t_1.txt"),
            "x",
        );
        write_file(root, "proj/notes.txt", "x");
        write_file(root, "proj/bad stem!.jsonl", "{}\n");
        write_file(root, "stray.jsonl", "{}\n"); // root-level files are not projects

        let found = WorkbuddyProvider.discover(root);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, format!("workbuddy:{SESSION_ID}"));
        assert_eq!(
            found[1].id,
            format!("workbuddy:{SESSION_ID}:subagent:agent-1")
        );
        assert_eq!(found[0].project_hint.as_deref(), Some("proj"));
        assert_eq!(found[1].project_hint.as_deref(), Some("proj"));
    }

    #[cfg(unix)]
    #[test]
    fn discover_follows_symlinked_project_dirs() {
        let real = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        write_file(real.path(), &format!("{SESSION_ID}.jsonl"), "{}\n");
        std::os::unix::fs::symlink(real.path(), root.path().join("linked-proj")).unwrap();
        let found = WorkbuddyProvider.discover(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, format!("workbuddy:{SESSION_ID}"));
        assert_eq!(found[0].project_hint.as_deref(), Some("linked-proj"));
    }

    #[test]
    fn cwd_root_does_not_override_project() {
        let dir = tempfile::tempdir().unwrap();
        let line = r#"{"timestamp":1778749186168,"type":"message","role":"user","content":[{"text":"hi"}],"cwd":"/"}
"#;
        let path = write_file(dir.path(), "discovered-proj/s1.jsonl", line);
        let s = &WorkbuddyProvider.parse(&path).unwrap()[0];
        assert_eq!(s.meta.project, "discovered-proj");
        assert_eq!(s.meta.cwd.as_deref(), Some("/"));
    }

    #[test]
    fn helper_edge_cases() {
        // Windows cwd → last component; drive roots are not project names.
        assert_eq!(
            project_from_cwd_str("C:\\Users\\alice\\projects\\report-builder").as_deref(),
            Some("report-builder")
        );
        assert_eq!(project_from_cwd_str("/"), None);
        assert_eq!(project_from_cwd_str("C:\\"), None);
        // function_call_result output decoding.
        assert_eq!(
            result_text(Some(&serde_json::json!({"type":"text","text":"plain"}))),
            "plain"
        );
        assert_eq!(
            result_text(Some(&serde_json::json!("already a string"))),
            "already a string"
        );
        assert_eq!(
            result_text(Some(&serde_json::json!({"files": ["a", "b"]}))),
            r#"{"files":["a","b"]}"#
        );
        assert_eq!(result_text(None), "");
        // Stringified arguments parse to structured input; garbage stays a string.
        assert_eq!(
            tool_input(Some(&serde_json::json!("{\"k\":1}"))),
            serde_json::json!({"k": 1})
        );
        assert_eq!(
            tool_input(Some(&serde_json::json!("not json"))),
            serde_json::json!("not json")
        );
    }
}
