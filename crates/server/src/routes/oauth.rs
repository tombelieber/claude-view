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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthIdentityResponse {
    pub has_auth: bool,
    pub email: Option<String>,
    pub org_name: Option<String>,
    pub subscription_type: Option<String>,
    pub auth_method: Option<String>,
}

// ── Parsed output of `claude auth status --json` ────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeAuthStatusOutput {
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    org_name: Option<String>,
    #[serde(default)]
    subscription_type: Option<String>,
    #[serde(default)]
    auth_method: Option<String>,
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
    /// Cents (API returns float, e.g. 5125.0). Null when disabled.
    #[serde(default)]
    used_credits: Option<f64>,
    /// Cents cap (0 = unlimited; API returns float). Null when disabled.
    #[serde(default)]
    monthly_limit: Option<f64>,
    /// 0–100 percentage
    #[serde(default)]
    utilization: Option<f64>,
}

// ── Constants ───────────────────────────────────────────────────────────

const ANTHROPIC_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// Timeout for the `claude auth status` subprocess.
const AUTH_STATUS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

// ── Helpers ─────────────────────────────────────────────────────────────

fn no_auth() -> OAuthUsageResponse {
    OAuthUsageResponse {
        has_auth: false,
        error: None,
        plan: None,
        tiers: vec![],
    }
}

fn auth_error(msg: impl Into<String>) -> OAuthUsageResponse {
    OAuthUsageResponse {
        has_auth: true,
        error: Some(msg.into()),
        plan: None,
        tiers: vec![],
    }
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
            let used_credits = extra.used_credits.unwrap_or(0.0);
            let monthly_limit = extra.monthly_limit.unwrap_or(0.0);
            let used_dollars = used_credits / 100.0;
            let limit_dollars = monthly_limit / 100.0;
            // Use API-provided utilization if available, else compute
            let pct = extra.utilization.unwrap_or_else(|| {
                if monthly_limit > 0.0 {
                    ((used_credits / monthly_limit) * 100.0).min(100.0)
                } else {
                    0.0
                }
            });
            let spent = if monthly_limit > 0.0 {
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
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(format!("API error {status}: {body}"));
    }

    tracing::debug!(body = %body, "Anthropic usage API raw response");

    serde_json::from_str::<AnthropicUsageResponse>(&body).map_err(|e| {
        tracing::warn!(error = %e, body = %body, "Failed to parse Anthropic usage response");
        format!("Parse error: {e}")
    })
}

/// Run `claude auth status --json` and parse the result.
/// Returns `None` on any failure (CLI missing, timeout, parse error).
fn fetch_auth_identity() -> Option<crate::state::AuthIdentity> {
    let cli_path = claude_view_core::resolved_cli_path()?;

    // Strip CLAUDE* env vars to prevent SIGKILL inside Claude Code sessions.
    let claude_vars: Vec<String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("CLAUDE"))
        .map(|(k, _)| k)
        .collect();

    let mut cmd = std::process::Command::new(cli_path);
    cmd.args(["auth", "status", "--json"]);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    for var in &claude_vars {
        cmd.env_remove(var);
    }

    // Spawn with timeout to prevent indefinite blocking.
    let mut child = cmd.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    tracing::debug!("claude auth status exited with {}", status);
                    return None;
                }
                break;
            }
            Ok(None) => {
                if start.elapsed() > AUTH_STATUS_TIMEOUT {
                    let _ = child.kill();
                    tracing::warn!(
                        "claude auth status timed out after {:?}",
                        AUTH_STATUS_TIMEOUT
                    );
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                tracing::debug!("claude auth status wait error: {e}");
                return None;
            }
        }
    }

    let output = child.wait_with_output().ok()?;

    let parsed: ClaudeAuthStatusOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                error = %e,
                stdout = %String::from_utf8_lossy(&output.stdout),
                "Failed to parse claude auth status JSON — command may not support --json"
            );
            return None;
        }
    };

    if !parsed.logged_in {
        return None;
    }

    Some(crate::state::AuthIdentity {
        email: parsed.email,
        org_name: parsed.org_name,
        subscription_type: parsed.subscription_type,
        auth_method: parsed.auth_method,
    })
}

