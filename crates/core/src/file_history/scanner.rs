//! Scanner -- discovers files + versions from disk.

use std::collections::HashMap;
use std::path::Path;

use super::helpers::{parse_backup_filename, quick_diff_stats};
use super::types::{DiffStats, DiffSummary, FileChange, FileHistoryResponse, FileVersion};

/// Scan ~/.claude/file-history/{sessionId}/ and return file metadata.
///
/// Groups backup files by hash, extracts version numbers, computes aggregate
/// diff stats by diffing v(N-1) -> v(N) for the latest version pair, or counts
/// all lines as added for single-version files.
///
/// `file_path_map` maps `hash` -> original file path (from JSONL
/// `file-history-snapshot` entries, with `@vN` suffix stripped).
/// If a hash has no entry, it is used as the display path.
pub fn scan_file_history(
    history_dir: &Path,
    session_id: &str,
    file_path_map: &HashMap<String, String>,
) -> FileHistoryResponse {
    let session_dir = history_dir.join(session_id);
    if !session_dir.is_dir() {
        return FileHistoryResponse {
            session_id: session_id.to_string(),
            files: Vec::new(),
            summary: DiffSummary {
                total_files: 0,
                total_added: 0,
                total_removed: 0,
            },
        };
    }

    // Group files by hash: hash -> [(version, full_path, size)]
    let mut groups: HashMap<String, Vec<(u32, std::path::PathBuf, u64)>> = HashMap::new();

    let entries = match std::fs::read_dir(&session_dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                path = %session_dir.display(),
                error = %e,
                "Failed to read session file-history directory"
            );
            return FileHistoryResponse {
                session_id: session_id.to_string(),
                files: Vec::new(),
                summary: DiffSummary {
                    total_files: 0,
                    total_added: 0,
                    total_removed: 0,
                },
            };
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Parse "hash@vN" pattern
        if let Some((hash, version)) = parse_backup_filename(&file_name) {
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            groups.entry(hash).or_default().push((version, path, size));
        }
    }

    let mut files: Vec<FileChange> = Vec::new();
    let mut total_added: u32 = 0;
    let mut total_removed: u32 = 0;

    for (hash, mut versions) in groups {
        versions.sort_by_key(|(v, _, _)| *v);

        let file_versions: Vec<FileVersion> = versions
            .iter()
            .map(|(v, path, size)| FileVersion {
                version: *v,
                backup_file_name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                size_bytes: *size,
            })
            .collect();

        // Compute aggregate stats from last version pair
        let stats = if versions.len() >= 2 {
            let prev = &versions[versions.len() - 2];
            let curr = &versions[versions.len() - 1];
            quick_diff_stats(&prev.1, &curr.1)
        } else {
            // Single version -- count all lines as added
            let line_count = match std::fs::read_to_string(&versions[0].1) {
                Ok(s) => s.lines().count() as u32,
                Err(e) => {
                    tracing::warn!(
                        path = %versions[0].1.display(),
                        error = %e,
                        "Failed to read single-version file for line count"
                    );
                    0
                }
            };
            DiffStats {
                added: line_count,
                removed: 0,
            }
        };

        total_added += stats.added;
        total_removed += stats.removed;

        // Resolve file path from map (keyed by hash), falling back to hash
        let file_path = file_path_map
            .get(&hash)
            .cloned()
            .unwrap_or_else(|| hash.clone());

        files.push(FileChange {
            file_path,
            file_hash: hash,
            versions: file_versions,
            stats,
        });
    }

    // Sort by version count descending (most-edited first)
    files.sort_by(|a, b| b.versions.len().cmp(&a.versions.len()));

    FileHistoryResponse {
        session_id: session_id.to_string(),
        summary: DiffSummary {
            total_files: files.len() as u32,
            total_added,
            total_removed,
        },
        files,
    }
}
