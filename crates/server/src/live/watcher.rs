//! File system watcher for JSONL session files.
//!
//! Watches `~/.claude/projects/` for changes to `.jsonl` files and emits
//! events when files are modified or removed.
//!
//! ## Architecture: Path Depth Filtering
//!
//! Claude Code stores session data in a structured hierarchy:
//! ```text
//! ~/.claude/projects/
//! ├── {project}/
//! │   ├── {sessionId}.jsonl                          ← Parent session (WATCH)
//! │   ├── {sessionId}/                               ← Session subdirectory (IGNORE)
//! │   │   ├── subagents/
//! │   │   │   └── agent-{id}.jsonl                   ← Sub-agent JSONL (IGNORE)
//! │   │   └── tool-results/
//! │   │       └── {toolUseId}.txt                    ← Large tool output (IGNORE)
//! ```
//!
//! **Systematic filtering:** The watcher only processes files exactly 2 path components
//! deep from `projects/` (format: `{project}/{sessionId}.jsonl`). This ensures:
//! - Parent sessions are monitored ✅
//! - Sub-agent files are ignored (depth 4+) ✅
//! - Tool result files are ignored (depth 4+) ✅
//! - No string matching on paths (robust to directory name changes) ✅
//!
//! This matches the structure used by `initial_scan()` which only reads files
//! directly in each project directory.

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{error, warn};

/// Events emitted by the file watcher, pre-filtered to only parent session JSONL files.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// A parent session JSONL file was modified (new lines appended).
    Modified(PathBuf),
    /// A parent session JSONL file was removed from disk.
    Removed(PathBuf),
    /// OS event queue overflowed — trigger full reconciliation scan.
    Rescan,
}

