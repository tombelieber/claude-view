use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ConfigFile {
    enabled: bool,
    #[serde(default)]
    active_model: Option<String>,
}

#[derive(Debug)]
pub struct LocalLlmConfig {
    enabled: AtomicBool,
    active_model: RwLock<Option<String>>,
    path: PathBuf,
}

impl LocalLlmConfig {
    /// Load from `~/.claude-view/local-llm.json`.
    /// Missing or corrupt file → disabled (fail-closed).
    pub fn load() -> Self {
        let path = config_path();
        let config = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<ConfigFile>(&s).ok());

        Self {
            enabled: AtomicBool::new(config.as_ref().map(|c| c.enabled).unwrap_or(false)),
            active_model: RwLock::new(config.as_ref().and_then(|c| c.active_model.clone())),
            path,
        }
    }

    /// Create with explicit disabled state (for tests and non-full app factories).
    pub fn new_disabled() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            active_model: RwLock::new(None),
            path: config_path(),
        }
    }

    /// Lock-free read. Hot path — called by lifecycle every poll.
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Atomic store + synchronous disk write.
    pub fn set_enabled(&self, val: bool) -> std::io::Result<()> {
        self.enabled.store(val, Ordering::Release);
        self.persist()
    }

    /// Active model ID, or None (callers fall back to registry default).
    pub fn active_model(&self) -> Option<String> {
        self.active_model.read().unwrap().clone()
    }

    pub fn set_active_model(&self, model_id: Option<String>) -> std::io::Result<()> {
        *self.active_model.write().unwrap() = model_id;
        self.persist()
    }

    fn persist(&self) -> std::io::Result<()> {
        let file = ConfigFile {
            enabled: self.enabled.load(Ordering::Acquire),
            active_model: self.active_model.read().unwrap().clone(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, json)
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude-view")
        .join("local-llm.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn config_at(dir: &std::path::Path) -> LocalLlmConfig {
        LocalLlmConfig {
            enabled: AtomicBool::new(false),
            active_model: RwLock::new(None),
            path: dir.join("local-llm.json"),
        }
    }

    #[test]
    fn defaults_disabled_when_file_missing() {
        let dir = tempdir().unwrap();
        let cfg = config_at(dir.path());
        assert!(!cfg.enabled());
        assert!(cfg.active_model().is_none());
    }

    #[test]
    fn round_trips_enabled_state() {
        let dir = tempdir().unwrap();
        let cfg = config_at(dir.path());

        cfg.set_enabled(true).unwrap();
        assert!(cfg.enabled());

        // Verify persisted to disk
        let content: ConfigFile =
            serde_json::from_str(&std::fs::read_to_string(&cfg.path).unwrap()).unwrap();
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

    #[test]
    fn active_model_round_trips() {
        let dir = tempdir().unwrap();
        let cfg = config_at(dir.path());

        assert!(cfg.active_model().is_none());

        cfg.set_active_model(Some("qwen3-8b-mlx-4bit".into()))
            .unwrap();
        assert_eq!(cfg.active_model().as_deref(), Some("qwen3-8b-mlx-4bit"));

        // Verify persisted to disk
        let content: ConfigFile =
            serde_json::from_str(&std::fs::read_to_string(&cfg.path).unwrap()).unwrap();
        assert_eq!(content.active_model.as_deref(), Some("qwen3-8b-mlx-4bit"));
    }

    #[test]
    fn persist_writes_both_fields() {
        let dir = tempdir().unwrap();
        let cfg = config_at(dir.path());

        cfg.set_enabled(true).unwrap();
        cfg.set_active_model(Some("test-model".into())).unwrap();

        let content: ConfigFile =
            serde_json::from_str(&std::fs::read_to_string(&cfg.path).unwrap()).unwrap();
        assert!(content.enabled);
        assert_eq!(content.active_model.as_deref(), Some("test-model"));
    }
}