// ── Handlers ────────────────────────────────────────────────────────────

/// GET /api/oauth/identity
///
/// Returns cached auth identity (email, org, plan).
/// Calls `claude auth status` on first request only, caches forever.
pub async fn get_auth_identity(State(state): State<Arc<AppState>>) -> Json<AuthIdentityResponse> {
    let identity = state
        .auth_identity
        .get_or_init(|| async {
            // Run subprocess in blocking task to avoid blocking the tokio runtime.
            tokio::task::spawn_blocking(fetch_auth_identity)
                .await
                .ok()
                .flatten()
        })
        .await;

    match identity {
        Some(id) => Json(AuthIdentityResponse {
            has_auth: true,
            email: id.email.clone(),
            org_name: id.org_name.clone(),
            subscription_type: id.subscription_type.clone(),
            auth_method: id.auth_method.clone(),
        }),
        None => Json(AuthIdentityResponse {
            has_auth: false,
            email: None,
            org_name: None,
            subscription_type: None,
            auth_method: None,
        }),
    }
}

/// GET /api/oauth/usage
pub async fn get_oauth_usage(State(_state): State<Arc<AppState>>) -> Json<OAuthUsageResponse> {
    // 1. Read credentials (file → keychain fallback).
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Json(no_auth()),
    };

    let creds_bytes = match claude_view_core::credentials::load_credentials_bytes(&home) {
        Some(b) => b,
        None => return Json(no_auth()),
    };

    let oauth = match claude_view_core::credentials::parse_credentials(&creds_bytes) {
        Some(o) => o,
        None => return Json(no_auth()),
    };

    // 2. Check expiry — we never refresh, just report the error.
    if claude_view_core::credentials::is_token_expired(oauth.expires_at) {
        return Json(auth_error(
            "Token expired. Run 'claude' to re-authenticate.",
        ));
    }

    let plan = oauth.subscription_type.as_deref().map(|s| {
        let mut c = s.chars();
        match c.next() {
            Some(first) => first.to_uppercase().to_string() + c.as_str(),
            None => s.to_string(),
        }
    });

    // 3. Fetch usage with the current token (no refresh, no retry).
    let client = reqwest::Client::new();
    let result = fetch_usage(&client, &oauth.access_token).await;

    let usage = match result {
        Ok(u) => u,
        Err(e) if e.contains("401") => {
            return Json(auth_error(
                "Token expired. Run 'claude' to re-authenticate.",
            ));
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
    Router::new()
        .route("/oauth/usage", get(get_oauth_usage))
        .route("/oauth/identity", get(get_auth_identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_identity_endpoint_returns_cached_identity() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        // Pre-populate the OnceCell with a known identity.
        state
            .auth_identity
            .get_or_init(|| async {
                Some(crate::state::AuthIdentity {
                    email: Some("test@example.com".into()),
                    org_name: Some("Test Corp".into()),
                    subscription_type: Some("max".into()),
                    auth_method: Some("claude.ai".into()),
                })
            })
            .await;

        let app = Router::new()
            .route("/api/oauth/identity", axum::routing::get(get_auth_identity))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/oauth/identity")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: AuthIdentityResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(body.has_auth);
        assert_eq!(body.email.as_deref(), Some("test@example.com"));
        assert_eq!(body.org_name.as_deref(), Some("Test Corp"));
    }

    #[tokio::test]
    async fn test_identity_endpoint_returns_no_auth_when_empty() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        // Pre-populate with None (no identity).
        state.auth_identity.get_or_init(|| async { None }).await;

        let app = Router::new()
            .route("/api/oauth/identity", axum::routing::get(get_auth_identity))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/oauth/identity")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: AuthIdentityResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(!body.has_auth);
        assert!(body.email.is_none());
    }
}
