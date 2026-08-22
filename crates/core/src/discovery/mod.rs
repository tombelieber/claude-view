// crates/core/src/discovery/mod.rs
//! Project discovery for Claude Code sessions.
//!
//! This module scans the configured Claude projects directories to discover all
//! Claude Code projects and their sessions. It handles the encoded directory
//! names that Claude uses and efficiently extracts session metadata without
//! fully parsing each file.
//!
//! The set of directories scanned defaults to `~/.claude/projects/` alone. See
//! [`claude_projects_dirs`] for the opt-in multi-config-dir resolution.

mod git;
mod metadata;
mod paths;
mod projects;
mod resolve;
mod roots;

// Re-export public API — preserves all downstream `use` paths.
pub use git::{
    infer_git_root_from_worktree_path, resolve_git_branch, resolve_git_root,
    resolve_worktree_branch,
};
pub use metadata::{extract_session_metadata, ExtractedMetadata};
pub use paths::{claude_projects_dir, truncate_preview};
pub use projects::{count_active_sessions, get_projects};
pub use resolve::{
    encode_project_name, resolve_project_path_with_cwd, resolve_worktree_parent, ResolvedProject,
};
pub use roots::{
    claude_config_dirs, claude_projects_dirs, claude_projects_dirs_or_empty, config_dir_for_path,
    config_dir_from_session_path, profile_name, DEFAULT_PROFILE,
};
