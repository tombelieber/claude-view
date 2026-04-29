//! Axum handlers for the `/api/oauth/usage` endpoints — credentials lookup,
//! upstream fetch, and `CachedUpstream` integration.

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, Json};

use crate::cache::CacheError;
use crate::state::AppState;

use super::anthropic::fetch_usage;
use super::tiers::{auth_error, build_tiers, no_auth, OAuthUsageResponse};

/// Read credentials, call Anthropic, project to the frontend response shape.
async fn fetch_oauth_usage_inner() -> Result<OAuthUsageResponse, String> {
    let home = dirs::home_dir().ok_or_else(|| "no home dir".to_string())?;

    let creds_bytes = claude_view_core::credentials::load_credentials_bytes(&home)
        .ok_or_else(|| "no credentials".to_string())?;

    let oauth = claude_view_core::credentials::parse_credentials(&creds_bytes)
        .ok_or_else(|| "invalid credentials".to_string())?;

    if claude_view_core::credentials::is_token_expired(oauth.expires_at) {
        return Ok(auth_error(
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

    let client = reqwest::Client::new();
    let usage = match fetch_usage(&client, &oauth.access_token).await {
        Ok(u) => u,
        Err(e) if e.contains("401") => {
            return Ok(auth_error(
                "Token expired. Run 'claude' to re-authenticate.",
            ));
        }
        Err(e) => {
            return Ok(OAuthUsageResponse {
                has_auth: true,
                error: Some(e),
                plan,
                tiers: vec![],
            });
        }
    };

    Ok(OAuthUsageResponse {
        has_auth: true,
        error: None,
        plan,
        tiers: build_tiers(&usage),
    })
}

/// `GET /api/oauth/usage`
///
/// Returns cached usage data (5-min TTL). Sets `Cache-Control: max-age=<ttl>`
/// so the frontend's polling interval follows the server's TTL.
#[utoipa::path(
    get,
    path = "/api/oauth/usage",
    tag = "oauth",
    responses(
        (status = 200, description = "OAuth usage tiers with cache headers", body = OAuthUsageResponse),
    )
)]
pub async fn get_oauth_usage(State(state): State<Arc<AppState>>) -> axum::response::Response {
    match state
        .oauth_usage_cache
        .get_or_fetch(fetch_oauth_usage_inner)
        .await
    {
        Ok((resp, remaining_ttl)) => with_cache_headers(resp, remaining_ttl),
        Err(_) => Json(no_auth()).into_response(),
    }
}

/// Minimum interval between forced refreshes (spam guard).
const FORCE_REFRESH_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// `POST /api/oauth/usage/refresh`
///
/// Bypass TTL cache and fetch fresh data. Returns 429 + `Retry-After` if
/// called within 60s of the last attempt.
#[utoipa::path(
    post,
    path = "/api/oauth/usage/refresh",
    tag = "oauth",
    responses(
        (status = 200, description = "Refreshed usage data", body = OAuthUsageResponse),
        (status = 429, description = "Rate limited, retry after header set"),
    )
)]
pub async fn post_oauth_usage_refresh(
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    match state
        .oauth_usage_cache
        .force_refresh(FORCE_REFRESH_MIN_INTERVAL, fetch_oauth_usage_inner)
        .await
    {
        Ok((resp, remaining_ttl)) => with_cache_headers(resp, remaining_ttl),
        Err(CacheError::TooSoon { wait_secs }) => (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            [(axum::http::header::RETRY_AFTER, wait_secs.to_string())],
        )
            .into_response(),
        Err(CacheError::Fetch(e)) => Json(OAuthUsageResponse {
            has_auth: true,
            error: Some(e),
            plan: None,
            tiers: vec![],
        })
        .into_response(),
    }
}

fn with_cache_headers(
    resp: OAuthUsageResponse,
    remaining_ttl: std::time::Duration,
) -> axum::response::Response {
    let max_age = remaining_ttl.as_secs();
    (
        [(
            axum::http::header::CACHE_CONTROL,
            format!("private, max-age={max_age}"),
        )],
        Json(resp),
    )
        .into_response()
}
