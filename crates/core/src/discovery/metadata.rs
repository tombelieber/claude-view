// crates/core/src/discovery/metadata.rs
//! Efficient session metadata extraction via line-scanning (no full JSON parse).

use super::paths::{clean_for_preview, truncate_preview};
use crate::types::ToolCounts;
use regex_lite::Regex;
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Metadata extracted from a session file without full parsing.
#[derive(Debug, Clone, Default)]
pub struct ExtractedMetadata {
    pub preview: String,
    pub last_message: String,
    pub files_touched: Vec<String>,
    pub skills_used: Vec<String>,
    pub tool_counts: ToolCounts,
    pub message_count: usize,
    pub turn_count: usize,
}

/// Extract metadata from a session file without fully parsing it.
/// This is an efficient scan that looks for specific patterns.
pub async fn extract_session_metadata(file_path: &Path) -> ExtractedMetadata {
    let file = match fs::File::open(file_path).await {
        Ok(f) => f,
        Err(_) => return ExtractedMetadata::default(),
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut metadata = ExtractedMetadata::default();
    let mut first_user_message: Option<String> = None;
    let mut last_user_message: Option<String> = None;
    let mut user_count = 0;
    let mut assistant_count = 0;

    // Regex for detecting skills (slash commands like /commit, /review-pr, /superpowers:brainstorm)
    // Must NOT be followed by another / (to exclude file paths like /Users/test)
    // Captures the full skill including the leading /
    // Pattern: /word with optional :word or -word segments, not followed by /
    let skill_regex = Regex::new(r"(?:^|[^/\w])(/[a-zA-Z][\w:-]*)(?:[^/]|$)").ok();

    // Regex for extracting skill names from Skill tool_use blocks in assistant lines
    // Matches: "skill":"superpowers:systematic-debugging" (with optional whitespace around colon)
    let skill_input_regex = Regex::new(r#""skill"\s*:\s*"([^"]+)""#).ok();

    // Regex for file paths in tool inputs
    let file_path_regex = Regex::new(r#""file_path"\s*:\s*"([^"]+)""#).ok();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        metadata.message_count += 1;

        // Quick type detection without full JSON parsing
        if line.contains(r#""type":"user""#) || line.contains(r#""type": "user""#) {
            user_count += 1;

            // Extract content for preview
            if let Some(content) = extract_content_quick(line) {
                // Check for skills (slash commands)
                if let Some(ref re) = skill_regex {
                    for cap in re.captures_iter(&content) {
                        if let Some(skill) = cap.get(1) {
                            let skill_name = skill.as_str().to_string();
                            // Double-check: skill must start with / and not look like a file path
                            // File paths typically have multiple / like /Users/foo/bar
                            if skill_name.starts_with('/')
                                && !metadata.skills_used.contains(&skill_name)
                            {
                                metadata.skills_used.push(skill_name);
                            }
                        }
                    }
                }

                // Track first and last user message
                if first_user_message.is_none() && !content.trim().is_empty() {
                    // Skip messages that are just command tags
                    let cleaned = clean_for_preview(&content);
                    if !cleaned.is_empty() {
                        first_user_message = Some(cleaned);
                    }
                }
                last_user_message = Some(content);
            }
        } else if line.contains(r#""type":"assistant""#) || line.contains(r#""type": "assistant""#)
        {
            assistant_count += 1;

            // Count tool uses
            count_tools_quick(line, &mut metadata.tool_counts);

            // Extract skill names from Skill tool_use blocks
            if line.contains(r#""name":"Skill""#) || line.contains(r#""name": "Skill""#) {
                if let Some(ref re) = skill_input_regex {
                    for cap in re.captures_iter(line) {
                        if let Some(skill) = cap.get(1) {
                            let skill_name = skill.as_str().to_string();
                            if !skill_name.is_empty() && !metadata.skills_used.contains(&skill_name)
                            {
                                metadata.skills_used.push(skill_name);
                            }
                        }
                    }
                }
            }

            // Extract file paths (filename only, limit to 5)
            if metadata.files_touched.len() < 5 {
                if let Some(ref re) = file_path_regex {
                    for cap in re.captures_iter(line) {
                        if metadata.files_touched.len() >= 5 {
                            break;
                        }
                        if let Some(path) = cap.get(1) {
                            // Extract just the filename from the path
                            let path_str = path.as_str();
                            let filename =
                                path_str.rsplit('/').next().unwrap_or(path_str).to_string();
                            if !filename.is_empty() && !metadata.files_touched.contains(&filename) {
                                metadata.files_touched.push(filename);
                            }
                        }
                    }
                }
            }
        }
    }

    // Set preview from first user message
    if let Some(preview) = first_user_message {
        metadata.preview = truncate_preview(&preview, 200);
    }

    // Set last message
    if let Some(last) = last_user_message {
        metadata.last_message = truncate_preview(&last, 200);
    }

    // Calculate turn count
    metadata.turn_count = user_count.min(assistant_count);

    metadata
}

/// Quick content extraction without full JSON parsing.
fn extract_content_quick(line: &str) -> Option<String> {
    // Look for "content":"..." pattern
    let content_start = line.find(r#""content":"#)?;
    let after_content = &line[content_start + 10..];

    // Handle string content
    if let Some(content_str) = after_content.strip_prefix('"') {
        if let Some(end) = find_string_end(content_str) {
            let content = &content_str[..end];
            // Unescape basic JSON escapes
            return Some(unescape_json_string(content));
        }
    }

    // Handle array content (blocks)
    if after_content.starts_with('[') {
        // Look for text blocks
        let mut result = String::new();
        let mut search_pos = 0;
        while let Some(text_pos) = after_content[search_pos..].find(r#""text":""#) {
            let start = search_pos + text_pos + 8;
            if start < after_content.len() {
                if let Some(end) = find_string_end(&after_content[start..]) {
                    let text = &after_content[start..start + end];
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(&unescape_json_string(text));
                    search_pos = start + end;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        if !result.is_empty() {
            return Some(result);
        }
    }

    None
}

/// Find the end of a JSON string (handling escapes).
fn find_string_end(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some(i),
            '\\' => {
                // Skip the next character
                chars.next();
            }
            _ => {}
        }
    }
    None
}

/// Basic JSON string unescaping.
fn unescape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Quick tool counting without full JSON parsing.
fn count_tools_quick(line: &str, counts: &mut ToolCounts) {
    // Count tool_use blocks by name (handle both with and without space after colon)
    counts.read += line.matches(r#""name":"Read""#).count();
    counts.read += line.matches(r#""name": "Read""#).count();
    counts.edit += line.matches(r#""name":"Edit""#).count();
    counts.edit += line.matches(r#""name": "Edit""#).count();
    counts.write += line.matches(r#""name":"Write""#).count();
    counts.write += line.matches(r#""name": "Write""#).count();
    counts.bash += line.matches(r#""name":"Bash""#).count();
    counts.bash += line.matches(r#""name": "Bash""#).count();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    // ============================================================================
    // extract_session_metadata Tests
    // ============================================================================

    #[tokio::test]
    async fn test_extract_session_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Hello, please help me"},"timestamp":"2026-01-27T10:00:00Z"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Sure!"},{"type":"tool_use","name":"Read","input":{"file_path":"/test/file.rs"}}]},"timestamp":"2026-01-27T10:00:01Z"}
{"type":"user","message":{"content":"Now /commit the changes"},"timestamp":"2026-01-27T10:00:02Z"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Done"},{"type":"tool_use","name":"Edit","input":{"file_path":"/test/other.rs"}}]},"timestamp":"2026-01-27T10:00:03Z"}"#;

        let mut file = tokio::fs::File::create(&file_path).await.unwrap();
        file.write_all(content.as_bytes()).await.unwrap();
        file.flush().await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert_eq!(metadata.preview, "Hello, please help me");
        assert_eq!(metadata.message_count, 4);
        assert_eq!(metadata.turn_count, 2);
        assert_eq!(metadata.tool_counts.read, 1);
        assert_eq!(metadata.tool_counts.edit, 1);
        assert!(metadata.skills_used.contains(&"/commit".to_string()));
        // Note: files_touched now contains only filenames, not full paths
        assert!(metadata.files_touched.contains(&"file.rs".to_string()));
    }

    #[tokio::test]
    async fn test_extract_session_metadata_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.jsonl");

        tokio::fs::File::create(&file_path).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(metadata.preview.is_empty());
        assert_eq!(metadata.message_count, 0);
        assert_eq!(metadata.turn_count, 0);
    }

    #[tokio::test]
    async fn test_extract_session_metadata_nonexistent_file() {
        let metadata = extract_session_metadata(Path::new("/nonexistent/file.jsonl")).await;
        assert!(metadata.preview.is_empty());
        assert_eq!(metadata.message_count, 0);
    }

    // ============================================================================
    // Issue 6: filesTouched - Limit to 5, Filename Only
    // ============================================================================

    #[tokio::test]
    async fn test_files_touched_limited_to_5() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file1.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file2.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file3.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file4.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file5.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file6.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file7.rs"}}]}}"#;

        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.files_touched.len() <= 5,
            "Should limit to 5 files, got: {}",
            metadata.files_touched.len()
        );
    }

    #[tokio::test]
    async fn test_files_touched_shows_filename_only() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/Users/test/project/src/main.rs"}}]}}"#;

        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.files_touched.contains(&"main.rs".to_string()),
            "Should contain filename only, got: {:?}",
            metadata.files_touched
        );
        assert!(
            !metadata.files_touched.iter().any(|f| f.contains('/')),
            "Should NOT contain paths with slashes, got: {:?}",
            metadata.files_touched
        );
    }

    // ============================================================================
    // Issue 4: Skills Extraction Tests
    // ============================================================================

    #[tokio::test]
    async fn test_skills_extraction_captures_slash_commands() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Please /commit my changes"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.skills_used.contains(&"/commit".to_string()),
            "Should contain /commit, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_with_colon_separator() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content =
            r#"{"type":"user","message":{"content":"Run /superpowers:brainstorm please"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata
                .skills_used
                .contains(&"/superpowers:brainstorm".to_string()),
            "Should contain /superpowers:brainstorm, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_with_hyphen() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Please /review-pr this"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.skills_used.contains(&"/review-pr".to_string()),
            "Should contain /review-pr, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_ignores_file_paths() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content =
            r#"{"type":"user","message":{"content":"Check file at /Users/test/path/file.rs"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            !metadata.skills_used.iter().any(|s| s.contains("Users")),
            "Should NOT contain /Users, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_multiple_skills() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"/commit then /push please"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.skills_used.contains(&"/commit".to_string()),
            "Should contain /commit"
        );
        assert!(
            metadata.skills_used.contains(&"/push".to_string()),
            "Should contain /push"
        );
    }

    // ============================================================================
    // Helper Function Tests
    // ============================================================================

    #[test]
    fn test_extract_content_quick_string() {
        let line = r#"{"type":"user","message":{"content":"Hello world"}}"#;
        let content = extract_content_quick(line);
        assert_eq!(content, Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_content_quick_with_escapes() {
        let line = r#"{"type":"user","message":{"content":"Hello\nworld"}}"#;
        let content = extract_content_quick(line);
        assert_eq!(content, Some("Hello\nworld".to_string()));
    }

    #[test]
    fn test_extract_content_quick_blocks() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"text","text":"World"}]}}"#;
        let content = extract_content_quick(line);
        assert!(content.is_some());
        let text = content.unwrap();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_count_tools_quick() {
        let line = r#"{"content":[{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Edit"}]}"#;
        let mut counts = ToolCounts::default();
        count_tools_quick(line, &mut counts);

        assert_eq!(counts.read, 2);
        assert_eq!(counts.edit, 1);
        assert_eq!(counts.write, 0);
        assert_eq!(counts.bash, 0);
    }

    #[test]
    fn test_unescape_json_string() {
        assert_eq!(unescape_json_string(r"hello\nworld"), "hello\nworld");
        assert_eq!(unescape_json_string(r"tab\there"), "tab\there");
        assert_eq!(unescape_json_string(r#"quote\"here"#), "quote\"here");
        assert_eq!(unescape_json_string(r"back\\slash"), "back\\slash");
    }

    #[test]
    fn test_find_string_end() {
        assert_eq!(find_string_end(r#"hello""#), Some(5));
        assert_eq!(find_string_end(r#"he\"llo""#), Some(7));
        assert_eq!(find_string_end(r#"no end"#), None);
    }
}
