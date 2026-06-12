// crates/providers/src/parsers/hermes/mod.rs
//
// Hermes Agent — root `~/.hermes/sessions`, THREE sources reconciled
// (ported from agentsview's hermes.go):
//   1. sibling `state.db` SQLite (`<root>/../state.db`) — authoritative
//      session registry + the only token accounting in the format;
//   2. `<yyyymmdd_hhmmss_hex>.jsonl` transcripts;
//   3. `session_<id>.json` envelopes.
// For each state.db session the richer of session_<id>.json, <id>.jsonl, or
// the state.db `messages` rows wins on a quality score; orphan transcript
// files (not in state.db) parse standalone; an unreadable state.db falls
// back wholesale to transcript files.
//
// Deviations from the Go source (deliberate, per the port rules):
//   - usage-only sessions (tokens but zero contentful messages) are skipped
//     — Trust over Accuracy beats Go's usage-event bookkeeping;
//   - session timestamps form a widened envelope instead of being hard
//     overridden by state.db values;
//   - cost columns / reasoning_tokens / parent_session_id are not surfaced.

mod msg;
mod state;
mod transcript;

use crate::discover::{split_virtual_path, stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use transcript::TranscriptDoc;

pub struct HermesProvider;

impl Provider for HermesProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Hermes
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let (state_db, sessions_dir) = resolve_layout(root);
        let mut out = Vec::new();
        let mut covered: HashSet<String> = HashSet::new();

        // Prefer state.db enumeration when present and readable.
        if let Some(db) = state_db {
            match state::open_ro(&db).and_then(|conn| state::session_ids(&conn)) {
                Ok(ids) => {
                    if let Some((mtime, size_bytes)) = stat_entry(&db) {
                        for raw in ids {
                            out.push(DiscoveredSession {
                                id: ProviderKind::Hermes.session_id(&raw),
                                provider: ProviderKind::Hermes,
                                path: virtual_path(&db, &raw),
                                project_hint: None,
                                mtime,
                                size_bytes,
                            });
                            covered.insert(raw);
                        }
                    }
                }
                Err(e) => tracing::debug!(
                    db = %db.display(),
                    error = %e,
                    "hermes state.db unreadable; falling back to transcripts"
                ),
            }
        }

        // Transcript files: orphans only when state.db enumerated above.
        for file in transcript_files(&sessions_dir) {
            let Some(name) = file.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let raw = session_id_from_name(name);
            if covered.contains(&raw) {
                continue;
            }
            let Some((mtime, size_bytes)) = stat_entry(&file) else {
                continue;
            };
            out.push(DiscoveredSession {
                id: ProviderKind::Hermes.session_id(&raw),
                provider: ProviderKind::Hermes,
                path: file,
                project_hint: None,
                mtime,
                size_bytes,
            });
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        if let Some((db, raw_id)) = split_virtual_path(path) {
            return parse_state_session(&db, &raw_id);
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            anyhow::bail!("hermes: invalid session path {}", path.display());
        };
        let doc = if name.ends_with(".json") {
            transcript::parse_json(path)?
        } else {
            transcript::parse_jsonl(path)?
        };
        let raw_id = session_id_from_name(name);
        Ok(build_session(&raw_id, path.to_path_buf(), doc, None))
    }
}

/// Parse one state.db-registered session: pick the richest message stream
/// (session_<id>.json, then <id>.jsonl, then the state.db rows) and overlay
/// the state.db metadata + usage.
fn parse_state_session(db: &Path, raw_id: &str) -> anyhow::Result<Vec<ForeignSession>> {
    let conn = state::open_ro(db)?;
    let Some(ss) = state::session_row(&conn, raw_id)? else {
        // Session vanished between discovery and parse.
        return Ok(Vec::new());
    };
    let state_msgs = state::session_messages(&conn, raw_id)?;
    let state_score = msg::quality(&state_msgs);
    let sessions_dir = db
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sessions");

    let mut chosen: Option<(TranscriptDoc, PathBuf)> = None;
    let json_path = sessions_dir.join(format!("session_{raw_id}.json"));
    if json_path.is_file() {
        if let Ok(doc) = transcript::parse_json(&json_path) {
            if !doc.msgs.is_empty() && msg::quality(&doc.msgs) >= state_score {
                chosen = Some((doc, json_path));
            }
        }
    }
    if chosen.is_none() {
        let jsonl_path = sessions_dir.join(format!("{raw_id}.jsonl"));
        if jsonl_path.is_file() {
            if let Ok(doc) = transcript::parse_jsonl(&jsonl_path) {
                if !doc.msgs.is_empty()
                    && (msg::quality(&doc.msgs) >= state_score || state_msgs.is_empty())
                {
                    chosen = Some((doc, jsonl_path));
                }
            }
        }
    }

    let (doc, source_path) = chosen.unwrap_or_else(|| {
        (
            TranscriptDoc {
                msgs: state_msgs,
                ..TranscriptDoc::default()
            },
            virtual_path(db, raw_id),
        )
    });
    Ok(build_session(raw_id, source_path, doc, Some(&ss)))
}

