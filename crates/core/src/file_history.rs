//! Scanner and diff engine for ~/.claude/file-history/{sessionId}/{hash}@v{N}.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::Path;
use ts_rs::TS;

// ============================================================================
// Wire types (Rust → TypeScript via ts-rs)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryResponse {
    pub session_id: String,
    pub files: Vec<FileChange>,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub total_files: u32,
    pub total_added: u32,
    pub total_removed: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub file_path: String,
    pub file_hash: String,
    pub versions: Vec<FileVersion>,
    pub stats: DiffStats,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    pub version: u32,
    pub backup_file_name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DiffStats {
    pub added: u32,
    pub removed: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FileDiffResponse {
    pub file_path: String,
    pub from_version: u32,
    pub to_version: u32,
    pub hunks: Vec<DiffHunk>,
    pub stats: DiffStats,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_line_no: Option<u32>,
    pub new_line_no: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Add,
    Remove,
}

// ============================================================================
// Scanner — discovers files + versions from disk
// ============================================================================

/// Scan ~/.claude/file-history/{sessionId}/ and return file metadata.
///
/// Groups backup files by hash, extracts version numbers, computes aggregate
/// diff stats by diffing v(N-1) → v(N) for the latest version pair, or counts
/// all lines as added for single-version files.
///
/// `file_path_map` maps `backupFileName` → original file path (from JSONL
/// `file-history-snapshot` entries). If not provided, the file hash is used
/// as the display path.
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

    // Group files by hash: hash → [(version, full_path, size)]
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
            // Single version — count all lines as added
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

        // Resolve file path from map, falling back to hash
        let first_backup = file_versions
            .first()
            .map(|v| v.backup_file_name.as_str())
            .unwrap_or("");
        let file_path = file_path_map
            .get(first_backup)
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

// ============================================================================
// Diff engine — computes unified diff between two version files
// ============================================================================

/// Compute a structured diff between two backup files.
///
/// `context_lines` controls how many unchanged lines surround each change (default 3).
pub fn compute_diff(
    history_dir: &Path,
    session_id: &str,
    file_hash: &str,
    from_version: u32,
    to_version: u32,
    file_path: &str,
    context_lines: usize,
) -> Result<FileDiffResponse, String> {
    let session_dir = history_dir.join(session_id);
    let to_file = session_dir.join(format!("{file_hash}@v{to_version}"));

    // from_version=0 means "diff against empty" — shows all lines as added
    let from_text = if from_version == 0 {
        String::new()
    } else {
        let from_file = session_dir.join(format!("{file_hash}@v{from_version}"));
        std::fs::read_to_string(&from_file)
            .map_err(|e| format!("Cannot read v{from_version}: {e}"))?
    };
    let to_text =
        std::fs::read_to_string(&to_file).map_err(|e| format!("Cannot read v{to_version}: {e}"))?;

    let diff = TextDiff::from_lines(&from_text, &to_text);
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut total_added: u32 = 0;
    let mut total_removed: u32 = 0;

    for group in diff.grouped_ops(context_lines) {
        let mut hunk_lines: Vec<DiffLine> = Vec::new();
        let mut old_start: u32 = 0;
        let mut new_start: u32 = 0;
        let mut old_count: u32 = 0;
        let mut new_count: u32 = 0;
        let mut first = true;

        // iter over each op in the group, then iter_changes on each op
        for op in &group {
            for change in diff.iter_changes(op) {
                let (old_line, new_line) = match change.tag() {
                    ChangeTag::Equal => {
                        old_count += 1;
                        new_count += 1;
                        let old_idx = change.old_index().map(|i| i as u32 + 1);
                        let new_idx = change.new_index().map(|i| i as u32 + 1);
                        if first {
                            old_start = old_idx.unwrap_or(1);
                            new_start = new_idx.unwrap_or(1);
                            first = false;
                        }
                        (old_idx, new_idx)
                    }
                    ChangeTag::Delete => {
                        old_count += 1;
                        total_removed += 1;
                        let old_idx = change.old_index().map(|i| i as u32 + 1);
                        if first {
                            old_start = old_idx.unwrap_or(1);
                            new_start = change.new_index().map(|i| i as u32 + 1).unwrap_or(1);
                            first = false;
                        }
                        (old_idx, None)
                    }
                    ChangeTag::Insert => {
                        new_count += 1;
                        total_added += 1;
                        let new_idx = change.new_index().map(|i| i as u32 + 1);
                        if first {
                            old_start = change.old_index().map(|i| i as u32 + 1).unwrap_or(1);
                            new_start = new_idx.unwrap_or(1);
                            first = false;
                        }
                        (None, new_idx)
                    }
                };

                let kind = match change.tag() {
                    ChangeTag::Equal => DiffLineKind::Context,
                    ChangeTag::Delete => DiffLineKind::Remove,
                    ChangeTag::Insert => DiffLineKind::Add,
                };

                // Strip trailing newline from content
                let content = change.value().trim_end_matches('\n').to_string();

                hunk_lines.push(DiffLine {
                    kind,
                    content,
                    old_line_no: old_line,
                    new_line_no: new_line,
                });
            }
        }

        if !hunk_lines.is_empty() {
            hunks.push(DiffHunk {
                old_start,
                old_lines: old_count,
                new_start,
                new_lines: new_count,
                lines: hunk_lines,
            });
        }
    }

    Ok(FileDiffResponse {
        file_path: file_path.to_string(),
        from_version,
        to_version,
        stats: DiffStats {
            added: total_added,
            removed: total_removed,
        },
        hunks,
    })
}

/// Resolve the ~/.claude/file-history/ directory.
pub fn claude_file_history_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("file-history"))
}

