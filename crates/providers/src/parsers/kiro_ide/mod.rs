// crates/providers/src/parsers/kiro_ide/mod.rs
//
// Kiro IDE — VS Code-fork agent storing sessions under
// `<root> = …/Kiro/User/globalStorage/kiro.kiroagent` in TWO generations:
//
// OLD (chat_format.rs): `<root>/<ws-hash>/<exec-hash>.chat` single-JSON
//   { executionId, chat: [{role: human|bot|tool, content}],
//     metadata: {modelId, startTime/endTime epoch-ms} }
//   `<ws-hash>` = hex(sha256(workspaceDirectory))[:32]; the project name is
//   recovered by reverse-lookup against workspace-sessions/*/sessions.json.
//
// NEW (new_format.rs): `<root>/workspace-sessions/<b64-path>/<uuid>.json`
//   { sessionId, title, workspaceDirectory,
//     history: [{message: {role, content: string | [{type:'text',text}]},
//                promptLogs: [{completion}], executionId}] }
//   Assistant content lives in SEPARATE exec-log files (exec_log.rs) at
//   `<root>/<ws-hash>/414d1636299d2b9e4ce7e17fb11f63e9/<file>` keyed by
//   executionId (sniffed from each file's first 1 KiB):
//   actions 'say' → text, 'replace' → Edit, 'create' → Write,
//   'readCode' → Read. When exec logs are missing the concatenated
//   promptLogs completions are the fallback transcript.
//
// Neither generation carries token usage — `has_usage` stays false.

mod chat_format;
mod exec_log;
mod new_format;

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use serde_json::Value;
use std::path::Path;

/// Fixed dir name holding new-format execution logs inside `<root>/<ws-hash>/`.
const EXEC_SUBDIR: &str = "414d1636299d2b9e4ce7e17fb11f63e9";

/// Top-level dirs that never hold old-format workspace chat dirs.
const SKIP_DIRS: [&str; 4] = ["default", "dev_data", "index", "workspace-sessions"];

pub struct KiroIdeProvider;

impl Provider for KiroIdeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::KiroIde
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let mut out = Vec::new();
        // Old format: <root>/<ws-hash>/<exec-hash>.chat
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let dir = entry.path();
                let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !dir.is_dir() || SKIP_DIRS.contains(&name) || name.starts_with('.') {
                    continue;
                }
                let Ok(chat_files) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for cf in chat_files.flatten() {
                    let path = cf.path();
                    if path.is_dir() || path.extension().and_then(|e| e.to_str()) != Some("chat") {
                        continue;
                    }
                    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    let Some((mtime, size_bytes)) = stat_entry(&path) else {
                        continue;
                    };
                    out.push(DiscoveredSession {
                        id: ProviderKind::KiroIde.session_id(&format!("{name}:{stem}")),
                        provider: ProviderKind::KiroIde,
                        path,
                        project_hint: None,
                        mtime,
                        size_bytes,
                    });
                }
            }
        }
        // New format: <root>/workspace-sessions/<b64-path>/<uuid>.json
        if let Ok(ws_dirs) = std::fs::read_dir(root.join("workspace-sessions")) {
            for ws in ws_dirs.flatten() {
                let ws_dir = ws.path();
                if !ws_dir.is_dir() {
                    continue;
                }
                let Ok(files) = std::fs::read_dir(&ws_dir) else {
                    continue;
                };
                for jf in files.flatten() {
                    let path = jf.path();
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    if path.is_dir() || name == "sessions.json" || !name.ends_with(".json") {
                        continue;
                    }
                    let Some((mtime, size_bytes)) = stat_entry(&path) else {
                        continue;
                    };
                    out.push(DiscoveredSession {
                        id: ProviderKind::KiroIde.session_id(name.trim_end_matches(".json")),
                        provider: ProviderKind::KiroIde,
                        path,
                        project_hint: None,
                        mtime,
                        size_bytes,
                    });
                }
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            new_format::parse(path)
        } else {
            chat_format::parse(path)
        }
    }
}

/// First entry's workspaceDirectory from a sessions.json index.
fn first_workspace_dir(sessions_json: &Path) -> Option<String> {
    let raw = crate::util::read_to_string_capped(sessions_json).ok()?;
    let doc: Value = serde_json::from_str(&raw).ok()?;
    let ws = doc
        .as_array()?
        .first()?
        .get("workspaceDirectory")?
        .as_str()?;
    (!ws.is_empty()).then(|| ws.to_string())
}

