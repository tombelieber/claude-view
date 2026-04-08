// crates/db/src/git_correlation/types.rs
//! Types for git commit scanning and session correlation.

use serde::{Deserialize, Serialize};

/// Timeout for git operations (10 seconds).
pub(crate) const GIT_TIMEOUT_SECS: u64 = 10;

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

/// Lightweight session data for git correlation.
/// Contains only the 4 fields needed -- no JOINs, no JSON arrays, no token sums.
#[derive(Debug, Clone)]
pub struct SessionSyncInfo {
    pub session_id: String,
    pub project_path: String,
    pub first_message_at: Option<i64>,
    pub last_message_at: Option<i64>,
}

/// Information needed to correlate a session with commits.
#[derive(Debug, Clone)]
pub struct SessionCorrelationInfo {
    pub session_id: String,
    pub project_path: String,
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    pub commit_skills: Vec<CommitSkillInvocation>,
}

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
    CorrelatingStarted { total_correlatable_sessions: usize },
    /// Emitted after each session is correlated (success or failure).
    SessionCorrelated {
        sessions_done: usize,
        total_correlatable_sessions: usize,
        links_in_session: u32,
    },
}