// ============================================================================
// JSONL snapshot extraction
// ============================================================================

/// Extract a hash → file_path mapping from `file-history-snapshot` entries in a JSONL file.
///
/// Scans the JSONL file for lines with `"file-history-snapshot"` type, extracts
/// `snapshot.trackedFileBackups`, and builds a map from the hash portion of each
/// `backupFileName` (stripping the `@vN` suffix) to the original file path.
///
/// All snapshots are processed; later entries overwrite earlier ones for the same hash,
/// reflecting the most recent file path (files may be renamed between snapshots).
pub fn extract_file_path_map(jsonl_path: &Path) -> HashMap<String, String> {
    use memchr::memmem;
    use std::io::{BufRead, BufReader};

    let mut map = HashMap::new();

    let file = match std::fs::File::open(jsonl_path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(
                path = %jsonl_path.display(),
                error = %e,
                "Failed to open JSONL file for file-path extraction"
            );
            return map;
        }
    };

    let reader = BufReader::new(file);
    let finder = memmem::Finder::new(b"file-history-snapshot");

    for (line_idx, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::debug!(
                    line_number = line_idx + 1,
                    error = %e,
                    "Failed to read line in JSONL file"
                );
                continue;
            }
        };

        // SIMD-accelerated pre-filter: skip lines that can't be file-history-snapshot
        if finder.find(line.as_bytes()).is_none() {
            continue;
        }

        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(
                    line_number = line_idx + 1,
                    error = %e,
                    "Failed to parse JSON on line that matched file-history-snapshot pre-filter"
                );
                continue;
            }
        };

        if value.get("type").and_then(|t| t.as_str()) != Some("file-history-snapshot") {
            continue;
        }

        let tracked = match value
            .get("snapshot")
            .and_then(|s| s.get("trackedFileBackups"))
            .and_then(|t| t.as_object())
        {
            Some(obj) => obj,
            None => continue,
        };

        for (file_path, backup_info) in tracked {
            let backup_file_name = match backup_info.get("backupFileName").and_then(|b| b.as_str())
            {
                Some(name) => name,
                None => continue,
            };

            // Extract hash by stripping @vN suffix
            if let Some((hash, _version)) = parse_backup_filename(backup_file_name) {
                map.insert(hash, file_path.clone());
            }
        }
    }

    map
}

// ============================================================================
// Helpers
// ============================================================================

/// Validate that a file hash is safe to use in filesystem paths.
///
/// Rejects any value containing path traversal characters (`/`, `\`, `..`).
/// Real backup filenames are hex hashes — no path separators.
pub fn validate_file_hash(hash: &str) -> Result<(), String> {
    if hash.is_empty() {
        return Err("File hash is empty".to_string());
    }
    if hash.contains('/') || hash.contains('\\') || hash.contains("..") {
        return Err(format!("Invalid file hash: {hash}"));
    }
    Ok(())
}

/// Parse "hash@vN" → (hash, N)
fn parse_backup_filename(name: &str) -> Option<(String, u32)> {
    let at_pos = name.rfind('@')?;
    let hash = &name[..at_pos];
    let version_str = name.get(at_pos + 1..)?;
    if !version_str.starts_with('v') {
        return None;
    }
    let version: u32 = version_str[1..].parse().ok()?;
    Some((hash.to_string(), version))
}

