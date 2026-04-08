// crates/core/src/discovery/resolve.rs
//! Project path resolution: encoding, worktree parent, display names.

use super::paths::derive_display_name;

/// Resolved project path information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProject {
    /// The full filesystem path (e.g., "/Users/foo/my-project")
    pub full_path: String,
    /// Human-readable display name (e.g., "my-project")
    pub display_name: String,
}

/// Resolve project path. Primary source: cwd from JSONL.
/// When cwd is None, returns the encoded name as-is (no guessing).
pub fn resolve_project_path_with_cwd(encoded_name: &str, cwd: Option<&str>) -> ResolvedProject {
    if let Some(path) = cwd {
        return ResolvedProject {
            full_path: path.to_string(),
            display_name: derive_display_name(path),
        };
    }

    // No cwd available — return encoded name verbatim as both fields.
    // Per design: "no heuristic / guessing, all evidence based."
    // The naive `-` split produces wrong paths for directories with `@` or
    // `-` in names (e.g. @acme-corp -> /@acme/corp). Without CWD evidence
    // we show the raw encoded string rather than guessing wrong.
    ResolvedProject {
        full_path: encoded_name.to_string(),
        display_name: encoded_name.to_string(),
    }
}

/// If the encoded project name represents a git worktree, return the parent
/// project's encoded name. Otherwise return None.
///
/// Worktree paths: `-Users-dev-project--worktrees-branch-name`
/// Parent:         `-Users-dev-project`
///
/// The `--worktrees-` segment maps to `/.worktrees/` on disk.
pub fn resolve_worktree_parent(encoded_name: &str) -> Option<String> {
    let marker = "--worktrees-";
    let pos = encoded_name.find(marker)?;
    if pos == 0 {
        return None; // edge case: name starts with marker
    }
    Some(encoded_name[..pos].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // cwd-based Resolution Tests
    // ============================================================================

    #[test]
    fn test_resolve_project_path_with_cwd_some() {
        let result =
            resolve_project_path_with_cwd("-Users-dev-claude-view", Some("/Users/dev/claude-view"));
        assert_eq!(result.full_path, "/Users/dev/claude-view");
        assert_eq!(result.display_name, "claude-view");
    }

    #[test]
    fn test_resolve_project_path_with_cwd_none_returns_encoded_name() {
        let result = resolve_project_path_with_cwd("-Users-dev-my-project", None);
        assert_eq!(result.full_path, "-Users-dev-my-project");
        assert_eq!(result.display_name, "-Users-dev-my-project");
    }

    #[test]
    fn test_resolve_project_path_with_cwd_empty_encoded() {
        let result = resolve_project_path_with_cwd("", None);
        assert_eq!(result.full_path, "");
        assert_eq!(result.display_name, "");
    }

    #[test]
    fn test_resolve_project_path_with_cwd_scoped_package() {
        let result = resolve_project_path_with_cwd(
            "-Users-dev--acme-corp-my-project",
            Some("/Users/dev/@acme-corp/my-project"),
        );
        assert_eq!(result.full_path, "/Users/dev/@acme-corp/my-project");
        assert_eq!(result.display_name, "my-project");
    }

    // ========================================================================
    // resolve_worktree_parent Tests
    // ========================================================================

    #[test]
    fn test_worktree_parent_basic() {
        assert_eq!(
            resolve_worktree_parent("-Users-dev-project--worktrees-feature-branch"),
            Some("-Users-dev-project".to_string())
        );
    }

    #[test]
    fn test_non_worktree_returns_none() {
        assert_eq!(resolve_worktree_parent("-Users-dev-project"), None);
    }

    #[test]
    fn test_worktree_parent_edge_cases() {
        assert_eq!(resolve_worktree_parent(""), None);
        assert_eq!(resolve_worktree_parent("--worktrees-foo"), None); // marker at pos 0
    }

    #[test]
    fn test_worktree_parent_preserves_complex_parent() {
        assert_eq!(
            resolve_worktree_parent(
                "-Users-dev--myorg-claude-view--worktrees-theme3-contributions"
            ),
            Some("-Users-dev--myorg-claude-view".to_string())
        );
    }
}
