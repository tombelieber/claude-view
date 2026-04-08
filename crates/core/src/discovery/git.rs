// crates/core/src/discovery/git.rs
//! Git root and branch resolution for repos and worktrees.

/// Resolve the canonical git repository root for a given working directory.
///
/// Uses `git rev-parse --path-format=absolute --git-common-dir` which returns
/// the shared `.git` directory across all worktrees of the same repo.
/// Taking its parent gives the repo root that is consistent whether you're in
/// the main worktree or any linked worktree.
///
/// Returns `None` if `cwd` is not inside a git repo, the directory doesn't
/// exist, or git is not installed.
///
/// Requires git >= 2.31 (March 2021) for `--path-format=absolute`.
pub async fn resolve_git_root(cwd: &str) -> Option<String> {
    use tokio::process::Command;
    let out = Command::new("git")
        .args([
            "-C",
            cwd,
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ])
        .output()
        .await
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let common_dir = std::str::from_utf8(&out.stdout).ok()?.trim();
    std::path::Path::new(common_dir)
        .parent()?
        .to_str()
        .map(|s| s.to_string())
}

/// Extract the parent repository root from a worktree `cwd` path.
///
/// Detects both legacy `/.worktrees/<name>` and current `/.claude/worktrees/<name>`
/// patterns. Returns the repo root (portion before the marker), or None.
///
/// This is a pure string operation — no disk access, no git commands.
pub fn infer_git_root_from_worktree_path(cwd: &str) -> Option<String> {
    for marker in &["/.worktrees/", "/.claude/worktrees/"] {
        if let Some(idx) = cwd.find(marker) {
            let root = &cwd[..idx];
            if !root.is_empty() {
                return Some(root.to_string());
            }
        }
    }
    None
}

/// Resolve the current git branch for any directory (regular repo or worktree).
///
/// 1. If `<cwd>/.git` is a **file** (worktree), delegates to `resolve_worktree_branch`.
/// 2. If `<cwd>/.git` is a **directory** (regular repo), reads `.git/HEAD`.
/// 3. Returns `None` if not a git repo, HEAD is detached, or any I/O error.
///
/// Pure filesystem operation — no git subprocess.
pub fn resolve_git_branch(cwd: &str) -> Option<String> {
    let dot_git = std::path::Path::new(cwd).join(".git");
    if dot_git.is_file() {
        return resolve_worktree_branch(cwd);
    }
    if dot_git.is_dir() {
        let head = std::fs::read_to_string(dot_git.join("HEAD")).ok()?;
        return head
            .trim()
            .strip_prefix("ref: refs/heads/")
            .map(|b| b.to_string());
    }
    None
}

