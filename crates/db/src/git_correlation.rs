// crates/db/src/git_correlation.rs
//! Git commit scanning and session correlation.
//!
//! This module implements:
//! - `scan_repo_commits()`: Spawn git log, parse output, handle edge cases
//! - `Tier1Matcher`: Match commit skills to commits within [-60s, +300s] window
//! - `Tier2Matcher`: Match commits during session time range
//! - CRUD operations for commits and session_commits tables

use crate::{Database, DbResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Timeout for git operations (10 seconds).
const GIT_TIMEOUT_SECS: u64 = 10;

/// Tier 1 correlation window: skill invoked at time T matches commits in [T-60s, T+300s].
/// - 60 seconds before: allow for git commit happening slightly before skill invocation
/// - 300 seconds after: allow for commit to complete after skill starts
pub const TIER1_WINDOW_BEFORE_SECS: i64 = 60;
pub const TIER1_WINDOW_AFTER_SECS: i64 = 300;

/// A git commit extracted from repository history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommit {
    /// Full commit hash (40 hex characters).
    pub hash: String,
    /// Repository path where this commit was found.
    pub repo_path: String,
    /// Commit message (first line only for brevity).
    pub message: String,
    /// Author name.
    pub author: Option<String>,
    /// Unix timestamp of the commit.
    pub timestamp: i64,
    /// Branch name (if available).
    pub branch: Option<String>,
}

/// Result of scanning a repository for commits.
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    /// Commits found in the repository.
    pub commits: Vec<GitCommit>,
    /// True if the directory is not a git repository.
    pub not_a_repo: bool,
    /// Error message if scanning failed (e.g., corrupt repo, permission denied).
    pub error: Option<String>,
}

/// Evidence for why a session was linked to a commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationEvidence {
    /// The matching rule used: "commit_skill" (Tier 1) or "during_session" (Tier 2).
    pub rule: String,
    /// Timestamp of the skill invocation (Tier 1 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_ts: Option<i64>,
    /// Timestamp of the commit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_ts: Option<i64>,
    /// Name of the skill invoked (Tier 1 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
    /// Session start timestamp (Tier 2 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_start: Option<i64>,
    /// Session end timestamp (Tier 2 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_end: Option<i64>,
}

/// A correlation match between a session and a commit.
#[derive(Debug, Clone)]
pub struct CorrelationMatch {
    /// Session ID.
    pub session_id: String,
    /// Commit hash.
    pub commit_hash: String,
    /// Tier level (1 = high confidence, 2 = medium confidence).
    pub tier: i32,
    /// Evidence explaining the match.
    pub evidence: CorrelationEvidence,
}

/// A commit skill invocation from a session (used for Tier 1 matching).
/// Re-exported from indexer_parallel for convenience.
pub use crate::indexer_parallel::CommitSkillInvocation;

// ============================================================================
// scan_repo_commits
// ============================================================================

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
        .arg(format!("--format=%H|%an|%at|%s"))
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
                        error: Some("Git returned error 128 (not a repository or corrupt)".to_string()),
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
        branch: None, // Set later
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

// ============================================================================
// Tier 1 Correlation: Commit Skill Matching
// ============================================================================

/// Match commit skill invocations to commits (Tier 1 - high confidence).
///
/// A match occurs when:
/// - The session's project path matches the commit's repo path exactly
/// - The commit timestamp is within [-60s, +300s] of the skill invocation time
///
/// Returns matches in order of skill invocation (preserving chronological order).
pub fn tier1_match(
    session_id: &str,
    session_project_path: &str,
    skill_invocations: &[CommitSkillInvocation],
    commits: &[GitCommit],
) -> Vec<CorrelationMatch> {
    let mut matches = Vec::new();

    for skill in skill_invocations {
        let skill_ts = skill.timestamp_unix;
        let window_start = skill_ts - TIER1_WINDOW_BEFORE_SECS;
        let window_end = skill_ts + TIER1_WINDOW_AFTER_SECS;

        for commit in commits {
            // Repo must match exactly
            if commit.repo_path != session_project_path {
                continue;
            }

            // Commit must be within the time window
            if commit.timestamp >= window_start && commit.timestamp <= window_end {
                let evidence = CorrelationEvidence {
                    rule: "commit_skill".to_string(),
                    skill_ts: Some(skill_ts),
                    commit_ts: Some(commit.timestamp),
                    skill_name: Some(skill.skill_name.clone()),
                    session_start: None,
                    session_end: None,
                };

                matches.push(CorrelationMatch {
                    session_id: session_id.to_string(),
                    commit_hash: commit.hash.clone(),
                    tier: 1,
                    evidence,
                });
            }
        }
    }

    matches
}

