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
    pub search: bool,
    pub system_monitor: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            search: true,
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

    #[test]
    fn default_has_all_features_on() {
        let config = AppConfig::default();
        assert!(config.features.search);
        assert!(config.features.system_monitor);
    }

    #[test]
    fn parse_empty_returns_defaults() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert!(config.features.search);
        assert!(config.features.system_monitor);
    }

    #[test]
    fn parse_partial_features() {
        let config: AppConfig = toml::from_str("[features]\nsearch = false").unwrap();
        assert!(!config.features.search);
        assert!(config.features.system_monitor);
    }

    #[test]
    fn parse_all_off() {
        let config: AppConfig =
            toml::from_str("[features]\nsearch = false\nsystem_monitor = false").unwrap();
        assert!(!config.features.search);
        assert!(!config.features.system_monitor);
    }
}
