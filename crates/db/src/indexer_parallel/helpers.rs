// crates/db/src/indexer_parallel/helpers.rs
// Utility functions: string extraction, line splitting, file path extraction,
// content extraction, skill extraction, timestamp helpers.

use memchr::memmem;

use super::types::ParseDiagnostics;

pub(crate) const SOURCE_MESSAGE_ROLE_USER: &str = "user";
pub(crate) const SOURCE_MESSAGE_ROLE_ASSISTANT: &str = "assistant";
pub(crate) const SOURCE_MESSAGE_ROLE_TOOL: &str = "tool";
pub(crate) const TOOL_INPUT_FILE_PATH_KEYS: [&str; 5] = [
    "file_path",
    "path",
    "filename",
    "filePath",
    "entrypoint_path",
];

pub(crate) fn is_valid_source_message_role(role: &str) -> bool {
    matches!(
        role,
        SOURCE_MESSAGE_ROLE_USER | SOURCE_MESSAGE_ROLE_ASSISTANT | SOURCE_MESSAGE_ROLE_TOOL
    )
}

pub(crate) fn sanitize_source_search_messages(
    messages: Vec<claude_view_core::SearchableMessage>,
    diag: &mut ParseDiagnostics,
) -> Vec<claude_view_core::SearchableMessage> {
    messages
        .into_iter()
        .filter_map(|msg| {
            if is_valid_source_message_role(msg.role.as_str()) {
                Some(msg)
            } else {
                diag.unknown_source_role_count = diag.unknown_source_role_count.saturating_add(1);
                None
            }
        })
        .collect()
}

pub(crate) fn note_rejected_derived_source_doc(diag: &mut ParseDiagnostics) {
    diag.derived_source_message_doc_count = diag.derived_source_message_doc_count.saturating_add(1);
    diag.source_message_non_source_provenance_count = diag
        .source_message_non_source_provenance_count
        .saturating_add(1);
}

pub(crate) fn extract_tool_input_file_path(input: &serde_json::Value) -> Option<&str> {
    for key in TOOL_INPUT_FILE_PATH_KEYS {
        if let Some(path) = input.get(key).and_then(|v| v.as_str()) {
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    None
}

/// Extract lines added/removed from assistant.message.content tool_use blocks.
///
/// LOC is derived only from Edit/Write tool inputs and aggregated across all
/// tool_use blocks in the line.
pub(crate) fn extract_loc_from_assistant_tool_uses(line: &[u8]) -> (u32, u32) {
    let value: serde_json::Value = match serde_json::from_slice(line) {
        Ok(v) => v,
        Err(_) => return (0, 0),
    };

    let content = match value
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        Some(c) => c,
        None => return (0, 0),
    };

    let mut added = 0u32;
    let mut removed = 0u32;

    for block in content {
        if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
            continue;
        }
        let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let input = match block.get("input") {
            Some(v) => v,
            None => continue,
        };

        match name {
            "Edit" => {
                let old = input
                    .get("old_string")
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                let new = input
                    .get("new_string")
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                removed = removed.saturating_add(old.lines().count() as u32);
                added = added.saturating_add(new.lines().count() as u32);
            }
            "Write" => {
                let content = input.get("content").and_then(|c| c.as_str()).unwrap_or("");
                added = added.saturating_add(content.lines().count() as u32);
            }
            _ => {}
        }
    }

    (added, removed)
}

/// Extract timestamp from an already-parsed JSON value.
/// Handles both ISO8601 strings and Unix integers.
pub(crate) fn extract_timestamp_from_value(value: &serde_json::Value) -> Option<i64> {
    value.get("timestamp").and_then(|v| {
        v.as_i64().or_else(|| {
            v.as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp())
        })
    })
}

/// Extract timestamp from raw JSONL bytes without JSON parsing.
/// Uses memmem::Finder to locate `"timestamp":` then parses the value inline.
pub(crate) fn extract_timestamp_from_bytes(line: &[u8], finder: &memmem::Finder) -> Option<i64> {
    let pos = finder.find(line)?;
    let rest = &line[pos + b"\"timestamp\":".len()..];
    // Skip whitespace
    let skip = rest.iter().position(|&b| b != b' ' && b != b'\t')?;
    let rest = &rest[skip..];
    match rest.first()? {
        b'-' | b'0'..=b'9' => {
            // Integer timestamp
            let end = rest
                .iter()
                .position(|&b| !(b == b'-' || b.is_ascii_digit()))
                .unwrap_or(rest.len());
            std::str::from_utf8(&rest[..end]).ok()?.parse().ok()
        }
        b'"' => {
            // ISO8601 string timestamp
            let s = extract_quoted_string(&rest[1..])?;
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.timestamp())
        }
        _ => None,
    }
}

