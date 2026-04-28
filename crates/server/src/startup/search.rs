//! Obsolete session search index cleanup.
//!
//! Session search runs directly over Claude JSONL files with ripgrep-core.
//! On startup, remove the old session-search cache so updated installs
//! automatically reclaim disk and stop old file-watcher noise.

/// Remove the obsolete session-search cache and log the migration outcome.
pub fn cleanup_obsolete_session_index_logged() {
    match cleanup_obsolete_session_index() {
        Ok(bytes) if bytes > 0 => {
            tracing::info!(
                bytes,
                "Removed obsolete session search cache; session search is grep-only"
            );
        }
        Ok(_) => {
            tracing::info!("Session search is grep-only; no obsolete cache cleanup needed");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Failed to remove obsolete session search cache"
            );
        }
    }
}

/// Remove the obsolete session-search cache directory and return bytes reclaimed.
/// Missing directories are treated as already migrated.
pub fn cleanup_obsolete_session_index() -> std::io::Result<u64> {
    let Some(index_dir) = claude_view_core::paths::obsolete_session_search_index_dir() else {
        return Ok(0);
    };

    if !index_dir.exists() {
        return Ok(0);
    }

    let bytes = calculate_dir_size(&index_dir);
    std::fs::remove_dir_all(index_dir)?;
    Ok(bytes)
}

fn calculate_dir_size(dir: &std::path::Path) -> u64 {
    if !dir.exists() {
        return 0;
    }

    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                total += calculate_dir_size(&path);
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn cleanup_obsolete_session_index_removes_legacy_directory() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CLAUDE_VIEW_DATA_DIR", dir.path());
        let index_dir = claude_view_core::paths::obsolete_session_search_index_dir().unwrap();
        std::fs::create_dir_all(index_dir.join("v7")).unwrap();
        std::fs::write(index_dir.join("v7").join("meta.json"), "legacy").unwrap();

        let bytes = cleanup_obsolete_session_index().unwrap();

        assert!(bytes > 0);
        assert!(!index_dir.exists());
    }

    #[test]
    #[serial_test::serial]
    fn cleanup_obsolete_session_index_ignores_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CLAUDE_VIEW_DATA_DIR", dir.path());
        let index_dir = claude_view_core::paths::obsolete_session_search_index_dir().unwrap();

        let bytes = cleanup_obsolete_session_index().unwrap();

        assert_eq!(bytes, 0);
        assert!(!index_dir.exists());
    }
}
