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

pub fn prompt_index_dir() -> PathBuf {
    data_dir().join("prompt-index")
}

/// Config/state directory for persistent data that must survive cache clears
/// (device keys, workflows, archives). When `CLAUDE_VIEW_DATA_DIR` is set,
/// this returns the same root as `data_dir()` — one env var controls everything.
/// Without override, falls back to `~/.claude-view/`.
pub fn config_dir() -> PathBuf {
    if std::env::var("CLAUDE_VIEW_DATA_DIR").is_ok() {
        data_dir()
    } else {
        dirs::home_dir()
            .expect("no home directory found")
            .join(".claude-view")
    }
}

pub fn crypto_dir() -> PathBuf {
    config_dir()
}

pub fn archive_dir() -> PathBuf {
    config_dir().join("archives")
}

pub fn workflows_official_dir() -> PathBuf {
    config_dir().join("workflows").join("official")
}

pub fn workflows_user_dir() -> PathBuf {
    config_dir().join("workflows").join("user")
}

/// Remove all claude-view cache data (DB, WAL, search index, prompt index).
pub fn remove_cache_data() -> Vec<String> {
    let dir = data_dir();
    let mut removed = Vec::new();

    for name in &["claude-view.db", "claude-view.db-wal", "claude-view.db-shm"] {
        let p = dir.join(name);
        if p.exists() && std::fs::remove_file(&p).is_ok() {
            removed.push(format!("Removed {}", p.display()));
        }
    }

    let idx = dir.join("search-index");
    if idx.exists() && std::fs::remove_dir_all(&idx).is_ok() {
        removed.push(format!("Removed {}", idx.display()));
    }

    let prompt_idx = dir.join("prompt-index");
    if prompt_idx.exists() && std::fs::remove_dir_all(&prompt_idx).is_ok() {
        removed.push(format!("Removed {}", prompt_idx.display()));
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
                if path.extension().map(|e| e == "lock").unwrap_or(false)
                    && std::fs::remove_file(&path).is_ok()
                {
                    removed.push(format!("Removed {}", path.display()));
                }
            }
        }
    }

    // Also clean up legacy /tmp/ lock files from older versions
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("claude-view-")
                && name_str.ends_with(".lock")
                && std::fs::remove_file(entry.path()).is_ok()
            {
                removed.push(format!("Removed legacy {}", entry.path().display()));
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

    #[test]
    fn test_config_dir_follows_data_dir_override() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv-sandbox");
        let dir = config_dir();
        assert_eq!(dir, PathBuf::from("/tmp/test-cv-sandbox"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_config_dir_falls_back_to_home_dot_claude_view() {
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
        let dir = config_dir();
        assert!(
            dir.ends_with(".claude-view"),
            "config_dir should fall back to ~/.claude-view, got: {}",
            dir.display()
        );
    }

    #[test]
    fn test_archive_dir_derives_from_config_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        let path = archive_dir();
        assert_eq!(path, PathBuf::from("/tmp/test-cv/archives"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_workflows_dirs_derive_from_config_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        assert_eq!(
            workflows_official_dir(),
            PathBuf::from("/tmp/test-cv/workflows/official")
        );
        assert_eq!(
            workflows_user_dir(),
            PathBuf::from("/tmp/test-cv/workflows/user")
        );
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }
}
