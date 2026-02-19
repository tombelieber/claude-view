//! OAuth usage endpoint — reads Claude Code credentials and fetches usage
//! from the Anthropic API.
//!
//! API details reverse-engineered from the OpenUsage project.
//! Endpoint is undocumented and uses `anthropic-beta: oauth-2025-04-20`.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ── Response types (sent to frontend) ───────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTier {
    pub id: String,
    pub label: String,
    /// 0.0–100.0
    pub percentage: f64,
    /// ISO-8601 reset timestamp
    pub reset_at: String,
    /// Dollar description for budget tiers
    pub spent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUsageResponse {
    pub has_auth: bool,
    pub error: Option<String>,
    pub plan: Option<String>,
    pub tiers: Vec<UsageTier>,
}

// ── Credential file types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsFile {
    claude_ai_oauth: Option<OAuthCredential>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthCredential {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    /// Unix milliseconds
    #[serde(default)]
    expires_at: Option<u64>,
    #[serde(default)]
    subscription_type: Option<String>,
}

// ── Anthropic API response types ────────────────────────────────────────

/// GET https://api.anthropic.com/api/oauth/usage
#[derive(Debug, Deserialize)]
struct AnthropicUsageResponse {
    #[serde(default)]
    five_hour: Option<RateLimitWindow>,
    #[serde(default)]
    seven_day: Option<RateLimitWindow>,
    #[serde(default)]
    seven_day_sonnet: Option<RateLimitWindow>,
    #[serde(default)]
    extra_usage: Option<ExtraUsage>,
}

#[derive(Debug, Deserialize)]
struct RateLimitWindow {
    /// 0–100 percentage (API returns float, e.g. 28.0)
    utilization: f64,
    #[serde(default)]
    resets_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtraUsage {
    #[serde(default)]
    is_enabled: bool,
    /// Cents (API returns float, e.g. 5125.0)
    #[serde(default)]
    used_credits: f64,
    /// Cents cap (0 = unlimited; API returns float)
    #[serde(default)]
    monthly_limit: f64,
    /// 0–100 percentage
    #[serde(default)]
    utilization: Option<f64>,
}

/// POST https://platform.claude.com/v1/oauth/token
#[derive(Debug, Deserialize)]
struct TokenRefreshResponse {
    access_token: String,
    #[serde(default)]
    #[allow(dead_code)]
    refresh_token: Option<String>,
    /// Seconds until expiry
    #[serde(default)]
    #[allow(dead_code)]
    expires_in: Option<u64>,
}

// ── Constants ───────────────────────────────────────────────────────────

const ANTHROPIC_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const TOKEN_REFRESH_URL: &str = "https://platform.claude.com/v1/oauth/token";
const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const OAUTH_SCOPE: &str = "user:profile user:inference user:sessions:claude_code user:mcp_servers";
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
/// Refresh proactively if token expires within 5 minutes.
const REFRESH_BUFFER_MS: u64 = 5 * 60 * 1000;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Read credentials JSON from macOS Keychain.
///
/// The `security find-generic-password -s <service> -w` command returns the
/// password value. On some macOS versions this comes back as hex-encoded
/// UTF-8 bytes (e.g. "7b0a22..." for `{\n"`), so we try plain text first,
/// then hex decode.
fn read_keychain_credentials() -> Option<Vec<u8>> {
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("security")
            .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if raw.is_empty() {
            return None;
        }

        // Try plain JSON first.
        if raw.starts_with('{') {
            return Some(raw.into_bytes());
        }

        // Try hex-decoding (macOS keychain sometimes returns hex-encoded UTF-8).
        let hex = raw.strip_prefix("0x").or(raw.strip_prefix("0X")).unwrap_or(&raw);
        if hex.len() % 2 != 0 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect();
        if bytes.is_empty() || bytes[0] != b'{' {
            return None;
        }
        Some(bytes)
    }
}

/// Load credentials from file, falling back to macOS Keychain.
fn load_credentials_bytes(home: &std::path::Path) -> Option<Vec<u8>> {
    let creds_path = home.join(".claude").join(".credentials.json");

    // Try file first.
    if let Ok(bytes) = std::fs::read(&creds_path) {
        tracing::debug!("Loaded credentials from file");
        return Some(bytes);
    }

    // Fallback: macOS Keychain.
    if let Some(bytes) = read_keychain_credentials() {
        tracing::debug!("Loaded credentials from macOS Keychain");
        return Some(bytes);
    }

    tracing::debug!("No credentials found (file or keychain)");
    None
}

fn no_auth() -> OAuthUsageResponse {
    OAuthUsageResponse { has_auth: false, error: None, plan: None, tiers: vec![] }
}

