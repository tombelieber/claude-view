//! Thin typed HTTP client for the Supabase REST + Edge Function surface.
//!
//! Single responsibility: mint a reqwest call with the right headers, parse
//! the JSON reply, and map HTTP statuses into a typed error enum that the
//! routes can translate to axum responses.
//!
//! Does NOT persist anything, does NOT cache, does NOT hold locks.

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::auth::session_store::AuthSession;
use crate::crypto::{box_secret_key, verifying_key_bytes, DeviceIdentity};

/// Minimal device row shape the Mac daemon needs. Fields align 1:1 with
/// `public.devices`.
///
/// Per feedback_external_data_serde_default.md every field is `#[serde(default)]`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, utoipa::ToSchema)]
pub struct DeviceRow {
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_seen_at: String,
    #[serde(default)]
    pub revoked_at: Option<String>,
    #[serde(default)]
    pub revoked_reason: Option<String>,
    /// Phase 2 Task 8: widened so relay_client can encrypt for the peer.
    /// PostgREST default emits `\x<hex>` for BYTEA; callers normalise.
    #[serde(default)]
    pub ed25519_pubkey: Option<String>,
    #[serde(default)]
    pub x25519_pubkey: Option<String>,
}

/// Payload sent to `POST /functions/v1/pair-offer`. Must match the
/// Phase 0 edge function's request schema.
#[derive(Clone, Debug, Serialize)]
pub struct PairOfferRequest {
    pub issuing_device_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, utoipa::ToSchema)]
pub struct PairOfferResponse {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub relay_ws_url: String,
    #[serde(default)]
    pub expires_at: String,
}

/// Typed Supabase error. Maps cleanly to HTTP in `routes/*.rs`.
#[derive(Debug, Error)]
pub enum SupabaseProxyError {
    #[error("supabase call failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("supabase rejected the JWT (401)")]
    Unauthorized,
    #[error("supabase denied the operation (403)")]
    Forbidden,
    #[error("supabase reported {code}: {message}")]
    Business { code: String, message: String },
    #[error("unexpected status {0}")]
    Unexpected(StatusCode),
    #[error("deserialization failed: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("missing Supabase config (SUPABASE_URL / SUPABASE_PUBLISHABLE_KEY)")]
    MissingConfig,
    #[error("identity error: {0}")]
    Identity(String),
}