/// Start a file system watcher over every configured projects directory.
///
/// Defaults to `~/.claude/projects/` alone; additional config dirs are opt-in
/// via the environment (see `claude_view_core::discovery::claude_projects_dirs`).
///
/// Modified/removed JSONL files are sent through the provided `mpsc::Sender`.
/// Returns the watcher handle which must be kept alive for the duration of
/// monitoring (dropping it stops the watch).
///
/// If no projects directory exists, logs a warning and returns a watcher that
/// watches nothing.
///
/// ## Filtering Strategy
///
/// Uses **path depth filtering** to ensure only parent session files are processed:
/// - Parent sessions: `{project}/{sessionId}.jsonl` (depth 2) ✅
/// - Sub-agents: `{project}/{sessionId}/subagents/agent-*.jsonl` (depth 4+) ❌
/// - Tool results: `{project}/{sessionId}/tool-results/*.txt` (depth 4+) ❌
///
/// This is systematic and robust — no string matching, just structural validation.
pub fn start_watcher(
    tx: mpsc::Sender<FileEvent>,
) -> notify::Result<(RecommendedWatcher, Arc<AtomicU64>)> {
    let projects_dirs = claude_view_core::discovery::claude_projects_dirs_or_empty();
    if projects_dirs.is_empty() {
        warn!("Could not determine home directory; file watcher disabled");
        let w = notify::recommended_watcher(move |_res: Result<notify::Event, notify::Error>| {})?;
        return Ok((w, Arc::new(AtomicU64::new(0))));
    }

    let dropped_events = Arc::new(AtomicU64::new(0));
    let dropped_counter = dropped_events.clone();

    // Clone the roots for use in the closure (must be moved). Canonicalized so
    // the depth-2 `strip_prefix` filter compares like with like, matching the
    // indexer_v2 watcher.
    let roots_for_filter: Vec<PathBuf> = projects_dirs
        .iter()
        .map(|dir| dir.canonicalize().unwrap_or_else(|_| dir.clone()))
        .collect();

    // Create the watcher with a callback that filters and forwards events
    let mut watcher = notify::recommended_watcher(
        move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Check for OS event queue overflow BEFORE path filtering,
                    // because overflow events have no JSONL paths and would be
                    // filtered out by the jsonl_paths.is_empty() guard.
                    if event.kind == EventKind::Other {
                        if tx.try_send(FileEvent::Rescan).is_err() {
                            warn!("File watcher channel full — rescan event dropped");
                        }
                        return; // Don't process individual paths for overflow events
                    }

                    // Filter to only parent session JSONL files (not sub-agents or tool-results)
                    // Parent sessions have path structure: {projects_dir}/{project}/{sessionId}.jsonl
                    // Sub-agents have deeper paths: {projects_dir}/{project}/{sessionId}/subagents/agent-*.jsonl
                    let jsonl_paths: Vec<PathBuf> = event
                        .paths
                        .into_iter()
                        .filter(|p| {
                            // Must be a .jsonl file
                            if !p.extension().map(|ext| ext == "jsonl").unwrap_or(false) {
                                return false;
                            }

                            // Must be exactly 2 path components deep from one of
                            // the roots. Format: {project}/{sessionId}.jsonl
                            roots_for_filter.iter().any(|root| {
                                p.strip_prefix(root)
                                    .map(|rel| rel.components().count() == 2)
                                    .unwrap_or(false)
                            })
                        })
                        .collect();

                    if jsonl_paths.is_empty() {
                        return;
                    }

                    for path in jsonl_paths {
                        let file_event = match event.kind {
                            EventKind::Remove(_) => FileEvent::Removed(path),
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                FileEvent::Modified(path)
                            }
                            _ => continue,
                        };
                        if tx.try_send(file_event).is_err() {
                            let count = dropped_counter.fetch_add(1, Ordering::Relaxed) + 1;
                            if count == 1 || count.is_multiple_of(100) {
                                warn!(
                                dropped_total = count,
                                "File watcher channel full — event dropped (process detector will catch up)"
                            );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("File watcher error: {} — triggering rescan", e);
                    if tx.try_send(FileEvent::Rescan).is_err() {
                        warn!("File watcher channel full — error-triggered rescan dropped");
                    }
                }
            }
        },
    )?;

    let mut watched = 0usize;
    for projects_dir in &projects_dirs {
        if projects_dir.exists() {
            watcher.watch(projects_dir, RecursiveMode::Recursive)?;
            watched += 1;
            tracing::info!(
                "Watching {} for parent session JSONL changes (depth-filtered)",
                projects_dir.display()
            );
        } else {
            warn!(
                "Claude projects directory does not exist: {}; not watched",
                projects_dir.display()
            );
        }
    }

    if watched == 0 {
        warn!("No Claude projects directory exists; file watcher idle");
    }

    Ok((watcher, dropped_events))
}

/// Scan the projects directory for existing JSONL files modified in the last 24 hours.
///
/// Returns paths sorted by modification time (newest first). This is used at
/// startup to populate the initial session state before the watcher kicks in.
///
/// ## Filtering Strategy
///
/// Only scans **direct children** of each project directory (depth 2):
/// - Parent sessions: `{project}/{sessionId}.jsonl` ✅
/// - Sub-agents: Ignored (not in direct project directory) ✅
///
/// This matches the watcher's depth-based filtering for consistency.
pub fn initial_scan(projects_dir: &Path) -> Vec<PathBuf> {
    if !projects_dir.exists() {
        return Vec::new();
    }

    let cutoff = SystemTime::now() - Duration::from_secs(24 * 60 * 60);

    let mut entries: Vec<(PathBuf, SystemTime)> = Vec::new();

    // Walk top-level project directories
    let read_dir = match std::fs::read_dir(projects_dir) {
        Ok(rd) => rd,
        Err(e) => {
            warn!(
                "Failed to read projects dir {}: {}",
                projects_dir.display(),
                e
            );
            return Vec::new();
        }
    };

    for project_entry in read_dir.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        // Read JSONL files in this project directory (depth 2 only)
        // This ignores subdirectories like {sessionId}/subagents/
        let sub_read = match std::fs::read_dir(&project_path) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for file_entry in sub_read.flatten() {
            let file_path = file_entry.path();

            // Must be a file (not directory) with .jsonl extension
            if !file_path.is_file() {
                continue;
            }
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            let modified = match file_entry.metadata().and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            if modified >= cutoff {
                entries.push((file_path, modified));
            }
        }
    }

    // Sort by modification time, newest first
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.into_iter().map(|(p, _)| p).collect()
}

