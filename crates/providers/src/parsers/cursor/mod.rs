// crates/providers/src/parsers/cursor/mod.rs
//
// Cursor — agent transcripts at
//   `<root>/<hyphen-encoded-abs-path>/agent-transcripts/<uuid>.txt|.jsonl`
// (flat) or `agent-transcripts/<stem>/<stem>.txt|.jsonl` (nested). Raw
// session id = filename stem; project = DecodeCursorProjectDir on the
// encoded directory name (ported exactly from agentsview cursor.go).
//
// DUAL FORMAT, auto-detected per file by checking whether the first
// non-empty line (located within a 4 KB scan window) is valid JSON:
//   (a) plain text with `user:` / `assistant:` role-marker lines; assistant
//       blocks carry `[Thinking]`, `[Tool call] <name>`, `[Tool result]`
//       headers whose indented bodies end at the first non-empty line at
//       column 0; user text is wrapped in <user_query>…</user_query>
//       (`text_format`);
//   (b) JSONL — one Anthropic-style message per line:
//       { role, message.content: string | [text|thinking|tool_use|…] }
//       (`jsonl_format`).
//
// The format carries NO model name and NO token usage (has_usage stays
// false) and no per-message timestamps (session window = file mtime).
// Deviation from the Go source (which flattens to a content string): the
// real bodies of `[Thinking]` / `[Tool call]` / `[Tool result]` sections
// are preserved as thinking text / tool input / tool results instead of
// being dropped — strictly more of the actual transcript, never invented.

mod jsonl_format;
mod project;
mod text_format;

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::jsonl;
use claude_view_types::block_types::ConversationBlock;
use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

/// Max transcript size we read (Go: maxCursorTranscriptSize = 10 << 20).
const MAX_TRANSCRIPT_BYTES: u64 = 10 * 1024 * 1024;

pub struct CursorProvider;

