// crates/core/src/contribution.rs
//! AI contribution tracking: count lines added/removed from Edit/Write tool_use.
//!
//! This module provides functions to count AI-generated lines from JSONL tool calls:
//! - `count_ai_lines()` - extracts line counts from a list of raw invocations
//! - `count_lines_in_edit()` - counts added/removed lines from Edit tool input
//! - `count_lines_in_write()` - counts lines from Write tool input
//!
//! ## Design Rationale
//!
//! We count *output* lines (what the AI wrote), not *effect* lines (what ended up in the file).
//! This is intentional:
//! - We want to measure AI contribution, not the final result after human edits
//! - The JSONL contains the raw AI output, which is what we're tracking
//! - Commit diff stats (from git) show the actual codebase impact separately

/// Result of counting AI-contributed lines from tool invocations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiLineCount {
    /// Lines added by AI (new_string in Edit, content in Write)
    pub lines_added: u32,
    /// Lines removed by AI (old_string in Edit, none for Write)
    pub lines_removed: u32,
}

impl AiLineCount {
    /// Create a new AiLineCount with zero values.
    pub fn zero() -> Self {
        Self::default()
    }

    /// Net lines (added - removed). Can be negative.
    pub fn net(&self) -> i32 {
        self.lines_added as i32 - self.lines_removed as i32
    }

    /// Merge another count into this one.
    pub fn merge(&mut self, other: AiLineCount) {
        self.lines_added += other.lines_added;
        self.lines_removed += other.lines_removed;
    }
}

/// Count lines in a string.
///
/// Empty strings have 0 lines. Non-empty strings have at least 1 line.
/// Each newline character adds one to the count.
///
/// Examples:
/// - "" -> 0
/// - "a" -> 1
/// - "a\n" -> 1 (trailing newline doesn't add a line)
/// - "a\nb" -> 2
/// - "a\nb\n" -> 2
/// - "\n" -> 1 (single newline = 1 empty line)
/// - "\n\n" -> 2 (two newlines = 2 empty lines)
fn count_lines(s: &str) -> u32 {
    if s.is_empty() {
        return 0;
    }
    // Count newlines, but if the string doesn't end with a newline,
    // we need to add 1 for the last line
    let newline_count = s.chars().filter(|&c| c == '\n').count();
    if s.ends_with('\n') {
        newline_count as u32
    } else {
        (newline_count + 1) as u32
    }
}

