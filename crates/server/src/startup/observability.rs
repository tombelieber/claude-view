//! TLS crypto provider install + observability (tracing, Sentry) init.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Both calls must run before
//! any network I/O, so they live at the very top of the runtime entry
//! point. Cargo feature unification compiles both `ring` and `aws-lc-rs`
//! into the binary; rustls 0.23+ panics at runtime without an explicit
//! provider selection.

use anyhow::Result;

/// Install the `aws-lc-rs` rustls crypto provider as the process default.
///
/// Idempotent — returns `Err(_)` if another provider is already installed
/// (we ignore that outcome to keep `cargo test` friendly).
pub fn install_tls_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

/// Initialise unified observability (structured JSON file + dev stderr +
/// optional Sentry) and return the handle.
///
/// Dropping the returned handle stops tracing, so keep it alive for the
/// entire process lifetime (bind it in `main` and leak via `_obs_handle`).
pub fn init_tracing() -> Result<claude_view_observability::ObservabilityHandle> {
    let mut obs_cfg = claude_view_observability::ServiceConfig::new(
        "claude-view-server",
        env!("CARGO_PKG_VERSION"),
    );
    obs_cfg.sentry_dsn = claude_view_observability::sentry_integration::load_dsn();
    claude_view_observability::init(obs_cfg).map_err(|e| anyhow::anyhow!("observability init: {e}"))
}

/// Load `~/.claude-view/config.toml` into an `AppConfig` and log it.
pub fn load_app_config() -> claude_view_core::app_config::AppConfig {
    let app_config = claude_view_core::app_config::AppConfig::load();
    tracing::info!(?app_config, "app.config.loaded");
    app_config
}
