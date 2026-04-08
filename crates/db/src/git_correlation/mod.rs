// crates/db/src/git_correlation/mod.rs
//! Git commit scanning and session correlation.
//!
//! This module implements:
//! - `scan_repo_commits()`: Spawn git log, parse output, handle edge cases
//! - `Tier1Matcher`: Match commit skills to commits within [-60s, +300s] window
//! - `Tier2Matcher`: Match commits during session time range
//! - CRUD operations for commits and session_commits tables

mod db_ops;
mod diff_stats;
mod matching;
mod scanning;
mod sync;
mod types;

// Re-export all public items to preserve the module's public API.

// Constants
pub use types::{TIER1_WINDOW_AFTER_SECS, TIER1_WINDOW_BEFORE_SECS};

// Types
pub use types::{
    CommitSkillInvocation, CorrelationEvidence, CorrelationMatch, DiffStats, GitCommit,
    GitSyncProgress, GitSyncResult, ScanResult, SessionCorrelationInfo, SessionSyncInfo,
};

// Git scanning
pub use scanning::scan_repo_commits;

// Diff stats
pub use diff_stats::{extract_commit_diff_stats, get_batch_diff_stats, get_commit_diff_stats};

// Correlation matching
pub use matching::{tier1_match, tier2_match};

// Sync pipeline
pub use sync::{correlate_session, run_git_sync};
