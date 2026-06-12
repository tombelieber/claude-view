// crates/providers/src/parsers/iflow/mod.rs
//
// iFlow CLI — Claude-Code-like JSONL sessions at
// `<root>/<project>/session-<uuid>.jsonl` (root: ~/.iflow/projects).
//
// Format (ported from agentsview's iflow.go, verified against fixtures):
//   { type: user|assistant, uuid, parentUuid, sessionId: "session-<uuid>",
//     timestamp: RFC3339, isMeta?, isCompactSummary?, cwd?, gitBranch?,
//     message: { content: string | [text/thinking/tool_use/tool_result] } }
//
// CRITICAL INVERSION vs Claude Code: the uuid/parentUuid DAG represents
// streaming sliding-window snapshots, NOT conversation forks — no fork
// splitting. Adjacent assistant entries sharing a parentUuid within a <1s
// timestamp gap are one streaming burst and merge into a single turn
// (see burst.rs). tool_result payloads nest the output at
// responseParts.functionResponse.response.output (see content.rs).
// No model name and no token usage exist anywhere in the format.

mod burst;
mod command;
mod content;

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, jsonl, project_from_cwd, time};
use claude_view_types::block_types::ConversationBlock;
use serde_json::Value;
use std::path::Path;

pub struct IflowProvider;