/// Run [`initial_scan`] over every configured projects directory and merge the
/// results into a single newest-first list.
///
/// Sorting happens after the merge, so recency ordering holds across roots
/// rather than only within each one. Session filenames are UUIDs, so files
/// from different roots cannot collide.
pub fn initial_scan_all(projects_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut entries: Vec<(PathBuf, SystemTime)> = Vec::new();

    for projects_dir in projects_dirs {
        for path in initial_scan(projects_dir) {
            let Ok(modified) = std::fs::metadata(&path).and_then(|m| m.modified()) else {
                continue;
            };
            entries.push((path, modified));
        }
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.into_iter().map(|(p, _)| p).collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    /// Verify that path depth filtering correctly identifies parent sessions vs sub-agents
    #[test]
    fn test_path_depth_filtering() {
        let projects_dir = PathBuf::from("/home/user/.claude/projects");

        // Parent session paths (depth 2) — should PASS
        let parent1 = projects_dir.join("my-project").join("abc123.jsonl");
        let parent2 = projects_dir.join("another-project").join("def456.jsonl");

        assert_eq!(
            parent1
                .strip_prefix(&projects_dir)
                .unwrap()
                .components()
                .count(),
            2,
            "Parent session should have depth 2"
        );
        assert_eq!(
            parent2
                .strip_prefix(&projects_dir)
                .unwrap()
                .components()
                .count(),
            2,
            "Parent session should have depth 2"
        );

        // Sub-agent paths (depth 4) — should FAIL
        let subagent1 = projects_dir
            .join("my-project")
            .join("abc123")
            .join("subagents")
            .join("agent-a123456.jsonl");
        let subagent2 = projects_dir
            .join("another-project")
            .join("def456")
            .join("subagents")
            .join("agent-b789012.jsonl");

        assert_eq!(
            subagent1
                .strip_prefix(&projects_dir)
                .unwrap()
                .components()
                .count(),
            4,
            "Sub-agent should have depth 4"
        );
        assert_eq!(
            subagent2
                .strip_prefix(&projects_dir)
                .unwrap()
                .components()
                .count(),
            4,
            "Sub-agent should have depth 4"
        );

        // Tool results paths (depth 4) — should FAIL
        let tool_result = projects_dir
            .join("my-project")
            .join("abc123")
            .join("tool-results")
            .join("toolu_xyz.txt");

        assert_eq!(
            tool_result
                .strip_prefix(&projects_dir)
                .unwrap()
                .components()
                .count(),
            4,
            "Tool result should have depth 4"
        );
    }

    /// Verify that the filtering logic in the watcher would correctly accept/reject paths
    #[test]
    fn test_watcher_filter_logic() {
        let projects_dir = PathBuf::from("/home/user/.claude/projects");

        // Simulate the watcher's filter logic
        let filter = |p: &PathBuf| -> bool {
            if !p.extension().map(|ext| ext == "jsonl").unwrap_or(false) {
                return false;
            }
            if let Ok(rel_path) = p.strip_prefix(&projects_dir) {
                rel_path.components().count() == 2
            } else {
                false
            }
        };

        // Parent sessions should pass
        assert!(filter(&projects_dir.join("proj").join("session.jsonl")));
        assert!(filter(
            &projects_dir.join("another-proj").join("abc123.jsonl")
        ));

        // Sub-agents should be rejected
        assert!(!filter(
            &projects_dir
                .join("proj")
                .join("session")
                .join("subagents")
                .join("agent-a.jsonl")
        ));
        assert!(!filter(
            &projects_dir
                .join("proj")
                .join("session")
                .join("tool-results")
                .join("tool.jsonl")
        ));

        // Non-JSONL files should be rejected
        assert!(!filter(&projects_dir.join("proj").join("session.txt")));
        assert!(!filter(&projects_dir.join("proj").join("README.md")));

        // Files outside projects_dir should be rejected
        let outside = PathBuf::from("/tmp/session.jsonl");
        assert!(!filter(&outside));
    }
}
