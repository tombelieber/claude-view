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
    /// Set true once the one-time `installed` ("acquired") event has been
    /// emitted for this persistent `anonymous_id`. Guards against re-firing
    /// when consent is toggled off then on again.
    #[serde(default)]
    pub install_reported: bool,
    /// ISO-8601 timestamp the one-time terminal privacy notice was shown.
    /// `None` = not shown yet (print once on a default-on official build,
    /// then stamp so it never repeats).
    #[serde(default)]
    pub notice_shown_at: Option<String>,
    /// UTC date (`YYYY-MM-DD`) of the last `app_active` heartbeat. Dedupes
    /// the daily-active event to once per calendar day per install.
    #[serde(default)]
    pub last_active_date: Option<String>,
    /// `FeatureId` of the first feature ever opened on this install — the
    /// activation "aha". Set once, never overwritten.
    #[serde(default)]
    pub first_feature_used: Option<String>,
}

impl TelemetryConfig {
    pub fn new_undecided() -> Self {
        Self {
            enabled: None,
            anonymous_id: Uuid::new_v4().to_string(),
            consent_given_at: None,
            last_milestone: None,
            first_index_completed: false,
            install_reported: false,
            notice_shown_at: None,
            last_active_date: None,
            first_feature_used: None,
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

/// Pure telemetry-status resolver — the complete state table, no I/O, no
/// globals, no env reads. [`resolve_telemetry_status`] is the impure shell
/// that gathers env + the consent file and delegates here; testing this
/// directly pins every row of the table without env-var races.
///
/// **Default-on:** an *official* build (compile-time `POSTHOG_API_KEY`
/// present) with no explicit user choice resolves to `Enabled`. Source
/// builds (no key) and the CI / `CLAUDE_VIEW_TELEMETRY=0` kill-switch
/// resolve to `Disabled`. An explicit `Some(false)` opt-out is honored
/// permanently and outranks the default. The kill-switch / CI / no-key
/// gates outrank an explicit opt-in.
pub fn resolve_status_pure(
    api_key: Option<&str>,
    consent: Option<bool>,
    kill_switch: bool,
    is_ci: bool,
) -> TelemetryStatus {
    match api_key {
        Some(k) if !k.is_empty() => {}
        // No compile-time key ⇒ built from source ⇒ telemetry impossible.
        _ => return TelemetryStatus::Disabled,
    }
    if kill_switch || is_ci {
        return TelemetryStatus::Disabled;
    }
    match consent {
        Some(false) => TelemetryStatus::Disabled,
        Some(true) => TelemetryStatus::Enabled,
        // No explicit choice on an official build → ON by default.
        None => TelemetryStatus::Enabled,
    }
}

pub fn resolve_telemetry_status(api_key: Option<&str>, config_path: &Path) -> TelemetryStatus {
    let kill_switch = std::env::var("CLAUDE_VIEW_TELEMETRY").ok().as_deref() == Some("0");
    let is_ci = std::env::var("CI").ok().as_deref() == Some("true");
    // Skip the consent-file read entirely on source builds (no key) — keeps
    // resolution allocation/IO-free in the dominant self-host path.
    let consent = match api_key {
        Some(k) if !k.is_empty() => read_telemetry_config(config_path).enabled,
        _ => None,
    };
    resolve_status_pure(api_key, consent, kill_switch, is_ci)
}

const MILESTONES: &[u64] = &[10, 50, 100, 500, 1000, 5000];

pub fn check_milestone(session_count: u64, last_milestone: u64) -> Option<u64> {
    MILESTONES
        .iter()
        .rev()
        .find(|&&m| session_count >= m && m > last_milestone)
        .copied()
}