/// Count files that appear 2+ times in the files_edited list.
pub(crate) fn count_reedited_files(files_edited: &[String]) -> u32 {
    use std::collections::HashMap;

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for path in files_edited {
        *counts.entry(path.as_str()).or_insert(0) += 1;
    }

    counts.values().filter(|&&count| count >= 2).count() as u32
}

/// Split data into lines using SIMD-accelerated newline search.
#[cfg(test)]
pub(crate) fn split_lines_simd(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut start = 0;
    let mut positions = memchr::memchr_iter(b'\n', data).chain(std::iter::once(data.len()));

    std::iter::from_fn(move || {
        if start > data.len() {
            return None;
        }
        positions.next().map(|end| {
            let line = &data[start..end];
            start = end + 1;
            line
        })
    })
}

/// Split data into lines with byte offsets using SIMD-accelerated newline search.
/// Returns `(byte_offset, line_slice)` for each line.
pub(crate) fn split_lines_with_offsets(data: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    let mut start = 0;
    let mut positions = memchr::memchr_iter(b'\n', data).chain(std::iter::once(data.len()));

    std::iter::from_fn(move || {
        if start > data.len() {
            return None;
        }
        positions.next().map(|end| {
            let offset = start;
            let line = &data[start..end];
            start = end + 1;
            (offset, line)
        })
    })
}

/// Extract text content from a JSONL `serde_json::Value` for full-text search indexing.
///
/// Returns a tuple of (text_content, tool_use_entries) where:
/// - text_content: concatenated text blocks from the message (for "user" or "assistant" role)
/// - tool_use_entries: individual tool_use block descriptions (role="tool")
///
/// Handles both string content (`"content": "..."`) and array content
/// (`"content": [{"type": "text", "text": "..."}, {"type": "tool_use", ...}]`).
pub(crate) fn extract_search_content_from_value(
    value: &serde_json::Value,
) -> (Option<String>, Vec<String>) {
    let mut tool_entries = Vec::new();

    let message = match value.get("message") {
        Some(m) => m,
        None => return (None, tool_entries),
    };

    let content = match message.get("content") {
        Some(c) => c,
        None => return (None, tool_entries),
    };

    match content {
        serde_json::Value::String(s) => (Some(s.clone()), tool_entries),
        serde_json::Value::Array(arr) => {
            let mut text_parts = Vec::new();
            for block in arr {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                    "tool_use" => {
                        // Include tool name + stringified input for searchability
                        let name = block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown");
                        if let Some(input) = block.get("input") {
                            if let Some(s) = input.as_str() {
                                tool_entries.push(format!("{}: {}", name, s));
                            }
                            // For object inputs, include only if small (e.g., command field)
                            else if let Some(cmd) = input.get("command").and_then(|c| c.as_str())
                            {
                                tool_entries.push(format!("{}: {}", name, cmd));
                            } else if let Some(fp) = extract_tool_input_file_path(input) {
                                tool_entries.push(format!("{}: {}", name, fp));
                            }
                        }
                    }
                    _ => {}
                }
            }
            let text = if text_parts.is_empty() {
                None
            } else {
                Some(text_parts.join("\n"))
            };
            (text, tool_entries)
        }
        _ => (None, tool_entries),
    }
}