/// Quick diff stats between two files (just counts, no hunk structure).
fn quick_diff_stats(from_path: &Path, to_path: &Path) -> DiffStats {
    let from_text = match std::fs::read_to_string(from_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                path = %from_path.display(),
                error = %e,
                "Failed to read 'from' file for diff stats"
            );
            return DiffStats::default();
        }
    };
    let to_text = match std::fs::read_to_string(to_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                path = %to_path.display(),
                error = %e,
                "Failed to read 'to' file for diff stats"
            );
            return DiffStats::default();
        }
    };
    let diff = TextDiff::from_lines(&from_text, &to_text);

    let mut added: u32 = 0;
    let mut removed: u32 = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    DiffStats { added, removed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_backup_filename() {
        assert_eq!(
            parse_backup_filename("abc123@v1"),
            Some(("abc123".to_string(), 1))
        );
        assert_eq!(
            parse_backup_filename("abc123@v12"),
            Some(("abc123".to_string(), 12))
        );
        assert_eq!(parse_backup_filename("noversion"), None);
        assert_eq!(parse_backup_filename("hash@x1"), None);
    }

    #[test]
    fn test_scan_file_history_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("sess-1");
        fs::create_dir_all(&session_dir).unwrap();

        fs::write(session_dir.join("abc@v1"), "line1\nline2\n").unwrap();
        fs::write(session_dir.join("abc@v2"), "line1\nline2\nline3\n").unwrap();

        let map = HashMap::from([("abc@v1".to_string(), "src/main.rs".to_string())]);

        let result = scan_file_history(tmp.path(), "sess-1", &map);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].file_path, "src/main.rs");
        assert_eq!(result.files[0].versions.len(), 2);
        assert_eq!(result.files[0].stats.added, 1);
        assert_eq!(result.files[0].stats.removed, 0);
        assert_eq!(result.summary.total_files, 1);
    }

    #[test]
    fn test_scan_file_history_missing_session() {
        let tmp = tempfile::tempdir().unwrap();
        let result = scan_file_history(tmp.path(), "nonexistent", &HashMap::new());
        assert!(result.files.is_empty());
    }

    #[test]
    fn test_scan_file_history_single_version() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("sess-2");
        fs::create_dir_all(&session_dir).unwrap();

        fs::write(session_dir.join("xyz@v1"), "a\nb\nc\n").unwrap();

        let result = scan_file_history(tmp.path(), "sess-2", &HashMap::new());
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].stats.added, 3); // 3 lines, all "new"
        assert_eq!(result.files[0].stats.removed, 0);
    }

    #[test]
    fn test_compute_diff_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("sess-d");
        fs::create_dir_all(&session_dir).unwrap();

        fs::write(session_dir.join("h1@v1"), "alpha\nbeta\ngamma\n").unwrap();
        fs::write(
            session_dir.join("h1@v2"),
            "alpha\nbeta modified\ngamma\ndelta\n",
        )
        .unwrap();

        let result = compute_diff(tmp.path(), "sess-d", "h1", 1, 2, "test.rs", 3).unwrap();

        assert_eq!(result.from_version, 1);
        assert_eq!(result.to_version, 2);
        assert_eq!(result.stats.added, 2); // "beta modified" + "delta"
        assert_eq!(result.stats.removed, 1); // "beta"
        assert!(!result.hunks.is_empty());
    }

    #[test]
    fn test_compute_diff_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("sess-m");
        fs::create_dir_all(&session_dir).unwrap();

        let result = compute_diff(tmp.path(), "sess-m", "nope", 1, 2, "test.rs", 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_files_sorted_by_version_count_desc() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("sess-s");
        fs::create_dir_all(&session_dir).unwrap();

        // File A: 1 version
        fs::write(session_dir.join("aaa@v1"), "a\n").unwrap();
        // File B: 3 versions
        fs::write(session_dir.join("bbb@v1"), "b\n").unwrap();
        fs::write(session_dir.join("bbb@v2"), "b\nc\n").unwrap();
        fs::write(session_dir.join("bbb@v3"), "b\nc\nd\n").unwrap();
        // File C: 2 versions
        fs::write(session_dir.join("ccc@v1"), "c\n").unwrap();
        fs::write(session_dir.join("ccc@v2"), "c\nd\n").unwrap();

        let result = scan_file_history(tmp.path(), "sess-s", &HashMap::new());
        assert_eq!(result.files.len(), 3);
        assert_eq!(result.files[0].versions.len(), 3); // bbb first
        assert_eq!(result.files[1].versions.len(), 2); // ccc second
        assert_eq!(result.files[2].versions.len(), 1); // aaa last
    }

    #[test]
    fn test_validate_file_hash() {
        assert!(validate_file_hash("abc123def456").is_ok());
        assert!(validate_file_hash("").is_err());
        assert!(validate_file_hash("../../../etc/passwd").is_err());
        assert!(validate_file_hash("hash/with/slashes").is_err());
        assert!(validate_file_hash("hash\\backslash").is_err());
        assert!(validate_file_hash("..").is_err());
    }

    // ====================================================================
    // parse_backup_filename edge cases
    // ====================================================================

    #[test]
    fn test_parse_backup_filename_edge_cases() {
        // Empty string
        assert_eq!(parse_backup_filename(""), None);

        // Hash containing '@' — rfind picks the last '@'
        assert_eq!(
            parse_backup_filename("hash@extra@v1"),
            Some(("hash@extra".to_string(), 1))
        );

        // Version 0
        assert_eq!(
            parse_backup_filename("hash@v0"),
            Some(("hash".to_string(), 0))
        );

        // Trailing content after version number — parse fails
        assert_eq!(parse_backup_filename("hash@v1.bak"), None);

        // Real-world hex hash
        assert_eq!(
            parse_backup_filename("be8cf68e283682af@v1"),
            Some(("be8cf68e283682af".to_string(), 1))
        );
    }

    // ====================================================================
    // extract_file_path_map tests
    // ====================================================================

    #[test]
    fn test_extract_file_path_map_happy_path() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let line = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/main.rs":{"backupFileName":"abc123@v1","version":1,"backupTime":"2026-01-01T00:00:00Z"},"/src/lib.rs":{"backupFileName":"def456@v2","version":2,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
        fs::write(tmp.path(), format!("{line}\n")).unwrap();

        let map = extract_file_path_map(tmp.path());
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("abc123").unwrap(), "/src/main.rs");
        assert_eq!(map.get("def456").unwrap(), "/src/lib.rs");
    }

    #[test]
    fn test_extract_file_path_map_nonexistent_file() {
        let map = extract_file_path_map(Path::new("/tmp/does-not-exist-ever-12345.jsonl"));
        assert!(map.is_empty());
    }

    #[test]
    fn test_extract_file_path_map_empty_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        fs::write(tmp.path(), "").unwrap();

        let map = extract_file_path_map(tmp.path());
        assert!(map.is_empty());
    }

    #[test]
    fn test_extract_file_path_map_mixed_valid_invalid_lines() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let valid_snapshot = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/main.rs":{"backupFileName":"abc123@v1","version":1,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
        let malformed_json = r#"{broken"#;
        let assistant_line = r#"{"type":"assistant","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]}}"#;
        let snapshot_missing_tracked = r#"{"type":"file-history-snapshot","snapshot":{}}"#;

        let content = format!(
            "{valid_snapshot}\n{malformed_json}\n{assistant_line}\n{snapshot_missing_tracked}\n"
        );
        fs::write(tmp.path(), content).unwrap();

        let map = extract_file_path_map(tmp.path());
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("abc123").unwrap(), "/src/main.rs");
    }

    #[test]
    fn test_extract_file_path_map_later_snapshot_overwrites_earlier() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let snapshot1 = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/old_path.rs":{"backupFileName":"samehash@v1","version":1,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
        let snapshot2 = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/new_path.rs":{"backupFileName":"samehash@v2","version":2,"backupTime":"2026-01-01T01:00:00Z"}}}}"#;

        let content = format!("{snapshot1}\n{snapshot2}\n");
        fs::write(tmp.path(), content).unwrap();

        let map = extract_file_path_map(tmp.path());
        // Both snapshots map hash "samehash" — second one wins
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("samehash").unwrap(), "/src/new_path.rs");
    }

    #[test]
    fn test_extract_file_path_map_missing_backup_file_name() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // Entry has version and backupTime but no backupFileName
        let snapshot = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/main.rs":{"version":1,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
        fs::write(tmp.path(), format!("{snapshot}\n")).unwrap();

        let map = extract_file_path_map(tmp.path());
        assert!(map.is_empty());
    }
}