impl Provider for CursorProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Cursor
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Ok(resolved_root) = root.canonicalize() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            // Symlinked project dirs are rejected (file_type doesn't follow).
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let transcripts_dir = entry.path().join("agent-transcripts");
            // The transcripts dir must resolve strictly inside the root —
            // symlink-escape guard ported from DiscoverCursorSessions.
            let Ok(resolved) = transcripts_dir.canonicalize() else {
                continue;
            };
            if !resolved.starts_with(&resolved_root) || resolved == resolved_root {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&transcripts_dir) else {
                continue;
            };
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            let decoded = project::decode_cursor_project_dir(&dir_name);
            let project = if decoded.is_empty() {
                "unknown".to_string()
            } else {
                decoded
            };
            // Dedup by filename stem, preferring .jsonl over .txt.
            let mut seen: HashMap<String, PathBuf> = HashMap::new();
            for f in files.flatten() {
                let Ok(ftype) = f.file_type() else { continue };
                let name = f.file_name().to_string_lossy().into_owned();
                if ftype.is_dir() {
                    collect_nested(&f.path(), &name, &mut seen);
                } else if ftype.is_file() && is_transcript_ext(&name) {
                    add_seen(&mut seen, &name, f.path());
                }
            }
            for (stem, path) in seen {
                let Some((mtime, size_bytes)) = stat_entry(&path) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id: ProviderKind::Cursor.session_id(&stem),
                    provider: ProviderKind::Cursor,
                    path,
                    project_hint: Some(project.clone()),
                    mtime,
                    size_bytes,
                });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let file = std::fs::File::open(path)?;
        let md = file.metadata()?;
        if !md.is_file() {
            anyhow::bail!("skip {}: not a regular file", path.display());
        }
        // Read one extra byte so growth past the cap is detectable.
        let mut data = Vec::new();
        file.take(MAX_TRANSCRIPT_BYTES + 1).read_to_end(&mut data)?;
        if data.len() as u64 > MAX_TRANSCRIPT_BYTES {
            anyhow::bail!(
                "skip {}: transcript exceeds {} bytes",
                path.display(),
                MAX_TRANSCRIPT_BYTES
            );
        }

        let raw_id = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(strip_ext)
            .unwrap_or("");
        if raw_id.is_empty() {
            anyhow::bail!("skip {}: no session id derivable", path.display());
        }

        let mut meta = ForeignSessionMeta::new(ProviderKind::Cursor, raw_id, path.to_path_buf());
        meta.project = project::project_from_path(path);
        // Session timestamps = file mtime only (the format has none).
        if let Some((mtime, _)) = stat_entry(path) {
            meta.observe_timestamp(mtime);
        }

        let text = String::from_utf8_lossy(&data);
        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        if jsonl_format::is_cursor_jsonl(&text) {
            let read =
                jsonl::read_values_from(BufReader::new(std::io::Cursor::new(data.as_slice())))?;
            meta.malformed_lines = read.malformed;
            let mut ordinal = 0usize;
            for value in &read.values {
                jsonl_format::handle_message(
                    value,
                    raw_id,
                    &mut ordinal,
                    &mut meta,
                    &mut out_blocks,
                );
            }
        } else {
            text_format::parse_transcript(&text, raw_id, &mut meta, &mut out_blocks);
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

/// Nested layout: `agent-transcripts/<stem>/<stem>.{txt,jsonl}` — only files
/// whose stem matches the parent directory name count.
fn collect_nested(sub_dir: &Path, dir_name: &str, seen: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(sub_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_transcript_ext(&name) || strip_ext(&name) != dir_name {
            continue;
        }
        add_seen(seen, &name, entry.path());
    }
}

/// Insert a transcript into the seen map, preferring .jsonl over .txt when
/// both exist for the same stem (port of cursorAddSeen).
fn add_seen(seen: &mut HashMap<String, PathBuf>, name: &str, path: PathBuf) {
    let stem = strip_ext(name).to_string();
    match seen.get(&stem) {
        Some(prev) => {
            if prev.to_str().is_some_and(|p| p.ends_with(".txt")) && name.ends_with(".jsonl") {
                seen.insert(stem, path);
            }
        }
        None => {
            seen.insert(stem, path);
        }
    }
}

fn is_transcript_ext(name: &str) -> bool {
    name.ends_with(".txt") || name.ends_with(".jsonl")
}

/// Strip the final extension like Go's `TrimSuffix(name, filepath.Ext(name))`.
fn strip_ext(name: &str) -> &str {
    name.rfind('.').map_or(name, |i| &name[..i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::AssistantSegment;
    use serde_json::Value;

    const TEXT_FIXTURE: &str = "user:\n\
        <user_query>\n\
        Fix the login bug\n\
        </user_query>\n\
        assistant:\n\
        [Thinking]\n\
        \x20 let me think...\n\
        First I'll check the file.\n\
        [Tool call] ReadFile\n\
        \x20 path=main.go\n\
        [Tool result]\n\
        \x20 package main\n\
        The file looks good.\n\
        user:\n\
        thanks\n";

    const JSONL_FIXTURE: &str = concat!(
        r#"{"role":"user","message":{"content":"<user_query>What is Go?</user_query>"}}"#,
        "\n",
        r#"{"role":"system","message":{"content":"sys"}}"#,
        "\n",
        "not json\n",
        r#"{"role":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"Let me check."},{"type":"tool_use","id":"tu_1","name":"Bash","input":{"command":"ls"}}]}}"#,
        "\n",
        r#"{"role":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu_1","content":[{"type":"text","text":"file1.go"}]}]}}"#,
        "\n",
        r#"{"role":"assistant","message":{"content":"Done."}}"#,
        "\n",
    );

    /// Write `contents` into the real Cursor layout under a temp root.
    fn write_layout(root: &Path, file_name: &str, contents: &str) -> PathBuf {
        let transcripts = root
            .join("Users-fiona-Documents-my-app")
            .join("agent-transcripts");
        std::fs::create_dir_all(&transcripts).unwrap();
        let path = transcripts.join(file_name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn parses_text_transcript_with_markers() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_layout(dir.path(), "0d3f5c1a.txt", TEXT_FIXTURE);
        let mut sessions = CursorProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "cursor:0d3f5c1a");
        assert_eq!(s.meta.project, "my_app");
        assert_eq!(s.meta.first_message, "Fix the login bug");
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.user_message_count, 2);
        assert!(!s.meta.usage.has_usage, "cursor carries no token usage");
        assert!(s.meta.models.is_empty(), "cursor carries no model name");
        assert!(s.meta.started_at.is_some());
        assert_eq!(s.meta.started_at, s.meta.ended_at, "mtime only");
        assert_eq!(s.blocks.len(), 3);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block")
        };
        assert_eq!(u.text, "Fix the login bug");
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("let me think..."));
        assert_eq!(a.segments.len(), 3, "text / tool / text interleave");
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "ReadFile");
        assert_eq!(execution.tool_input, Value::String("path=main.go".into()));
        assert_eq!(execution.result.as_ref().unwrap().output, "package main");
    }

    #[test]
    fn parses_jsonl_transcript_and_counts_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_layout(dir.path(), "abc.jsonl", JSONL_FIXTURE);
        let mut sessions = CursorProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "cursor:abc");
        assert_eq!(s.meta.malformed_lines, 1, "the 'not json' line");
        assert_eq!(s.meta.message_count, 3, "system + result-only skipped");
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.first_message, "What is Go?");
        assert_eq!(s.blocks.len(), 3);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("hmm"));
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "Bash");
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "file1.go",
            "tool_result from the user line attaches to the call"
        );
        let ConversationBlock::Assistant(last) = &s.blocks[2] else {
            panic!("expected assistant block")
        };
        let AssistantSegment::Text { text, .. } = &last.segments[0] else {
            panic!("expected text segment")
        };
        assert_eq!(text, "Done.");
    }

    #[test]
    fn empty_or_noninteractive_files_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("empty.txt");
        std::fs::write(&p1, "").unwrap();
        assert!(CursorProvider.parse(&p1).unwrap().is_empty());
        // Thinking-only assistant block: dropped per the Go skip rule.
        let p2 = dir.path().join("markers-only.txt");
        std::fs::write(&p2, "user:\n\nassistant:\n[Thinking]\n  idle reasoning\n").unwrap();
        assert!(CursorProvider.parse(&p2).unwrap().is_empty());
        let p3 = dir.path().join("system-only.jsonl");
        std::fs::write(
            &p3,
            "{\"role\":\"system\",\"message\":{\"content\":\"x\"}}\n",
        )
        .unwrap();
        assert!(CursorProvider.parse(&p3).unwrap().is_empty());
    }

    #[test]
    fn oversize_transcript_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        std::fs::write(&path, vec![b'a'; (MAX_TRANSCRIPT_BYTES + 1) as usize]).unwrap();
        assert!(CursorProvider.parse(&path).is_err());
    }

    #[test]
    fn discover_flat_nested_and_jsonl_preference() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let transcripts = root
            .join("Users-fiona-Documents-my-app")
            .join("agent-transcripts");
        std::fs::create_dir_all(transcripts.join("bbb")).unwrap();
        std::fs::write(transcripts.join("aaa.txt"), TEXT_FIXTURE).unwrap();
        std::fs::write(transcripts.join("aaa.jsonl"), JSONL_FIXTURE).unwrap();
        std::fs::write(transcripts.join("bbb").join("bbb.txt"), TEXT_FIXTURE).unwrap();
        // Nested file whose stem mismatches its directory is ignored.
        std::fs::write(transcripts.join("bbb").join("ccc.txt"), TEXT_FIXTURE).unwrap();
        std::fs::write(transcripts.join("notes.md"), "x").unwrap();
        std::fs::write(root.join("stray.txt"), "x").unwrap();
        let found = CursorProvider.discover(root);
        assert_eq!(found.len(), 2);
        let ids: Vec<&str> = found.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"cursor:aaa"));
        assert!(ids.contains(&"cursor:bbb"));
        let aaa = found.iter().find(|d| d.id == "cursor:aaa").unwrap();
        assert!(
            aaa.path.to_str().unwrap().ends_with("aaa.jsonl"),
            "prefer .jsonl over .txt for the same stem"
        );
        for d in &found {
            assert_eq!(d.project_hint.as_deref(), Some("my_app"));
            assert_eq!(d.provider, ProviderKind::Cursor);
        }
    }
}
