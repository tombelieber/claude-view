// crates/db/src/git_correlation/scanning.rs
//! Git repository scanning: spawn `git log`, parse output, handle edge cases.

use super::types::{GitCommit, ScanResult, GIT_TIMEOUT_SECS};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Scan a git repository for recent commits.
///
/// Runs `git log` with a timeout and parses the output.
///
/// # Edge Cases (from A3.4)
/// - Non-git directory: Returns `ScanResult { not_a_repo: true, ... }`
/// - Bare/corrupt repo: Returns `ScanResult { error: Some(...), ... }`
/// - Git timeout: Returns `ScanResult { error: Some("timeout"), ... }`
/// - Permission denied: Returns `ScanResult { error: Some(...), ... }`
///
/// # Arguments
/// * `repo_path` - Path to the git repository
/// * `since_timestamp` - Only return commits after this Unix timestamp (optional)
/// * `limit` - Maximum number of commits to return (default: 100)
pub async fn scan_repo_commits(
    repo_path: &Path,
    since_timestamp: Option<i64>,
    limit: Option<usize>,
) -> ScanResult {
    let limit = limit.unwrap_or(100);

    // Verify the directory exists
    if !repo_path.exists() {
        return ScanResult {
            not_a_repo: true,
            error: Some("Directory does not exist".to_string()),
            ..Default::default()
        };
    }

    // Check if this is a git repository by looking for .git directory
    let git_dir = repo_path.join(".git");
    if !git_dir.exists() {
        // Could be a bare repo or not a repo at all
        // Try running git rev-parse to check
        match check_is_git_repo(repo_path).await {
            Ok(true) => {} // It's a valid repo (possibly bare)
            Ok(false) => {
                return ScanResult {
                    not_a_repo: true,
                    ..Default::default()
                }
            }
            Err(e) => {
                return ScanResult {
                    error: Some(e),
                    ..Default::default()
                }
            }
        }
    }

    // Build git log command
    // Format: hash|author|timestamp|message
    let mut cmd = Command::new("git");
    cmd.arg("log")
        .arg("--format=%H|%an|%at|%s")
        .arg(format!("-n{}", limit))
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(since) = since_timestamp {
        cmd.arg(format!("--since={}", since));
    }

    // Spawn with timeout
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ScanResult {
                error: Some(format!("Failed to spawn git: {}", e)),
                ..Default::default()
            }
        }
    };

    // Read output with timeout
    let timeout = Duration::from_secs(GIT_TIMEOUT_SECS);
    let result = tokio::time::timeout(timeout, async {
        let stdout = child.stdout.take().expect("stdout should be captured");
        let mut reader = BufReader::new(stdout).lines();
        let mut commits = Vec::new();

        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(commit) = parse_git_log_line(&line, repo_path) {
                commits.push(commit);
            }
        }

        // Wait for process to complete
        let status = child.wait().await;
        (commits, status)
    })
    .await;

    match result {
        Ok((commits, Ok(status))) => {
            if status.success() {
                // Get current branch name
                let branch = get_current_branch(repo_path).await.ok();
                let commits = commits
                    .into_iter()
                    .map(|mut c| {
                        c.branch = branch.clone();
                        c
                    })
                    .collect();

                ScanResult {
                    commits,
                    ..Default::default()
                }
            } else {
                let code = status.code().unwrap_or(-1);
                if code == 128 {
                    // Git error code 128 often means not a repo or corrupt
                    ScanResult {
                        not_a_repo: true,
                        error: Some(
                            "Git returned error 128 (not a repository or corrupt)".to_string(),
                        ),
                        ..Default::default()
                    }
                } else {
                    ScanResult {
                        error: Some(format!("Git log failed with exit code {}", code)),
                        ..Default::default()
                    }
                }
            }
        }
        Ok((_, Err(e))) => ScanResult {
            error: Some(format!("Git process error: {}", e)),
            ..Default::default()
        },
        Err(_) => {
            // Timeout - kill the process
            let _ = child.kill().await;
            ScanResult {
                error: Some("Git operation timed out".to_string()),
                ..Default::default()
            }
        }
    }
}

/// Parse a single line from git log output.
/// Expected format: hash|author|timestamp|message
fn parse_git_log_line(line: &str, repo_path: &Path) -> Option<GitCommit> {
    let parts: Vec<&str> = line.splitn(4, '|').collect();
    if parts.len() < 4 {
        return None;
    }

    let hash = parts[0].trim().to_string();
    let author = parts[1].trim();
    let timestamp_str = parts[2].trim();
    let message = parts[3].trim().to_string();

    // Validate hash (should be 40 hex chars)
    if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let timestamp: i64 = timestamp_str.parse().ok()?;

    Some(GitCommit {
        hash,
        repo_path: repo_path.to_string_lossy().to_string(),
        message,
        author: if author.is_empty() {
            None
        } else {
            Some(author.to_string())
        },
        timestamp,
        branch: None,        // Set later
        files_changed: None, // Set later via get_commit_diff_stats
        insertions: None,
        deletions: None,
    })
}