/// Extract the first text content from a JSONL line (best-effort, no full JSON parse).
pub(crate) fn extract_first_text_content(
    line: &[u8],
    content_finder: &memmem::Finder,
    text_finder: &memmem::Finder,
) -> Option<String> {
    // Check "text":"..." first -- more specific to actual text content blocks.
    // "content":"..." can match tool input fields (e.g. Write tool's file content),
    // so it should only be used as a fallback.
    if let Some(pos) = text_finder.find(line) {
        let start = pos + b"\"text\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    // Fall back to "content":"..." (simple string content, e.g. user messages)
    if let Some(pos) = content_finder.find(line) {
        let start = pos + b"\"content\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    None
}

/// Extract a JSON-escaped string value starting from after the opening quote.
pub(crate) fn extract_quoted_string(data: &[u8]) -> Option<String> {
    let mut end = 0;
    let mut escaped = false;
    for &b in data {
        if escaped {
            escaped = false;
            end += 1;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            end += 1;
            continue;
        }
        if b == b'"' {
            break;
        }
        end += 1;
    }

    if end > 0 {
        String::from_utf8(data[..end].to_vec()).ok()
    } else {
        None
    }
}

/// SIMD fallback: extract skill names from raw bytes (looking for "skill":"..." patterns).
/// Used when the typed AssistantLine parse fails but we still want to capture skills.
pub(crate) fn extract_skills_from_line(
    line: &[u8],
    skill_name_finder: &memmem::Finder,
    skills: &mut Vec<String>,
) {
    let mut start = 0;
    while start < line.len() {
        if let Some(pos) = skill_name_finder.find(&line[start..]) {
            let begin = start + pos + b"\"skill\":\"".len();
            if let Some(name) = extract_quoted_string(&line[begin..]) {
                if !name.is_empty() {
                    skills.push(name);
                }
            }
            start = begin;
        } else {
            break;
        }
    }
}

/// Truncate a string to at most `max_len` characters (not bytes).
pub(crate) fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect::<String>() + "..."
    }
}

/// Extract commit skill invocations from raw tool_use invocations.
///
/// Filters for invocations where:
/// - `name == "Skill"`
/// - `input.skill` is one of the commit-related skill names
///
/// Returns a list of `CommitSkillInvocation` with skill name and timestamp.
pub fn extract_commit_skill_invocations(
    raw_invocations: &[super::types::RawInvocation],
) -> Vec<super::types::CommitSkillInvocation> {
    raw_invocations
        .iter()
        .filter_map(|inv| {
            // Only process Skill tool invocations
            if inv.name != "Skill" {
                return None;
            }

            // Extract the skill name from input.skill
            let skill_name = inv.input.as_ref()?.get("skill")?.as_str()?;

            // Check if it's a commit-related skill
            if super::types::COMMIT_SKILL_NAMES.contains(&skill_name) {
                Some(super::types::CommitSkillInvocation {
                    skill_name: skill_name.to_string(),
                    timestamp_unix: inv.timestamp,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Extract a structured `HookEventRow` from a raw JSONL line with `"type":"progress"` /
/// `"data":{"type":"hook_progress", ...}`.  Returns `None` if the line doesn't contain
/// the expected hook_progress shape.
pub(crate) fn parse_hook_progress_line(
    line: &[u8],
) -> Option<crate::queries::hook_events::HookEventRow> {
    let json: serde_json::Value = serde_json::from_slice(line).ok()?;
    let hook_event = json.pointer("/data/hookEvent")?.as_str()?;
    let hook_name = json
        .pointer("/data/hookName")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let (tool_name, source) = if let Some(pos) = hook_name.find(':') {
        let suffix = &hook_name[pos + 1..];
        if hook_event == "SessionStart" {
            (None, Some(suffix))
        } else {
            (Some(suffix), None)
        }
    } else {
        (None, None)
    };

    let group = match hook_event {
        "SessionStart" if source == Some("compact") => "autonomous",
        "SessionStart" => "needs_you",
        "PreToolUse"
            if matches!(
                tool_name,
                Some("AskUserQuestion") | Some("EnterPlanMode") | Some("ExitPlanMode")
            ) =>
        {
            "needs_you"
        }
        "PostToolUseFailure" | "PostToolUse" | "PreToolUse" => "autonomous",
        "Stop" => "needs_you",
        _ => "autonomous",
    };

    let label = match tool_name {
        Some(t) => format!("{}: {}", hook_event, t),
        None => hook_event.to_string(),
    };

    // Extract timestamp from JSON directly -- SIMD fast path skips timestamp
    // extraction for progress lines, so caller's last_timestamp would be stale.
    let timestamp = json
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or(0);

    Some(crate::queries::hook_events::HookEventRow {
        timestamp,
        event_name: hook_event.to_string(),
        tool_name: tool_name.map(|s| s.to_string()),
        label,
        group_name: group.to_string(),
        context: None,
        source: "hook_progress".to_string(),
    })
}
