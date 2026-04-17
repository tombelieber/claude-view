//! Desktop pairing API — thin proxy over Supabase Edge Functions.
//!
//! Pre-Phase-2 this file talked to the relay's /pair endpoint (deleted) and
//! stored an HMAC verification secret on disk (deleted). The new flow:
//!
//! 1. User clicks "Pair phone" in the web UI.
//! 2. Web UI (or Mac tray) hits GET /api/pairing/qr.
//! 3. This handler reads the cached Supabase session, calls
//!    `POST /functions/v1/pair-offer` with {issuing_device_id: <mac-device-id>}.
//! 4. Supabase Edge Function creates a row in `public.pairing_offers`,
//!    returns {token, relay_ws_url, expires_at}.
//! 5. We wrap the response into a QrPayload that encodes the mobile URL
//!    and hand it back to the client.
//!
//! See design spec §5.1.1 for the edge-function request/response shapes.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::auth::AuthSession;
use crate::crypto::load_or_create_identity;
use crate::state::AppState;
use crate::supabase_proxy::{pair_offer, PairOfferRequest, SupabaseProxyError};

/// QR payload served to the web UI. The `url` field is the QR-encoded value;
/// the rest are duplicates for frontends that prefer named fields.
#[derive(Serialize, utoipa::ToSchema)]
pub struct QrPayload {
    /// Mobile URL — what the QR encodes. Points at the claim page with `?token=...`.
    pub url: String,
    /// Relay WebSocket URL (so the phone knows where to connect after claim).
    pub r: String,
    /// One-time pairing token. Sent to `pair-claim` edge fn by the phone.
    pub t: String,
    /// ISO-8601 expiry timestamp — UI shows the countdown.
    pub expires_at: String,
    /// Protocol version (2 — HMAC removed in Phase 2).
    pub v: u8,
}

/// Legacy schema kept for OpenAPI component compatibility. No routes emit it
/// anymore — devices live at /api/devices after Phase 2.
#[derive(Serialize, utoipa::ToSchema)]
pub struct PairedDeviceResponse {
    pub device_id: String,
    pub name: String,
    pub paired_at: u64,
}

async fn require_session(state: &AppState) -> Result<AuthSession, StatusCode> {
    let snapshot = {
        let guard = state.auth_session.read().await;
        guard.clone()
    };
    snapshot.ok_or(StatusCode::UNAUTHORIZED)
}

fn supabase_env() -> Result<(String, String), StatusCode> {
    let url = std::env::var("SUPABASE_URL")
        .ok()
        .or_else(|| option_env!("SUPABASE_URL").map(str::to_string))
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let publishable = std::env::var("SUPABASE_PUBLISHABLE_KEY")
        .ok()
        .or_else(|| std::env::var("SUPABASE_ANON_KEY").ok())
        .or_else(|| option_env!("SUPABASE_PUBLISHABLE_KEY").map(str::to_string))
        .or_else(|| option_env!("SUPABASE_ANON_KEY").map(str::to_string))
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok((url, publishable))
}

fn map_err(err: SupabaseProxyError) -> StatusCode {
    match err {
        SupabaseProxyError::Unauthorized => StatusCode::UNAUTHORIZED,
        SupabaseProxyError::Forbidden => StatusCode::FORBIDDEN,
        SupabaseProxyError::Business { code, message } => {
            tracing::warn!(code, message, "pair-offer business error");
            StatusCode::BAD_REQUEST
        }
        SupabaseProxyError::MissingConfig => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_GATEWAY,
    }
}

/// GET /api/pairing/qr — Generate a QR payload via Supabase pair-offer.
#[utoipa::path(get, path = "/api/pairing/qr", tag = "pairing",
    responses(
        (status = 200, description = "QR payload", body = QrPayload),
        (status = 401, description = "Not signed in"),
        (status = 503, description = "Supabase not configured"),
    )
)]
pub async fn generate_qr(
    State(state): State<Arc<AppState>>,
) -> Result<Json<QrPayload>, StatusCode> {
    let session = require_session(&state).await?;
    let (url, publishable) = supabase_env()?;
    let identity = load_or_create_identity().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let http = reqwest::Client::new();
    let req = PairOfferRequest {
        issuing_device_id: identity.device_id.clone(),
    };
    let resp = pair_offer(&http, &url, &publishable, &session.access_token, &req)
        .await
        .map_err(map_err)?;

    // Build the mobile URL. The mobile app deep-links off `token` and
    // `relay_ws_url`. No HMAC, no pubkey — the phone authenticates via
    // Supabase (claimant holds their own JWT already).
    let mobile_url = format!(
        "https://claudeview.ai/pair?token={}",
        urlencoding::encode(&resp.token),
    );

    Ok(Json(QrPayload {
        url: mobile_url,
        r: resp.relay_ws_url,
        t: resp.token,
        expires_at: resp.expires_at,
        v: 2,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/pairing/qr", get(generate_qr))
}
