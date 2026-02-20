//! Centralized path functions for all app storage locations.
//!
//! Single source of truth — eliminates ad-hoc `dirs::cache_dir().join(...)` scattered across crates.

use std::path::PathBuf;

/// App cache root: `~/Library/Caches/claude-view/` (macOS) or `~/.cache/claude-view/` (Linux).
pub fn app_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("claude-view"))
}

/// Tantivy search index directory: `<app_cache_dir>/search-index/`.
pub fn search_index_dir() -> Option<PathBuf> {
    app_cache_dir().map(|d| d.join("search-index"))
}

/// SQLite database file: `<app_cache_dir>/claude-view.db`.
pub fn db_path() -> Option<PathBuf> {
    app_cache_dir().map(|d| d.join("claude-view.db"))
}

/// Remove all claude-view cache data (DB, WAL, SHM, search index).
/// Returns list of what was removed for user feedback.
pub fn remove_cache_data() -> Vec<String> {
    let mut removed = Vec::new();
    if let Some(dir) = app_cache_dir() {
        if dir.exists() {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => removed.push(format!("Removed cache directory: {}", dir.display())),
                Err(e) => removed.push(format!("Failed to remove {}: {}", dir.display(), e)),
            }
        }
    }
    removed
}

/// Remove restart-detection lock files from /tmp.
pub fn remove_lock_files() -> Vec<String> {
    let mut removed = Vec::new();
    let tmp = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("claude-view-") && name.ends_with(".lock") {
                match std::fs::remove_file(entry.path()) {
                    Ok(()) => removed.push(format!("Removed lock file: {}", entry.path().display())),
                    Err(e) => removed.push(format!("Failed to remove {}: {}", entry.path().display(), e)),
                }
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_cache_dir() {
        let dir = app_cache_dir();
        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.to_string_lossy().contains("claude-view"));
    }

    #[test]
    fn test_search_index_dir() {
        let dir = search_index_dir();
        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.to_string_lossy().ends_with("search-index"));
    }

    #[test]
    fn test_db_path() {
        let path = db_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().ends_with("claude-view.db"));
    }
}
