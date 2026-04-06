//! Reader for ~/.claude/memory/ (global) and ~/.claude/projects/*/memory/ (per-project)
//! Claude Code memory files with YAML frontmatter.
//!
//! On-demand read, NO SQLite indexing — follows plan_files.rs / task_files.rs pattern.
//! ~159 files, ~500 KB — no perf concern.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Memory type enum matching Claude Code's 4 memory types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::User => write!(f, "user"),
            MemoryType::Feedback => write!(f, "feedback"),
            MemoryType::Project => write!(f, "project"),
            MemoryType::Reference => write!(f, "reference"),
        }
    }
}

/// A single memory entry parsed from a .md file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    /// Frontmatter `name` field
    pub name: String,
    /// Frontmatter `description` field
    pub description: String,
    /// Frontmatter `type` field
    pub memory_type: MemoryType,
    /// Markdown body (after frontmatter)
    pub body: String,
    /// Filename (e.g. "feedback_always_fsm_tree_diagram.md")
    pub filename: String,
    /// Relative path from ~/.claude/ (for API reference)
    pub relative_path: String,
    /// Scope: "global" or project display name
    pub scope: String,
    /// Encoded project dir name (empty for global)
    pub project_dir: String,
    /// File size in bytes
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// Last modified timestamp (unix seconds)
    #[ts(type = "number")]
    pub modified_at: i64,
}

/// Summary of all memories grouped by scope.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MemoryIndex {
    pub total_count: usize,
    pub global: Vec<MemoryEntry>,
    pub projects: Vec<ProjectMemoryGroup>,
}

/// A group of memories for one project.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemoryGroup {
    /// Encoded project directory name
    pub project_dir: String,
    /// Human-readable project display name
    pub display_name: String,
    /// Number of memories
    pub count: usize,
    /// The memory entries
    pub memories: Vec<MemoryEntry>,
}

/// YAML frontmatter parsed from memory files.
#[derive(Debug, Deserialize)]
struct MemoryFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "type")]
    memory_type: Option<String>,
}

/// Parse YAML frontmatter + markdown body from a memory file.
///
/// Expected format:
/// ```text
/// ---
/// name: ...
/// description: ...
/// type: feedback
/// ---
///
/// {markdown body}
/// ```
fn parse_memory_file(content: &str) -> (Option<MemoryFrontmatter>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    if let Some(end_idx) = after_first.find("\n---") {
        let yaml_str = &after_first[..end_idx];
        let body_start = end_idx + 4; // skip "\n---"
        let body = after_first[body_start..]
            .trim_start_matches('\n')
            .to_string();

        match serde_yaml::from_str::<MemoryFrontmatter>(yaml_str) {
            Ok(fm) => (Some(fm), body),
            Err(_) => (None, content.to_string()),
        }
    } else {
        (None, content.to_string())
    }
}

/// Parse memory type string to enum.
fn parse_memory_type(s: &str) -> MemoryType {
    match s.to_lowercase().as_str() {
        "user" => MemoryType::User,
        "feedback" => MemoryType::Feedback,
        "project" => MemoryType::Project,
        "reference" => MemoryType::Reference,
        _ => MemoryType::Feedback, // safe default
    }
}

/// Read all memory .md files from a directory, excluding MEMORY.md index.
fn read_memory_dir(
    dir: &Path,
    scope: &str,
    project_dir: &str,
    claude_dir: &Path,
) -> Vec<MemoryEntry> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut memories = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };

        // Skip non-md files and the index file
        if !fname.ends_with(".md") || fname == "MEMORY.md" {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let metadata = std::fs::metadata(&path).ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified_at = metadata
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let (frontmatter, body) = parse_memory_file(&content);

        let relative_path = path
            .strip_prefix(claude_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| fname.to_string());

        let name = frontmatter
            .as_ref()
            .and_then(|fm| fm.name.clone())
            .unwrap_or_else(|| fname.trim_end_matches(".md").replace('_', " "));

        let description = frontmatter
            .as_ref()
            .and_then(|fm| fm.description.clone())
            .unwrap_or_default();

        let memory_type = frontmatter
            .as_ref()
            .and_then(|fm| fm.memory_type.as_deref())
            .map(parse_memory_type)
            .unwrap_or(MemoryType::Feedback);

        memories.push(MemoryEntry {
            name,
            description,
            memory_type,
            body,
            filename: fname.to_string(),
            relative_path,
            scope: scope.to_string(),
            project_dir: project_dir.to_string(),
            size_bytes,
            modified_at,
        });
    }

    // Sort by type priority (feedback > project > reference > user), then by name
    memories.sort_by(|a, b| {
        let type_order = |t: &MemoryType| -> u8 {
            match t {
                MemoryType::Feedback => 0,
                MemoryType::Project => 1,
                MemoryType::Reference => 2,
                MemoryType::User => 3,
            }
        };
        type_order(&a.memory_type)
            .cmp(&type_order(&b.memory_type))
            .then_with(|| a.name.cmp(&b.name))
    });

    memories
}