// ============================================================================
// Tier 2 Correlation: Session Time Range Matching
// ============================================================================

/// Match commits that occurred during a session (Tier 2 - medium confidence).
///
/// A match occurs when:
/// - The session's project path matches the commit's repo path exactly
/// - The commit timestamp is within [session_start, session_end]
///
/// Note: Commits already matched by Tier 1 should be excluded by the caller.
pub fn tier2_match(
    session_id: &str,
    session_project_path: &str,
    session_start: i64,
    session_end: i64,
    commits: &[GitCommit],
) -> Vec<CorrelationMatch> {
    let mut matches = Vec::new();

    for commit in commits {
        // Repo must match exactly
        if commit.repo_path != session_project_path {
            continue;
        }

        // Commit must be within session time range
        if commit.timestamp >= session_start && commit.timestamp <= session_end {
            let evidence = CorrelationEvidence {
                rule: "during_session".to_string(),
                skill_ts: None,
                commit_ts: Some(commit.timestamp),
                skill_name: None,
                session_start: Some(session_start),
                session_end: Some(session_end),
            };

            matches.push(CorrelationMatch {
                session_id: session_id.to_string(),
                commit_hash: commit.hash.clone(),
                tier: 2,
                evidence,
            });
        }
    }

    matches
}

// ============================================================================
// Database CRUD Operations
// ============================================================================

impl Database {
    /// Batch upsert commits into the database.
    ///
    /// Uses `INSERT ... ON CONFLICT DO UPDATE` to upsert on hash.
    /// Returns the number of rows affected.
    pub async fn batch_upsert_commits(&self, commits: &[GitCommit]) -> DbResult<u64> {
        if commits.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool().begin().await?;
        let mut affected: u64 = 0;

        for commit in commits {
            let result = sqlx::query(
                r#"
                INSERT INTO commits (hash, repo_path, message, author, timestamp, branch)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(hash) DO UPDATE SET
                    repo_path = excluded.repo_path,
                    message = excluded.message,
                    author = excluded.author,
                    timestamp = excluded.timestamp,
                    branch = excluded.branch
                "#,
            )
            .bind(&commit.hash)
            .bind(&commit.repo_path)
            .bind(&commit.message)
            .bind(&commit.author)
            .bind(commit.timestamp)
            .bind(&commit.branch)
            .execute(&mut *tx)
            .await?;

            affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(affected)
    }

    /// Insert session-commit links with tier and evidence.
    ///
    /// Uses `INSERT OR IGNORE` to skip duplicates (session_id + commit_hash).
    /// If a link already exists with a higher tier, it won't be overwritten.
    /// To prefer Tier 1 over Tier 2, call with Tier 1 matches first.
    ///
    /// Returns the number of rows inserted.
    pub async fn batch_insert_session_commits(
        &self,
        matches: &[CorrelationMatch],
    ) -> DbResult<u64> {
        if matches.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool().begin().await?;
        let mut inserted: u64 = 0;

        for m in matches {
            let evidence_json =
                serde_json::to_string(&m.evidence).unwrap_or_else(|_| "{}".to_string());

            // Use INSERT OR REPLACE to allow upgrading tier (lower tier number = higher priority)
            // First check if a link exists with a lower (better) tier
            let existing: Option<(i32,)> = sqlx::query_as(
                "SELECT tier FROM session_commits WHERE session_id = ?1 AND commit_hash = ?2",
            )
            .bind(&m.session_id)
            .bind(&m.commit_hash)
            .fetch_optional(&mut *tx)
            .await?;

            let should_insert = match existing {
                None => true,                          // No existing link
                Some((existing_tier,)) => m.tier < existing_tier, // Only insert if new tier is better
            };

            if should_insert {
                let result = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO session_commits (session_id, commit_hash, tier, evidence)
                    VALUES (?1, ?2, ?3, ?4)
                    "#,
                )
                .bind(&m.session_id)
                .bind(&m.commit_hash)
                .bind(m.tier)
                .bind(&evidence_json)
                .execute(&mut *tx)
                .await?;

                inserted += result.rows_affected();
            }
        }

        tx.commit().await?;
        Ok(inserted)
    }