/// Auth-session-refresh endpoint.
///
/// POST {supabase_url}/auth/v1/token?grant_type=refresh_token
/// Headers: apikey: <publishable_key>  (NOT Authorization)
/// Body: {"refresh_token": "..."}
pub async fn refresh_access_token(
    http: &Client,
    supabase_url: &str,
    publishable_key: &str,
    refresh_token: &str,
) -> Result<AuthSession, SupabaseProxyError> {
    let url = format!("{supabase_url}/auth/v1/token?grant_type=refresh_token");
    let body = serde_json::json!({ "refresh_token": refresh_token });
    let resp = http
        .post(&url)
        .header("apikey", publishable_key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    match status {
        s if s.is_success() => {}
        StatusCode::UNAUTHORIZED => return Err(SupabaseProxyError::Unauthorized),
        StatusCode::FORBIDDEN => return Err(SupabaseProxyError::Forbidden),
        other => return Err(SupabaseProxyError::Unexpected(other)),
    }

    #[derive(Deserialize)]
    struct SupabaseTokenResponse {
        #[serde(default)]
        access_token: String,
        #[serde(default)]
        refresh_token: String,
        #[serde(default)]
        expires_in: u64,
        #[serde(default)]
        user: Option<SupabaseUser>,
    }
    #[derive(Deserialize)]
    struct SupabaseUser {
        #[serde(default)]
        id: String,
        #[serde(default)]
        email: Option<String>,
    }

    let parsed: SupabaseTokenResponse = resp.json().await?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (user_id, email) = parsed
        .user
        .map(|u| (u.id, u.email))
        .unwrap_or_else(|| (String::new(), None));
    Ok(AuthSession {
        user_id,
        email,
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        expires_at_unix: now + parsed.expires_in,
    })
}

/// GET /rest/v1/devices filtered to active rows, ordered by last_seen_at desc.
///
/// Selects pubkey columns as base64-encoded strings via PostgREST's `encode()`
/// filter so we don't have to deal with BYTEA's default `\x<hex>` wire format.
pub async fn list_devices(
    http: &Client,
    supabase_url: &str,
    publishable_key: &str,
    access_token: &str,
) -> Result<Vec<DeviceRow>, SupabaseProxyError> {
    let url = format!(
        "{supabase_url}/rest/v1/devices?select=device_id,user_id,platform,display_name,created_at,last_seen_at,revoked_at,revoked_reason,ed25519_pubkey,x25519_pubkey&order=last_seen_at.desc"
    );
    let resp = http
        .get(&url)
        .header("apikey", publishable_key)
        .bearer_auth(access_token)
        .send()
        .await?;
    let status = resp.status();
    match status {
        s if s.is_success() => {}
        StatusCode::UNAUTHORIZED => return Err(SupabaseProxyError::Unauthorized),
        other => return Err(SupabaseProxyError::Unexpected(other)),
    }
    let rows: Vec<DeviceRow> = resp.json().await?;
    Ok(rows)
}

/// POST /functions/v1/pair-offer — returns the QR payload.
pub async fn pair_offer(
    http: &Client,
    supabase_url: &str,
    publishable_key: &str,
    access_token: &str,
    request: &PairOfferRequest,
) -> Result<PairOfferResponse, SupabaseProxyError> {
    let url = format!("{supabase_url}/functions/v1/pair-offer");
    let resp = http
        .post(&url)
        .header("apikey", publishable_key)
        .bearer_auth(access_token)
        .json(&request)
        .send()
        .await?;
    let status = resp.status();
    match status {
        s if s.is_success() => {}
        StatusCode::UNAUTHORIZED => return Err(SupabaseProxyError::Unauthorized),
        StatusCode::FORBIDDEN => return Err(SupabaseProxyError::Forbidden),
        other => return Err(business_or_unexpected(other, resp).await),
    }
    let parsed: PairOfferResponse = resp.json().await?;
    Ok(parsed)
}

/// POST /functions/v1/devices-revoke
pub async fn revoke_device(
    http: &Client,
    supabase_url: &str,
    publishable_key: &str,
    access_token: &str,
    device_id: &str,
    reason: &str,
) -> Result<DeviceRow, SupabaseProxyError> {
    let url = format!("{supabase_url}/functions/v1/devices-revoke");
    let body = serde_json::json!({ "device_id": device_id, "reason": reason });
    let resp = http
        .post(&url)
        .header("apikey", publishable_key)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    match status {
        s if s.is_success() => {}
        StatusCode::UNAUTHORIZED => return Err(SupabaseProxyError::Unauthorized),
        StatusCode::FORBIDDEN => return Err(SupabaseProxyError::Forbidden),
        other => return Err(business_or_unexpected(other, resp).await),
    }
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default)]
        device: DeviceRow,
    }
    let parsed: Wrap = resp.json().await?;
    Ok(parsed.device)
}

/// POST /functions/v1/devices-terminate-others
pub async fn terminate_others(
    http: &Client,
    supabase_url: &str,
    publishable_key: &str,
    access_token: &str,
    calling_device_id: &str,
) -> Result<u32, SupabaseProxyError> {
    let url = format!("{supabase_url}/functions/v1/devices-terminate-others");
    let body = serde_json::json!({ "calling_device_id": calling_device_id });
    let resp = http
        .post(&url)
        .header("apikey", publishable_key)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    match status {
        s if s.is_success() => {}
        StatusCode::UNAUTHORIZED => return Err(SupabaseProxyError::Unauthorized),
        StatusCode::FORBIDDEN => return Err(SupabaseProxyError::Forbidden),
        other => return Err(business_or_unexpected(other, resp).await),
    }
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default)]
        revoked_count: u32,
    }
    let parsed: Wrap = resp.json().await?;
    Ok(parsed.revoked_count)
}