/// Assemble the final ForeignSession from the chosen message stream plus
/// optional state.db metadata. Empty when no contentful messages remain.
fn build_session(
    raw_id: &str,
    source_path: PathBuf,
    doc: TranscriptDoc,
    db_session: Option<&state::StateSession>,
) -> Vec<ForeignSession> {
    let mut meta = ForeignSessionMeta::new(ProviderKind::Hermes, raw_id, source_path);
    meta.malformed_lines = doc.malformed_lines;
    let source = db_session.map_or("", |s| s.source.as_str());
    meta.project = project_name(source, &doc.platform);
    if let Some(ss) = db_session {
        meta.title = (!ss.title.is_empty()).then(|| ss.title.clone());
        if let Some(ts) = ss.started_at {
            meta.observe_timestamp(ts);
        }
        if let Some(ts) = ss.ended_at {
            meta.observe_timestamp(ts);
        }
        if !ss.model.is_empty() {
            meta.record_model(&ss.model);
            // The sessions row is the ONLY token accounting in the format;
            // one aggregate record per session, attributed to its model.
            if !ss.usage.is_zero() {
                meta.usage.record(&ss.model, ss.usage);
            }
        }
    }
    if !doc.model.is_empty() {
        meta.record_model(&doc.model);
    }
    if let Some(ts) = doc.started_at {
        meta.observe_timestamp(ts);
    }
    if let Some(ts) = doc.ended_at {
        meta.observe_timestamp(ts);
    }
    let blocks = msg::build_blocks(raw_id, doc.msgs, &mut meta);
    if meta.message_count == 0 {
        // Non-interactive noise (or usage-only state rows) — skip.
        return Vec::new();
    }
    vec![ForeignSession { meta, blocks }]
}

/// `hermes-<source>` (state.db) beats `hermes-<platform>` (transcript
/// header) beats the bare `hermes` fallback — Go's derivation order.
fn project_name(source: &str, platform: &str) -> String {
    if !source.is_empty() {
        format!("hermes-{source}")
    } else if !platform.is_empty() {
        format!("hermes-{platform}")
    } else {
        "hermes".to_string()
    }
}

/// Locate the optional state.db and the transcript dir for a configured
/// root (default root is `~/.hermes/sessions`; env overrides may point at
/// `~/.hermes` or directly at state.db).
fn resolve_layout(root: &Path) -> (Option<PathBuf>, PathBuf) {
    if root.is_file() && root.file_name().is_some_and(|n| n == "state.db") {
        let dir = root.parent().unwrap_or_else(|| Path::new("."));
        return (Some(root.to_path_buf()), dir.join("sessions"));
    }
    let in_root = root.join("state.db");
    if in_root.is_file() {
        return (Some(in_root), root.join("sessions"));
    }
    if root.file_name().is_some_and(|n| n == "sessions") {
        if let Some(parent) = root.parent() {
            let sibling = parent.join("state.db");
            if sibling.is_file() {
                return (Some(sibling), root.to_path_buf());
            }
        }
    }
    // No state.db; Go also probes a nested `sessions/` dir for transcripts.
    let child = root.join("sessions");
    if child.is_dir() {
        return (None, child);
    }
    (None, root.to_path_buf())
}