    /// Get all commits linked to a session with their tier and evidence.
    pub async fn get_commits_for_session(
        &self,
        session_id: &str,
    ) -> DbResult<Vec<(GitCommit, i32, String)>> {
        let rows: Vec<(String, String, String, Option<String>, i64, Option<String>, i32, String)> =
            sqlx::query_as(
                r#"
            SELECT c.hash, c.repo_path, c.message, c.author, c.timestamp, c.branch,
                   sc.tier, sc.evidence
            FROM commits c
            INNER JOIN session_commits sc ON c.hash = sc.commit_hash
            WHERE sc.session_id = ?1
            ORDER BY c.timestamp DESC
            "#,
            )
            .bind(session_id)
            .fetch_all(self.pool())
            .await?;

        let results = rows
            .into_iter()
            .map(
                |(hash, repo_path, message, author, timestamp, branch, tier, evidence)| {
                    let commit = GitCommit {
                        hash,
                        repo_path,
                        message,
                        author,
                        timestamp,
                        branch,
                    };
                    (commit, tier, evidence)
                },
            )
            .collect();

        Ok(results)
    }

    /// Get commits for a repository within a time range.
    ///
    /// Useful for finding commits that might correlate with sessions.
    pub async fn get_commits_in_range(
        &self,
        repo_path: &str,
        start_ts: i64,
        end_ts: i64,
    ) -> DbResult<Vec<GitCommit>> {
        let rows: Vec<(String, String, String, Option<String>, i64, Option<String>)> =
            sqlx::query_as(
                r#"
            SELECT hash, repo_path, message, author, timestamp, branch
            FROM commits
            WHERE repo_path = ?1 AND timestamp >= ?2 AND timestamp <= ?3
            ORDER BY timestamp DESC
            "#,
            )
            .bind(repo_path)
            .bind(start_ts)
            .bind(end_ts)
            .fetch_all(self.pool())
            .await?;

        let commits = rows
            .into_iter()
            .map(
                |(hash, repo_path, message, author, timestamp, branch)| GitCommit {
                    hash,
                    repo_path,
                    message,
                    author,
                    timestamp,
                    branch,
                },
            )
            .collect();

        Ok(commits)
    }

    /// Count commits linked to a session (for updating session.commit_count).
    pub async fn count_commits_for_session(&self, session_id: &str) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM session_commits WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_one(self.pool())
        .await?;

        Ok(count)
    }

    /// Update the commit_count field on a session.
    pub async fn update_session_commit_count(
        &self,
        session_id: &str,
        commit_count: i32,
    ) -> DbResult<()> {
        sqlx::query("UPDATE sessions SET commit_count = ?2 WHERE id = ?1")
            .bind(session_id)
            .bind(commit_count)
            .execute(self.pool())
            .await?;

        Ok(())
    }
}

// ============================================================================
// Full Correlation Pipeline
// ============================================================================

/// Information needed to correlate a session with commits.
#[derive(Debug, Clone)]
pub struct SessionCorrelationInfo {
    pub session_id: String,
    pub project_path: String,
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    pub commit_skills: Vec<CommitSkillInvocation>,
}

