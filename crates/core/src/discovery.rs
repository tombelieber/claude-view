// crates/core/src/discovery.rs
//! Project discovery for Claude Code sessions.
//!
//! This module scans `~/.claude/projects/` to discover all Claude Code projects
//! and their sessions. It handles the encoded directory names that Claude uses
//! and efficiently extracts session metadata without fully parsing each file.

use crate::error::DiscoveryError;
use crate::types::{ProjectInfo, SessionInfo, ToolCounts};
use regex_lite::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

/// Returns the path to the Claude projects directory (~/.claude/projects).
///
/// # Errors
/// Returns `DiscoveryError::HomeDirNotFound` if the home directory cannot be determined.
pub fn claude_projects_dir() -> Result<PathBuf, DiscoveryError> {
    let home = dirs::home_dir().ok_or(DiscoveryError::HomeDirNotFound)?;
    Ok(home.join(".claude").join("projects"))
}

/// Resolved project path information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProject {
    /// The full filesystem path (e.g., "/Users/foo/my-project")
    pub full_path: String,
    /// Human-readable display name (e.g., "my-project")
    pub display_name: String,
}

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

/// Resolve an encoded project directory name to a filesystem path.
///
/// Claude encodes paths like `/Users/foo/my-project` as `-Users-foo-my-project`.
/// The challenge is that hyphens in real directory names look like path separators.
///
/// Strategy:
/// 1. Try increasingly shorter prefix paths until we find one that exists
/// 2. For ambiguous cases, prefer longer existing paths
/// 3. Fall back to basic decode if no path exists
pub fn resolve_project_path(encoded_name: &str) -> ResolvedProject {
    // Handle empty or single-character names
    if encoded_name.is_empty() {
        return ResolvedProject {
            full_path: String::new(),
            display_name: String::new(),
        };
    }

    // Get all possible join variants
    let variants = get_join_variants(encoded_name);

    // Try each variant, prefer the one that exists on filesystem
    for variant in &variants {
        if Path::new(variant).exists() {
            let display_name = Path::new(variant)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| variant.clone());
            return ResolvedProject {
                full_path: variant.clone(),
                display_name,
            };
        }
    }

    // Fallback: use the first (most likely) variant
    let fallback = variants.into_iter().next().unwrap_or_default();
    let display_name = Path::new(&fallback)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| fallback.clone());

    ResolvedProject {
        full_path: fallback,
        display_name,
    }
}

/// Generate all possible path interpretations of an encoded name.
///
/// For `-Users-foo-my-project`, this generates:
/// - `/Users/foo/my-project` (all hyphens as separators)
/// - `/Users/foo/my/project` (more separators)
/// - etc.
///
/// The function uses a heuristic: leading hyphen becomes `/`, internal hyphens
/// could be path separators or literal hyphens.
pub fn get_join_variants(encoded_name: &str) -> Vec<String> {
    // Remove leading hyphen if present
    let name = encoded_name.strip_prefix('-').unwrap_or(encoded_name);

    if name.is_empty() {
        return vec!["/".to_string()];
    }

    // Split by hyphens
    let parts: Vec<&str> = name.split('-').collect();

    if parts.is_empty() {
        return vec![format!("/{}", name)];
    }

    // Generate variants by trying different groupings
    // For simplicity, we generate the most common patterns:
    // 1. All hyphens as path separators
    // 2. Keep hyphens in the last component (most common for project names)
    // 3. Keep hyphens in the last two components

    let mut variants = Vec::new();

    // Variant 1: All hyphens as path separators
    let all_sep = format!("/{}", parts.join("/"));
    variants.push(all_sep);

    // Variant 2: Last component keeps its hyphens
    if parts.len() >= 2 {
        let last = parts.last().unwrap();
        let rest = &parts[..parts.len() - 1];
        let v = format!("/{}/{}", rest.join("/"), last);
        if !variants.contains(&v) {
            variants.push(v);
        }
    }

    // Variant 3: Last two components might be hyphenated project name
    if parts.len() >= 3 {
        let last_two = parts[parts.len() - 2..].join("-");
        let rest = &parts[..parts.len() - 2];
        let v = format!("/{}/{}", rest.join("/"), last_two);
        if !variants.contains(&v) {
            variants.push(v);
        }
    }

    // Variant 4: Last three components might be hyphenated project name
    if parts.len() >= 4 {
        let last_three = parts[parts.len() - 3..].join("-");
        let rest = &parts[..parts.len() - 3];
        let v = format!("/{}/{}", rest.join("/"), last_three);
        if !variants.contains(&v) {
            variants.push(v);
        }
    }

    variants
}