/// POST /functions/v1/devices-register-self — idempotent self-registration.
///
/// Called from `routes/auth.rs::post_session()` on every successful sign-in.
/// Same-Mac re-sign-in is a no-op-plus-refresh by design (the RPC handles
/// idempotency). Deployed 2026-04-17 (function slug `devices-register-self`).
pub async fn bootstrap_device_row(
    http: &Client,
    supabase_url: &str,
    access_token: &str,
    identity: &DeviceIdentity,
) -> Result<DeviceRow, SupabaseProxyError> {
    let ed25519_b64 =
        STANDARD.encode(verifying_key_bytes(identity).map_err(SupabaseProxyError::Identity)?);
    let x25519_secret = box_secret_key(identity).map_err(SupabaseProxyError::Identity)?;
    let x25519_b64 = STANDARD.encode(x25519_secret.public_key().as_bytes());
    let display_name = gethostname::gethostname().to_string_lossy().into_owned();
    let os_version = sysinfo::System::os_version().unwrap_or_default();

    let url = format!("{supabase_url}/functions/v1/devices-register-self");
    let body = serde_json::json!({
        "device_id": identity.device_id,
        "ed25519_pubkey": ed25519_b64,
        "x25519_pubkey":  x25519_b64,
        "platform": "mac",
        "display_name": display_name,
        "app_version": env!("CARGO_PKG_VERSION"),
        "os_version": os_version,
    });
    let resp = http
        .post(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    match status {
        s if s.is_success() => {}
        StatusCode::UNAUTHORIZED => return Err(SupabaseProxyError::Unauthorized),
        StatusCode::FORBIDDEN => return Err(SupabaseProxyError::Forbidden),
        other => return Err(business_or_unexpected(other, resp).await),
    }
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default)]
        device: DeviceRow,
    }
    let parsed: Wrap = resp.json().await?;
    Ok(parsed.device)
}

/// Try to parse a `{error:{code,message}}` body; fall back to Unexpected.
async fn business_or_unexpected(status: StatusCode, resp: reqwest::Response) -> SupabaseProxyError {
    #[derive(Deserialize)]
    struct ErrWrap {
        #[serde(default)]
        error: ErrBody,
    }
    #[derive(Default, Deserialize)]
    struct ErrBody {
        #[serde(default)]
        code: String,
        #[serde(default)]
        message: String,
    }
    let body_text = resp.text().await.unwrap_or_default();
    if let Ok(w) = serde_json::from_str::<ErrWrap>(&body_text) {
        if !w.error.code.is_empty() || !w.error.message.is_empty() {
            return SupabaseProxyError::Business {
                code: w.error.code,
                message: w.error.message,
            };
        }
    }
    SupabaseProxyError::Unexpected(status)
}

/// Decode a pubkey column value from PostgREST/Supabase. Accepts:
///   - Base64-encoded 32-byte key (Edge Function output)
///   - PostgREST BYTEA default: `\x<hex>` (44-bit hex, prefix 2 chars)
///
/// Returns base64. Callers render this back into `crypto::encrypt_for_device`.
pub fn normalize_pubkey_field(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if let Some(hex_part) = trimmed.strip_prefix("\\x") {
        let bytes = hex::decode(hex_part).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        return Some(STANDARD.encode(bytes));
    }
    // Already base64 — verify length.
    let decoded = STANDARD.decode(trimmed).ok()?;
    if decoded.len() != 32 {
        return None;
    }
    Some(STANDARD.encode(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pubkey_roundtrip_base64() {
        let bytes = [7u8; 32];
        let b64 = STANDARD.encode(bytes);
        assert_eq!(normalize_pubkey_field(&b64), Some(b64));
    }

    #[test]
    fn normalize_pubkey_decodes_hex_escape() {
        let bytes = [0x5au8; 32];
        let hex_escape = format!("\\x{}", hex::encode(bytes));
        assert_eq!(
            normalize_pubkey_field(&hex_escape),
            Some(STANDARD.encode(bytes))
        );
    }

    #[test]
    fn normalize_pubkey_rejects_wrong_length() {
        assert!(normalize_pubkey_field("\\x00").is_none());
        assert!(normalize_pubkey_field("not-base64!!!").is_none());
    }
}