/// Transcript files in one dir: every `*.jsonl`, plus `session_*.json`
/// envelopes whose session id is not already covered by a .jsonl.
fn transcript_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut jsonl_ids: HashSet<String> = HashSet::new();
    let mut jsonl_files = Vec::new();
    let mut json_files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.ends_with(".jsonl") {
            jsonl_ids.insert(session_id_from_name(name));
            jsonl_files.push(path);
        } else if name.ends_with(".json") && name.starts_with("session_") {
            json_files.push(path);
        }
    }
    let mut out = jsonl_files;
    for path in json_files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !jsonl_ids.contains(&session_id_from_name(name)) {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// `session_20260403_abc.json` → `20260403_abc`;
/// `20260403_153620_hex.jsonl` → `20260403_153620_hex`.
fn session_id_from_name(name: &str) -> String {
    let s = name
        .strip_suffix(".jsonl")
        .or_else(|| name.strip_suffix(".json"))
        .unwrap_or(name);
    s.strip_prefix("session_").unwrap_or(s).to_string()
}

fn virtual_path(db: &Path, raw_id: &str) -> PathBuf {
    PathBuf::from(format!("{}#{raw_id}", db.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::{
        AssistantSegment, ConversationBlock, NoticeVariant, ToolStatus,
    };
    use serde_json::Value;

    const JSONL_FIXTURE: &str = r#"{"role":"session_meta","platform":"linux","model":"hermes-4-405b","timestamp":"2026-04-03T10:00:00Z"}
{"role":"user","content":"Read main.go","timestamp":"2026-04-03T10:00:05Z"}
{"role":"assistant","content":"","reasoning":"Let me read it.","finish_reason":"tool_calls","tool_calls":[{"id":"tc1","function":{"name":"read_file","arguments":"{\"path\":\"main.go\"}"}}],"timestamp":"2026-04-03T10:00:06Z"}
{"role":"tool","content":"package main\n","tool_call_id":"tc1","timestamp":"2026-04-03T10:00:07Z"}
{"role":"assistant","content":"Done.","timestamp":"2026-04-03T10:05:00Z"}
"#;

    /// state.db fixture mirroring the real Hermes schema (extra columns
    /// present but unread by the parser).
    fn create_state_db(root: &Path) -> PathBuf {
        let db = root.join("state.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                model TEXT,
                parent_session_id TEXT,
                started_at REAL NOT NULL,
                ended_at REAL,
                message_count INTEGER DEFAULT 0,
                input_tokens INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                cache_read_tokens INTEGER DEFAULT 0,
                cache_write_tokens INTEGER DEFAULT 0,
                reasoning_tokens INTEGER DEFAULT 0,
                estimated_cost_usd REAL,
                actual_cost_usd REAL,
                cost_status TEXT,
                cost_source TEXT,
                title TEXT,
                api_call_count INTEGER DEFAULT 0
            );
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                tool_call_id TEXT,
                tool_calls TEXT,
                timestamp REAL NOT NULL,
                reasoning TEXT
            );
            INSERT INTO sessions (
                id, source, model, parent_session_id, started_at, ended_at,
                message_count, input_tokens, output_tokens, cache_read_tokens,
                cache_write_tokens, reasoning_tokens, estimated_cost_usd,
                cost_status, cost_source, title, api_call_count
            ) VALUES (
                'child', 'discord', 'gpt-5.4', 'parent',
                1778767200.0, 1778767800.0, 1, 300, 70, 20, 5, 9,
                0.123, 'estimated', 'hermes', 'Child Session', 4
            );
            INSERT INTO messages (session_id, role, content, timestamp)
            VALUES ('child', 'user', 'state db only has one message', 1778767210.0);
            "#,
        )
        .unwrap();
        db
    }

    /// tempdir shaped like ~/.hermes: returns (guard, sessions dir).
    fn hermes_home() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        (dir, sessions)
    }

    #[test]
    fn parses_jsonl_transcript_with_tools_and_thinking() {
        let (_g, sessions) = hermes_home();
        let path = sessions.join("20260403_153620_5a3e2ff1.jsonl");
        std::fs::write(&path, JSONL_FIXTURE).unwrap();
        let mut out = HermesProvider.parse(&path).unwrap();
        assert_eq!(out.len(), 1);
        let s = out.remove(0);

        assert_eq!(s.meta.id, "hermes:20260403_153620_5a3e2ff1");
        assert_eq!(s.meta.project, "hermes-linux");
        assert_eq!(s.meta.models, vec!["hermes-4-405b".to_string()]);
        assert!(!s.meta.usage.has_usage, "transcripts carry no tokens");
        assert_eq!(s.meta.first_message, "Read main.go");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.malformed_lines, 0);
        // Envelope spans session_meta ts → last assistant ts (UTC inputs).
        assert_eq!(s.meta.started_at, Some(1775210400.0));
        assert_eq!(s.meta.ended_at, Some(1775210700.0));

        assert_eq!(s.blocks.len(), 3);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block");
        };
        assert_eq!(a.thinking.as_deref(), Some("Let me read it."));
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment");
        };
        assert_eq!(execution.tool_name, "read_file");
        assert_eq!(execution.tool_use_id, "tc1");
        assert_eq!(execution.tool_input, serde_json::json!({"path": "main.go"}));
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "package main\n");
    }

    #[test]
    fn json_envelope_skill_prefix_and_compaction() {
        let (_g, sessions) = hermes_home();
        let path = sessions.join("session_test1.json");
        std::fs::write(
            &path,
            r#"{
              "platform": "darwin",
              "session_start": "2026-05-14T10:00:00Z",
              "last_updated": "2026-05-14T10:20:00Z",
              "messages": [
                {"role":"user","content":"[CONTEXT COMPACTION - REFERENCE ONLY]\nold context","timestamp":"2026-05-14T10:01:00Z"},
                {"role":"user","content":"[SYSTEM: The user has invoked the \"commit\" skill.]\n\n---\nname: commit\n---\nbody\n\nThe user has provided the following instruction alongside the skill invocation: Please commit my changes\n\n[Runtime note: internal]","timestamp":"2026-05-14T10:02:00Z"}
              ]
            }"#,
        )
        .unwrap();
        let mut out = HermesProvider.parse(&path).unwrap();
        assert_eq!(out.len(), 1);
        let s = out.remove(0);

        assert_eq!(s.meta.id, "hermes:test1");
        assert_eq!(s.meta.project, "hermes-darwin");
        // Compaction boundary is NOT a user message and never first_message.
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.first_message, "Please commit my changes");
        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::Notice(n) = &s.blocks[0] else {
            panic!("expected compaction notice");
        };
        assert_eq!(n.variant, NoticeVariant::ContextCompacted);
        assert_eq!(
            n.data.get("summary").and_then(Value::as_str),
            Some("old context")
        );
        let ConversationBlock::User(u) = &s.blocks[1] else {
            panic!("expected user block");
        };
        assert_eq!(u.text, "Please commit my changes");
    }

    #[test]
    fn state_db_session_prefers_richer_json_transcript() {
        let (_g, sessions) = hermes_home();
        let db = create_state_db(sessions.parent().unwrap());
        std::fs::write(
            sessions.join("session_child.json"),
            r#"{
              "platform": "discord",
              "session_start": "2026-05-14T10:00:00Z",
              "last_updated": "2026-05-14T10:20:00Z",
              "messages": [
                {"role":"user","content":"hello from transcript","timestamp":"2026-05-14T10:01:00Z"},
                {"role":"assistant","content":"reply from transcript","timestamp":"2026-05-14T10:02:00Z"}
              ]
            }"#,
        )
        .unwrap();

        let mut out = HermesProvider.parse(&virtual_path(&db, "child")).unwrap();
        assert_eq!(out.len(), 1);
        let s = out.remove(0);

        assert_eq!(s.meta.id, "hermes:child");
        assert_eq!(s.meta.title.as_deref(), Some("Child Session"));
        assert_eq!(s.meta.project, "hermes-discord");
        assert_eq!(s.meta.first_message, "hello from transcript");
        // Transcript won the quality contest → blocks + source from the file.
        assert_eq!(s.blocks.len(), 2);
        assert_eq!(s.meta.source_path, sessions.join("session_child.json"));
        // Usage always comes from state.db; cache_write → cache_creation.
        assert!(s.meta.usage.has_usage);
        let totals = &s.meta.usage.per_model["gpt-5.4"];
        assert_eq!(totals.input_tokens, 300);
        assert_eq!(totals.output_tokens, 70);
        assert_eq!(totals.cache_read_input_tokens, 20);
        assert_eq!(totals.cache_creation_input_tokens, 5);
        assert_eq!(s.meta.models, vec!["gpt-5.4".to_string()]);
        // Envelope = min(transcript start, db start) … max(db end, …).
        assert_eq!(s.meta.started_at, Some(1778752800.0));
        assert_eq!(s.meta.ended_at, Some(1778767800.0));
    }

    #[test]
    fn state_db_rows_win_over_thin_jsonl() {
        let (_g, sessions) = hermes_home();
        let db = create_state_db(sessions.parent().unwrap());
        // Quality 1001 (one 1-char msg) loses to state's 1029.
        std::fs::write(
            sessions.join("child.jsonl"),
            "{\"role\":\"session_meta\",\"platform\":\"discord\"}\n{\"role\":\"user\",\"content\":\"x\",\"timestamp\":\"2026-05-14T10:01:00Z\"}\n",
        )
        .unwrap();

        let mut out = HermesProvider.parse(&virtual_path(&db, "child")).unwrap();
        assert_eq!(out.len(), 1);
        let s = out.remove(0);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block");
        };
        assert_eq!(u.text, "state db only has one message");
        assert_eq!(s.meta.source_path, virtual_path(&db, "child"));
        assert!(s.meta.usage.has_usage);
    }

    #[test]
    fn discovery_prefers_state_db_and_dedups_transcripts() {
        let (_g, sessions) = hermes_home();
        let db = create_state_db(sessions.parent().unwrap());
        // child is in state.db → its transcript must NOT double-discover.
        std::fs::write(sessions.join("child.jsonl"), "{}\n").unwrap();
        // Orphans: jsonl wins over the same-id json envelope.
        std::fs::write(sessions.join("extra.jsonl"), "{}\n").unwrap();
        std::fs::write(sessions.join("session_extra.json"), "{}").unwrap();
        std::fs::write(sessions.join("session_orphan.json"), "{}").unwrap();
        std::fs::write(sessions.join("config.json"), "{}").unwrap();
        std::fs::write(sessions.join("notes.txt"), "hi").unwrap();
        std::fs::create_dir_all(sessions.join("subdir")).unwrap();
        std::fs::write(sessions.join("subdir").join("nested.jsonl"), "{}\n").unwrap();

        let mut found = HermesProvider.discover(&sessions);
        found.sort_by(|a, b| a.id.cmp(&b.id));
        let ids: Vec<&str> = found.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["hermes:child", "hermes:extra", "hermes:orphan"]);
        assert_eq!(found[0].path, virtual_path(&db, "child"));
        assert_eq!(found[1].path, sessions.join("extra.jsonl"));
        assert_eq!(found[2].path, sessions.join("session_orphan.json"));
    }

    #[test]
    fn discovery_falls_back_when_state_db_unreadable_or_absent() {
        // Corrupt state.db → wholesale transcript fallback.
        let (_g, sessions) = hermes_home();
        std::fs::write(sessions.parent().unwrap().join("state.db"), "not sqlite").unwrap();
        std::fs::write(sessions.join("a.jsonl"), "{}\n").unwrap();
        let found = HermesProvider.discover(&sessions);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "hermes:a");
        assert_eq!(found[0].path, sessions.join("a.jsonl"));

        // No state.db at all.
        let (_g2, sessions2) = hermes_home();
        std::fs::write(sessions2.join("b.jsonl"), "{}\n").unwrap();
        std::fs::write(sessions2.join("session_c.json"), "{}").unwrap();
        let mut found = HermesProvider.discover(&sessions2);
        found.sort_by(|a, b| a.id.cmp(&b.id));
        let ids: Vec<&str> = found.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["hermes:b", "hermes:c"]);
    }

    #[test]
    fn empty_and_usage_only_sessions_are_skipped() {
        // Transcript with only a session_meta header → no session.
        let (_g, sessions) = hermes_home();
        let path = sessions.join("20260403_000000_aa.jsonl");
        std::fs::write(
            &path,
            "{\"role\":\"session_meta\",\"platform\":\"linux\"}\n",
        )
        .unwrap();
        assert!(HermesProvider.parse(&path).unwrap().is_empty());

        // state.db session with tokens but zero messages and no transcript
        // → skipped (deviation from Go, which keeps usage-only sessions).
        let db = create_state_db(sessions.parent().unwrap());
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute(
                "INSERT INTO sessions (id, source, model, started_at, input_tokens)
                 VALUES ('usage-only', 'cli', 'gpt-5.4', 1778767200.0, 10)",
                [],
            )
            .unwrap();
        }
        assert!(HermesProvider
            .parse(&virtual_path(&db, "usage-only"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn malformed_lines_are_counted_not_hidden() {
        let (_g, sessions) = hermes_home();
        let path = sessions.join("20260403_000000_bb.jsonl");
        std::fs::write(
            &path,
            "{\"role\":\"session_meta\"}\nnot json at all\n{\"role\":\"user\",\"content\":\"hello\",\"timestamp\":\"2026-04-03T10:00:00Z\"}\n",
        )
        .unwrap();
        let s = &HermesProvider.parse(&path).unwrap()[0];
        assert_eq!(s.meta.malformed_lines, 1);
        assert_eq!(s.meta.message_count, 1);
    }

    #[test]
    fn naive_timestamps_resolve_as_local_time() {
        let (_g, sessions) = hermes_home();
        let path = sessions.join("20260403_000000_cc.jsonl");
        std::fs::write(
            &path,
            "{\"role\":\"user\",\"content\":\"hi\",\"timestamp\":\"2026-04-03T15:27:21.014566\"}\n",
        )
        .unwrap();
        let s = &HermesProvider.parse(&path).unwrap()[0];
        let want = crate::util::time::parse_timestamp("2026-04-03T15:27:21.014566", true).unwrap();
        assert_eq!(s.meta.started_at, Some(want));
    }
}