/// Truncate a string to a maximum length, adding ellipsis if needed.
/// Truncates at word boundary when possible. Handles multi-byte UTF-8 safely.
pub fn truncate_preview(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();

    // Count characters, not bytes
    let char_count = trimmed.chars().count();
    if char_count <= max_len {
        return trimmed.to_string();
    }

    // Collect characters up to max_len
    let truncated: String = trimmed.chars().take(max_len).collect();

    // Try to find a word boundary (space) in the truncated string
    // rfind returns byte index, so we need to find char index instead
    if let Some(last_space_byte_idx) = truncated.rfind(' ') {
        // Convert byte index to char index
        let char_idx_at_space = truncated[..last_space_byte_idx].chars().count();
        if char_idx_at_space > max_len / 2 {
            // Take chars up to the space
            let up_to_space: String = truncated.chars().take(char_idx_at_space).collect();
            return format!("{}...", up_to_space.trim_end());
        }
    }

    format!("{}...", truncated.trim_end())
}

/// Discover all Claude Code projects and their sessions.
///
/// # Returns
/// A list of `ProjectInfo` sorted by most recent session activity.
///
/// # Errors
/// Returns an error only for permission denied. Missing directories return empty vec.
pub async fn get_projects() -> Result<Vec<ProjectInfo>, DiscoveryError> {
    let projects_dir = claude_projects_dir()?;

    // If the directory doesn't exist, return empty list (not an error)
    if !projects_dir.exists() {
        debug!("Claude projects directory does not exist: {:?}", projects_dir);
        return Ok(vec![]);
    }

    let mut entries = match fs::read_dir(&projects_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(DiscoveryError::PermissionDenied {
                path: projects_dir,
            });
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => {
            return Err(DiscoveryError::Io {
                path: projects_dir,
                source: e,
            });
        }
    };

    let mut projects = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|e| DiscoveryError::Io {
        path: projects_dir.clone(),
        source: e,
    })? {
        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        let encoded_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let resolved = resolve_project_path(&encoded_name);

        // Get sessions for this project
        let sessions = match get_project_sessions(&path, &encoded_name, &resolved).await {
            Ok(s) => s,
            Err(e) => {
                debug!("Error reading sessions for {:?}: {}", path, e);
                continue;
            }
        };

        // Skip projects with no sessions
        if sessions.is_empty() {
            continue;
        }

        projects.push(ProjectInfo {
            name: encoded_name,
            display_name: resolved.display_name,
            path: resolved.full_path,
            active_count: sessions.len(),
            sessions,
        });
    }

    // Sort projects by most recent session
    projects.sort_by(|a, b| {
        let a_latest = a.sessions.iter().map(|s| s.modified_at).max().unwrap_or(0);
        let b_latest = b.sessions.iter().map(|s| s.modified_at).max().unwrap_or(0);
        b_latest.cmp(&a_latest)
    });

    Ok(projects)
}

