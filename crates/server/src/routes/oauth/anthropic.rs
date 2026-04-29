//! Anthropic upstream API types + fetcher.
//!
//! `GET https://api.anthropic.com/api/oauth/usage` — undocumented, requires
//! `anthropic-beta: oauth-2025-04-20`. The response shape evolves as Anthropic
//! ships new model pools; we capture what we know and flatten the rest.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

const ANTHROPIC_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// Top-level response from Anthropic.
///
/// **Forward compat:** explicit fields cover every tier we have a curated
/// label for. Anything else (`seven_day_omelette`, `iguana_necktie`, future
/// codenames) is captured by `#[serde(flatten)] extra_windows`. Trust gate
/// in [`super::tiers`] decides whether to surface them.
#[derive(Debug, Default, Deserialize)]
pub struct AnthropicUsageResponse {
    #[serde(default)]
    pub five_hour: Option<RateLimitWindow>,
    #[serde(default)]
    pub seven_day: Option<RateLimitWindow>,
    #[serde(default)]
    pub seven_day_opus: Option<RateLimitWindow>,
    #[serde(default)]
    pub seven_day_sonnet: Option<RateLimitWindow>,
    #[serde(default)]
    pub seven_day_oauth_apps: Option<RateLimitWindow>,
    #[serde(default)]
    pub extra_usage: Option<ExtraUsage>,
    /// Catch-all for tier names we haven't promoted to first-class fields.
    /// Stored as raw `Value` so a future malformed entry can't fail the whole
    /// parse — the conversion to `RateLimitWindow` happens lazily in `tiers`.
    #[serde(flatten)]
    pub extra_windows: HashMap<String, Value>,
}

/// One rate-limit window (5h, 7d, 7d-opus, …).
///
/// `utilization` is `Option<f64>` deliberately. Upstream has been observed to
/// send `null` for windows that exist in the schema but aren't tracked for
/// the account. Treating it as `f64` (as the previous code did) made the
/// whole response a parse-failure on a single null — see the trust gate in
/// [`super::tiers::build_tiers`] for how `None` is handled.
#[derive(Debug, Default, Deserialize)]
pub struct RateLimitWindow {
    #[serde(default)]
    pub utilization: Option<f64>,
    #[serde(default)]
    pub resets_at: Option<String>,
}

/// Pay-as-you-go credit bucket.
#[derive(Debug, Default, Deserialize)]
pub struct ExtraUsage {
    #[serde(default)]
    pub is_enabled: bool,
    /// Cents (API returns float, e.g. 5125.0). `None` when disabled.
    #[serde(default)]
    pub used_credits: Option<f64>,
    /// Cents cap (0 = unlimited). `None` when disabled.
    #[serde(default)]
    pub monthly_limit: Option<f64>,
    /// 0–100 percentage. `None` when disabled.
    #[serde(default)]
    pub utilization: Option<f64>,
    /// ISO 4217 code (added by upstream around 2026-04). `None` when disabled
    /// or on older accounts.
    #[serde(default)]
    pub currency: Option<String>,
}

/// Fetch usage from Anthropic with the caller-supplied OAuth access token.
pub async fn fetch_usage(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<AnthropicUsageResponse, String> {
    let resp = client
        .get(ANTHROPIC_USAGE_URL)
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    let status = resp.status();
    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "failed to read Anthropic API response body");
            String::new()
        }
    };

    if !status.is_success() {
        return Err(format!("API error {status}: {body}"));
    }

    tracing::debug!(body = %body, "Anthropic usage API raw response");

    serde_json::from_str::<AnthropicUsageResponse>(&body).map_err(|e| {
        tracing::warn!(error = %e, body = %body, "Failed to parse Anthropic usage response");
        format!("Parse error: {e}")
    })
}
