use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum TelemetryStatus {
    Undecided,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub enabled: Option<bool>,
    pub anonymous_id: String,
    pub consent_given_at: Option<String>,
    pub last_milestone: Option<u64>,
    #[serde(default)]
    pub first_index_completed: bool,
}

impl TelemetryConfig {
    pub fn new_undecided() -> Self {
        Self {
            enabled: None,
            anonymous_id: Uuid::new_v4().to_string(),
            consent_given_at: None,
            last_milestone: None,
            first_index_completed: false,
        }
    }
}

/// Returns the telemetry config path.
///
/// When `CLAUDE_VIEW_DATA_DIR` is set (e.g., enterprise/DACS sandbox where
/// `~/.claude-view/` is read-only), uses `$CLAUDE_VIEW_DATA_DIR/telemetry.json`.
/// Otherwise defaults to `~/.claude-view/telemetry.json` (user-scoped).
pub fn telemetry_config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("CLAUDE_VIEW_DATA_DIR") {
        return PathBuf::from(dir).join("telemetry.json");
    }
    dirs::home_dir()
        .expect("home directory must exist")
        .join(".claude-view")
        .join("telemetry.json")
}

pub fn read_telemetry_config(path: &Path) -> TelemetryConfig {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            serde_json::from_str(&contents).unwrap_or_else(|_| TelemetryConfig::new_undecided())
        }
        Err(_) => TelemetryConfig::new_undecided(),
    }
}

pub fn write_telemetry_config(path: &Path, config: &TelemetryConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, serde_json::to_string_pretty(config).unwrap())?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

pub fn create_telemetry_config_if_missing(path: &Path) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            let config = TelemetryConfig::new_undecided();
            let json = serde_json::to_string_pretty(&config).unwrap();
            file.write_all(json.as_bytes())?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn resolve_telemetry_status(api_key: Option<&str>, config_path: &Path) -> TelemetryStatus {
    let key = match api_key {
        Some(k) if !k.is_empty() => k,
        _ => return TelemetryStatus::Disabled,
    };
    let _ = key;
    if std::env::var("CLAUDE_VIEW_TELEMETRY").ok().as_deref() == Some("0") {
        return TelemetryStatus::Disabled;
    }
    if std::env::var("CI").ok().as_deref() == Some("true") {
        return TelemetryStatus::Disabled;
    }
    let config = read_telemetry_config(config_path);
    match config.enabled {
        Some(true) => TelemetryStatus::Enabled,
        Some(false) => TelemetryStatus::Disabled,
        None => TelemetryStatus::Undecided,
    }
}

const MILESTONES: &[u64] = &[10, 50, 100, 500, 1000, 5000];

pub fn check_milestone(session_count: u64, last_milestone: u64) -> Option<u64> {
    MILESTONES
        .iter()
        .rev()
        .find(|&&m| session_count >= m && m > last_milestone)
        .copied()
}