/// Get all sessions for a project directory.
async fn get_project_sessions(
    project_path: &Path,
    encoded_name: &str,
    resolved: &ResolvedProject,
) -> Result<Vec<SessionInfo>, std::io::Error> {
    let mut entries = fs::read_dir(project_path).await?;
    let mut sessions = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Only process .jsonl files
        if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
            continue;
        }

        let metadata = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Extract session ID from filename
        let session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract metadata efficiently
        let extracted = extract_session_metadata(&path).await;

        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        sessions.push(SessionInfo {
            id: session_id,
            project: encoded_name.to_string(),
            project_path: resolved.full_path.clone(),
            file_path: path.to_string_lossy().to_string(),
            modified_at,
            size_bytes: metadata.len(),
            preview: extracted.preview,
            last_message: extracted.last_message,
            files_touched: extracted.files_touched,
            skills_used: extracted.skills_used,
            tool_counts: extracted.tool_counts,
            message_count: extracted.message_count,
            turn_count: extracted.turn_count,
        });
    }

    // Sort sessions by modification time (most recent first)
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(sessions)
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

    // Regex for detecting skills (slash commands)
    let skill_regex = Regex::new(r"/([a-zA-Z][a-zA-Z0-9_-]*)").ok();

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
                // Check for skills
                if let Some(ref re) = skill_regex {
                    for cap in re.captures_iter(&content) {
                        if let Some(skill) = cap.get(1) {
                            let skill_name = skill.as_str().to_string();
                            if !metadata.skills_used.contains(&skill_name) {
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
        } else if line.contains(r#""type":"assistant""#) || line.contains(r#""type": "assistant""#) {
            assistant_count += 1;

            // Count tool uses
            count_tools_quick(line, &mut metadata.tool_counts);

            // Extract file paths
            if let Some(ref re) = file_path_regex {
                for cap in re.captures_iter(line) {
                    if let Some(path) = cap.get(1) {
                        let path_str = path.as_str().to_string();
                        if !metadata.files_touched.contains(&path_str) {
                            metadata.files_touched.push(path_str);
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

/// Clean content for preview display.
fn clean_for_preview(content: &str) -> String {
    // Remove command tags
    let tag_regex = Regex::new(r"<command-name>[^<]*</command-name>\s*").unwrap();
    let args_regex = Regex::new(r"<command-args>[^<]*</command-args>\s*").unwrap();

    let cleaned = tag_regex.replace_all(content, "");
    let cleaned = args_regex.replace_all(&cleaned, "");
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    // ============================================================================
    // claude_projects_dir Tests
    // ============================================================================

    #[test]
    fn test_claude_projects_dir() {
        let result = claude_projects_dir();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().contains(".claude"));
        assert!(path.to_string_lossy().contains("projects"));
        assert!(path.ends_with("projects"));
    }

    #[test]
    fn test_claude_projects_dir_format() {
        let path = claude_projects_dir().unwrap();
        let path_str = path.to_string_lossy();

        // Should be an absolute path
        assert!(path_str.starts_with('/') || path_str.contains(':'));

        // Should end with .claude/projects
        assert!(path_str.ends_with(".claude/projects") || path_str.ends_with(".claude\\projects"));
    }

    // ============================================================================
    // resolve_project_path Tests
    // ============================================================================

    #[test]
    fn test_resolve_simple_path() {
        // This is a fallback test since /tmp probably doesn't exist as encoded
        let resolved = resolve_project_path("-tmp");
        assert_eq!(resolved.full_path, "/tmp");
        assert_eq!(resolved.display_name, "tmp");
    }

    #[test]
    fn test_resolve_nonexistent_path() {
        // A path that definitely doesn't exist
        let resolved = resolve_project_path("-nonexistent-path-abc123");
        // Should fall back to basic decode
        assert!(resolved.full_path.starts_with('/'));
        assert!(!resolved.display_name.is_empty());
    }

    #[test]
    fn test_resolve_empty_path() {
        let resolved = resolve_project_path("");
        assert_eq!(resolved.full_path, "");
        assert_eq!(resolved.display_name, "");
    }

    #[test]
    fn test_resolve_complex_path() {
        // A typical Claude project path
        let resolved = resolve_project_path("-Users-test-dev-my-cool-project");
        // Should produce a reasonable path
        assert!(resolved.full_path.starts_with('/'));
        // The display name should be the last component
        assert!(!resolved.display_name.is_empty());
    }

    // ============================================================================
    // get_join_variants Tests
    // ============================================================================

    #[test]
    fn test_get_join_variants_simple() {
        let variants = get_join_variants("-tmp");
        assert!(!variants.is_empty());
        assert!(variants.contains(&"/tmp".to_string()));
    }

    #[test]
    fn test_get_join_variants_hyphenated_name() {
        let variants = get_join_variants("-Users-test-my-project");
        assert!(!variants.is_empty());

        // Should include various interpretations
        assert!(variants.contains(&"/Users/test/my/project".to_string()));
        assert!(variants.contains(&"/Users/test/my-project".to_string()));
    }

    #[test]
    fn test_get_join_variants_without_leading_hyphen() {
        let variants = get_join_variants("Users-test-project");
        assert!(!variants.is_empty());
        // Should still work, treating it as relative
        assert!(variants.iter().any(|v| v.contains("Users")));
    }

    #[test]
    fn test_get_join_variants_single_part() {
        let variants = get_join_variants("-tmp");
        assert!(variants.contains(&"/tmp".to_string()));
    }

    #[test]
    fn test_get_join_variants_empty() {
        let variants = get_join_variants("");
        assert!(variants.contains(&"/".to_string()) || !variants.is_empty());
    }

    // ============================================================================
    // truncate_preview Tests
    // ============================================================================

    #[test]
    fn test_truncate_preview_short_string() {
        let text = "Hello world";
        let result = truncate_preview(text, 50);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_truncate_preview_long_string() {
        let text = "This is a very long string that definitely exceeds the maximum length";
        let result = truncate_preview(text, 30);
        assert!(result.len() <= 33); // 30 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_word_boundary() {
        let text = "Hello world this is a test";
        let result = truncate_preview(text, 15);
        // Should truncate at word boundary if possible
        assert!(result.ends_with("..."));
        assert!(result.len() <= 18);
    }

    #[test]
    fn test_truncate_preview_exact_length() {
        let text = "Exactly 10";
        let result = truncate_preview(text, 10);
        assert_eq!(result, "Exactly 10");
    }

    #[test]
    fn test_truncate_preview_whitespace() {
        let text = "  Hello world  ";
        let result = truncate_preview(text, 50);
        assert_eq!(result, "Hello world");
    }

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
        assert!(metadata.skills_used.contains(&"commit".to_string()));
        assert!(metadata.files_touched.contains(&"/test/file.rs".to_string()));
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
    // get_projects Tests
    // ============================================================================

    #[tokio::test]
    async fn test_get_projects_empty_dir() {
        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join(".claude").join("projects");
        tokio::fs::create_dir_all(&projects_dir).await.unwrap();

        // We can't easily test get_projects with a custom path since it uses claude_projects_dir()
        // Instead, test that it handles the real path gracefully
        let result = get_projects().await;
        // Should not error, even if dir doesn't exist or is empty
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_projects_returns_vec() {
        let result = get_projects().await;
        assert!(result.is_ok());
        // The result is a Vec, which may or may not have items
        let _projects: Vec<ProjectInfo> = result.unwrap();
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
    fn test_clean_for_preview() {
        let content = "<command-name>/commit</command-name>\nPlease commit my changes";
        let cleaned = clean_for_preview(content);
        assert_eq!(cleaned, "Please commit my changes");
    }

    #[test]
    fn test_clean_for_preview_no_tags() {
        let content = "Normal message without tags";
        let cleaned = clean_for_preview(content);
        assert_eq!(cleaned, "Normal message without tags");
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

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[tokio::test]
    async fn test_get_project_sessions_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved).await;
        assert!(sessions.is_ok());
        assert!(sessions.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_project_sessions_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        // Create a test session file
        let session_path = temp_dir.path().join("session-123.jsonl");
        let content = r#"{"type":"user","message":{"content":"Test"}}"#;
        tokio::fs::write(&session_path, content).await.unwrap();

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved)
            .await
            .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-123");
        assert_eq!(sessions[0].preview, "Test");
    }

    #[tokio::test]
    async fn test_get_project_sessions_ignores_non_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        // Create various files
        tokio::fs::write(temp_dir.path().join("session.jsonl"), r#"{"type":"user","message":{"content":"Test"}}"#).await.unwrap();
        tokio::fs::write(temp_dir.path().join("notes.txt"), "some notes").await.unwrap();
        tokio::fs::write(temp_dir.path().join("config.json"), "{}").await.unwrap();

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved)
            .await
            .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session");
    }
}
