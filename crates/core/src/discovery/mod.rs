// crates/core/src/discovery/mod.rs
//! Project discovery for Claude Code sessions.
//!
//! This module scans `~/.claude/projects/` to discover all Claude Code projects
//! and their sessions. It handles the encoded directory names that Claude uses
//! and efficiently extracts session metadata without fully parsing each file.

mod git;
mod metadata;
mod paths;
mod projects;
mod resolve;

// Re-export public API — preserves all downstream `use` paths.
pub use git::{
    infer_git_root_from_worktree_path, resolve_git_branch, resolve_git_root,
    resolve_worktree_branch,
};
pub use metadata::{extract_session_metadata, ExtractedMetadata};
pub use paths::{claude_projects_dir, truncate_preview};
pub use projects::{count_active_sessions, get_projects};
pub use resolve::{resolve_project_path_with_cwd, resolve_worktree_parent, ResolvedProject};
