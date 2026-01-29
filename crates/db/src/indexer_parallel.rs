// crates/db/src/indexer_parallel.rs
// Fast JSONL parsing with memory-mapped I/O and SIMD-accelerated scanning.

use memchr::memmem;
use std::io;
use std::path::Path;
use vibe_recall_core::ToolCounts;

/// Extended metadata extracted from JSONL deep parsing (Pass 2 only).
/// These fields are NOT available from sessions-index.json.
#[derive(Debug, Clone, Default)]
pub struct ExtendedMetadata {
    pub tool_counts: ToolCounts,
    pub skills_used: Vec<String>,
    pub files_touched: Vec<String>,
    pub last_message: String,
    pub turn_count: usize,
}

/// Read a file using memory-mapped I/O with fallback to regular read.
/// mmap is faster for large files (>64KB) because it avoids copying data through kernel buffers.
pub fn read_file_fast(path: &Path) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len() as usize;

    if len == 0 {
        return Ok(Vec::new());
    }

    // For small files, regular read is faster (no mmap overhead)
    if len < 64 * 1024 {
        return std::fs::read(path);
    }

    // Try mmap, fall back to regular read on failure
    // SAFETY: The file is read-only and we only hold the mapping briefly.
    // Claude Code appends to JSONL (never truncates), so the file won't shrink.
    match unsafe { memmap2::Mmap::map(&file) } {
        Ok(mmap) => Ok(mmap.to_vec()),
        Err(_) => std::fs::read(path),
    }
}

/// SIMD-accelerated line scanner that extracts only the fields NOT in sessions-index.json.
pub fn parse_bytes(data: &[u8]) -> ExtendedMetadata {
    let mut meta = ExtendedMetadata::default();
    let mut user_count = 0usize;
    let mut assistant_count = 0usize;
    let mut last_user_content: Option<String> = None;

    let user_finder = memmem::Finder::new(b"\"type\":\"user\"");
    let asst_finder = memmem::Finder::new(b"\"type\":\"assistant\"");

    // Tool name patterns for counting
    let read_finder = memmem::Finder::new(b"\"Read\"");
    let edit_finder = memmem::Finder::new(b"\"Edit\"");
    let write_finder = memmem::Finder::new(b"\"Write\"");
    let bash_finder = memmem::Finder::new(b"\"Bash\"");

    // File path pattern (from tool_use inputs)
    let file_path_finder = memmem::Finder::new(b"\"file_path\"");

    for line in split_lines_simd(data) {
        if line.is_empty() {
            continue;
        }

        if user_finder.find(line).is_some() {
            user_count += 1;
            // Extract content for last_message tracking
            if let Some(content) = extract_first_text_content(line) {
                last_user_content = Some(content);
            }
            // Check for skill invocations in user messages
            extract_skills_from_line(line, &mut meta.skills_used);
        } else if asst_finder.find(line).is_some() {
            assistant_count += 1;
            // Count tool usage
            if read_finder.find(line).is_some() {
                meta.tool_counts.read += count_occurrences(line, &read_finder);
            }
            if edit_finder.find(line).is_some() {
                meta.tool_counts.edit += count_occurrences(line, &edit_finder);
            }
            if write_finder.find(line).is_some() {
                meta.tool_counts.write += count_occurrences(line, &write_finder);
            }
            if bash_finder.find(line).is_some() {
                meta.tool_counts.bash += count_occurrences(line, &bash_finder);
            }
            // Extract file paths from tool_use inputs
            if file_path_finder.find(line).is_some() {
                extract_file_paths_from_line(line, &mut meta.files_touched);
            }
        }
    }

    meta.turn_count = user_count.min(assistant_count);
    meta.last_message = last_user_content
        .map(|c| truncate(&c, 200))
        .unwrap_or_default();

    // Deduplicate
    meta.skills_used.sort();
    meta.skills_used.dedup();
    meta.files_touched.sort();
    meta.files_touched.dedup();

    meta
}

