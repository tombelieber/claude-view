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
    /// Number of files changed in this commit.
    pub files_changed: Option<u32>,
    /// Number of lines inserted in this commit.
    pub insertions: Option<u32>,
    /// Number of lines deleted in this commit.
    pub deletions: Option<u32>,
}

/// Diff stats for a commit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffStats {
    /// Number of files changed.
    pub files_changed: u32,
    /// Number of lines inserted.
    pub insertions: u32,
    /// Number of lines deleted.
    pub deletions: u32,
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
        files_changed: None, // Set later via get_commit_diff_stats
        insertions: None,
        deletions: None,
    })
}

/// Get diff stats for a single commit using `git show --stat`.
///
/// # Arguments
/// * `repo_path` - Path to the git repository
/// * `commit_hash` - The commit hash to get stats for
///
/// # Returns
/// `DiffStats` with files_changed, insertions, and deletions, or default if unavailable.
pub async fn get_commit_diff_stats(repo_path: &Path, commit_hash: &str) -> DiffStats {
    // Use git show --stat --format="" to get only the stat line
    // Output format: "N files changed, N insertions(+), N deletions(-)"
    let output = tokio::time::timeout(
        Duration::from_secs(GIT_TIMEOUT_SECS),
        Command::new("git")
            .args(["show", "--stat", "--format=", commit_hash])
            .current_dir(repo_path)
            .output(),
    )
    .await;

    let output = match output {
        Ok(Ok(o)) if o.status.success() => o,
        Ok(Ok(o)) => {
            tracing::debug!("git show returned non-zero for {commit_hash}: {:?}", o.status);
            return DiffStats::default();
        }
        Ok(Err(e)) => {
            tracing::warn!("git show spawn failed for {commit_hash}: {e}");
            return DiffStats::default();
        }
        Err(_) => {
            tracing::warn!("git show timed out for commit {commit_hash}");
            return DiffStats::default();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_diff_stats_from_output(&stdout)
}

/// Get diff stats for multiple commits in a batch.
///
/// Spawns one `git show --stat` per commit, parallelized within batches
/// using [`tokio::task::JoinSet`] to limit concurrent subprocesses.
///
/// # Arguments
/// * `repo_path` - Path to the git repository
/// * `commit_hashes` - Slice of commit hashes to get stats for
///
/// # Returns
/// Vec of (hash, DiffStats) tuples. Commits that fail to parse will have default stats.
pub async fn get_batch_diff_stats(
    repo_path: &Path,
    commit_hashes: &[String],
) -> Vec<(String, DiffStats)> {
    let mut results = Vec::with_capacity(commit_hashes.len());

    // Process commits in batches to bound concurrent subprocess count
    const BATCH_SIZE: usize = 50;

    for batch in commit_hashes.chunks(BATCH_SIZE) {
        let mut set = tokio::task::JoinSet::new();

        for (idx, hash) in batch.iter().enumerate() {
            let repo = repo_path.to_path_buf();
            let h = hash.clone();
            set.spawn(async move {
                let stats = get_commit_diff_stats(&repo, &h).await;
                (idx, h, stats)
            });
        }

        // Collect results preserving original order
        let mut batch_results: Vec<(usize, String, DiffStats)> =
            Vec::with_capacity(batch.len());
        while let Some(res) = set.join_next().await {
            match res {
                Ok(tuple) => batch_results.push(tuple),
                Err(e) => {
                    tracing::warn!("JoinSet task panicked in get_batch_diff_stats: {e}");
                }
            }
        }
        batch_results.sort_by_key(|(idx, _, _)| *idx);

        for (_, hash, stats) in batch_results {
            results.push((hash, stats));
        }
    }

    results
}

/// Parse diff stats from git show --stat output.
///
/// Expected format in the last line:
/// " N files changed, N insertions(+), N deletions(-)"
/// or partial variants like:
/// " N files changed, N insertions(+)"
/// " N files changed, N deletions(-)"
/// " N file changed"
fn parse_diff_stats_from_output(output: &str) -> DiffStats {
    // Find the summary line (last non-empty line containing "changed")
    let summary_line = output
        .lines()
        .rev()
        .find(|line| line.contains("changed"));

    let line = match summary_line {
        Some(l) => l.trim(),
        None => return DiffStats::default(),
    };

    let mut stats = DiffStats::default();

    // Parse "N file(s) changed"
    if let Some(pos) = line.find("file") {
        let prefix = &line[..pos].trim();
        if let Some(num_str) = prefix.split_whitespace().last() {
            if let Ok(n) = num_str.parse::<u32>() {
                stats.files_changed = n;
            }
        }
    }

    // Parse "N insertion(s)(+)"
    if let Some(pos) = line.find("insertion") {
        // Find the number before "insertion"
        let prefix = &line[..pos];
        if let Some(num_str) = prefix.split(',').next_back().and_then(|s| s.split_whitespace().next()) {
            if let Ok(n) = num_str.parse::<u32>() {
                stats.insertions = n;
            }
        }
    }

    // Parse "N deletion(s)(-)"
    if let Some(pos) = line.find("deletion") {
        // Find the number before "deletion"
        let prefix = &line[..pos];
        if let Some(num_str) = prefix.split(',').next_back().and_then(|s| s.split_whitespace().next()) {
            if let Ok(n) = num_str.parse::<u32>() {
                stats.deletions = n;
            }
        }
    }

    stats
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

impl DiffStats {
    /// Parse numstat output and aggregate stats.
    ///
    /// Expected format (one line per file):
    /// ```text
    /// 10\t5\tfile.rs
    /// 3\t0\tREADME.md
    /// ```
    ///
    /// Binary files show as `-\t-\tfile.bin` and are ignored.
    pub fn from_numstat(output: &str) -> Self {
        let mut stats = Self::default();

        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }

            // Parse additions and deletions
            let added = parts[0].parse::<u32>().ok();
            let removed = parts[1].parse::<u32>().ok();

            if let (Some(a), Some(r)) = (added, removed) {
                stats.files_changed += 1;
                stats.insertions += a;
                stats.deletions += r;
            }
            // Binary files (- - filename) are skipped
        }

        stats
    }

    /// Aggregate multiple DiffStats together.
    pub fn aggregate(stats: &[DiffStats]) -> Self {
        stats.iter().fold(Self::default(), |mut acc, s| {
            acc.files_changed += s.files_changed;
            acc.insertions += s.insertions;
            acc.deletions += s.deletions;
            acc
        })
    }
}

/// Extract diff stats for a single commit using `git show --numstat`.
///
/// Uses `git show` instead of `git diff` to handle initial commits
/// (which have no parent).
///
/// Returns None if:
/// - Git command fails
/// - Repo path doesn't exist
/// - Commit hash is invalid
pub async fn extract_commit_diff_stats(
    repo_path: &Path,
    commit_hash: &str,
) -> Option<DiffStats> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("git")
            .args([
                "show",
                "--numstat",
                "--format=", // Don't show commit message
                commit_hash,
            ])
            .current_dir(repo_path)
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(DiffStats::from_numstat(&stdout))
}

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
                INSERT INTO commits (hash, repo_path, message, author, timestamp, branch, files_changed, insertions, deletions)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(hash) DO UPDATE SET
                    repo_path = excluded.repo_path,
                    message = excluded.message,
                    author = excluded.author,
                    timestamp = excluded.timestamp,
                    branch = excluded.branch,
                    files_changed = COALESCE(excluded.files_changed, commits.files_changed),
                    insertions = COALESCE(excluded.insertions, commits.insertions),
                    deletions = COALESCE(excluded.deletions, commits.deletions)
                "#,
            )
            .bind(&commit.hash)
            .bind(&commit.repo_path)
            .bind(&commit.message)
            .bind(&commit.author)
            .bind(commit.timestamp)
            .bind(&commit.branch)
            .bind(commit.files_changed.map(|v| v as i64))
            .bind(commit.insertions.map(|v| v as i64))
            .bind(commit.deletions.map(|v| v as i64))
            .execute(&mut *tx)
            .await?;

            affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(affected)
    }

    /// Update diff stats for a commit.
    ///
    /// Used to populate diff stats for commits that were initially created without them.
    pub async fn update_commit_diff_stats(
        &self,
        commit_hash: &str,
        stats: DiffStats,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE commits SET
                files_changed = ?2,
                insertions = ?3,
                deletions = ?4
            WHERE hash = ?1
            "#,
        )
        .bind(commit_hash)
        .bind(stats.files_changed as i64)
        .bind(stats.insertions as i64)
        .bind(stats.deletions as i64)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Get commits missing diff stats (for backfill).
    pub async fn get_commits_without_diff_stats(&self, limit: usize) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT hash FROM commits
            WHERE files_changed IS NULL OR insertions IS NULL OR deletions IS NULL
            LIMIT ?1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(|(h,)| h).collect())
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
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, Option<String>, i64, Option<String>, Option<i64>, Option<i64>, Option<i64>, i32, String)> =
            sqlx::query_as(
                r#"
            SELECT c.hash, c.repo_path, c.message, c.author, c.timestamp, c.branch,
                   c.files_changed, c.insertions, c.deletions,
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
                |(hash, repo_path, message, author, timestamp, branch, files_changed, insertions, deletions, tier, evidence)| {
                    let commit = GitCommit {
                        hash,
                        repo_path,
                        message,
                        author,
                        timestamp,
                        branch,
                        files_changed: files_changed.map(|v| v as u32),
                        insertions: insertions.map(|v| v as u32),
                        deletions: deletions.map(|v| v as u32),
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
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, Option<String>, i64, Option<String>, Option<i64>, Option<i64>, Option<i64>)> =
            sqlx::query_as(
                r#"
            SELECT hash, repo_path, message, author, timestamp, branch, files_changed, insertions, deletions
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
                |(hash, repo_path, message, author, timestamp, branch, files_changed, insertions, deletions)| GitCommit {
                    hash,
                    repo_path,
                    message,
                    author,
                    timestamp,
                    branch,
                    files_changed: files_changed.map(|v| v as u32),
                    insertions: insertions.map(|v| v as u32),
                    deletions: deletions.map(|v| v as u32),
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

    /// Update session LOC stats from git diff (Phase F: Git Diff Stats Overlay).
    ///
    /// Sets lines_added, lines_removed, and loc_source = 2 (git verified).
    /// Only updates if new stats are provided (not 0+0).
    pub async fn update_session_loc_from_git(
        &self,
        session_id: &str,
        stats: &DiffStats,
    ) -> DbResult<()> {
        // Only update if we have actual stats
        if stats.insertions == 0 && stats.deletions == 0 {
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE sessions
            SET lines_added = ?2, lines_removed = ?3, loc_source = 2
            WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .bind(stats.insertions as i64)
        .bind(stats.deletions as i64)
        .execute(self.pool())
        .await?;

        Ok(())
    }
}

// ============================================================================
// Git Sync Session Query
// ============================================================================

/// Lightweight session data for git correlation.
/// Contains only the 4 fields needed — no JOINs, no JSON arrays, no token sums.
#[derive(Debug, Clone)]
pub struct SessionSyncInfo {
    pub session_id: String,
    pub project_path: String,
    pub first_message_at: Option<i64>,
    pub last_message_at: Option<i64>,
}

impl Database {
    /// Fetch all sessions eligible for git correlation.
    ///
    /// Filters:
    /// - `project_path` must be non-empty (sessions without a project can't have a repo)
    /// - `last_message_at` must be non-NULL (need at least one timestamp for time window)
    ///
    /// This is deliberately lightweight: a single-table SELECT with no JOINs.
    pub async fn get_sessions_for_git_sync(&self) -> DbResult<Vec<SessionSyncInfo>> {
        let rows: Vec<(String, String, Option<i64>, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT id, project_path, first_message_at, last_message_at
            FROM sessions
            WHERE project_path != '' AND last_message_at IS NOT NULL
            ORDER BY last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(session_id, project_path, first_message_at, last_message_at)| {
                SessionSyncInfo {
                    session_id,
                    project_path,
                    first_message_at,
                    last_message_at,
                }
            })
            .collect())
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
/// 5. Extract git diff stats for newly linked commits (Phase F)
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

    // Phase F: Extract git diff stats for newly linked commits
    if inserted > 0 {
        let repo_path = std::path::Path::new(&session.project_path);
        let mut all_stats = Vec::new();

        for commit_match in &all_matches {
            if let Some(stats) =
                extract_commit_diff_stats(repo_path, &commit_match.commit_hash).await
            {
                all_stats.push(stats);
            }
        }

        if !all_stats.is_empty() {
            let aggregated = DiffStats::aggregate(&all_stats);
            db.update_session_loc_from_git(&session.session_id, &aggregated)
                .await?;
        }
    }

    // Update session commit count
    let commit_count = db.count_commits_for_session(&session.session_id).await?;
    db.update_session_commit_count(&session.session_id, commit_count as i32)
        .await?;

    Ok(inserted)
}

// ============================================================================
// Git Sync Orchestrator
// ============================================================================

/// Result of a full git sync run.
#[derive(Debug, Clone, Default)]
pub struct GitSyncResult {
    /// Number of unique repositories scanned.
    pub repos_scanned: u32,
    /// Total commits found across all repos.
    pub commits_found: u32,
    /// Total session-commit links created or updated.
    pub links_created: u32,
    /// Non-fatal errors encountered (one per failed repo).
    pub errors: Vec<String>,
}

/// Progress updates emitted by `run_git_sync` via callback.
///
/// Used by the server crate to feed SSE progress events to the frontend.
#[derive(Debug, Clone)]
pub enum GitSyncProgress {
    /// Emitted after grouping sessions by repo, before scanning starts.
    ScanningStarted { total_repos: usize },
    /// Emitted after each repo is scanned and commits are found.
    RepoScanned {
        repos_done: usize,
        total_repos: usize,
        commits_in_repo: u32,
    },
    /// Emitted before the session correlation loop begins.
    CorrelatingStarted {
        total_correlatable_sessions: usize,
    },
    /// Emitted after each session is correlated (success or failure).
    SessionCorrelated {
        sessions_done: usize,
        total_correlatable_sessions: usize,
        links_in_session: u32,
    },
}

/// Run the full git sync pipeline: scan repos, correlate sessions, update metadata.
///
/// Auto-sync produces Tier 2 only (no skill data available at this stage).
/// Tier 1 links from pass_2 deep indexing are never overwritten.
/// Per-repo error isolation ensures one bad repo doesn't abort the sync.
/// Idempotent: safe to run multiple times.
pub async fn run_git_sync<F>(db: &Database, on_progress: F) -> DbResult<GitSyncResult>
where
    F: Fn(GitSyncProgress) + Send + 'static,
{
    let mut result = GitSyncResult::default();

    // Step 1: Fetch all eligible sessions
    let sessions = db.get_sessions_for_git_sync().await?;
    if sessions.is_empty() {
        tracing::debug!("Git sync: no eligible sessions found");
        db.update_git_sync_metadata_on_success(0, 0).await?;
        return Ok(result);
    }

    tracing::info!("Git sync: {} eligible sessions", sessions.len());

    // Step 2: Group sessions by project_path to deduplicate repo scans
    let mut sessions_by_repo: std::collections::HashMap<String, Vec<&SessionSyncInfo>> =
        std::collections::HashMap::new();
    for session in &sessions {
        sessions_by_repo
            .entry(session.project_path.clone())
            .or_default()
            .push(session);
    }

    let total_repos = sessions_by_repo.len();
    tracing::info!(
        "Git sync: {} unique project paths to scan",
        total_repos
    );

    on_progress(GitSyncProgress::ScanningStarted { total_repos });

    // Step 3: Scan each unique repo and upsert commits
    let mut commits_by_repo: std::collections::HashMap<String, Vec<GitCommit>> =
        std::collections::HashMap::new();

    for project_path in sessions_by_repo.keys() {
        let path = std::path::Path::new(project_path.as_str());
        let scan = scan_repo_commits(path, None, None).await;

        if scan.not_a_repo {
            continue;
        }

        if let Some(err) = &scan.error {
            tracing::warn!("Git sync: error scanning {}: {}", project_path, err);
            result.errors.push(format!("{}: {}", project_path, err));
            continue;
        }

        if scan.commits.is_empty() {
            continue;
        }

        let commits_in_repo = scan.commits.len() as u32;
        result.repos_scanned += 1;
        result.commits_found += commits_in_repo;

        on_progress(GitSyncProgress::RepoScanned {
            repos_done: result.repos_scanned as usize,
            total_repos,
            commits_in_repo,
        });

        db.batch_upsert_commits(&scan.commits).await?;

        commits_by_repo.insert(project_path.clone(), scan.commits);
    }

    tracing::info!(
        "Git sync: scanned {} repos, found {} commits",
        result.repos_scanned,
        result.commits_found
    );

    // Step 4: Correlate each session with its repo's commits
    // Count sessions that actually have commits to correlate against
    let correlatable_count = sessions
        .iter()
        .filter(|s| commits_by_repo.contains_key(&s.project_path))
        .count();
    on_progress(GitSyncProgress::CorrelatingStarted {
        total_correlatable_sessions: correlatable_count,
    });

    let mut sessions_done: usize = 0;

    for session in &sessions {
        let commits = match commits_by_repo.get(&session.project_path) {
            Some(c) => c,
            None => continue,
        };

        let info = SessionCorrelationInfo {
            session_id: session.session_id.clone(),
            project_path: session.project_path.clone(),
            first_timestamp: session.first_message_at,
            last_timestamp: session.last_message_at,
            commit_skills: Vec::new(), // No skill data in auto-sync -> Tier 2 only
        };

        match correlate_session(db, &info, commits).await {
            Ok(links) => {
                let links_in_session = links as u32;
                result.links_created += links_in_session;
                sessions_done += 1;
                on_progress(GitSyncProgress::SessionCorrelated {
                    sessions_done,
                    total_correlatable_sessions: correlatable_count,
                    links_in_session,
                });
            }
            Err(e) => {
                tracing::warn!(
                    "Git sync: correlation failed for session {}: {}",
                    session.session_id,
                    e
                );
                result.errors.push(format!("session {}: {}", session.session_id, e));
                sessions_done += 1;
                on_progress(GitSyncProgress::SessionCorrelated {
                    sessions_done,
                    total_correlatable_sessions: correlatable_count,
                    links_in_session: 0,
                });
            }
        }
    }

    // Step 5: Update metadata to record successful sync
    db.update_git_sync_metadata_on_success(
        result.commits_found as i64,
        result.links_created as i64,
    )
    .await?;

    tracing::info!(
        "Git sync complete: {} repos, {} commits, {} links, {} errors",
        result.repos_scanned,
        result.commits_found,
        result.links_created,
        result.errors.len()
    );

    Ok(result)
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Second commit".to_string(),
                author: None,
                timestamp: 1706400520, // Matches second skill
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "End commit".to_string(),
                author: None,
                timestamp: 1706400300, // At session end
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
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
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "After session".to_string(),
                author: None,
                timestamp: 1706400301, // 1 second after session
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Second commit".to_string(),
                author: Some("Author 2".to_string()),
                timestamp: 1706400200,
                branch: Some("feature".to_string()),
                files_changed: None,
                insertions: None,
                deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Second".to_string(),
                author: None,
                timestamp: 1706400200,
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
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
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Tier 2 only match".to_string(),
                author: None,
                timestamp: 1706400500, // Outside Tier 1 window (>1706400400), inside session
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
                repo_path: "/different/repo".to_string(), // Wrong repo
                message: "No match".to_string(),
                author: None,
                timestamp: 1706400200,
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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
            files_changed: None,
            insertions: None,
            deletions: None,
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

    // ========================================================================
    // get_sessions_for_git_sync tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_sessions_for_git_sync_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let sessions = db.get_sessions_for_git_sync().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_get_sessions_for_git_sync_filters_correctly() {
        let db = Database::new_in_memory().await.unwrap();

        // Session 1: eligible (has project_path and last_message_at)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', '/home/user/project-a', 1000, 2000, '/tmp/s1.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        // Session 2: ineligible (empty project_path)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, last_message_at, file_path)
             VALUES ('s2', 'p2', '', 3000, '/tmp/s2.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        // Session 3: ineligible (NULL last_message_at)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, file_path)
             VALUES ('s3', 'p3', '/home/user/project-b', '/tmp/s3.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        // Session 4: eligible (has project_path and last_message_at, no first_message_at)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, last_message_at, file_path)
             VALUES ('s4', 'p4', '/home/user/project-a', 4000, '/tmp/s4.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        let sessions = db.get_sessions_for_git_sync().await.unwrap();
        assert_eq!(sessions.len(), 2);

        // Ordered by last_message_at DESC
        assert_eq!(sessions[0].session_id, "s4");
        assert_eq!(sessions[0].project_path, "/home/user/project-a");
        assert_eq!(sessions[0].first_message_at, None);
        assert_eq!(sessions[0].last_message_at, Some(4000));

        assert_eq!(sessions[1].session_id, "s1");
        assert_eq!(sessions[1].project_path, "/home/user/project-a");
        assert_eq!(sessions[1].first_message_at, Some(1000));
        assert_eq!(sessions[1].last_message_at, Some(2000));
    }

    // ========================================================================
    // run_git_sync tests
    // ========================================================================

    #[tokio::test]
    async fn test_run_git_sync_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let result = run_git_sync(&db, |_| {}).await.unwrap();

        assert_eq!(result.repos_scanned, 0);
        assert_eq!(result.commits_found, 0);
        assert_eq!(result.links_created, 0);
        assert!(result.errors.is_empty());

        // Metadata should still be updated (records that sync ran)
        let meta = db.get_index_metadata().await.unwrap();
        assert!(meta.last_git_sync_at.is_some());
    }

    #[tokio::test]
    async fn test_run_git_sync_non_git_dirs() {
        let db = Database::new_in_memory().await.unwrap();

        // Use /tmp to ensure we're outside any git repository hierarchy
        let tmp = TempDir::new_in("/tmp").unwrap();
        let dir = tmp.path().to_str().unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, 1000, 2000, '/tmp/s1.jsonl')"
        )
        .bind(dir)
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db, |_| {}).await.unwrap();

        assert_eq!(result.repos_scanned, 0);
        assert_eq!(result.commits_found, 0);
        assert_eq!(result.links_created, 0);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_run_git_sync_with_real_repo() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config name");

        std::fs::write(repo_path.join("file.txt"), "hello").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        // Get the commit timestamp
        let output = std::process::Command::new("git")
            .args(["log", "-1", "--format=%at"])
            .current_dir(repo_path)
            .output()
            .expect("git log timestamp");
        let commit_ts: i64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap();

        let dir_str = repo_path.to_str().unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')"
        )
        .bind(dir_str)
        .bind(commit_ts - 600)
        .bind(commit_ts + 600)
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db, |_| {}).await.unwrap();

        assert_eq!(result.repos_scanned, 1);
        assert_eq!(result.commits_found, 1);
        assert_eq!(result.links_created, 1);
        assert!(result.errors.is_empty());

        // Verify the link was created in the DB
        let commits = db.get_commits_for_session("s1").await.unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].1, 2); // Tier 2

        // Verify session commit_count was updated
        let count = db.count_commits_for_session("s1").await.unwrap();
        assert_eq!(count, 1);

        // Verify metadata was updated
        let meta = db.get_index_metadata().await.unwrap();
        assert!(meta.last_git_sync_at.is_some());
        assert_eq!(meta.commits_found, 1);
        assert_eq!(meta.links_created, 1);
    }

    #[tokio::test]
    async fn test_run_git_sync_deduplicates_repos() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config name");

        std::fs::write(repo_path.join("file.txt"), "hello").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let dir_str = repo_path.to_str().unwrap();
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')"
        )
        .bind(dir_str)
        .bind(now - 7200)
        .bind(now + 7200)
        .execute(db.pool())
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s2', 'p1', ?1, ?2, ?3, '/tmp/s2.jsonl')"
        )
        .bind(dir_str)
        .bind(now - 3600)
        .bind(now + 3600)
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db, |_| {}).await.unwrap();

        // Only 1 repo scanned despite 2 sessions
        assert_eq!(result.repos_scanned, 1);
        // Both sessions should get linked
        assert_eq!(result.links_created, 2);
    }

    #[tokio::test]
    async fn test_run_git_sync_nonexistent_dir() {
        let db = Database::new_in_memory().await.unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', '/nonexistent/path/abc123', 1000, 2000, '/tmp/s1.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db, |_| {}).await.unwrap();

        assert_eq!(result.repos_scanned, 0);
        assert_eq!(result.links_created, 0);
        assert!(result.errors.is_empty()); // not_a_repo is silent skip
    }

    // ========================================================================
    // Phase F: Git Diff Stats tests
    // ========================================================================

    #[test]
    fn test_diff_stats_from_numstat_empty() {
        let stats = DiffStats::from_numstat("");
        assert_eq!(stats.insertions, 0);
        assert_eq!(stats.deletions, 0);
    }

    #[test]
    fn test_diff_stats_from_numstat_single_file() {
        let output = "10\t5\tfile.rs";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 10);
        assert_eq!(stats.deletions, 5);
    }

    #[test]
    fn test_diff_stats_from_numstat_multiple_files() {
        let output = "10\t5\tfile.rs\n3\t0\tREADME.md\n0\t7\ttest.rs";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 13); // 10 + 3 + 0
        assert_eq!(stats.deletions, 12); // 5 + 0 + 7
    }

    #[test]
    fn test_diff_stats_from_numstat_binary_files() {
        // Binary files show as "- - filename"
        let output = "10\t5\tfile.rs\n-\t-\timage.png\n3\t0\tREADME.md";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 13); // Binary file ignored
        assert_eq!(stats.deletions, 5);
    }

    #[test]
    fn test_diff_stats_from_numstat_malformed_lines() {
        let output = "10\t5\tfile.rs\nmalformed line\n3\t0\tREADME.md";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 13); // Malformed line ignored
        assert_eq!(stats.deletions, 5);
    }

    #[test]
    fn test_diff_stats_aggregate() {
        let stats1 = DiffStats {
            files_changed: 1,
            insertions: 10,
            deletions: 5,
        };
        let stats2 = DiffStats {
            files_changed: 1,
            insertions: 3,
            deletions: 7,
        };
        let stats3 = DiffStats {
            files_changed: 1,
            insertions: 0,
            deletions: 2,
        };

        let aggregated = DiffStats::aggregate(&[stats1, stats2, stats3]);
        assert_eq!(aggregated.insertions, 13);
        assert_eq!(aggregated.deletions, 14);
    }

    #[test]
    fn test_diff_stats_aggregate_empty() {
        let aggregated = DiffStats::aggregate(&[]);
        assert_eq!(aggregated.insertions, 0);
        assert_eq!(aggregated.deletions, 0);
    }

    #[tokio::test]
    async fn test_extract_commit_diff_stats_invalid_repo() {
        let result = extract_commit_diff_stats(
            Path::new("/nonexistent/path"),
            "abc123def456789012345678901234567890abcd",
        )
        .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_extract_commit_diff_stats_real_commit() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        // Create a file with 10 lines
        std::fs::write(repo_path.join("file.txt"), "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "initial commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        // Get the commit hash
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .expect("git rev-parse");
        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Extract stats
        let stats = extract_commit_diff_stats(repo_path, &commit_hash)
            .await
            .expect("should extract stats");

        // Initial commit adds 10 lines, removes 0
        assert_eq!(stats.insertions, 10);
        assert_eq!(stats.deletions, 0);
    }

    #[tokio::test]
    async fn test_update_session_loc_from_git() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a session
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

        // Update LOC from git
        let stats = DiffStats {
            files_changed: 2,
            insertions: 42,
            deletions: 13,
        };
        db.update_session_loc_from_git("sess-1", &stats)
            .await
            .unwrap();

        // Verify the update
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 42, "lines_added should be updated");
        assert_eq!(row.1, 13, "lines_removed should be updated");
        assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
    }

    #[tokio::test]
    async fn test_update_session_loc_from_git_zero_stats() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a session
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

        // Try to update with zero stats (should be no-op)
        let stats = DiffStats {
            files_changed: 0,
            insertions: 0,
            deletions: 0,
        };
        db.update_session_loc_from_git("sess-1", &stats)
            .await
            .unwrap();

        // Verify nothing changed
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 0, "lines_added should remain 0");
        assert_eq!(row.1, 0, "lines_removed should remain 0");
        assert_eq!(row.2, 0, "loc_source should remain 0 (not computed)");
    }

    #[tokio::test]
    async fn test_correlate_session_extracts_git_diff_stats() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        // Create and commit a file
        std::fs::write(repo_path.join("file.txt"), "line1\nline2\nline3\n").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .expect("git rev-parse");
        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let output = std::process::Command::new("git")
            .args(["log", "-1", "--format=%at"])
            .current_dir(repo_path)
            .output()
            .expect("git log timestamp");
        let commit_ts: i64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap();

        // Insert session
        let dir_str = repo_path.to_str().unwrap();
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            dir_str,
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            commit_ts + 600, // Session ends after commit
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        // Create commit and correlate
        let commits = vec![GitCommit {
            hash: commit_hash.clone(),
            repo_path: dir_str.to_string(),
            message: "test commit".to_string(),
            author: Some("Test".to_string()),
            timestamp: commit_ts,
            branch: Some("main".to_string()),
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        db.batch_upsert_commits(&commits).await.unwrap();

        let session_info = SessionCorrelationInfo {
            session_id: "sess-1".to_string(),
            project_path: dir_str.to_string(),
            first_timestamp: Some(commit_ts - 600),
            last_timestamp: Some(commit_ts + 600),
            commit_skills: Vec::new(), // Tier 2 match
        };

        let inserted = correlate_session(&db, &session_info, &commits)
            .await
            .unwrap();

        assert_eq!(inserted, 1, "Should insert one match");

        // Verify LOC stats were extracted
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 3, "Should have 3 lines added");
        assert_eq!(row.1, 0, "Should have 0 lines removed");
        assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
    }

    #[tokio::test]
    async fn test_correlate_session_aggregates_multiple_commits() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        // First commit: add 5 lines
        std::fs::write(repo_path.join("file1.txt"), "1\n2\n3\n4\n5\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");
        std::process::Command::new("git")
            .args(["commit", "-m", "first"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .expect("git rev-parse");
        let hash1 = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Second commit: add 3 more lines
        std::fs::write(repo_path.join("file2.txt"), "a\nb\nc\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");
        std::process::Command::new("git")
            .args(["commit", "-m", "second"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .expect("git rev-parse");
        let hash2 = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let now = chrono::Utc::now().timestamp();
        let dir_str = repo_path.to_str().unwrap();

        // Insert session
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            dir_str,
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            now + 600,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        // Create commits
        let commits = vec![
            GitCommit {
                hash: hash1.clone(),
                repo_path: dir_str.to_string(),
                message: "first".to_string(),
                author: Some("Test".to_string()),
                timestamp: now,
                branch: Some("main".to_string()),
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: hash2.clone(),
                repo_path: dir_str.to_string(),
                message: "second".to_string(),
                author: Some("Test".to_string()),
                timestamp: now + 100,
                branch: Some("main".to_string()),
                files_changed: None,
                insertions: None,
                deletions: None,
            },
        ];

        db.batch_upsert_commits(&commits).await.unwrap();

        let session_info = SessionCorrelationInfo {
            session_id: "sess-1".to_string(),
            project_path: dir_str.to_string(),
            first_timestamp: Some(now - 600),
            last_timestamp: Some(now + 600),
            commit_skills: Vec::new(),
        };

        let inserted = correlate_session(&db, &session_info, &commits)
            .await
            .unwrap();

        assert_eq!(inserted, 2, "Should insert two matches");

        // Verify LOC stats were aggregated
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 8, "Should have 8 lines added (5 + 3)");
        assert_eq!(row.1, 0, "Should have 0 lines removed");
        assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
    }

    #[tokio::test]
    async fn test_correlate_session_idempotent_loc_stats() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        // Initialize git repo with one commit
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        std::fs::write(repo_path.join("file.txt"), "line1\nline2\nline3\n").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .expect("git rev-parse");
        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let now = chrono::Utc::now().timestamp();
        let dir_str = repo_path.to_str().unwrap();

        // Insert session
        db.insert_session_from_index(
            "sess-1",
            "project-1",
            "Project 1",
            dir_str,
            "/path/to/sess-1.jsonl",
            "Test session",
            None,
            10,
            now + 600,
            None,
            false,
            5000,
        )
        .await
        .unwrap();

        let commits = vec![GitCommit {
            hash: commit_hash.clone(),
            repo_path: dir_str.to_string(),
            message: "test commit".to_string(),
            author: Some("Test".to_string()),
            timestamp: now,
            branch: Some("main".to_string()),
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        db.batch_upsert_commits(&commits).await.unwrap();

        let session_info = SessionCorrelationInfo {
            session_id: "sess-1".to_string(),
            project_path: dir_str.to_string(),
            first_timestamp: Some(now - 600),
            last_timestamp: Some(now + 600),
            commit_skills: Vec::new(),
        };

        // First correlation: should extract LOC stats
        let inserted1 = correlate_session(&db, &session_info, &commits)
            .await
            .unwrap();
        assert_eq!(inserted1, 1, "Should insert one match on first run");

        let row1: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row1.0, 3, "Should have 3 lines added after first run");
        assert_eq!(row1.1, 0, "Should have 0 lines removed after first run");
        assert_eq!(row1.2, 2, "loc_source should be 2 after first run");

        // Second correlation: should be idempotent (no new links, LOC unchanged)
        let inserted2 = correlate_session(&db, &session_info, &commits)
            .await
            .unwrap();
        assert_eq!(inserted2, 0, "Should insert zero matches on second run (idempotent)");

        let row2: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        // LOC stats should remain unchanged
        assert_eq!(row2.0, 3, "lines_added should remain unchanged");
        assert_eq!(row2.1, 0, "lines_removed should remain unchanged");
        assert_eq!(row2.2, 2, "loc_source should remain unchanged");
    }

    #[tokio::test]
    async fn test_phase_f_git_sync_extracts_loc_stats() {
        // Phase F integration test: verify git sync extracts and updates LOC stats
        let db = Database::new_in_memory().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        // Setup git repo with commits
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config");

        // Commit adding 10 lines
        std::fs::write(repo_path.join("file.txt"), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");
        std::process::Command::new("git")
            .args(["commit", "-m", "add 10 lines"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let now = chrono::Utc::now().timestamp();
        let dir_str = repo_path.to_str().unwrap();

        // Insert session covering the commit time
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('sess-1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')"
        )
        .bind(dir_str)
        .bind(now - 600)
        .bind(now + 600)
        .execute(db.pool())
        .await
        .unwrap();

        // Run git sync (Phase F: should extract LOC stats)
        let result = run_git_sync(&db, |_| {}).await.unwrap();

        assert_eq!(result.repos_scanned, 1);
        assert_eq!(result.commits_found, 1);
        assert_eq!(result.links_created, 1);

        // Verify LOC stats were extracted and set to git-verified (loc_source = 2)
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 10, "Should extract 10 lines added from git diff");
        assert_eq!(row.1, 0, "Should extract 0 lines removed from git diff");
        assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
    }

    #[tokio::test]
    async fn test_run_git_sync_idempotent() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config name");

        std::fs::write(repo_path.join("file.txt"), "hello").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let dir_str = repo_path.to_str().unwrap();
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')"
        )
        .bind(dir_str)
        .bind(now - 7200)
        .bind(now + 7200)
        .execute(db.pool())
        .await
        .unwrap();

        // Run sync TWICE
        let result1 = run_git_sync(&db, |_| {}).await.unwrap();
        let result2 = run_git_sync(&db, |_| {}).await.unwrap();

        assert_eq!(result1.links_created, 1);
        // Second run: link already exists at same tier, so 0 new links
        assert_eq!(result2.links_created, 0);

        // Only 1 link total in DB
        let count = db.count_commits_for_session("s1").await.unwrap();
        assert_eq!(count, 1);
    }
}