/// Decode a Claude Code project directory name to a display name.
///
/// Claude Code encoding is lossy: `/` → `-`, `/@` → `--`.
/// Since `-` also appears in literal directory names (e.g. `claude-view`),
/// we can't purely decode from the string.
///
/// Strategy: try to reconstruct the actual filesystem path by probing
/// progressively, then fall back to heuristics.
///
/// Examples:
/// - `-Users-TBGor-dev--vicky-ai-claude-view` → `claude-view`
/// - `-Users-TBGor-dev--ok-trending-radar` → `trending-radar`
/// - `-Users-TBGor-dev--vicky-ai-gomoku` → `gomoku`
pub fn project_dir_to_display_name(encoded: &str) -> String {
    // Step 1: Strip worktree suffixes
    let base = encoded.split("--worktrees-").next().unwrap_or(encoded);
    let base = base.split("--claude-worktrees-").next().unwrap_or(base);

    // Step 2: Try filesystem probing — reconstruct path from encoded name
    // by testing which `-` are path separators vs literal dashes
    if let Some(path) = probe_encoded_path(base) {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            return name.to_string();
        }
    }

    // Step 3: Heuristic fallback — use the last `--` boundary
    // `--` reliably marks `/@` (scoped packages).
    // After `--` we have `{org}-{project}` but can't distinguish `-` as
    // separator vs literal. Return the full scoped portion.
    if let Some(pos) = base.rfind("--") {
        let after = &base[pos + 2..];
        return after.to_string();
    }

    // No scope marker — return the whole encoded name
    base.to_string()
}

/// Probe filesystem to reconstruct the actual path from an encoded name.
/// Returns the real path if found.
fn probe_encoded_path(encoded: &str) -> Option<std::path::PathBuf> {
    // The encoded name starts with `-` representing root `/`
    let trimmed = encoded.trim_start_matches('-');
    // Split into characters and try to reconstruct by testing if each `-` is `/`
    let chars: Vec<char> = trimmed.chars().collect();
    let mut path = std::path::PathBuf::from("/");
    let mut segment = String::new();

    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-' {
            // Check if this is `--` (scope marker = `/@`)
            if i + 1 < chars.len() && chars[i + 1] == '-' {
                // `--` → `/@`
                let test_path = path.join(&segment);
                if test_path.is_dir() {
                    path = test_path;
                    segment = String::from("@");
                    i += 2;
                    continue;
                }
                // Not a valid dir, treat as literal
                segment.push('-');
                segment.push('-');
                i += 2;
                continue;
            }

            // Single `-` — could be `/` or literal `-`
            // Try treating as `/` first
            let test_path = path.join(&segment);
            if !segment.is_empty() && test_path.is_dir() {
                path = test_path;
                segment.clear();
            } else {
                // Keep as literal `-`
                segment.push('-');
            }
            i += 1;
        } else {
            segment.push(chars[i]);
            i += 1;
        }
    }

    // Final segment
    if !segment.is_empty() {
        let final_path = path.join(&segment);
        if final_path.exists() {
            return Some(final_path);
        }
    }

    None
}

/// Resolve the ~/.claude/ directory.
pub fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Discover and read all memory files (global + per-project).
pub fn discover_all_memories() -> MemoryIndex {
    let Some(claude) = claude_dir() else {
        return MemoryIndex {
            total_count: 0,
            global: Vec::new(),
            projects: Vec::new(),
        };
    };

    // Global memories
    let global_dir = claude.join("memory");
    let global = read_memory_dir(&global_dir, "Global", "", &claude);

    // Per-project memories
    let projects_dir = claude.join("projects");
    let mut project_groups: Vec<ProjectMemoryGroup> = Vec::new();

    if projects_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
            dirs.sort_by_key(|e| e.file_name());

            for entry in dirs {
                let project_dir_name = entry.file_name().to_string_lossy().to_string();
                let memory_dir = entry.path().join("memory");

                if !memory_dir.is_dir() {
                    continue;
                }

                let display_name = project_dir_to_display_name(&project_dir_name);
                let memories =
                    read_memory_dir(&memory_dir, &display_name, &project_dir_name, &claude);

                if !memories.is_empty() {
                    let count = memories.len();
                    project_groups.push(ProjectMemoryGroup {
                        project_dir: project_dir_name,
                        display_name,
                        count,
                        memories,
                    });
                }
            }
        }
    }

    let total_count = global.len() + project_groups.iter().map(|g| g.count).sum::<usize>();

    MemoryIndex {
        total_count,
        global,
        projects: project_groups,
    }
}