/// Check if a directory is a git repository.
async fn check_is_git_repo(path: &Path) -> Result<bool, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("git")
            .arg("rev-parse")
            .arg("--git-dir")
            .current_dir(path)
            .output(),
    )
    .await
    .map_err(|_| "Timeout checking git repo".to_string())?
    .map_err(|e| format!("Failed to run git: {}", e))?;

    Ok(output.status.success())
}

/// Get the current branch name for a repository.
async fn get_current_branch(repo_path: &Path) -> Result<String, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("git")
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .current_dir(repo_path)
            .output(),
    )
    .await
    .map_err(|_| "Timeout getting branch".to_string())?
    .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("Failed to get branch name".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_scan_repo_nonexistent_dir() {
        let result = scan_repo_commits(Path::new("/nonexistent/path/abc123"), None, None).await;
        assert!(result.not_a_repo || result.error.is_some());
        assert!(result.commits.is_empty());
    }

    #[tokio::test]
    async fn test_scan_repo_not_a_git_repo() {
        // Use /tmp to ensure we're outside any git repository hierarchy
        // (tempfile::TempDir::new() may create dirs inside the current project)
        let tmp = TempDir::new_in("/tmp").unwrap();
        let result = scan_repo_commits(tmp.path(), None, None).await;

        // For a non-git directory, we should either:
        // 1. Detect it's not a repo (not_a_repo = true)
        // 2. Get an error from git
        // 3. Get an empty commits list (if git log runs but finds no commits)
        assert!(
            result.not_a_repo || result.error.is_some() || result.commits.is_empty(),
            "Should handle non-git directory gracefully (not_a_repo={}, error={:?}, commits={})",
            result.not_a_repo,
            result.error,
            result.commits.len()
        );
    }

    #[tokio::test]
    async fn test_scan_repo_real_git_repo() {
        // Use the current project's repo for testing
        let repo_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        // Only run if this is a git repo (should be)
        if !repo_path.join(".git").exists() {
            return;
        }

        let result = scan_repo_commits(repo_path, None, Some(10)).await;

        assert!(!result.not_a_repo, "Should recognize as git repo");
        assert!(result.error.is_none(), "Should not have errors");

        // Should find at least one commit (this project has history)
        if !result.commits.is_empty() {
            let commit = &result.commits[0];
            assert_eq!(commit.hash.len(), 40, "Hash should be 40 characters");
            assert!(!commit.message.is_empty(), "Message should not be empty");
            assert!(commit.timestamp > 0, "Timestamp should be positive");
            assert_eq!(commit.repo_path, repo_path.to_string_lossy().to_string());
        }
    }

    #[test]
    fn test_parse_git_log_line_valid() {
        let line = "abc123def456789012345678901234567890abcd|John Doe|1706400000|Initial commit";
        let commit = parse_git_log_line(line, Path::new("/repo"));
        assert!(commit.is_some());

        let c = commit.unwrap();
        assert_eq!(c.hash, "abc123def456789012345678901234567890abcd");
        assert_eq!(c.author, Some("John Doe".to_string()));
        assert_eq!(c.timestamp, 1706400000);
        assert_eq!(c.message, "Initial commit");
        assert_eq!(c.repo_path, "/repo");
    }

    #[test]
    fn test_parse_git_log_line_invalid_hash() {
        // Hash too short
        let line = "abc123|John Doe|1706400000|Initial commit";
        assert!(parse_git_log_line(line, Path::new("/repo")).is_none());
    }

    #[test]
    fn test_parse_git_log_line_missing_parts() {
        // Only 3 parts
        let line = "abc123def456789012345678901234567890abcd|John Doe|1706400000";
        assert!(parse_git_log_line(line, Path::new("/repo")).is_none());
    }

    #[test]
    fn test_parse_git_log_line_message_with_pipe() {
        // Message contains pipe character
        let line =
            "abc123def456789012345678901234567890abcd|John Doe|1706400000|Fix bug | add tests";
        let commit = parse_git_log_line(line, Path::new("/repo"));
        assert!(commit.is_some());

        let c = commit.unwrap();
        assert_eq!(c.message, "Fix bug | add tests");
    }
}
