// crates/core/src/discovery/paths.rs
//! Path helpers for project discovery.

use crate::error::DiscoveryError;
use regex_lite::Regex;
use std::path::{Path, PathBuf};

/// Returns the path to the Claude projects directory (~/.claude/projects).
///
/// # Errors
/// Returns `DiscoveryError::HomeDirNotFound` if the home directory cannot be determined.
pub fn claude_projects_dir() -> Result<PathBuf, DiscoveryError> {
    let home = dirs::home_dir().ok_or(DiscoveryError::HomeDirNotFound)?;
    Ok(home.join(".claude").join("projects"))
}

/// Derive a human-friendly display name from a resolved filesystem path.
///
/// Strategy:
/// 1. Walk up from the resolved path to find the nearest `.git` directory
/// 2. Then walk further up to find the **topmost** `.git` within 5 levels
///    (handles worktrees/nested repos like `my-app/web` inside `my-app`)
/// 3. Display name = topmost git root name + relative path
///
/// Examples:
/// - `/Users/foo/dev/@org/my-project` (git at my-project) -> `my-project`
/// - `/Users/foo/dev/@org/repo/web`   (git at both repo and web) -> `repo/web`
/// - `/Users/foo`                     (no git root)             -> `foo`
pub(crate) fn derive_display_name(resolved_path: &str) -> String {
    let path = Path::new(resolved_path);

    // Find the topmost git root within 5 levels above the resolved path
    let mut topmost_git_root: Option<&Path> = None;
    let mut current = path;

    for _ in 0..5 {
        if current.join(".git").exists() {
            topmost_git_root = Some(current);
        }

        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    if let Some(git_root) = topmost_git_root {
        let git_root_name = git_root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        if git_root == path {
            return git_root_name;
        }

        // path is deeper than git root — include relative suffix
        if let Ok(relative) = path.strip_prefix(git_root) {
            return format!("{}/{}", git_root_name, relative.display());
        }

        return git_root_name;
    }

    // No git root found — fall back to last path component
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| resolved_path.to_string())
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

/// Clean content for preview display.
pub(crate) fn clean_for_preview(content: &str) -> String {
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
    // Display Name Tests
    // ============================================================================

    #[test]
    fn test_display_name_git_root_at_resolved_path() {
        let temp = TempDir::new().unwrap();
        let deep = temp.path().join("a/b/c/d/e/f/my-project");
        std::fs::create_dir_all(deep.join(".git")).unwrap();

        let name = derive_display_name(&deep.to_string_lossy());
        assert_eq!(name, "my-project");
    }

    #[test]
    fn test_display_name_subdirectory_of_git_root() {
        let temp = TempDir::new().unwrap();
        let deep = temp.path().join("a/b/c/d/e/f/repo");
        std::fs::create_dir_all(deep.join(".git")).unwrap();
        let subdir = deep.join("web");
        std::fs::create_dir_all(&subdir).unwrap();

        let name = derive_display_name(&subdir.to_string_lossy());
        assert_eq!(name, "repo/web");
    }

    #[test]
    fn test_display_name_nested_git_uses_topmost() {
        let temp = TempDir::new().unwrap();
        let deep = temp.path().join("a/b/c/d/e/f");
        let parent = deep.join("parent");
        std::fs::create_dir_all(parent.join(".git")).unwrap();
        let child = parent.join("child");
        std::fs::create_dir_all(child.join(".git")).unwrap();

        let name = derive_display_name(&child.to_string_lossy());
        assert_eq!(name, "parent/child");
    }

    #[test]
    fn test_display_name_no_git_root_fallback() {
        let temp = TempDir::new().unwrap();
        let deep = temp.path().join("a/b/c/d/e/f/some-dir");
        std::fs::create_dir_all(&deep).unwrap();

        let name = derive_display_name(&deep.to_string_lossy());
        assert_eq!(name, "some-dir");
    }

    // ============================================================================
    // clean_for_preview Tests
    // ============================================================================

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
}
