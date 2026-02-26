//! Centralized path functions for all app storage locations.
//!
//! Single source of truth — all write paths derive from `data_dir()`.
//! Set `CLAUDE_VIEW_DATA_DIR` to override (e.g., `./.data` for sandbox dev).

use std::path::PathBuf;

/// Single source of truth for ALL claude-view write paths.
/// Set CLAUDE_VIEW_DATA_DIR to override (e.g., `./.data` for sandbox dev).
/// Falls back to platform cache dir (~/Library/Caches/claude-view on macOS).
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CLAUDE_VIEW_DATA_DIR") {
        let path = PathBuf::from(&dir);
        if path.is_relative() {
            std::env::current_dir()
                .expect("cannot determine current directory")
                .join(path)
        } else {
            path
        }
    } else {
        dirs::cache_dir()
            .map(|d| d.join("claude-view"))
            .expect("no platform cache directory found")
    }
}

pub fn db_path() -> Option<PathBuf> {
    Some(data_dir().join("claude-view.db"))
}

pub fn search_index_dir() -> Option<PathBuf> {
    Some(data_dir().join("search-index"))
}

pub fn lock_dir() -> Option<PathBuf> {
    Some(data_dir().join("locks"))
}

/// Remove all claude-view cache data (DB, WAL, search index).
pub fn remove_cache_data() -> Vec<String> {
    let dir = data_dir();
    let mut removed = Vec::new();

    for name in &["claude-view.db", "claude-view.db-wal", "claude-view.db-shm"] {
        let p = dir.join(name);
        if p.exists() {
            if std::fs::remove_file(&p).is_ok() {
                removed.push(format!("Removed {}", p.display()));
            }
        }
    }

    let idx = dir.join("search-index");
    if idx.exists() {
        if std::fs::remove_dir_all(&idx).is_ok() {
            removed.push(format!("Removed {}", idx.display()));
        }
    }

    removed
}

/// Remove lock files from data_dir/locks/.
pub fn remove_lock_files() -> Vec<String> {
    let mut removed = Vec::new();
    if let Some(dir) = lock_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "lock").unwrap_or(false) {
                    if std::fs::remove_file(&path).is_ok() {
                        removed.push(format!("Removed {}", path.display()));
                    }
                }
            }
        }
    }

    // Also clean up legacy /tmp/ lock files from older versions
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("claude-view-") && name_str.ends_with(".lock") {
                if std::fs::remove_file(entry.path()).is_ok() {
                    removed.push(format!("Removed legacy {}", entry.path().display()));
                }
            }
        }
    }

    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_data_dir_uses_env_var_absolute() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-claude-view-data");
        let dir = data_dir();
        assert_eq!(dir, PathBuf::from("/tmp/test-claude-view-data"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_data_dir_resolves_relative_path() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "./.data");
        let dir = data_dir();
        assert!(
            dir.is_absolute(),
            "relative path should be resolved to absolute"
        );
        assert!(dir.ends_with(".data"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_data_dir_falls_back_to_cache_dir() {
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
        let dir = data_dir();
        assert!(dir.ends_with("claude-view"));
    }

    #[test]
    fn test_db_path_derives_from_data_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        let path = db_path().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-cv/claude-view.db"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_search_index_dir_derives_from_data_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        let path = search_index_dir().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-cv/search-index"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_lock_dir_derives_from_data_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        let path = lock_dir().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-cv/locks"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }
}
