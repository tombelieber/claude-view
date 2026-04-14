use serde::Deserialize;

/// Holds the Sentry client alive -- dropping flushes + shuts down.
/// The inner field is never read; its Drop impl does the work.
#[allow(dead_code)]
pub struct SentryGuard(pub(crate) sentry::ClientInitGuard);

/// Consent file schema: `~/.claude-view/telemetry-consent.json`
#[derive(Deserialize)]
struct Consent {
    #[serde(default)]
    sentry_enabled: bool,
    #[serde(default)]
    sentry_dsn: Option<String>,
}

/// Resolve a Sentry DSN from (in priority order):
/// 1. `CLAUDE_VIEW_SENTRY_DSN` env var
/// 2. `~/.claude-view/telemetry-consent.json` (opt-in file)
pub fn load_dsn() -> Option<String> {
    // Priority 1: env var
    if let Ok(dsn) = std::env::var("CLAUDE_VIEW_SENTRY_DSN") {
        if !dsn.is_empty() {
            return Some(dsn);
        }
    }

    // Priority 2: consent file
    let path = claude_view_core::paths::config_dir().join("telemetry-consent.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let consent: Consent = serde_json::from_str(&content).ok()?;
    if consent.sentry_enabled {
        consent.sentry_dsn
    } else {
        None
    }
}

/// Initialize Sentry if a DSN is available and non-empty.
///
/// Returns `Some(SentryGuard)` when Sentry is active -- the guard MUST be held
/// for the process lifetime (dropping it flushes and shuts down the client).
pub fn init_if_enabled(
    dsn: Option<String>,
    service_name: &'static str,
    mode: &crate::config::DeploymentMode,
) -> Option<SentryGuard> {
    let dsn = dsn.filter(|s| !s.is_empty())?;

    let env = match mode {
        crate::config::DeploymentMode::Dev => "development",
        crate::config::DeploymentMode::NpxInstall => "production",
        crate::config::DeploymentMode::DockerImage => "docker",
    };

    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some(service_name.into()),
            environment: Some(env.into()),
            traces_sample_rate: 0.05,
            ..Default::default()
        },
    ));

    Some(SentryGuard(guard))
}
