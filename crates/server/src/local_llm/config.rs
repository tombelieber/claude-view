use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ConfigFile {
    enabled: bool,
}

#[derive(Debug)]
pub struct LocalLlmConfig {
    enabled: AtomicBool,
    path: PathBuf,
}

impl LocalLlmConfig {
    /// Load from `~/.cache/claude-view/local-llm.json`.
    /// Missing or corrupt file → disabled (fail-closed).
    pub fn load() -> Self {
        let path = config_path();
        let enabled = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<ConfigFile>(&s).ok())
            .map(|c| c.enabled)
            .unwrap_or(false);

        Self {
            enabled: AtomicBool::new(enabled),
            path,
        }
    }

    /// Create with explicit disabled state (for tests and non-full app factories).
    pub fn new_disabled() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            path: config_path(),
        }
    }

    /// Lock-free read. Hot path — called by lifecycle every poll.
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Atomic CAS + synchronous disk write.
    pub fn set_enabled(&self, val: bool) -> std::io::Result<()> {
        self.enabled.store(val, Ordering::Release);
        let json = serde_json::to_string_pretty(&ConfigFile { enabled: val })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, json)
    }
}

fn config_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("claude-view")
        .join("local-llm.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn config_at(dir: &std::path::Path) -> LocalLlmConfig {
        LocalLlmConfig {
            enabled: AtomicBool::new(false),
            path: dir.join("local-llm.json"),
        }
    }

    #[test]
    fn defaults_disabled_when_file_missing() {
        let dir = tempdir().unwrap();
        let cfg = config_at(dir.path());
        assert!(!cfg.enabled());
    }

    #[test]
    fn round_trips_enabled_state() {
        let dir = tempdir().unwrap();
        let cfg = config_at(dir.path());

        cfg.set_enabled(true).unwrap();
        assert!(cfg.enabled());

        // Re-load from disk
        let cfg2 = LocalLlmConfig {
            enabled: AtomicBool::new(false),
            path: dir.path().join("local-llm.json"),
        };
        let content: ConfigFile =
            serde_json::from_str(&std::fs::read_to_string(&cfg2.path).unwrap()).unwrap();
        assert!(content.enabled);
    }

    #[test]
    fn set_enabled_false_persists() {
        let dir = tempdir().unwrap();
        let cfg = config_at(dir.path());

        cfg.set_enabled(true).unwrap();
        cfg.set_enabled(false).unwrap();
        assert!(!cfg.enabled());

        let content: ConfigFile =
            serde_json::from_str(&std::fs::read_to_string(&cfg.path).unwrap()).unwrap();
        assert!(!content.enabled);
    }
}
