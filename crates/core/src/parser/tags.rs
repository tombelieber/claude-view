// crates/core/src/parser/tags.rs
//! Command tag stripping from user messages.
//!
//! Claude Code user messages may contain XML-like command tags injected by
//! the CLI (e.g., `<command-name>`, `<command-args>`, `<system-reminder>`).
//! This module strips those tags, extracting the actual user input.

use regex_lite::Regex;

/// Pre-compiled regex set for stripping command tags from user messages.
pub(super) struct TagRegexes {
    pub command_name: Regex,
    pub command_args: Regex,
    pub command_message: Regex,
    pub local_stdout: Regex,
    pub system_reminder: Regex,
}

impl TagRegexes {
    /// Compile all tag-stripping regexes. Called once per parse_session invocation.
    pub fn new() -> Self {
        Self {
            command_name: Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap(),
            command_args: Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap(),
            command_message: Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap(),
            local_stdout: Regex::new(r"(?s)<local-command-stdout>.*?</local-command-stdout>\s*")
                .unwrap(),
            system_reminder: Regex::new(r"(?s)<system-reminder>.*?</system-reminder>\s*").unwrap(),
        }
    }
}

/// Clean command tags from user messages.
///
/// Extracts content from `<command-args>` (the actual user input for slash commands),
/// strips `<command-name>` and `<command-message>` tags. If `<command-args>` is present,
/// its inner content becomes the message; otherwise the remaining text after stripping
/// the other tags is used.
pub(super) fn clean_command_tags(content: &str, regexes: &TagRegexes) -> String {
    // Try to extract command-args content first
    if let Some(caps) = regexes.command_args.captures(content) {
        if let Some(args_content) = caps.get(1) {
            let extracted = args_content.as_str().trim();
            if !extracted.is_empty() {
                return extracted.to_string();
            }
        }
    }

    // No command-args found (or empty), strip all command/system tags
    let cleaned = regexes.command_name.replace_all(content, "");
    let cleaned = regexes.command_message.replace_all(&cleaned, "");
    let cleaned = regexes.local_stdout.replace_all(&cleaned, "");
    let cleaned = regexes.system_reminder.replace_all(&cleaned, "");
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regexes() -> TagRegexes {
        TagRegexes::new()
    }

    #[test]
    fn test_clean_command_tags_basic() {
        let r = regexes();
        let input = "<command-name>/commit</command-name>\nPlease commit";
        let result = clean_command_tags(input, &r);
        assert_eq!(result, "Please commit");
    }

    #[test]
    fn test_clean_command_tags_with_args() {
        let r = regexes();
        // When command-args is present, its content becomes the message
        let input =
            "<command-name>/review</command-name>\n<command-args>123</command-args>\nReview PR";
        let result = clean_command_tags(input, &r);
        assert_eq!(result, "123");
    }

    #[test]
    fn test_clean_command_tags_with_multiline_args() {
        let r = regexes();
        // command-args can contain < characters and span multiple lines
        let input = "<command-name>/review</command-name>\n<command-args>Fix the <T> generic\nacross files</command-args>";
        let result = clean_command_tags(input, &r);
        assert_eq!(result, "Fix the <T> generic\nacross files");
    }

    #[test]
    fn test_clean_command_tags_no_tags() {
        let r = regexes();
        let input = "Normal message without tags";
        let result = clean_command_tags(input, &r);
        assert_eq!(result, "Normal message without tags");
    }

    #[test]
    fn test_clean_command_tags_strips_local_stdout_and_system_reminder() {
        let r = regexes();
        let input = "<local-command-stdout>some output</local-command-stdout>\n<system-reminder>injected context</system-reminder>\nActual user message";
        let result = clean_command_tags(input, &r);
        assert_eq!(result, "Actual user message");
    }

    #[test]
    fn test_clean_command_tags_strips_system_reminder_only() {
        let r = regexes();
        let input = "<system-reminder>SessionStart hook context</system-reminder>\nFix the bug";
        let result = clean_command_tags(input, &r);
        assert_eq!(result, "Fix the bug");
    }

    #[test]
    fn test_clean_command_message_tags() {
        let r = regexes();
        let input = "<command-name>/commit</command-name>\n<command-message>System prompt text</command-message>\nPlease commit";
        let result = clean_command_tags(input, &r);
        assert_eq!(result, "Please commit");
    }
}
