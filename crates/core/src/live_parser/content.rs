//! Content extraction, noise tag stripping, and text truncation.

use super::finders::TailFinders;

/// Content extraction result from `extract_content_and_tools`.
pub(crate) type ContentExtraction = (
    String,         // content_preview
    String,         // content_extended (1500-char truncation for phase classifier)
    Vec<String>,    // tool_names
    Vec<String>,    // skill_names
    Vec<String>,    // bash_commands
    Vec<String>,    // edited_files
    bool,           // has_tool_result
    Option<String>, // ide_file
    Vec<String>,    // at_files
);

/// Extract content preview (truncated to 200 chars), tool_use names,
/// skill names (from Skill tool_use `input.skill`), and whether the
/// content array contains a `tool_result` block.
pub(crate) fn extract_content_and_tools(
    parsed: &serde_json::Value,
    finders: &TailFinders,
) -> ContentExtraction {
    use std::sync::OnceLock;
    static AT_FILE_RE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let at_file_re = AT_FILE_RE
        .get_or_init(|| regex_lite::Regex::new(r"(?:^|\s)@([\w./-]+\.\w{1,15})").unwrap());

    let mut preview = String::new();
    let mut extended = String::new();
    let mut tool_names = Vec::new();
    let mut skill_names = Vec::new();
    let mut bash_commands = Vec::new();
    let mut edited_files = Vec::new();
    let mut has_tool_result = false;
    let mut ide_file: Option<String> = None;
    let mut at_files: Vec<String> = Vec::new();

    match parsed.get("content") {
        Some(serde_json::Value::String(s)) => {
            // Extract @file references from raw string before noise stripping
            if finders.at_file_key.find(s.as_bytes()).is_some() {
                for caps in at_file_re.captures_iter(s) {
                    if let Some(m) = caps.get(1) {
                        at_files.push(m.as_str().to_string());
                    }
                }
            }
            let (stripped, file) = strip_noise_tags(s);
            preview = truncate_str(&stripped, 200);
            extended = truncate_str(&stripped, 1500);
            ide_file = file;
        }
        Some(serde_json::Value::Array(blocks)) => {
            for block in blocks {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            // Extract @file references from raw text before noise stripping
                            if finders.at_file_key.find(text.as_bytes()).is_some() {
                                for caps in at_file_re.captures_iter(text) {
                                    if let Some(m) = caps.get(1) {
                                        at_files.push(m.as_str().to_string());
                                    }
                                }
                            }
                            if preview.is_empty() {
                                let (stripped, file) = strip_noise_tags(text);
                                preview = truncate_str(&stripped, 200);
                                extended = truncate_str(&stripped, 1500);
                                if ide_file.is_none() {
                                    ide_file = file;
                                }
                            }
                        }
                    }
                    Some("tool_use") => {
                        if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                            tool_names.push(name.to_string());
                            let input = block.get("input");
                            match name {
                                "Skill" => {
                                    if let Some(skill) =
                                        input.and_then(|i| i.get("skill")).and_then(|s| s.as_str())
                                    {
                                        if !skill.is_empty() {
                                            skill_names.push(skill.to_string());
                                        }
                                    }
                                }
                                "Bash" => {
                                    if let Some(cmd) = input
                                        .and_then(|i| i.get("command"))
                                        .and_then(|s| s.as_str())
                                    {
                                        if !cmd.is_empty() {
                                            bash_commands.push(truncate_str(cmd, 200).to_string());
                                        }
                                    }
                                }
                                "Edit" | "Write" => {
                                    if let Some(fp) = input
                                        .and_then(|i| i.get("file_path"))
                                        .and_then(|s| s.as_str())
                                    {
                                        if !fp.is_empty() {
                                            edited_files.push(fp.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Some("tool_result") => {
                        has_tool_result = true;
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    (
        preview,
        extended,
        tool_names,
        skill_names,
        bash_commands,
        edited_files,
        has_tool_result,
        ide_file,
        at_files,
    )
}

/// Strip XML noise tags from user message content and extract IDE file context.
///
/// Returns `(clean_text, ide_file)` where:
/// - `clean_text` is the content with all noise tags removed, trimmed
/// - `ide_file` is the last path component from `<ide_opened_file>` if present
///
/// Tags stripped: system-reminder, ide_opened_file, ide_selection, command-name,
/// command-args, command-message, local-command-stdout, local-command-caveat,
/// task-notification, user-prompt-submit-hook.
///
/// NOTE: The IDE filename extraction regex ("the file\s+(\S+)\s+in the IDE") is
/// coupled to Claude Code's current hook output format. If the format changes,
/// extraction gracefully degrades to `None`.
pub(crate) fn strip_noise_tags(content: &str) -> (String, Option<String>) {
    use regex_lite::Regex;
    use std::sync::OnceLock;

    // `regex-lite` does NOT support backreferences (\1), so each tag must be
    // enumerated with its explicit closing tag. Uses OnceLock to match codebase
    // convention (see cli.rs, sync.rs, metrics.rs).
    static NOISE_TAGS: OnceLock<Regex> = OnceLock::new();
    let noise_re = NOISE_TAGS.get_or_init(|| {
        Regex::new(concat!(
            r"(?s)<system-reminder>.*?</system-reminder>\s*",
            r"|<ide_selection>.*?</ide_selection>\s*",
            r"|<command-name>.*?</command-name>\s*",
            r"|<command-args>.*?</command-args>\s*",
            r"|<command-message>.*?</command-message>\s*",
            r"|<local-command-stdout>.*?</local-command-stdout>\s*",
            r"|<local-command-caveat>.*?</local-command-caveat>\s*",
            r"|<task-notification>.*?</task-notification>\s*",
            r"|<user-prompt-submit-hook>.*?</user-prompt-submit-hook>\s*",
        ))
        .unwrap()
    });

    // Separate regex for ide_opened_file to extract filename before stripping
    static IDE_FILE_TAG: OnceLock<Regex> = OnceLock::new();
    let ide_tag_re = IDE_FILE_TAG
        .get_or_init(|| Regex::new(r"(?s)<ide_opened_file>.*?</ide_opened_file>\s*").unwrap());

    static IDE_FILE_PATH: OnceLock<Regex> = OnceLock::new();
    let ide_path_re =
        IDE_FILE_PATH.get_or_init(|| Regex::new(r"the file\s+(\S+)\s+in the IDE").unwrap());

    // Extract IDE file before stripping
    let ide_file = ide_tag_re.find(content).and_then(|m| {
        ide_path_re.captures(m.as_str()).and_then(|caps| {
            caps.get(1).map(|p| {
                let path = p.as_str();
                // Extract last path component (filename)
                path.rsplit('/').next().unwrap_or(path).to_string()
            })
        })
    });

    // Strip all noise tags
    let cleaned = noise_re.replace_all(content, "");
    // Strip ide_opened_file tag
    let cleaned = ide_tag_re.replace_all(&cleaned, "");
    let cleaned = cleaned.trim().to_string();

    (cleaned, ide_file)
}

/// Truncate a string to at most `max` characters, appending "..." if trimmed.
pub(crate) fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}

/// Extract a `<task-notification>` from the full content JSON value.
///
/// Walks the content field (string or array of text blocks) looking for
/// `<task-id>` and `<status>` XML tags within a `<task-notification>` block.
/// Falls back to regex extraction if strict XML parsing fails.
pub(crate) fn extract_task_notification(
    content_source: &serde_json::Value,
) -> Option<super::types::SubAgentNotification> {
    if let Some(result) = extract_task_notification_xml(content_source) {
        return Some(result);
    }
    // Fallback: regex tolerates whitespace and minor malformation
    extract_task_notification_regex(content_source)
}

/// Collect the full text from the content field (string or array of text blocks)
/// that contains `<task-notification>`.
fn collect_notification_text(content_source: &serde_json::Value) -> Option<&str> {
    match content_source.get("content") {
        Some(serde_json::Value::String(s)) => {
            if s.contains("<task-notification>") || s.contains("<task-id>") {
                Some(s.as_str())
            } else {
                None
            }
        }
        Some(serde_json::Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                    b.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
            .find(|text| text.contains("<task-notification>") || text.contains("<task-id>")),
        _ => None,
    }
}

/// Strict XML extraction — requires exact tag structure.
fn extract_task_notification_xml(
    content_source: &serde_json::Value,
) -> Option<super::types::SubAgentNotification> {
    let full_text = collect_notification_text(content_source)?;

    let tn_start = full_text.find("<task-notification>")?;
    let after_tn = &full_text[tn_start..];

    let agent_id = {
        let start = after_tn.find("<task-id>")? + "<task-id>".len();
        let end = start + after_tn[start..].find("</task-id>")?;
        let id = after_tn[start..end].trim();
        if id.is_empty() {
            return None;
        }
        id.to_string()
    };

    let status = {
        let start = after_tn.find("<status>")? + "<status>".len();
        let end = start + after_tn[start..].find("</status>")?;
        let s = after_tn[start..end].trim();
        if s.is_empty() {
            return None;
        }
        s.to_string()
    };

    Some(super::types::SubAgentNotification { agent_id, status })
}

/// Regex fallback — tolerates whitespace inside tags.
fn extract_task_notification_regex(
    content_source: &serde_json::Value,
) -> Option<super::types::SubAgentNotification> {
    let full_text = collect_notification_text(content_source)?;

    let id_re = regex_lite::Regex::new(r"<task-id>\s*(.*?)\s*</task-id>").ok()?;
    let status_re = regex_lite::Regex::new(r"<status>\s*(.*?)\s*</status>").ok()?;

    let agent_id = id_re
        .captures(full_text)?
        .get(1)
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())?;
    let status = status_re
        .captures(full_text)?
        .get(1)
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())?;

    tracing::debug!(
        agent_id = %agent_id,
        "Task notification extracted via regex fallback — XML parse failed"
    );

    Some(super::types::SubAgentNotification { agent_id, status })
}
