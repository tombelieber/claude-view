//! Runtime configuration loaded from `~/.claude-view/config.toml`.

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub features: FeatureFlags,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FeatureFlags {
    pub system_monitor: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            system_monitor: true,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = crate::paths::data_dir().join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
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
    fn default_has_system_monitor_on() {
        let config = AppConfig::default();
        assert!(config.features.system_monitor);
    }

    #[test]
    fn parse_empty_returns_defaults() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert!(config.features.system_monitor);
    }

    #[test]
    fn parse_partial_features() {
        let config: AppConfig = toml::from_str("[features]\nsearch = false").unwrap();
        assert!(config.features.system_monitor);
    }

    #[test]
    #[serial_test::serial]
    fn load_ignores_removed_search_feature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "[features]\nsearch = true\nsystem_monitor = true\n",
        )
        .unwrap();
        let _guard = EnvGuard::set("CLAUDE_VIEW_DATA_DIR", dir.path());
        let config = AppConfig::load();

        assert!(config.features.system_monitor);
    }

    #[test]
    fn parse_all_off() {
        let config: AppConfig =
            toml::from_str("[features]\nsearch = false\nsystem_monitor = false").unwrap();
        assert!(!config.features.system_monitor);
    }
}
