//! File system watcher for JSONL session files.
//!
//! Watches `~/.claude/projects/` for changes to `.jsonl` files and emits
//! events when files are modified or removed.

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{error, warn};

/// Events emitted by the file watcher, pre-filtered to only JSONL files.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// A JSONL file was modified (new lines appended).
    Modified(PathBuf),
    /// A JSONL file was removed from disk.
    Removed(PathBuf),
}

/// Start a file system watcher on `~/.claude/projects/`.
///
/// Modified/removed JSONL files are sent through the provided `mpsc::Sender`.
/// Returns the watcher handle which must be kept alive for the duration of
/// monitoring (dropping it stops the watch).
///
/// If the projects directory does not exist, logs a warning and returns a
/// watcher that watches nothing.
pub fn start_watcher(tx: mpsc::Sender<FileEvent>) -> notify::Result<RecommendedWatcher> {
    let projects_dir = match dirs::home_dir() {
        Some(home) => home.join(".claude").join("projects"),
        None => {
            warn!("Could not determine home directory; file watcher disabled");
            return notify::recommended_watcher(move |_res: Result<notify::Event, notify::Error>| {});
        }
    };

    // Create the watcher with a callback that filters and forwards events
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        match res {
            Ok(event) => {
                // Filter to only .jsonl files
                let jsonl_paths: Vec<PathBuf> = event
                    .paths
                    .into_iter()
                    .filter(|p| {
                        p.extension()
                            .map(|ext| ext == "jsonl")
                            .unwrap_or(false)
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
                    // Best-effort send; if the receiver is full/closed, drop
                    if tx.try_send(file_event).is_err() {
                        // Channel full or closed — not fatal
                    }
                }
            }
            Err(e) => {
                error!("File watcher error: {}", e);
            }
        }
    })?;

    if projects_dir.exists() {
        watcher.watch(&projects_dir, RecursiveMode::Recursive)?;
        tracing::info!("Watching {} for JSONL changes", projects_dir.display());
    } else {
        warn!(
            "Claude projects directory does not exist: {}; file watcher idle",
            projects_dir.display()
        );
    }

    Ok(watcher)
}

/// Scan the projects directory for existing JSONL files modified in the last 24 hours.
///
/// Returns paths sorted by modification time (newest first). This is used at
/// startup to populate the initial session state before the watcher kicks in.
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
            warn!("Failed to read projects dir {}: {}", projects_dir.display(), e);
            return Vec::new();
        }
    };

    for project_entry in read_dir.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        // Read JSONL files in this project directory
        let sub_read = match std::fs::read_dir(&project_path) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for file_entry in sub_read.flatten() {
            let file_path = file_entry.path();
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
    entries.into_iter().map(|(path, _)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_initial_scan_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = initial_scan(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_initial_scan_with_jsonl_files() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("test-project");
        fs::create_dir(&project_dir).unwrap();

        // Create a recent JSONL file
        let file_path = project_dir.join("session-123.jsonl");
        let mut f = fs::File::create(&file_path).unwrap();
        writeln!(f, r#"{{"type":"user","content":"hello"}}"#).unwrap();

        let result = initial_scan(dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], file_path);
    }

    #[test]
    fn test_initial_scan_ignores_non_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("test-project");
        fs::create_dir(&project_dir).unwrap();

        // Create a non-JSONL file
        let file_path = project_dir.join("notes.txt");
        fs::File::create(&file_path).unwrap();

        let result = initial_scan(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_initial_scan_nonexistent_dir() {
        let result = initial_scan(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(result.is_empty());
    }
}