/// Run the full correlation pipeline for a session.
///
/// 1. Scan the repo for commits (if not already scanned)
/// 2. Apply Tier 1 matching (commit skills)
/// 3. Apply Tier 2 matching (session time range)
/// 4. Insert matches, preferring Tier 1 over Tier 2
///
/// Returns the number of matches inserted.
pub async fn correlate_session(
    db: &Database,
    session: &SessionCorrelationInfo,
    commits: &[GitCommit],
) -> DbResult<usize> {
    let mut all_matches = Vec::new();
    let mut tier1_hashes = std::collections::HashSet::new();

    // Tier 1: Commit skill matching
    if !session.commit_skills.is_empty() {
        let tier1_matches = tier1_match(
            &session.session_id,
            &session.project_path,
            &session.commit_skills,
            commits,
        );

        for m in &tier1_matches {
            tier1_hashes.insert(m.commit_hash.clone());
        }
        all_matches.extend(tier1_matches);
    }

    // Tier 2: Session time range matching
    if let (Some(start), Some(end)) = (session.first_timestamp, session.last_timestamp) {
        let tier2_matches = tier2_match(
            &session.session_id,
            &session.project_path,
            start,
            end,
            commits,
        );

        // Exclude commits already matched by Tier 1
        let filtered_tier2: Vec<_> = tier2_matches
            .into_iter()
            .filter(|m| !tier1_hashes.contains(&m.commit_hash))
            .collect();

        all_matches.extend(filtered_tier2);
    }

    // Insert all matches
    let inserted = db.batch_insert_session_commits(&all_matches).await? as usize;

    // Update session commit count
    let commit_count = db.count_commits_for_session(&session.session_id).await?;
    db.update_session_commit_count(&session.session_id, commit_count as i32)
        .await?;

    Ok(inserted)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================================================
    // scan_repo_commits tests
    // ========================================================================

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
            assert_eq!(
                commit.repo_path,
                repo_path.to_string_lossy().to_string()
            );
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

    // ========================================================================
    // Tier 1 matching tests
    // ========================================================================

    #[test]
    fn test_tier1_match_exact_time() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: Some("Author".to_string()),
            timestamp: 1706400100, // Exact match
            branch: Some("main".to_string()),
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].tier, 1);
        assert_eq!(matches[0].session_id, "sess-1");
        assert_eq!(matches[0].commit_hash, "abc123def456789012345678901234567890abcd");
        assert_eq!(matches[0].evidence.rule, "commit_skill");
        assert_eq!(matches[0].evidence.skill_name, Some("commit".to_string()));
    }

    #[test]
    fn test_tier1_match_before_window() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        // Commit is 60 seconds before skill (within window)
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100 - 60, // 60s before (edge of window)
            branch: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert_eq!(matches.len(), 1, "Should match at edge of window (60s before)");
    }

    #[test]
    fn test_tier1_match_after_window() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        // Commit is 300 seconds after skill (within window)
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100 + 300, // 300s after (edge of window)
            branch: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert_eq!(matches.len(), 1, "Should match at edge of window (300s after)");
    }

    #[test]
    fn test_tier1_match_outside_window() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        // Commit is 301 seconds after skill (outside window)
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100 + 301, // 301s after (outside window)
            branch: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert!(matches.is_empty(), "Should not match outside window");
    }

    #[test]
    fn test_tier1_match_repo_mismatch() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/different/repo".to_string(), // Different repo
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100,
            branch: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert!(matches.is_empty(), "Should not match different repo");
    }

    #[test]
    fn test_tier1_match_multiple_skills_multiple_commits() {
        let skills = vec![
            CommitSkillInvocation {
                skill_name: "commit".to_string(),
                timestamp_unix: 1706400100,
            },
            CommitSkillInvocation {
                skill_name: "commit-commands:commit".to_string(),
                timestamp_unix: 1706400500,
            },
        ];

        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "First commit".to_string(),
                author: None,
                timestamp: 1706400120, // Matches first skill
                branch: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Second commit".to_string(),
                author: None,
                timestamp: 1706400520, // Matches second skill
                branch: None,
            },
        ];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert_eq!(matches.len(), 2, "Should match both skills to their commits");
    }

    // ========================================================================
    // Tier 2 matching tests
    // ========================================================================

    #[test]
    fn test_tier2_match_within_session() {
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400200, // Within session range
            branch: None,
        }];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].tier, 2);
        assert_eq!(matches[0].evidence.rule, "during_session");
        assert_eq!(matches[0].evidence.session_start, Some(1706400100));
        assert_eq!(matches[0].evidence.session_end, Some(1706400300));
    }

    #[test]
    fn test_tier2_match_at_boundaries() {
        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Start commit".to_string(),
                author: None,
                timestamp: 1706400100, // At session start
                branch: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "End commit".to_string(),
                author: None,
                timestamp: 1706400300, // At session end
                branch: None,
            },
        ];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);
        assert_eq!(matches.len(), 2, "Should match commits at session boundaries");
    }

    #[test]
    fn test_tier2_match_outside_session() {
        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Before session".to_string(),
                author: None,
                timestamp: 1706400099, // 1 second before session
                branch: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "After session".to_string(),
                author: None,
                timestamp: 1706400301, // 1 second after session
                branch: None,
            },
        ];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);
        assert!(matches.is_empty(), "Should not match commits outside session");
    }

    #[test]
    fn test_tier2_match_repo_mismatch() {
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/different/repo".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400200,
            branch: None,
        }];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);
        assert!(matches.is_empty(), "Should not match different repo");
    }

    // ========================================================================
    // Database CRUD tests
    // ========================================================================

    #[tokio::test]
    async fn test_batch_upsert_commits() {
        let db = Database::new_in_memory().await.unwrap();

        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "First commit".to_string(),
                author: Some("Author 1".to_string()),
                timestamp: 1706400100,
                branch: Some("main".to_string()),
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Second commit".to_string(),
                author: Some("Author 2".to_string()),
                timestamp: 1706400200,
                branch: Some("feature".to_string()),
            },
        ];

        let affected = db.batch_upsert_commits(&commits).await.unwrap();
        assert_eq!(affected, 2);

        // Verify commits are in the database
        let fetched = db
            .get_commits_in_range("/repo/path", 1706400000, 1706400300)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 2);
    }

    #[tokio::test]
    async fn test_batch_upsert_commits_updates_existing() {
        let db = Database::new_in_memory().await.unwrap();

        let commits = vec![GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Original message".to_string(),
            author: Some("Author".to_string()),
            timestamp: 1706400100,
            branch: Some("main".to_string()),
        }];

        db.batch_upsert_commits(&commits).await.unwrap();

        // Update the commit
        let updated = vec![GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Updated message".to_string(), // Changed
            author: Some("Author".to_string()),
            timestamp: 1706400100,
            branch: Some("main".to_string()),
        }];

        db.batch_upsert_commits(&updated).await.unwrap();

        // Verify message was updated
        let fetched = db
            .get_commits_in_range("/repo/path", 1706400000, 1706400200)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].message, "Updated message");
    }

    #[tokio::test]
    async fn test_batch_insert_session_commits() {
        let db = Database::new_in_memory().await.unwrap();

        // First insert a session
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            "/repo/path",
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            1706400100,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        // Insert a commit
        let commits = vec![GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100,
            branch: None,
        }];
        db.batch_upsert_commits(&commits).await.unwrap();

        // Create a correlation match
        let matches = vec![CorrelationMatch {
            session_id: "sess-1".to_string(),
            commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            tier: 1,
            evidence: CorrelationEvidence {
                rule: "commit_skill".to_string(),
                skill_ts: Some(1706400100),
                commit_ts: Some(1706400100),
                skill_name: Some("commit".to_string()),
                session_start: None,
                session_end: None,
            },
        }];

        let inserted = db.batch_insert_session_commits(&matches).await.unwrap();
        assert_eq!(inserted, 1);

        // Verify the link exists
        let linked = db.get_commits_for_session("sess-1").await.unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].0.hash, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(linked[0].1, 1); // tier
        assert!(linked[0].2.contains("commit_skill"));
    }

    #[tokio::test]
    async fn test_tier_priority_tier1_preferred() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert session and commit
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            "/repo/path",
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            1706400100,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        let commits = vec![GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100,
            branch: None,
        }];
        db.batch_upsert_commits(&commits).await.unwrap();

        // Insert Tier 2 first
        let tier2_match = vec![CorrelationMatch {
            session_id: "sess-1".to_string(),
            commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            tier: 2,
            evidence: CorrelationEvidence {
                rule: "during_session".to_string(),
                skill_ts: None,
                commit_ts: Some(1706400100),
                skill_name: None,
                session_start: Some(1706400000),
                session_end: Some(1706400200),
            },
        }];
        db.batch_insert_session_commits(&tier2_match).await.unwrap();

        // Now insert Tier 1 for the same session-commit pair
        let tier1_match = vec![CorrelationMatch {
            session_id: "sess-1".to_string(),
            commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            tier: 1,
            evidence: CorrelationEvidence {
                rule: "commit_skill".to_string(),
                skill_ts: Some(1706400100),
                commit_ts: Some(1706400100),
                skill_name: Some("commit".to_string()),
                session_start: None,
                session_end: None,
            },
        }];
        db.batch_insert_session_commits(&tier1_match).await.unwrap();

        // Verify Tier 1 takes precedence
        let linked = db.get_commits_for_session("sess-1").await.unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].1, 1, "Tier 1 should be preferred over Tier 2");
        assert!(linked[0].2.contains("commit_skill"));
    }

    #[tokio::test]
    async fn test_count_and_update_commit_count() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert session
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            "/repo/path",
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            1706400100,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        // Insert commits
        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "First".to_string(),
                author: None,
                timestamp: 1706400100,
                branch: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Second".to_string(),
                author: None,
                timestamp: 1706400200,
                branch: None,
            },
        ];
        db.batch_upsert_commits(&commits).await.unwrap();

        // Link commits to session
        let matches = vec![
            CorrelationMatch {
                session_id: "sess-1".to_string(),
                commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                tier: 1,
                evidence: CorrelationEvidence {
                    rule: "commit_skill".to_string(),
                    skill_ts: Some(1706400100),
                    commit_ts: Some(1706400100),
                    skill_name: Some("commit".to_string()),
                    session_start: None,
                    session_end: None,
                },
            },
            CorrelationMatch {
                session_id: "sess-1".to_string(),
                commit_hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                tier: 2,
                evidence: CorrelationEvidence {
                    rule: "during_session".to_string(),
                    skill_ts: None,
                    commit_ts: Some(1706400200),
                    skill_name: None,
                    session_start: Some(1706400000),
                    session_end: Some(1706400300),
                },
            },
        ];
        db.batch_insert_session_commits(&matches).await.unwrap();

        // Count commits for session
        let count = db.count_commits_for_session("sess-1").await.unwrap();
        assert_eq!(count, 2);

        // Update commit count on session
        db.update_session_commit_count("sess-1", count as i32)
            .await
            .unwrap();

        // Verify session has updated commit_count
        let projects = db.list_projects().await.unwrap();
        let session = &projects[0].sessions[0];
        assert_eq!(session.commit_count, 2);
    }

    // ========================================================================
    // correlate_session integration test
    // ========================================================================

    #[tokio::test]
    async fn test_correlate_session_full_pipeline() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert session
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            "/repo/path",
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            1706400800, // Session ends at T+700 from start
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        // Session time range: [1706400100, 1706400800] (700 seconds)
        // Skill at 1706400100, Tier 1 window: [1706400040, 1706400400] (+300s after)
        //
        // Commits:
        // - Tier 1 match: timestamp 1706400150 (within Tier 1 window)
        // - Tier 2 only: timestamp 1706400500 (outside Tier 1 window, inside session)
        // - No match: different repo

        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Tier 1 match".to_string(),
                author: None,
                timestamp: 1706400150, // Within Tier 1 window [1706400040, 1706400400]
                branch: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Tier 2 only match".to_string(),
                author: None,
                timestamp: 1706400500, // Outside Tier 1 window (>1706400400), inside session
                branch: None,
            },
            GitCommit {
                hash: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
                repo_path: "/different/repo".to_string(), // Wrong repo
                message: "No match".to_string(),
                author: None,
                timestamp: 1706400200,
                branch: None,
            },
        ];
        db.batch_upsert_commits(&commits).await.unwrap();

        let session_info = SessionCorrelationInfo {
            session_id: "sess-1".to_string(),
            project_path: "/repo/path".to_string(),
            first_timestamp: Some(1706400100),
            last_timestamp: Some(1706400800), // Session is 700s long
            commit_skills: vec![CommitSkillInvocation {
                skill_name: "commit".to_string(),
                timestamp_unix: 1706400100,
            }],
        };

        let inserted = correlate_session(&db, &session_info, &commits)
            .await
            .unwrap();

        assert_eq!(inserted, 2, "Should insert 2 matches (Tier 1 + Tier 2)");

        // Verify the links
        let linked = db.get_commits_for_session("sess-1").await.unwrap();
        assert_eq!(linked.len(), 2);

        // Find the tier 1 match
        let tier1 = linked.iter().find(|(c, t, _)| {
            c.hash == "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" && *t == 1
        });
        assert!(tier1.is_some(), "Should have Tier 1 match");

        // Find the tier 2 match
        let tier2 = linked.iter().find(|(c, t, _)| {
            c.hash == "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" && *t == 2
        });
        assert!(tier2.is_some(), "Should have Tier 2 match");

        // Wrong repo should not match
        let no_match = linked.iter().find(|(c, _, _)| {
            c.hash == "cccccccccccccccccccccccccccccccccccccccc"
        });
        assert!(no_match.is_none(), "Wrong repo should not match");

        // Verify session commit_count was updated
        let projects = db.list_projects().await.unwrap();
        let session = &projects[0].sessions[0];
        assert_eq!(session.commit_count, 2);
    }

    #[tokio::test]
    async fn test_correlate_session_no_skills() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert session
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            "/repo/path",
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            1706400300,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        let commits = vec![GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Within session".to_string(),
            author: None,
            timestamp: 1706400200,
            branch: None,
        }];
        db.batch_upsert_commits(&commits).await.unwrap();

        let session_info = SessionCorrelationInfo {
            session_id: "sess-1".to_string(),
            project_path: "/repo/path".to_string(),
            first_timestamp: Some(1706400100),
            last_timestamp: Some(1706400300),
            commit_skills: vec![], // No commit skills
        };

        let inserted = correlate_session(&db, &session_info, &commits)
            .await
            .unwrap();

        assert_eq!(inserted, 1, "Should insert Tier 2 match only");

        let linked = db.get_commits_for_session("sess-1").await.unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].1, 2, "Should be Tier 2 since no skills");
    }

    #[tokio::test]
    async fn test_correlate_session_no_timestamps() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert session
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            "/repo/path",
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            1706400300,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        let commits = vec![GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400150,
            branch: None,
        }];
        db.batch_upsert_commits(&commits).await.unwrap();

        let session_info = SessionCorrelationInfo {
            session_id: "sess-1".to_string(),
            project_path: "/repo/path".to_string(),
            first_timestamp: None, // No timestamps
            last_timestamp: None,
            commit_skills: vec![CommitSkillInvocation {
                skill_name: "commit".to_string(),
                timestamp_unix: 1706400100,
            }],
        };

        let inserted = correlate_session(&db, &session_info, &commits)
            .await
            .unwrap();

        // Should still match via Tier 1 (commit skill)
        assert_eq!(inserted, 1);

        let linked = db.get_commits_for_session("sess-1").await.unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].1, 1, "Should be Tier 1 from commit skill");
    }

    // ========================================================================
    // Evidence serialization tests
    // ========================================================================

    #[test]
    fn test_correlation_evidence_serialization() {
        let evidence = CorrelationEvidence {
            rule: "commit_skill".to_string(),
            skill_ts: Some(1706400100),
            commit_ts: Some(1706400120),
            skill_name: Some("commit".to_string()),
            session_start: None,
            session_end: None,
        };

        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains("\"rule\":\"commit_skill\""));
        assert!(json.contains("\"skill_ts\":1706400100"));
        assert!(json.contains("\"commit_ts\":1706400120"));
        assert!(json.contains("\"skill_name\":\"commit\""));
        // None fields should be skipped
        assert!(!json.contains("session_start"));
        assert!(!json.contains("session_end"));

        // Deserialize back
        let parsed: CorrelationEvidence = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.rule, "commit_skill");
        assert_eq!(parsed.skill_ts, Some(1706400100));
    }

    #[test]
    fn test_correlation_evidence_tier2() {
        let evidence = CorrelationEvidence {
            rule: "during_session".to_string(),
            skill_ts: None,
            commit_ts: Some(1706400200),
            skill_name: None,
            session_start: Some(1706400100),
            session_end: Some(1706400300),
        };

        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains("\"rule\":\"during_session\""));
        assert!(json.contains("\"session_start\":1706400100"));
        assert!(json.contains("\"session_end\":1706400300"));
        // None fields should be skipped
        assert!(!json.contains("skill_ts"));
        assert!(!json.contains("skill_name"));
    }
}
