// crates/providers/src/parsers/cursor/text_format.rs
//
// Plain-text Cursor transcripts: `user:` / `assistant:` role-marker lines;
// assistant sections carry `[Thinking]`, `[Tool call] <name>`, and
// `[Tool result]` headers whose indented bodies end at the first non-empty
// line at column 0 (port of parseCursorMessages and friends).

use crate::model::ForeignSessionMeta;
use crate::util::{blocks, preview};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock, ToolResult};
use serde_json::Value;

enum TextRole {
    User,
    Assistant,
}

pub(super) fn parse_transcript(
    text: &str,
    raw_id: &str,
    meta: &mut ForeignSessionMeta,
    out: &mut Vec<ConversationBlock>,
) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut ordinal = 0usize;
    for (role, block_lines) in split_role_blocks(&lines) {
        match role {
            TextRole::User => {
                let content = extract_user_query(&block_lines.join("\n"));
                if content.is_empty() {
                    continue;
                }
                let id = blocks::block_id(raw_id, ordinal);
                ordinal += 1;
                if meta.first_message.is_empty() {
                    meta.first_message = preview(&content, 200);
                }
                meta.message_count += 1;
                meta.user_message_count += 1;
                out.push(blocks::user(id, content, None));
            }
            TextRole::Assistant => {
                let id = blocks::block_id(raw_id, ordinal);
                let (segments, thinking) = parse_assistant_block(&block_lines, &id);
                // Go skip rule: a block with no prose and no tool calls is
                // dropped (thinking alone does not keep it).
                if segments.is_empty() {
                    continue;
                }
                ordinal += 1;
                meta.message_count += 1;
                out.push(blocks::assistant(id, segments, thinking, None));
            }
        }
    }
}