/// Resolve the actual git branch for a worktree directory.
///
/// Reads the worktree's `.git` file to find the gitdir, then reads
/// `<gitdir>/HEAD` to extract the branch ref. Returns `None` if:
/// - The directory has no `.git` file (not a worktree)
/// - HEAD is detached (raw SHA, no `ref:` prefix)
/// - Any I/O error occurs
///
/// This is a pure filesystem operation — no git subprocess.
pub fn resolve_worktree_branch(worktree_cwd: &str) -> Option<String> {
    let dot_git_path = std::path::Path::new(worktree_cwd).join(".git");
    let dot_git_content = std::fs::read_to_string(&dot_git_path).ok()?;

    // .git file contains "gitdir: <path>" (absolute or relative)
    let gitdir = dot_git_content.strip_prefix("gitdir: ")?.trim();
    let gitdir_path = std::path::Path::new(gitdir);
    let gitdir_abs = if gitdir_path.is_absolute() {
        gitdir_path.to_path_buf()
    } else {
        std::path::Path::new(worktree_cwd).join(gitdir_path)
    };
    let head_path = gitdir_abs.join("HEAD");
    let head_content = std::fs::read_to_string(&head_path).ok()?;

    // HEAD contains "ref: refs/heads/<branch>" or a raw SHA (detached)
    let head_trimmed = head_content.trim();
    head_trimmed
        .strip_prefix("ref: refs/heads/")
        .map(|branch| branch.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // resolve_git_root Tests
    // ========================================================================

    #[tokio::test]
    async fn resolves_git_root_for_current_repo() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let root = resolve_git_root(manifest).await;
        assert!(root.is_some(), "expected a git root for {manifest}");
        let root = root.unwrap();
        assert!(
            manifest.starts_with(&root),
            "git root {root} should be ancestor of {manifest}"
        );
    }

    #[tokio::test]
    async fn returns_none_for_non_git_dir() {
        let root = resolve_git_root("/tmp").await;
        assert!(root.is_none());
    }

    // ========================================================================
    // infer_git_root_from_worktree_path Tests
    // ========================================================================

    #[test]
    fn test_infer_worktree_legacy_pattern() {
        assert_eq!(
            infer_git_root_from_worktree_path("/Users/u/dev/repo/.worktrees/feature-x"),
            Some("/Users/u/dev/repo".to_string())
        );
    }

    #[test]
    fn test_infer_worktree_claude_pattern() {
        assert_eq!(
            infer_git_root_from_worktree_path(
                "/Users/u/dev/@org/repo/.claude/worktrees/mobile-remote"
            ),
            Some("/Users/u/dev/@org/repo".to_string())
        );
    }

    #[test]
    fn test_infer_worktree_subdir() {
        assert_eq!(
            infer_git_root_from_worktree_path("/Users/u/dev/repo/.worktrees/feat/crates/server"),
            Some("/Users/u/dev/repo".to_string())
        );
    }

    #[test]
    fn test_infer_worktree_non_worktree_returns_none() {
        assert_eq!(infer_git_root_from_worktree_path("/Users/u/dev/repo"), None);
        assert_eq!(
            infer_git_root_from_worktree_path("/Users/u/dev/repo-cold-start"),
            None
        );
        assert_eq!(infer_git_root_from_worktree_path(""), None);
    }

    // ========================================================================
    // resolve_worktree_branch Tests
    // ========================================================================

    #[test]
    fn test_resolve_worktree_branch_from_dotgit_file() {
        let dir = tempfile::tempdir().unwrap();
        let wt_dir = dir.path().join(".worktrees").join("my-feature");
        std::fs::create_dir_all(&wt_dir).unwrap();

        let gitdir_path = dir.path().join(".git").join("worktrees").join("my-feature");
        std::fs::create_dir_all(&gitdir_path).unwrap();
        std::fs::write(
            wt_dir.join(".git"),
            format!("gitdir: {}", gitdir_path.display()),
        )
        .unwrap();

        std::fs::write(
            gitdir_path.join("HEAD"),
            "ref: refs/heads/feat/my-feature\n",
        )
        .unwrap();

        let result = resolve_worktree_branch(wt_dir.to_str().unwrap());
        assert_eq!(result, Some("feat/my-feature".to_string()));
    }

    #[test]
    fn test_resolve_worktree_branch_relative_gitdir() {
        let dir = tempfile::tempdir().unwrap();
        let wt_dir = dir.path().join(".worktrees").join("rel-feat");
        std::fs::create_dir_all(&wt_dir).unwrap();

        let gitdir_path = dir.path().join(".git").join("worktrees").join("rel-feat");
        std::fs::create_dir_all(&gitdir_path).unwrap();

        std::fs::write(wt_dir.join(".git"), "gitdir: ../../.git/worktrees/rel-feat").unwrap();

        std::fs::write(gitdir_path.join("HEAD"), "ref: refs/heads/feat/relative\n").unwrap();

        let result = resolve_worktree_branch(wt_dir.to_str().unwrap());
        assert_eq!(result, Some("feat/relative".to_string()));
    }

    #[test]
    fn test_resolve_worktree_branch_detached_head() {
        let dir = tempfile::tempdir().unwrap();
        let wt_dir = dir.path().join(".worktrees").join("detached");
        std::fs::create_dir_all(&wt_dir).unwrap();

        let gitdir_path = dir.path().join(".git").join("worktrees").join("detached");
        std::fs::create_dir_all(&gitdir_path).unwrap();
        std::fs::write(
            wt_dir.join(".git"),
            format!("gitdir: {}", gitdir_path.display()),
        )
        .unwrap();

        std::fs::write(gitdir_path.join("HEAD"), "abc123def456\n").unwrap();

        let result = resolve_worktree_branch(wt_dir.to_str().unwrap());
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_worktree_branch_no_dotgit_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_worktree_branch(dir.path().to_str().unwrap());
        assert_eq!(result, None);
    }

    // ========================================================================
    // resolve_git_branch Tests (unified: regular repo + worktree)
    // ========================================================================

    #[test]
    fn test_resolve_git_branch_regular_repo() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

        let result = resolve_git_branch(dir.path().to_str().unwrap());
        assert_eq!(result, Some("main".to_string()));
    }

    #[test]
    fn test_resolve_git_branch_regular_repo_feature_branch() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/feat/auth-flow\n").unwrap();

        let result = resolve_git_branch(dir.path().to_str().unwrap());
        assert_eq!(result, Some("feat/auth-flow".to_string()));
    }

    #[test]
    fn test_resolve_git_branch_detached_head() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "abc123def456789\n").unwrap();

        let result = resolve_git_branch(dir.path().to_str().unwrap());
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_git_branch_not_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_git_branch(dir.path().to_str().unwrap());
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_git_branch_worktree_delegates() {
        let dir = tempfile::tempdir().unwrap();
        let wt_dir = dir.path().join("worktree");
        std::fs::create_dir_all(&wt_dir).unwrap();

        let gitdir_path = dir.path().join(".git").join("worktrees").join("worktree");
        std::fs::create_dir_all(&gitdir_path).unwrap();
        std::fs::write(
            wt_dir.join(".git"),
            format!("gitdir: {}", gitdir_path.display()),
        )
        .unwrap();
        std::fs::write(gitdir_path.join("HEAD"), "ref: refs/heads/wt-branch\n").unwrap();

        let result = resolve_git_branch(wt_dir.to_str().unwrap());
        assert_eq!(result, Some("wt-branch".to_string()));
    }
}