impl Provider for IflowProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Iflow
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(projects) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for proj in projects.flatten() {
            let proj_path = proj.path();
            if !proj_path.is_dir() {
                continue;
            }
            let project = proj.file_name().to_string_lossy().into_owned();
            let Ok(files) = std::fs::read_dir(&proj_path) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some(raw) = name
                    .strip_prefix("session-")
                    .and_then(|n| n.strip_suffix(".jsonl"))
                    .filter(|r| !r.is_empty())
                else {
                    continue;
                };
                if path.is_dir() {
                    continue;
                }
                let Some((mtime, size_bytes)) = stat_entry(&path) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id: ProviderKind::Iflow.session_id(raw),
                    provider: ProviderKind::Iflow,
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
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid iFlow session filename"))?;
        let raw_id = stem.strip_prefix("session-").unwrap_or(stem).to_string();

        let read = jsonl::read_values(path)?;
        let mut meta = ForeignSessionMeta::new(ProviderKind::Iflow, &raw_id, path.to_path_buf());
        meta.malformed_lines = read.malformed;

        // First pass: collect user/assistant entries; observe timestamps
        // from EVERY valid line (any type) for the session envelope. The
        // line index counts valid JSON lines only — it drives burst
        // adjacency, so non-message events still break runs.
        let mut entries: Vec<Entry> = Vec::new();
        for (line_index, value) in read.values.into_iter().enumerate() {
            let timestamp = value
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false));
            if let Some(ts) = timestamp {
                meta.observe_timestamp(ts);
            }
            let is_assistant = match value.get("type").and_then(Value::as_str) {
                Some("assistant") => true,
                Some("user") => false,
                _ => continue,
            };
            let uuid = str_field(&value, "uuid");
            let parent_uuid = str_field(&value, "parentUuid");
            entries.push(Entry {
                value,
                is_assistant,
                uuid,
                parent_uuid,
                line_index,
                timestamp,
            });
        }

        // Project hints: first non-meta user entry carrying cwd/gitBranch.
        for e in entries
            .iter()
            .filter(|e| !e.is_assistant && !is_meta_entry(&e.value))
        {
            if meta.cwd.is_none() {
                meta.cwd = Some(str_field(&e.value, "cwd")).filter(|c| !c.is_empty());
            }
            if meta.git_branch.is_none() {
                meta.git_branch = Some(str_field(&e.value, "gitBranch")).filter(|b| !b.is_empty());
            }
            if meta.cwd.is_some() && meta.git_branch.is_some() {
                break;
            }
        }
        meta.project = match meta.cwd.as_deref() {
            Some(cwd) => project_from_session_cwd(cwd),
            None => path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "iflow".to_string()),
        };

        // Burst merge requires the full uuid DAG (mirrors Go's
        // hasAnyUUID && allHaveUUID gate).
        let entries = if !entries.is_empty() && entries.iter().all(|e| !e.uuid.is_empty()) {
            burst::merge_streaming_bursts(entries)
        } else {
            entries
        };

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        for (ordinal, entry) in entries.iter().enumerate() {
            let id = blocks::block_id(&raw_id, ordinal);
            if entry.is_assistant {
                content::handle_assistant(&mut out_blocks, &mut meta, id, entry);
            } else {
                content::handle_user(&mut out_blocks, &mut meta, id, entry);
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

/// One user/assistant JSONL entry participating in the streaming DAG.
struct Entry {
    value: Value,
    is_assistant: bool,
    uuid: String,
    parent_uuid: String,
    /// Index among valid JSON lines (any type) — burst adjacency key.
    line_index: usize,
    timestamp: Option<f64>,
}

fn is_meta_entry(v: &Value) -> bool {
    v.get("isMeta").and_then(Value::as_bool).unwrap_or(false)
        || v.get("isCompactSummary")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Project from the session's recorded cwd. Real iFlow data contains
/// Windows paths (e.g. `C:\exp\app`) regardless of the host OS, so
/// backslash separators are normalized before taking the basename.
fn project_from_session_cwd(cwd: &str) -> String {
    let bytes = cwd.as_bytes();
    let windows = (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\')
        || cwd.starts_with("\\\\");
    if windows {
        project_from_cwd(cwd.replace('\\', "/").trim_end_matches('/'))
    } else {
        project_from_cwd(cwd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::{AssistantSegment, ToolStatus};

    // Modeled on the real agentsview fixture: a user prompt, a 2-snapshot
    // streaming burst (overlapping tool_use ids), a tool_result entry with
    // the nested functionResponse envelope, a final text turn, and one
    // malformed line.
    const FIXTURE: &str = r#"{"type":"user","uuid":"u1","parentUuid":null,"sessionId":"session-5de701fc-7454","timestamp":"2026-01-21T05:56:34.812Z","cwd":"C:\\exp\\docker-image-retagger","gitBranch":null,"message":{"role":"user","content":"启动app时确保环境变量 DOCKER_API_VERSION"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","sessionId":"session-5de701fc-7454","timestamp":"2026-01-21T05:56:52.470Z","message":{"content":[{"type":"text","text":"Let me read the project files."},{"type":"tool_use","id":"call_1","name":"read_file","input":{"absolute_path":"C:\\exp\\README.md"}}]}}
{"type":"assistant","uuid":"a2","parentUuid":"u1","sessionId":"session-5de701fc-7454","timestamp":"2026-01-21T05:56:52.487Z","message":{"content":[{"type":"tool_use","id":"call_1","name":"read_file","input":{"absolute_path":"C:\\exp\\DIFFERENT.md"}},{"type":"tool_use","id":"call_2","name":"read_file","input":{"absolute_path":"C:\\exp\\Cargo.toml"}}]}}
{"type":"user","uuid":"u2","parentUuid":"a2","sessionId":"session-5de701fc-7454","timestamp":"2026-01-21T05:56:52.718Z","cwd":"C:\\exp\\docker-image-retagger","message":{"content":[{"type":"tool_result","tool_use_id":"call_1","content":{"callId":"call_1","responseParts":{"functionResponse":{"id":"call_1","name":"read_file","response":{"output":"readme contents"}}}}}]}}
{"type":"assistant","uuid":"a3","parentUuid":"u2","sessionId":"session-5de701fc-7454","timestamp":"2026-01-21T05:57:03.810Z","message":{"content":[{"type":"text","text":"All done."}]}}
not json garbage
"#;

    fn write_session(dir: &Path, project: &str, raw_id: &str, body: &str) -> std::path::PathBuf {
        let proj = dir.join(project);
        std::fs::create_dir_all(&proj).unwrap();
        let path = proj.join(format!("session-{raw_id}.jsonl"));
        std::fs::write(&path, body).unwrap();
        path
    }

    fn parse_fixture() -> ForeignSession {
        let dir = tempfile::tempdir().unwrap();
        let path = write_session(dir.path(), "projA", "5de701fc-7454", FIXTURE);
        let mut sessions = IflowProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1, "iFlow never fork-splits");
        sessions.remove(0)
    }

    #[test]
    fn parses_session_with_meta() {
        let s = parse_fixture();
        assert_eq!(s.meta.id, "iflow:5de701fc-7454");
        // Windows cwd normalizes to its basename for the project.
        assert_eq!(s.meta.project, "docker-image-retagger");
        assert_eq!(
            s.meta.cwd.as_deref(),
            Some("C:\\exp\\docker-image-retagger")
        );
        assert_eq!(s.meta.git_branch, None, "gitBranch is null in fixture");
        assert_eq!(
            s.meta.first_message,
            "启动app时确保环境变量 DOCKER_API_VERSION"
        );
        // user prompt + merged burst + final assistant (tool_result-only
        // user entries attach results, they are not standalone messages).
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.blocks.len(), 3);
        assert_eq!(s.meta.malformed_lines, 1);
        assert!(!s.meta.usage.has_usage, "iFlow carries no usage");
        assert!(s.meta.models.is_empty());
        assert_eq!(
            s.meta.started_at,
            time::parse_timestamp("2026-01-21T05:56:34.812Z", false)
        );
        assert_eq!(
            s.meta.ended_at,
            time::parse_timestamp("2026-01-21T05:57:03.810Z", false)
        );
    }

    #[test]
    fn streaming_burst_merges_into_one_turn() {
        let s = parse_fixture();
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block after user prompt");
        };
        // text from snapshot 1 + two unique tool calls; call_1 deduped
        // first-wins (input must come from snapshot 1, not DIFFERENT.md).
        assert_eq!(a.segments.len(), 3);
        let AssistantSegment::Text { text, .. } = &a.segments[0] else {
            panic!("expected text segment first");
        };
        assert_eq!(text, "Let me read the project files.");
        let AssistantSegment::Tool { execution: t1 } = &a.segments[1] else {
            panic!("expected tool segment");
        };
        assert_eq!(t1.tool_use_id, "call_1");
        assert_eq!(t1.tool_name, "read_file");
        assert_eq!(t1.tool_input["absolute_path"], "C:\\exp\\README.md");
        // nested functionResponse output attached as the tool result.
        let result = t1.result.as_ref().expect("tool result attached");
        assert_eq!(result.output, "readme contents");
        assert_eq!(t1.status, ToolStatus::Complete);
        let AssistantSegment::Tool { execution: t2 } = &a.segments[2] else {
            panic!("expected second tool segment");
        };
        assert_eq!(t2.tool_use_id, "call_2");
        // merged turn keeps the LAST snapshot's timestamp.
        assert_eq!(
            a.timestamp,
            time::parse_timestamp("2026-01-21T05:56:52.487Z", false)
        );
    }

    #[test]
    fn burst_boundaries_are_respected() {
        // Same parentUuid + sub-second gaps, but an interleaved user entry
        // breaks line adjacency; later, adjacent snapshots >1s apart also
        // stay separate. Neither pair may merge.
        let body = r#"{"type":"user","uuid":"u0","parentUuid":null,"timestamp":"2026-01-21T05:00:00.000Z","cwd":"/home/x/app","message":{"content":"hi"}}
{"type":"assistant","uuid":"a1","parentUuid":"u0","timestamp":"2026-01-21T05:00:01.000Z","message":{"content":[{"type":"text","text":"first"}]}}
{"type":"user","uuid":"u1","parentUuid":"a1","timestamp":"2026-01-21T05:00:01.100Z","message":{"content":"between"}}
{"type":"assistant","uuid":"a2","parentUuid":"u0","timestamp":"2026-01-21T05:00:01.200Z","message":{"content":[{"type":"text","text":"second"}]}}
{"type":"assistant","uuid":"a3","parentUuid":"u0","timestamp":"2026-01-21T05:00:10.000Z","message":{"content":[{"type":"text","text":"third"}]}}
{"type":"assistant","uuid":"a4","parentUuid":"u0","timestamp":"2026-01-21T05:00:11.500Z","message":{"content":[{"type":"text","text":"fourth"}]}}
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_session(dir.path(), "p", "b1", body);
        let s = &IflowProvider.parse(&path).unwrap()[0];
        // 2 user + 4 unmerged assistant turns.
        assert_eq!(s.blocks.len(), 6);
        assert_eq!(s.meta.message_count, 6);
        assert_eq!(s.meta.user_message_count, 2);
        assert_eq!(s.meta.project, "app", "unix cwd basename");
        let texts: Vec<&str> = s
            .blocks
            .iter()
            .filter_map(|b| match b {
                ConversationBlock::Assistant(a) => match &a.segments[0] {
                    AssistantSegment::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["first", "second", "third", "fourth"]);
    }

    #[test]
    fn meta_command_and_system_entries_are_filtered() {
        let body = r#"{"type":"user","uuid":"u1","parentUuid":null,"timestamp":"2026-01-21T05:00:00.000Z","isMeta":true,"message":{"content":"Caveat: injected context"}}
{"type":"user","uuid":"u2","parentUuid":"u1","timestamp":"2026-01-21T05:00:01.000Z","cwd":"/w/proj","message":{"content":"<command-name>/goal</command-name>\n<command-args>ship it</command-args>"}}
{"type":"user","uuid":"u3","parentUuid":"u2","timestamp":"2026-01-21T05:00:02.000Z","isCompactSummary":true,"message":{"content":"summary of earlier turns"}}
{"type":"user","uuid":"u4","parentUuid":"u3","timestamp":"2026-01-21T05:00:03.000Z","message":{"content":"This session is being continued from a previous conversation."}}
{"type":"user","uuid":"u5","parentUuid":"u4","timestamp":"2026-01-21T05:00:04.000Z","message":{"content":"<command-name></command-name>"}}
{"type":"assistant","uuid":"a1","parentUuid":"u5","timestamp":"2026-01-21T05:00:05.000Z","message":{"content":[{"type":"text","text":"ok"}]}}
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_session(dir.path(), "p", "f1", body);
        let s = &IflowProvider.parse(&path).unwrap()[0];
        // Only the normalized command + the assistant reply survive.
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.first_message, "/goal ship it");
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block");
        };
        assert_eq!(u.text, "/goal ship it");
    }

    #[test]
    fn noise_only_sessions_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let meta_only = r#"{"type":"user","uuid":"u1","parentUuid":null,"timestamp":"2026-01-21T05:00:00.000Z","isMeta":true,"message":{"content":"injected"}}
"#;
        let path = write_session(dir.path(), "p", "e1", meta_only);
        assert!(IflowProvider.parse(&path).unwrap().is_empty());
        let path = write_session(dir.path(), "p", "e2", "\n");
        assert!(IflowProvider.parse(&path).unwrap().is_empty());
    }

    #[test]
    fn discovery_walks_project_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_session(root, "projA", "abc-123", FIXTURE);
        std::fs::write(root.join("projA/notes.jsonl"), "{}").unwrap();
        std::fs::write(root.join("projA/session-x.txt"), "nope").unwrap();
        // session files directly under the root (not in a project dir)
        // are not part of the layout.
        std::fs::write(root.join("session-top.jsonl"), "{}").unwrap();
        let found = IflowProvider.discover(root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "iflow:abc-123");
        assert_eq!(found[0].provider, ProviderKind::Iflow);
        assert_eq!(found[0].project_hint.as_deref(), Some("projA"));
    }
}