fn auth_error(msg: impl Into<String>) -> OAuthUsageResponse {
    OAuthUsageResponse { has_auth: true, error: Some(msg.into()), plan: None, tiers: vec![] }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Build the usage tiers from the Anthropic API response.
fn build_tiers(resp: &AnthropicUsageResponse) -> Vec<UsageTier> {
    let mut tiers = Vec::with_capacity(4);

    if let Some(w) = &resp.five_hour {
        tiers.push(UsageTier {
            id: "session".into(),
            label: "Session (5hr)".into(),
            percentage: w.utilization,
            reset_at: w.resets_at.clone().unwrap_or_default(),
            spent: None,
        });
    }

    if let Some(w) = &resp.seven_day {
        tiers.push(UsageTier {
            id: "weekly".into(),
            label: "Weekly (7 day)".into(),
            percentage: w.utilization,
            reset_at: w.resets_at.clone().unwrap_or_default(),
            spent: None,
        });
    }

    if let Some(w) = &resp.seven_day_sonnet {
        tiers.push(UsageTier {
            id: "weekly_sonnet".into(),
            label: "Weekly Sonnet".into(),
            percentage: w.utilization,
            reset_at: w.resets_at.clone().unwrap_or_default(),
            spent: None,
        });
    }

    if let Some(extra) = &resp.extra_usage {
        if extra.is_enabled {
            let used_dollars = extra.used_credits / 100.0;
            let limit_dollars = extra.monthly_limit / 100.0;
            // Use API-provided utilization if available, else compute
            let pct = extra.utilization.unwrap_or_else(|| {
                if extra.monthly_limit > 0.0 {
                    ((extra.used_credits / extra.monthly_limit) * 100.0).min(100.0)
                } else {
                    0.0
                }
            });
            let spent = if extra.monthly_limit > 0.0 {
                format!("${:.2} / ${:.2} spent", used_dollars, limit_dollars)
            } else {
                format!("${:.2} spent (no limit)", used_dollars)
            };
            tiers.push(UsageTier {
                id: "extra".into(),
                label: "Extra usage".into(),
                percentage: pct,
                reset_at: String::new(), // extra_usage has no resets_at
                spent: Some(spent),
            });
        }
    }

    tiers
}

/// Try to refresh the OAuth token. Returns a new access token on success.
async fn try_refresh_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Option<String> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": OAUTH_CLIENT_ID,
        "scope": OAUTH_SCOPE,
    });

    let resp = client
        .post(TOKEN_REFRESH_URL)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "Token refresh failed");
        return None;
    }

    let data: TokenRefreshResponse = resp.json().await.ok()?;
    // TODO: persist refreshed token back to credentials file
    Some(data.access_token)
}

/// Fetch usage from the Anthropic API with the given access token.
async fn fetch_usage(
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
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("API error {status}: {body}"));
    }

    resp.json::<AnthropicUsageResponse>()
        .await
        .map_err(|e| format!("Parse error: {e}"))
}

// ── Handler ─────────────────────────────────────────────────────────────

/// GET /api/oauth/usage
pub async fn get_oauth_usage(
    State(_state): State<Arc<AppState>>,
) -> Json<OAuthUsageResponse> {
    // 1. Read credentials (file → keychain fallback).
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Json(no_auth()),
    };

    let creds_bytes = match load_credentials_bytes(&home) {
        Some(b) => b,
        None => return Json(no_auth()),
    };

    let creds_file: CredentialsFile = match serde_json::from_slice(&creds_bytes) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to parse credentials");
            return Json(no_auth());
        }
    };

    let oauth = match creds_file.claude_ai_oauth {
        Some(o) if !o.access_token.is_empty() => o,
        _ => return Json(no_auth()),
    };

    let plan = oauth
        .subscription_type
        .as_deref()
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                Some(first) => first.to_uppercase().to_string() + c.as_str(),
                None => s.to_string(),
            }
        });

    // 2. Refresh token if expiring soon.
    let client = reqwest::Client::new();
    let access_token = if let Some(expires_at) = oauth.expires_at {
        if expires_at <= now_ms() + REFRESH_BUFFER_MS {
            if let Some(ref rt) = oauth.refresh_token {
                match try_refresh_token(&client, rt).await {
                    Some(new_token) => new_token,
                    None => oauth.access_token.clone(),
                }
            } else {
                oauth.access_token.clone()
            }
        } else {
            oauth.access_token.clone()
        }
    } else {
        oauth.access_token.clone()
    };

    // 3. Fetch usage (retry once on 401 with token refresh).
    let result = fetch_usage(&client, &access_token).await;

    let usage = match result {
        Ok(u) => u,
        Err(e) if e.contains("401") => {
            // Token might be stale — try one refresh.
            if let Some(ref rt) = oauth.refresh_token {
                if let Some(new_token) = try_refresh_token(&client, rt).await {
                    match fetch_usage(&client, &new_token).await {
                        Ok(u) => u,
                        Err(e2) => return Json(auth_error(e2)),
                    }
                } else {
                    return Json(auth_error("Token expired. Run 'claude' to log in again."));
                }
            } else {
                return Json(auth_error(e));
            }
        }
        Err(e) => return Json(auth_error(e)),
    };

    let tiers = build_tiers(&usage);

    Json(OAuthUsageResponse {
        has_auth: true,
        error: None,
        plan,
        tiers,
    })
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/oauth/usage", get(get_oauth_usage))
}
