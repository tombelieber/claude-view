//! Utility for resolving sub-agent JSONL file paths.
//!
//! Given a parent session's JSONL path and a sub-agent's alphanumeric ID,
//! resolves the path to the sub-agent's own JSONL file:
//!
//! ```text
//! Parent: ~/.claude/projects/{project}/{sessionId}.jsonl
//! Agent:  ~/.claude/projects/{project}/{sessionId}/subagents/agent-{agentId}.jsonl
//! ```

use std::path::{Path, PathBuf};

/// Resolve the filesystem path to a sub-agent's JSONL file.
///
/// The path structure is:
/// `{parent_dir}/{session_stem}/subagents/agent-{agent_id}.jsonl`
///
/// where `session_stem` is the parent JSONL filename without `.jsonl` extension.
pub fn resolve_subagent_path(parent_jsonl: &Path, agent_id: &str) -> PathBuf {
    let parent_dir = parent_jsonl.parent().unwrap_or(Path::new("."));
    let session_stem = parent_jsonl
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    parent_dir
        .join(session_stem)
        .join("subagents")
        .join(format!("agent-{agent_id}.jsonl"))
}

/// Check if a sub-agent's JSONL file exists on disk.
pub fn subagent_file_exists(parent_jsonl: &Path, agent_id: &str) -> bool {
    resolve_subagent_path(parent_jsonl, agent_id).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_subagent_path() {
        let parent_jsonl = PathBuf::from(
            "/home/user/.claude/projects/my-project/abc123-def456.jsonl",
        );
        let agent_id = "a951849";
        let resolved = resolve_subagent_path(&parent_jsonl, agent_id);
        assert_eq!(
            resolved,
            PathBuf::from(
                "/home/user/.claude/projects/my-project/abc123-def456/subagents/agent-a951849.jsonl"
            )
        );
    }

    #[test]
    fn test_resolve_subagent_path_strips_extension() {
        let parent_jsonl = PathBuf::from("/path/to/session.jsonl");
        let resolved = resolve_subagent_path(&parent_jsonl, "b789012");
        assert_eq!(
            resolved,
            PathBuf::from("/path/to/session/subagents/agent-b789012.jsonl")
        );
    }
}