/// Split lines into blocks delimited by `user:` / `assistant:` on a line by
/// itself; lines before the first marker are ignored.
fn split_role_blocks<'a>(lines: &[&'a str]) -> Vec<(TextRole, Vec<&'a str>)> {
    let mut out: Vec<(TextRole, Vec<&'a str>)> = Vec::new();
    let mut current: Option<(TextRole, Vec<&'a str>)> = None;
    for &line in lines {
        let trimmed = line.trim();
        if trimmed == "user:" || trimmed == "assistant:" {
            if let Some(b) = current.take() {
                out.push(b);
            }
            let role = if trimmed == "user:" {
                TextRole::User
            } else {
                TextRole::Assistant
            };
            current = Some((role, Vec::new()));
            continue;
        }
        if let Some((_, ls)) = current.as_mut() {
            ls.push(line);
        }
    }
    if let Some(b) = current {
        out.push(b);
    }
    out
}

/// Text inside <user_query>…</user_query>, falling back to the whole text.
/// Shared with the JSONL format, whose user strings carry the same tags.
pub(super) fn extract_user_query(text: &str) -> String {
    if let (Some(start), Some(end)) = (text.find("<user_query>"), text.find("</user_query>")) {
        if end > start {
            return text[start + "<user_query>".len()..end].trim().to_string();
        }
    }
    text.trim().to_string()
}

/// Walk one assistant block's lines extracting interleaved prose / tool
/// calls (with their results) and thinking text.
fn parse_assistant_block(
    lines: &[&str],
    block_id: &str,
) -> (Vec<AssistantSegment>, Option<String>) {
    let mut segments: Vec<AssistantSegment> = Vec::new();
    let mut thinking_parts: Vec<String> = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    let mut tool_seq = 0usize;
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("[Thinking]") {
            let (body, next) = collect_block_body(lines, i + 1);
            i = next;
            if !body.is_empty() {
                thinking_parts.push(body);
            }
            continue;
        }
        if let Some(name) = trimmed.strip_prefix("[Tool call] ") {
            flush_prose(&mut prose, &mut segments);
            let (body, next) = collect_block_body(lines, i + 1);
            i = next;
            tool_seq += 1;
            let input = if body.is_empty() {
                Value::Null
            } else {
                Value::String(body)
            };
            // The text format carries no tool ids — synthesize a
            // deterministic one (same scheme as ordinal block ids).
            segments.push(blocks::tool_segment(
                name.to_string(),
                input,
                format!("{block_id}:t{tool_seq}"),
            ));
            continue;
        }
        if trimmed.starts_with("[Tool result]") {
            let (body, next) = collect_block_body(lines, i + 1);
            i = next;
            attach_result(&mut segments, body);
            continue;
        }
        prose.push(lines[i]);
        i += 1;
    }
    flush_prose(&mut prose, &mut segments);
    let thinking = (!thinking_parts.is_empty()).then(|| thinking_parts.join("\n\n"));
    (segments, thinking)
}

fn flush_prose(prose: &mut Vec<&str>, segments: &mut Vec<AssistantSegment>) {
    if prose.is_empty() {
        return;
    }
    let text = prose.join("\n").trim().to_string();
    prose.clear();
    if !text.is_empty() {
        segments.push(blocks::text_segment(text));
    }
}

/// Attach a `[Tool result]` body to the most recent tool call in this block
/// that has no result yet; with no preceding call the body is dropped (the
/// Go source drops these bodies unconditionally).
fn attach_result(segments: &mut [AssistantSegment], output: String) {
    for seg in segments.iter_mut().rev() {
        if let AssistantSegment::Tool { execution } = seg {
            if execution.result.is_none() {
                execution.result = Some(ToolResult {
                    output,
                    is_error: false,
                    is_replay: false,
                });
                return;
            }
        }
    }
}

/// Collect a structured block body starting at `i`: indented or empty lines
/// belong to the body; a marker or a non-empty line at column 0 ends it.
fn collect_block_body(lines: &[&str], mut i: usize) -> (String, usize) {
    let start = i;
    while i < lines.len() && !is_block_body_end(lines[i]) {
        i += 1;
    }
    (lines[start..i].join("\n").trim().to_string(), i)
}

fn is_assistant_marker(trimmed: &str) -> bool {
    trimmed.starts_with("[Thinking]")
        || trimmed.starts_with("[Tool call] ")
        || trimmed.starts_with("[Tool result]")
}

fn is_block_body_end(line: &str) -> bool {
    let trimmed = line.trim();
    if is_assistant_marker(trimmed) {
        return true;
    }
    !trimmed.is_empty() && !line.starts_with([' ', '\t'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_body_end_rules() {
        // Table ported from Go TestIsContainedIn_EdgeCases (isBlockBodyEnd).
        assert!(is_block_body_end("[Tool call] Shell"));
        assert!(!is_block_body_end("  param=value"));
        assert!(!is_block_body_end("\tparam=value"));
        assert!(!is_block_body_end(""));
        assert!(is_block_body_end("Here is text."));
    }

    #[test]
    fn user_query_tags_extracted_with_fallback() {
        assert_eq!(extract_user_query("<user_query>hi</user_query>"), "hi");
        assert_eq!(extract_user_query("  plain text  "), "plain text");
        // Closing tag before opening tag → fallback to whole text.
        assert_eq!(
            extract_user_query("</user_query>x<user_query>"),
            "</user_query>x<user_query>"
        );
    }

    #[test]
    fn prose_between_markers_interleaves() {
        // Go test case "prose between markers": two tools, three prose runs.
        let lines = [
            "First I'll check the file.",
            "[Tool call] ReadFile",
            "  path=main.go",
            "[Tool result]",
            "  package main",
            "The file looks good.",
            "[Tool call] Shell",
            "  command=go build",
            "Build succeeded.",
        ];
        let (segments, thinking) = parse_assistant_block(&lines, "s:0");
        assert!(thinking.is_none());
        assert_eq!(segments.len(), 5);
        let texts: Vec<&str> = segments
            .iter()
            .filter_map(|s| match s {
                AssistantSegment::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            [
                "First I'll check the file.",
                "The file looks good.",
                "Build succeeded."
            ]
        );
        let AssistantSegment::Tool { execution } = &segments[1] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.result.as_ref().unwrap().output, "package main");
        let AssistantSegment::Tool { execution } = &segments[3] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "Shell");
        assert!(execution.result.is_none(), "second call has no result");
    }
}