/// Read memories for a specific project.
pub fn read_project_memories(project_dir: &str) -> Vec<MemoryEntry> {
    let Some(claude) = claude_dir() else {
        return Vec::new();
    };

    let memory_dir = claude.join("projects").join(project_dir).join("memory");
    let display_name = project_dir_to_display_name(project_dir);
    read_memory_dir(&memory_dir, &display_name, project_dir, &claude)
}

/// Read a single memory file by its relative path from ~/.claude/.
pub fn read_memory_file(relative_path: &str) -> Option<MemoryEntry> {
    let claude = claude_dir()?;
    let full_path = claude.join(relative_path);

    if !full_path.exists() || !full_path.is_file() {
        return None;
    }

    // Security: ensure the path is within ~/.claude/
    let canonical = full_path.canonicalize().ok()?;
    let claude_canonical = claude.canonicalize().ok()?;
    if !canonical.starts_with(&claude_canonical) {
        return None;
    }

    let content = std::fs::read_to_string(&full_path).ok()?;
    let metadata = std::fs::metadata(&full_path).ok();
    let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified_at = metadata
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let fname = full_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown.md");

    let (frontmatter, body) = parse_memory_file(&content);

    let name = frontmatter
        .as_ref()
        .and_then(|fm| fm.name.clone())
        .unwrap_or_else(|| fname.trim_end_matches(".md").replace('_', " "));

    let description = frontmatter
        .as_ref()
        .and_then(|fm| fm.description.clone())
        .unwrap_or_default();

    let memory_type = frontmatter
        .as_ref()
        .and_then(|fm| fm.memory_type.as_deref())
        .map(parse_memory_type)
        .unwrap_or(MemoryType::Feedback);

    // Determine scope from path
    let (scope, project_dir) = if relative_path.starts_with("projects/") {
        let parts: Vec<&str> = relative_path.split('/').collect();
        if parts.len() >= 2 {
            let pd = parts[1];
            (project_dir_to_display_name(pd), pd.to_string())
        } else {
            ("Global".to_string(), String::new())
        }
    } else {
        ("Global".to_string(), String::new())
    };

    Some(MemoryEntry {
        name,
        description,
        memory_type,
        body,
        filename: fname.to_string(),
        relative_path: relative_path.to_string(),
        scope,
        project_dir,
        size_bytes,
        modified_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_memory_file_with_frontmatter() {
        let content = r#"---
name: Test Memory
description: A test memory entry
type: feedback
---

This is the body content.

**Why:** Because testing is important."#;

        let (fm, body) = parse_memory_file(content);
        let fm = fm.unwrap();
        assert_eq!(fm.name.unwrap(), "Test Memory");
        assert_eq!(fm.description.unwrap(), "A test memory entry");
        assert_eq!(fm.memory_type.unwrap(), "feedback");
        assert!(body.starts_with("This is the body content."));
        assert!(body.contains("**Why:**"));
    }

    #[test]
    fn test_parse_memory_file_no_frontmatter() {
        let content = "Just plain markdown content.";
        let (fm, body) = parse_memory_file(content);
        assert!(fm.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn test_project_dir_to_display_name() {
        // Existing dir on disk → probe_encoded_path finds it
        let cv = project_dir_to_display_name("-Users-TBGor-dev--vicky-ai-claude-view");
        assert_eq!(cv, "claude-view");

        // Scoped package fallback (dir may not exist) → returns after last --
        // This test doesn't rely on filesystem state — it tests the heuristic
        let result = project_dir_to_display_name("-fake-path--org-project");
        assert_eq!(result, "org-project");

        // Worktree suffix stripped before decoding
        let result = project_dir_to_display_name("-fake--org-proj--worktrees-main");
        assert_eq!(result, "org-proj");
    }

    #[test]
    fn test_read_memory_dir_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let mem_dir = tmp.path().join("memory");
        fs::create_dir_all(&mem_dir).unwrap();

        fs::write(
            mem_dir.join("test_memory.md"),
            "---\nname: Test\ndescription: Desc\ntype: user\n---\n\nBody text.",
        )
        .unwrap();

        // MEMORY.md should be skipped
        fs::write(mem_dir.join("MEMORY.md"), "# Index").unwrap();

        let entries = read_memory_dir(&mem_dir, "Global", "", tmp.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Test");
        assert_eq!(entries[0].memory_type, MemoryType::User);
        assert_eq!(entries[0].body, "Body text.");
    }

    #[test]
    fn test_read_memory_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let entries = read_memory_dir(&tmp.path().join("nope"), "Global", "", tmp.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_memory_type() {
        assert_eq!(parse_memory_type("user"), MemoryType::User);
        assert_eq!(parse_memory_type("feedback"), MemoryType::Feedback);
        assert_eq!(parse_memory_type("project"), MemoryType::Project);
        assert_eq!(parse_memory_type("reference"), MemoryType::Reference);
        assert_eq!(parse_memory_type("unknown"), MemoryType::Feedback);
    }
}