/// Count AI-contributed lines from an Edit tool invocation.
///
/// Edit tool input schema:
/// ```json
/// {
///     "file_path": "/path/to/file.rs",
///     "old_string": "code to replace",
///     "new_string": "replacement code"
/// }
/// ```
///
/// Returns (lines_added, lines_removed) where:
/// - lines_added = line count of new_string
/// - lines_removed = line count of old_string
pub fn count_lines_in_edit(input: &serde_json::Value) -> AiLineCount {
    let old_string = input
        .get("old_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new_string = input
        .get("new_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    AiLineCount {
        lines_added: count_lines(new_string),
        lines_removed: count_lines(old_string),
    }
}

/// Count AI-contributed lines from a Write tool invocation.
///
/// Write tool input schema:
/// ```json
/// {
///     "file_path": "/path/to/file.rs",
///     "content": "full file content"
/// }
/// ```
///
/// Returns lines_added = line count of content, lines_removed = 0.
/// Write creates a new file, so there's nothing removed.
pub fn count_lines_in_write(input: &serde_json::Value) -> AiLineCount {
    let content = input
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    AiLineCount {
        lines_added: count_lines(content),
        lines_removed: 0,
    }
}

/// Count AI-contributed lines from a list of raw tool invocations.
///
/// Processes Edit and Write tool invocations and sums up the line counts.
/// Other tools (Read, Bash, etc.) are ignored.
///
/// # Arguments
/// * `invocations` - Slice of (tool_name, input_json) tuples
///
/// # Returns
/// Total `AiLineCount` across all Edit/Write invocations.
pub fn count_ai_lines<'a, I>(invocations: I) -> AiLineCount
where
    I: IntoIterator<Item = (&'a str, &'a serde_json::Value)>,
{
    let mut total = AiLineCount::zero();

    for (tool_name, input) in invocations {
        let count = match tool_name {
            "Edit" => count_lines_in_edit(input),
            "Write" => count_lines_in_write(input),
            _ => continue, // Ignore Read, Bash, etc.
        };
        total.merge(count);
    }

    total
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // count_lines tests
    // ========================================================================

    #[test]
    fn test_count_lines_empty() {
        assert_eq!(count_lines(""), 0);
    }

    #[test]
    fn test_count_lines_single_char() {
        assert_eq!(count_lines("a"), 1);
    }

    #[test]
    fn test_count_lines_single_line() {
        assert_eq!(count_lines("hello world"), 1);
    }

    #[test]
    fn test_count_lines_single_line_with_trailing_newline() {
        assert_eq!(count_lines("hello\n"), 1);
    }

    #[test]
    fn test_count_lines_two_lines() {
        assert_eq!(count_lines("hello\nworld"), 2);
    }

    #[test]
    fn test_count_lines_two_lines_trailing_newline() {
        assert_eq!(count_lines("hello\nworld\n"), 2);
    }

    #[test]
    fn test_count_lines_multiple() {
        assert_eq!(count_lines("a\nb\nc\nd"), 4);
    }

    #[test]
    fn test_count_lines_multiple_trailing() {
        assert_eq!(count_lines("a\nb\nc\nd\n"), 4);
    }

    #[test]
    fn test_count_lines_only_newline() {
        // Single newline = 1 empty line
        assert_eq!(count_lines("\n"), 1);
    }

    #[test]
    fn test_count_lines_two_newlines() {
        // Two newlines = 2 empty lines
        assert_eq!(count_lines("\n\n"), 2);
    }

    #[test]
    fn test_count_lines_blank_lines_middle() {
        // "a\n\nb" = line 1: "a", line 2: "", line 3: "b"
        assert_eq!(count_lines("a\n\nb"), 3);
    }

    // ========================================================================
    // count_lines_in_edit tests
    // ========================================================================

    #[test]
    fn test_edit_simple_replacement() {
        let input = json!({
            "file_path": "/test.rs",
            "old_string": "old code",
            "new_string": "new code"
        });
        let count = count_lines_in_edit(&input);
        assert_eq!(count.lines_added, 1);
        assert_eq!(count.lines_removed, 1);
    }

    #[test]
    fn test_edit_multiline_replacement() {
        let input = json!({
            "file_path": "/test.rs",
            "old_string": "line1\nline2\nline3",
            "new_string": "new1\nnew2"
        });
        let count = count_lines_in_edit(&input);
        assert_eq!(count.lines_added, 2);
        assert_eq!(count.lines_removed, 3);
    }

    #[test]
    fn test_edit_addition_only() {
        // Empty old_string = pure addition
        let input = json!({
            "file_path": "/test.rs",
            "old_string": "",
            "new_string": "new line 1\nnew line 2\n"
        });
        let count = count_lines_in_edit(&input);
        assert_eq!(count.lines_added, 2);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_edit_deletion_only() {
        // Empty new_string = pure deletion
        let input = json!({
            "file_path": "/test.rs",
            "old_string": "line to delete\nanother line",
            "new_string": ""
        });
        let count = count_lines_in_edit(&input);
        assert_eq!(count.lines_added, 0);
        assert_eq!(count.lines_removed, 2);
    }

    #[test]
    fn test_edit_missing_fields() {
        // Malformed input - missing fields
        let input = json!({
            "file_path": "/test.rs"
        });
        let count = count_lines_in_edit(&input);
        assert_eq!(count.lines_added, 0);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_edit_null_values() {
        let input = json!({
            "file_path": "/test.rs",
            "old_string": null,
            "new_string": null
        });
        let count = count_lines_in_edit(&input);
        assert_eq!(count.lines_added, 0);
        assert_eq!(count.lines_removed, 0);
    }

    // ========================================================================
    // count_lines_in_write tests
    // ========================================================================

    #[test]
    fn test_write_single_line() {
        let input = json!({
            "file_path": "/test.rs",
            "content": "fn main() {}"
        });
        let count = count_lines_in_write(&input);
        assert_eq!(count.lines_added, 1);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_write_multiline() {
        let input = json!({
            "file_path": "/test.rs",
            "content": "fn main() {\n    println!(\"Hello\");\n}\n"
        });
        let count = count_lines_in_write(&input);
        assert_eq!(count.lines_added, 3);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_write_empty_content() {
        let input = json!({
            "file_path": "/test.rs",
            "content": ""
        });
        let count = count_lines_in_write(&input);
        assert_eq!(count.lines_added, 0);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_write_missing_content() {
        let input = json!({
            "file_path": "/test.rs"
        });
        let count = count_lines_in_write(&input);
        assert_eq!(count.lines_added, 0);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_write_large_file() {
        // Simulate writing a 100-line file
        let content = (0..100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let input = json!({
            "file_path": "/test.rs",
            "content": content
        });
        let count = count_lines_in_write(&input);
        assert_eq!(count.lines_added, 100);
        assert_eq!(count.lines_removed, 0);
    }

    // ========================================================================
    // count_ai_lines tests
    // ========================================================================

    #[test]
    fn test_count_ai_lines_empty() {
        let invocations: Vec<(&str, &serde_json::Value)> = vec![];
        let count = count_ai_lines(invocations);
        assert_eq!(count.lines_added, 0);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_count_ai_lines_single_edit() {
        let edit_input = json!({
            "file_path": "/test.rs",
            "old_string": "old",
            "new_string": "new\nline"
        });
        let invocations = vec![("Edit", &edit_input)];
        let count = count_ai_lines(invocations);
        assert_eq!(count.lines_added, 2);
        assert_eq!(count.lines_removed, 1);
    }

    #[test]
    fn test_count_ai_lines_single_write() {
        let write_input = json!({
            "file_path": "/test.rs",
            "content": "line1\nline2\nline3"
        });
        let invocations = vec![("Write", &write_input)];
        let count = count_ai_lines(invocations);
        assert_eq!(count.lines_added, 3);
        assert_eq!(count.lines_removed, 0);
    }

    #[test]
    fn test_count_ai_lines_mixed() {
        let edit_input = json!({
            "file_path": "/a.rs",
            "old_string": "old1\nold2",
            "new_string": "new1"
        });
        let write_input = json!({
            "file_path": "/b.rs",
            "content": "write1\nwrite2\nwrite3"
        });
        let invocations = vec![("Edit", &edit_input), ("Write", &write_input)];
        let count = count_ai_lines(invocations);
        // Edit: +1, -2; Write: +3, -0
        assert_eq!(count.lines_added, 4);  // 1 + 3
        assert_eq!(count.lines_removed, 2); // 2 + 0
    }

    #[test]
    fn test_count_ai_lines_ignores_other_tools() {
        let read_input = json!({ "file_path": "/test.rs" });
        let bash_input = json!({ "command": "ls -la" });
        let edit_input = json!({
            "file_path": "/test.rs",
            "old_string": "old",
            "new_string": "new"
        });
        let invocations = vec![
            ("Read", &read_input),
            ("Bash", &bash_input),
            ("Edit", &edit_input),
        ];
        let count = count_ai_lines(invocations);
        // Only Edit should be counted
        assert_eq!(count.lines_added, 1);
        assert_eq!(count.lines_removed, 1);
    }

    #[test]
    fn test_count_ai_lines_multiple_edits() {
        let edit1 = json!({
            "file_path": "/a.rs",
            "old_string": "line1\nline2",
            "new_string": "new1\nnew2\nnew3"
        });
        let edit2 = json!({
            "file_path": "/b.rs",
            "old_string": "old",
            "new_string": ""
        });
        let invocations = vec![("Edit", &edit1), ("Edit", &edit2)];
        let count = count_ai_lines(invocations);
        // Edit1: +3, -2; Edit2: +0, -1
        assert_eq!(count.lines_added, 3);
        assert_eq!(count.lines_removed, 3);
    }

    // ========================================================================
    // AiLineCount tests
    // ========================================================================

    #[test]
    fn test_ai_line_count_net() {
        let count = AiLineCount {
            lines_added: 10,
            lines_removed: 3,
        };
        assert_eq!(count.net(), 7);

        let count2 = AiLineCount {
            lines_added: 5,
            lines_removed: 10,
        };
        assert_eq!(count2.net(), -5);
    }

    #[test]
    fn test_ai_line_count_merge() {
        let mut count = AiLineCount {
            lines_added: 10,
            lines_removed: 5,
        };
        let other = AiLineCount {
            lines_added: 3,
            lines_removed: 2,
        };
        count.merge(other);
        assert_eq!(count.lines_added, 13);
        assert_eq!(count.lines_removed, 7);
    }

    #[test]
    fn test_ai_line_count_zero() {
        let count = AiLineCount::zero();
        assert_eq!(count.lines_added, 0);
        assert_eq!(count.lines_removed, 0);
        assert_eq!(count.net(), 0);
    }
}
