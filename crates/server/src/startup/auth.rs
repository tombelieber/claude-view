//! Auth bootstrap — Supabase JWKS, share-worker config, PostHog telemetry.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Each sub-bootstrap is
//! independently optional — missing env vars produce a `None` value plus
//! an info log; they do not fail startup.

use std::sync::Arc;

use crate::auth::supabase::{fetch_decoding_key, JwksCache};
use crate::state::ShareConfig;
use crate::telemetry::TelemetryClient;

/// JWKS cache wrapped in an async RwLock for the Supabase auth verifier.
pub type JwksHandle = Arc<tokio::sync::RwLock<JwksCache>>;

/// Fetch the Supabase JWKS if `SUPABASE_URL` is configured. Returns `None`
/// when the URL is absent (dev mode) or when the fetch fails.
pub async fn load_jwks() -> Option<JwksHandle> {
    let url = std::env::var("SUPABASE_URL")
        .ok()
        .or_else(|| option_env!("SUPABASE_URL").map(str::to_string))?;

    match fetch_decoding_key(&url).await {
        Ok(cache) => {
            tracing::info!("Supabase JWKS loaded");
            Some(Arc::new(tokio::sync::RwLock::new(cache)))
        }
        Err(e) => {
            tracing::warn!("Failed to load Supabase JWKS: {e}. Auth will be disabled.");
            None
        }
    }
}

/// Build the share-worker config from env vars. Returns `None` when either
/// `SHARE_WORKER_URL` or `SHARE_VIEWER_URL` is missing.
pub fn load_share_config() -> Option<ShareConfig> {
    let worker_url = std::env::var("SHARE_WORKER_URL")
        .ok()
        .or_else(|| option_env!("SHARE_WORKER_URL").map(str::to_string))?;
    let viewer_url = std::env::var("SHARE_VIEWER_URL")
        .ok()
        .or_else(|| option_env!("SHARE_VIEWER_URL").map(str::to_string))?;

    Some(ShareConfig {
        worker_url,
        viewer_url,
        http_client: reqwest::Client::new(),
    })
}

/// Log when sharing is disabled so the operator can confirm intent.
pub fn log_share_disabled_if_needed(share: &Option<ShareConfig>) {
    if share.is_none() {
        tracing::info!("SHARE_WORKER_URL/SHARE_VIEWER_URL not set — sharing disabled");
    }
}

/// Initialise the PostHog telemetry client using the compile-time key.
///
/// Creates `~/.claude-view/telemetry.toml` on first boot if missing, reads
/// the anon id, and applies the resolved enabled flag. Returns `None` when
/// the compile-time key is empty (dev builds).
pub fn init_telemetry() -> Option<TelemetryClient> {
    let key = option_env!("POSTHOG_API_KEY").filter(|k| !k.is_empty())?;

    let config_path = claude_view_core::telemetry_config::telemetry_config_path();
    let _ = claude_view_core::telemetry_config::create_telemetry_config_if_missing(&config_path);
    let config = claude_view_core::telemetry_config::read_telemetry_config(&config_path);
    let status =
        claude_view_core::telemetry_config::resolve_telemetry_status(Some(key), &config_path);

    let client = TelemetryClient::new(key, &config.anonymous_id);
    if status == claude_view_core::telemetry_config::TelemetryStatus::Enabled {
        client.set_enabled(true);
    }
    Some(client)
}