/// Split data into lines using SIMD-accelerated newline search.
fn split_lines_simd(data: &[u8]) -> impl Iterator<Item = &[u8]> {
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

/// Count occurrences of a pattern in a line.
fn count_occurrences(line: &[u8], finder: &memmem::Finder) -> usize {
    let mut count = 0;
    let mut start = 0;
    while start < line.len() {
        if let Some(pos) = finder.find(&line[start..]) {
            count += 1;
            start += pos + finder.needle().len();
        } else {
            break;
        }
    }
    count
}

/// Extract the first text content from a JSONL line (best-effort, no full JSON parse).
fn extract_first_text_content(line: &[u8]) -> Option<String> {
    // Look for "content":"..." pattern (simple string content)
    let text_finder = memmem::Finder::new(b"\"content\":\"");
    if let Some(pos) = text_finder.find(line) {
        let start = pos + b"\"content\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    // or "text":"..." in content blocks
    let text_finder2 = memmem::Finder::new(b"\"text\":\"");
    if let Some(pos) = text_finder2.find(line) {
        let start = pos + b"\"text\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    None
}

/// Extract a JSON-escaped string value starting from after the opening quote.
fn extract_quoted_string(data: &[u8]) -> Option<String> {
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

/// Extract skill names from a user message line (looking for "skill":"..." patterns).
fn extract_skills_from_line(line: &[u8], skills: &mut Vec<String>) {
    let skill_name_finder = memmem::Finder::new(b"\"skill\":\"");
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

/// Extract file_path values from tool_use inputs.
fn extract_file_paths_from_line(line: &[u8], paths: &mut Vec<String>) {
    let finder = memmem::Finder::new(b"\"file_path\":\"");
    let mut start = 0;
    while start < line.len() {
        if let Some(pos) = finder.find(&line[start..]) {
            let begin = start + pos + b"\"file_path\":\"".len();
            if let Some(path) = extract_quoted_string(&line[begin..]) {
                if !path.is_empty() {
                    paths.push(path);
                }
            }
            start = begin;
        } else {
            break;
        }
    }
}

/// Truncate a string to at most `max_len` characters (not bytes).
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect::<String>() + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bytes_empty() {
        let meta = parse_bytes(b"");
        assert_eq!(meta.turn_count, 0);
        assert!(meta.last_message.is_empty());
        assert!(meta.tool_counts.is_empty());
    }

    #[test]
    fn test_parse_bytes_counts_tools() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Edit"}]}}
{"type":"user","message":{"content":"thanks"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}
"#;
        let meta = parse_bytes(data);
        assert_eq!(meta.turn_count, 2);
        assert_eq!(meta.tool_counts.read, 1);
        assert_eq!(meta.tool_counts.edit, 1);
        assert_eq!(meta.tool_counts.bash, 1);
        assert_eq!(meta.tool_counts.write, 0);
    }

    #[test]
    fn test_parse_bytes_last_message() {
        let data = br#"{"type":"user","message":{"content":"first question"}}
{"type":"assistant","message":{"content":"answer 1"}}
{"type":"user","message":{"content":"second question"}}
{"type":"assistant","message":{"content":"answer 2"}}
"#;
        let meta = parse_bytes(data);
        assert_eq!(meta.last_message, "second question");
    }

    #[test]
    fn test_read_file_fast_nonexistent() {
        let result = read_file_fast(std::path::Path::new("/nonexistent/file.jsonl"));
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_fast_small_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello world").unwrap();
        let data = read_file_fast(tmp.path()).unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    #[test]
    fn test_split_lines_simd() {
        let data = b"line1\nline2\nline3";
        let lines: Vec<&[u8]> = split_lines_simd(data).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], b"line1");
        assert_eq!(lines[1], b"line2");
        assert_eq!(lines[2], b"line3");
    }

    #[test]
    fn test_extract_file_paths() {
        let line = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}"#;
        let mut paths = Vec::new();
        extract_file_paths_from_line(line, &mut paths);
        assert!(paths.contains(&"/src/main.rs".to_string()));
        assert!(paths.contains(&"/src/lib.rs".to_string()));
    }
}
