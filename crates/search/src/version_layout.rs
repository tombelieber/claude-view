//! Pure path resolution helpers for the versioned search index layout.
//!
//! The on-disk layout is `<base>/v{N}/...` where N is the schema version
//! the index was built against. This module computes paths and detects
//! existing layouts; it never mutates the filesystem.

use std::path::{Path, PathBuf};

use crate::SEARCH_SCHEMA_VERSION;

/// Returns the path for a specific schema version's index directory.
pub fn versioned_path(base: &Path, version: u32) -> PathBuf {
    base.join(format!("v{version}"))
}

/// Returns the path for the *current* schema version's index directory.
pub fn current_versioned_path(base: &Path) -> PathBuf {
    versioned_path(base, SEARCH_SCHEMA_VERSION)
}

/// Read `base` and return all existing `v{N}/` subdirectories paired with
/// their version number, sorted by version DESCENDING (newest first).
///
/// Ignores any non-`v*` entries (legacy flat layout files, lockfiles, etc.).
/// Returns an empty Vec if `base` does not exist or contains no versioned dirs.
pub fn list_versioned_dirs(base: &Path) -> Vec<(u32, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(base) else {
        return Vec::new();
    };
    let mut versions: Vec<(u32, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = path.file_name()?.to_str()?;
            let version = name.strip_prefix('v')?.parse::<u32>().ok()?;
            Some((version, path))
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions
}

/// Returns true if `base` contains a legacy flat-layout index — i.e. a
/// `schema_version` file at the root (not inside a `v*/` subdir) AND
/// at least one tantivy data file (`*.idx`, `*.store`, etc.) at the root.
///
/// We require BOTH signals to avoid false positives during partial migrations.
pub fn is_legacy_flat_layout(base: &Path) -> bool {
    if !base.join("schema_version").is_file() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(base) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| matches!(ext, "idx" | "store" | "term" | "pos" | "fast" | "fieldnorm"))
                .unwrap_or(false)
    })
}

/// Returns the schema version recorded inside an index directory, if any.
/// Reads `<dir>/schema_version` and parses it as u32.
pub fn read_schema_version_file(dir: &Path) -> Option<u32> {
    std::fs::read_to_string(dir.join("schema_version"))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

/// Returns true if the given versioned directory is "complete" — i.e. it
/// has a valid schema_version file matching the directory's version number.
///
/// During a background rebuild, a directory exists but has no schema_version
/// file yet; that is "incomplete" and must be discarded on next startup.
pub fn is_complete_versioned_index(dir: &Path, expected_version: u32) -> bool {
    read_schema_version_file(dir) == Some(expected_version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn versioned_path_produces_v_prefix() {
        let p = versioned_path(Path::new("/foo"), 7);
        assert_eq!(p, PathBuf::from("/foo/v7"));
    }

    #[test]
    fn current_versioned_path_uses_constant() {
        let p = current_versioned_path(Path::new("/foo"));
        assert_eq!(p, PathBuf::from(format!("/foo/v{}", SEARCH_SCHEMA_VERSION)));
    }

    #[test]
    fn list_versioned_dirs_empty_when_missing() {
        assert!(list_versioned_dirs(Path::new("/nonexistent/path")).is_empty());
    }

    #[test]
    fn list_versioned_dirs_sorts_descending() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("v3")).unwrap();
        std::fs::create_dir(dir.path().join("v7")).unwrap();
        std::fs::create_dir(dir.path().join("v5")).unwrap();
        // Non-versioned entries must be ignored
        std::fs::create_dir(dir.path().join("backup")).unwrap();
        std::fs::write(dir.path().join("schema_version"), "7").unwrap();

        let versions = list_versioned_dirs(dir.path());
        let nums: Vec<u32> = versions.iter().map(|(n, _)| *n).collect();
        assert_eq!(nums, vec![7, 5, 3]);
    }

    #[test]
    fn legacy_flat_layout_requires_both_signals() {
        let dir = tempdir().unwrap();
        // Just a schema_version file: NOT legacy (could be a fresh empty layout)
        std::fs::write(dir.path().join("schema_version"), "7").unwrap();
        assert!(!is_legacy_flat_layout(dir.path()));

        // Add a tantivy data file: NOW it's legacy flat layout
        std::fs::write(dir.path().join("abc123.idx"), b"fake").unwrap();
        assert!(is_legacy_flat_layout(dir.path()));
    }

    #[test]
    fn legacy_flat_layout_false_for_versioned_only() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("v7")).unwrap();
        std::fs::write(dir.path().join("v7/schema_version"), "7").unwrap();
        std::fs::write(dir.path().join("v7/abc.idx"), b"fake").unwrap();
        assert!(!is_legacy_flat_layout(dir.path()));
    }

    #[test]
    fn read_schema_version_file_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        assert_eq!(read_schema_version_file(dir.path()), None);
    }

    #[test]
    fn read_schema_version_file_parses_value() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("schema_version"), "42").unwrap();
        assert_eq!(read_schema_version_file(dir.path()), Some(42));
    }

    #[test]
    fn is_complete_versioned_index_checks_version_match() {
        let dir = tempdir().unwrap();
        let v7_path = dir.path().join("v7");
        std::fs::create_dir(&v7_path).unwrap();

        // No schema_version file → incomplete
        assert!(!is_complete_versioned_index(&v7_path, 7));

        // Wrong version → incomplete
        std::fs::write(v7_path.join("schema_version"), "6").unwrap();
        assert!(!is_complete_versioned_index(&v7_path, 7));

        // Matching version → complete
        std::fs::write(v7_path.join("schema_version"), "7").unwrap();
        assert!(is_complete_versioned_index(&v7_path, 7));
    }
}