/// hex(sha256(s))[:32] — Kiro's workspace dir hash.
fn hash32(s: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let digest = Sha256::digest(s.as_bytes());
    let mut hex = String::with_capacity(32);
    for b in digest.iter().take(16) {
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::exec_log::sniff_execution_id;
    use super::*;
    use claude_view_types::block_types::{AssistantSegment, ConversationBlock, ToolStatus};
    use std::path::PathBuf;

    const WS_DIR: &str = "/Users/dev/projects/my-app";

    /// Build an old-format fixture tree:
    /// root/<hash32(WS_DIR)>/abc123.chat + workspace-sessions index for the
    /// project reverse-lookup.
    fn write_old_fixture(root: &Path) -> PathBuf {
        let ws_hash = hash32(WS_DIR);
        let chat_dir = root.join(&ws_hash);
        std::fs::create_dir_all(&chat_dir).unwrap();
        let chat_path = chat_dir.join("abc123.chat");
        std::fs::write(
            &chat_path,
            r##"{
              "executionId": "exec-old-1",
              "chat": [
                { "role": "human", "content": "# System Prompt\nYou are Kiro." },
                { "role": "human", "content": "<kiro-ide-message>fix the login bug</kiro-ide-message>" },
                { "role": "bot", "content": "I will follow these instructions." },
                { "role": "bot", "content": "Patched the auth handler." },
                { "role": "tool", "content": "raw tool output" }
              ],
              "metadata": { "modelId": "claude-sonnet-4", "startTime": 1767323045000, "endTime": 1767326645000 }
            }"##,
        )
        .unwrap();
        let idx_dir = root.join("workspace-sessions").join("L1VzZXJzL2Rldg");
        std::fs::create_dir_all(&idx_dir).unwrap();
        std::fs::write(
            idx_dir.join("sessions.json"),
            format!(r#"[{{ "sessionId": "s1", "workspaceDirectory": "{WS_DIR}" }}]"#),
        )
        .unwrap();
        chat_path
    }

    #[test]
    fn parses_old_chat_format_with_filters() {
        let dir = tempfile::tempdir().unwrap();
        let chat_path = write_old_fixture(dir.path());
        let mut sessions = KiroIdeProvider.parse(&chat_path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        let ws_hash = hash32(WS_DIR);
        assert_eq!(s.meta.id, format!("kiro-ide:{ws_hash}:abc123"));
        // Project recovered by sha256 reverse-lookup through sessions.json.
        assert_eq!(s.meta.project, "my-app");
        // System prompt + ack + tool rows filtered; wrapper stripped.
        assert_eq!(s.meta.first_message, "fix the login bug");
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.models, vec!["claude-sonnet-4".to_string()]);
        assert_eq!(s.meta.started_at, Some(1767323045.0));
        assert_eq!(s.meta.ended_at, Some(1767326645.0));
        assert!(!s.meta.usage.has_usage, "kiro-ide carries no token usage");
        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block");
        };
        assert_eq!(u.text, "fix the login bug");
    }

    /// Build a new-format fixture tree with a resolvable exec log.
    fn write_new_fixture(root: &Path, with_exec_log: bool) -> PathBuf {
        let ws = "/Users/dev/projects/web-app";
        let session_dir = root.join("workspace-sessions").join("d2ViLWFwcA");
        std::fs::create_dir_all(&session_dir).unwrap();
        let session_path = session_dir.join("3f9a1c2e-1111-2222-3333-444455556666.json");
        std::fs::write(
            &session_path,
            format!(
                r#"{{
                  "sessionId": "3f9a1c2e-1111-2222-3333-444455556666",
                  "title": "Add dark mode",
                  "workspaceDirectory": "{ws}",
                  "history": [
                    {{ "message": {{ "role": "user", "id": "m1",
                       "content": [ {{ "type": "text", "text": "add dark mode" }} ] }} }},
                    {{ "message": {{ "role": "assistant", "id": "m2", "content": "" }},
                       "promptLogs": [ {{ "completion": "fallback completion text" }} ],
                       "executionId": "exec-42" }},
                    {{ "message": {{ "role": "tool", "id": "m3", "content": "tool result" }} }}
                  ]
                }}"#
            ),
        )
        .unwrap();
        if with_exec_log {
            std::fs::write(
                session_dir.join("sessions.json"),
                format!(r#"[{{ "sessionId": "x", "workspaceDirectory": "{ws}" }}]"#),
            )
            .unwrap();
            let exec_dir = root.join(hash32(ws)).join(EXEC_SUBDIR);
            std::fs::create_dir_all(&exec_dir).unwrap();
            std::fs::write(
                exec_dir.join("log1.json"),
                r#"{ "version": 1, "executionId": "exec-42", "actions": [
                  { "actionId": "a1", "actionType": "say",
                    "input": {}, "output": { "message": "Adding dark mode now." } },
                  { "actionId": "a2", "actionType": "replace",
                    "input": { "file": "src/theme.ts", "originalContent": "light", "modifiedContent": "dark" },
                    "output": {} },
                  { "actionId": "a3", "actionType": "create",
                    "input": { "file": "src/dark.css", "modifiedContent": "body{}" }, "output": {} },
                  { "actionId": "a4", "actionType": "readCode",
                    "input": { "file": "src/app.ts" }, "output": {} }
                ] }"#,
            )
            .unwrap();
        }
        session_path
    }

    #[test]
    fn new_format_joins_exec_logs_into_tools() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_new_fixture(dir.path(), true);
        let mut sessions = KiroIdeProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "kiro-ide:3f9a1c2e-1111-2222-3333-444455556666");
        assert_eq!(s.meta.title.as_deref(), Some("Add dark mode"));
        assert_eq!(s.meta.project, "web-app");
        assert_eq!(s.meta.first_message, "add dark mode");
        assert_eq!(s.blocks.len(), 2, "tool role rows are skipped");
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block");
        };
        // Exec-log 'say' wins over the promptLogs fallback.
        let AssistantSegment::Text { text, .. } = &a.segments[0] else {
            panic!("expected text segment");
        };
        assert_eq!(text, "Adding dark mode now.");
        let names: Vec<&str> = a.segments[1..]
            .iter()
            .map(|seg| {
                let AssistantSegment::Tool { execution } = seg else {
                    panic!("expected tool segment");
                };
                assert_eq!(execution.status, ToolStatus::Complete);
                execution.tool_name.as_str()
            })
            .collect();
        assert_eq!(names, ["Edit", "Write", "Read"]);
        // Edit carries the REAL contents — no synthesized diff.
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!();
        };
        assert_eq!(
            execution.tool_input,
            serde_json::json!({
                "file": "src/theme.ts",
                "originalContent": "light",
                "modifiedContent": "dark"
            })
        );
        assert_eq!(execution.tool_use_id, "a2");
    }

    #[test]
    fn new_format_falls_back_to_prompt_logs() {
        let dir = tempfile::tempdir().unwrap();
        // No sessions.json / exec dir → promptLogs are the transcript.
        let path = write_new_fixture(dir.path(), false);
        let mut sessions = KiroIdeProvider.parse(&path).unwrap();
        let s = sessions.remove(0);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block");
        };
        let AssistantSegment::Text { text, .. } = &a.segments[0] else {
            panic!("expected text segment");
        };
        assert_eq!(text, "fallback completion text");
    }

    #[test]
    fn non_interactive_sessions_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        // Old format: every row filtered → no session.
        let chat_dir = dir.path().join("aaaa");
        std::fs::create_dir_all(&chat_dir).unwrap();
        let chat = chat_dir.join("x.chat");
        std::fs::write(
            &chat,
            r##"{ "chat": [
              { "role": "human", "content": "# Identity\nYou are Kiro." },
              { "role": "bot", "content": "I will follow these instructions." }
            ], "metadata": {} }"##,
        )
        .unwrap();
        assert!(KiroIdeProvider.parse(&chat).unwrap().is_empty());
        // New format: empty history → no session.
        let sess_dir = dir.path().join("workspace-sessions").join("eA");
        std::fs::create_dir_all(&sess_dir).unwrap();
        let sess = sess_dir.join("u1.json");
        std::fs::write(&sess, r#"{ "sessionId": "u1", "history": [] }"#).unwrap();
        assert!(KiroIdeProvider.parse(&sess).unwrap().is_empty());
    }

    #[test]
    fn discover_finds_both_formats_and_skips_reserved_dirs() {
        let dir = tempfile::tempdir().unwrap();
        write_old_fixture(dir.path()); // <ws-hash>/abc123.chat + index
        write_new_fixture(dir.path(), false); // workspace-sessions/…/<uuid>.json
        for reserved in ["default", "dev_data", "index", ".hidden"] {
            let d = dir.path().join(reserved);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("nope.chat"), "{}").unwrap();
        }
        let found = KiroIdeProvider.discover(dir.path());
        let ids: Vec<&str> = found.iter().map(|f| f.id.as_str()).collect();
        let ws_hash = hash32(WS_DIR);
        assert_eq!(
            found.len(),
            2,
            "reserved dirs and sessions.json excluded: {ids:?}"
        );
        assert!(ids.contains(&format!("kiro-ide:{ws_hash}:abc123").as_str()));
        assert!(ids.contains(&"kiro-ide:3f9a1c2e-1111-2222-3333-444455556666"));
    }

    #[test]
    fn exec_id_sniff_respects_1k_window() {
        let dir = tempfile::tempdir().unwrap();
        let near = dir.path().join("near.json");
        std::fs::write(&near, r#"{"meta":"x","executionId":"e-1","actions":[]}"#).unwrap();
        assert_eq!(sniff_execution_id(&near).as_deref(), Some("e-1"));
        // executionId past the 1 KiB window is invisible (matches Go).
        let far = dir.path().join("far.json");
        let padding = "x".repeat(1100);
        std::fs::write(
            &far,
            format!(r#"{{"pad":"{padding}","executionId":"e-2"}}"#),
        )
        .unwrap();
        assert_eq!(sniff_execution_id(&far), None);
    }
}
