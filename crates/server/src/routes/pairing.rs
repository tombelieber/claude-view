//! Desktop pairing API — QR code generation, device list, unpair.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::sync::Arc;

use crate::crypto::{
    box_secret_key, load_or_create_identity, load_paired_devices, remove_paired_device,
    store_verification_secret, verifying_key_bytes,
};
use crate::state::AppState;

/// Relay base URL for HTTP API calls (Mac server → relay).
/// Derived from RELAY_URL env var (e.g. wss://host/ws → https://host).
fn relay_http_url() -> Option<String> {
    (std::env::var("RELAY_URL").ok())
        .or_else(|| option_env!("RELAY_URL").map(str::to_string))
        .map(|u| {
            u.replace("wss://", "https://")
                .replace("ws://", "http://")
                .trim_end_matches("/ws")
                .to_string()
        })
}

/// Relay WebSocket URL for QR code (phone → relay). Read from RELAY_URL env var.
fn relay_ws_url() -> Option<String> {
    std::env::var("RELAY_URL")
        .ok()
        .or_else(|| option_env!("RELAY_URL").map(str::to_string))
}

#[derive(Serialize)]
struct QrPayload {
    /// Mobile page URL — the QR code encodes this directly.
    url: String,
    /// Relay WebSocket URL.
    r: String,
    /// Mac X25519 public key (base64).
    k: String,
    /// One-time pairing token.
    t: String,
    /// Verification secret (base64, 32 bytes). Included in QR URL only,
    /// never sent to the relay. Phone uses it to compute HMAC binding.
    s: String,
    /// Protocol version.
    v: u8,
}

#[derive(Serialize)]
struct PairedDeviceResponse {
    device_id: String,
    name: String,
    paired_at: u64,
}

/// GET /pairing/qr — Generate QR payload for mobile pairing.
async fn generate_qr(State(_state): State<Arc<AppState>>) -> Result<Json<QrPayload>, StatusCode> {
    use rand::Rng;

    let identity = load_or_create_identity().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let box_secret = box_secret_key(&identity).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let box_public = box_secret.public_key();

    let token = uuid::Uuid::new_v4().to_string();

    // Generate 32-byte verification secret for HMAC anti-MITM binding.
    // This secret is embedded in the QR URL and NEVER sent to the relay.
    let verification_secret: [u8; 32] = rand::thread_rng().gen();
    store_verification_secret(&token, &verification_secret)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let s_b64 = STANDARD.encode(verification_secret);

    let relay_ws = relay_ws_url().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let relay_http = relay_http_url().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let ed25519_pubkey =
        verifying_key_bytes(&identity).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{relay_http}/pair"))
        .json(&serde_json::json!({
            "device_id": identity.device_id,
            "pubkey": STANDARD.encode(&ed25519_pubkey),
            "one_time_token": &token,
        }))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to register pairing offer with relay: {e}");
            StatusCode::BAD_GATEWAY
        })?;
    if !resp.status().is_success() {
        tracing::error!("Relay rejected pairing offer: HTTP {}", resp.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    let k_b64 = STANDARD.encode(box_public.as_bytes());
    // Include verification secret in QR URL so phone can compute HMAC binding.
    // The relay never sees `s` — it's only in the direct QR scan.
    let mobile_url = format!(
        "{}/mobile?k={}&t={}&r={}&s={}",
        relay_http,
        urlencoding::encode(&k_b64),
        urlencoding::encode(&token),
        urlencoding::encode(&relay_ws),
        urlencoding::encode(&s_b64),
    );

    Ok(Json(QrPayload {
        url: mobile_url,
        r: relay_ws,
        k: k_b64,
        t: token,
        s: s_b64,
        v: 1,
    }))
}

/// GET /pairing/devices — List paired devices.
async fn list_devices() -> Json<Vec<PairedDeviceResponse>> {
    let devices = load_paired_devices();
    Json(
        devices
            .into_iter()
            .map(|d| PairedDeviceResponse {
                device_id: d.device_id,
                name: d.name,
                paired_at: d.paired_at,
            })
            .collect(),
    )
}

/// DELETE /pairing/devices/:id — Unpair a device.
async fn unpair_device(Path(device_id): Path<String>) -> StatusCode {
    match remove_paired_device(&device_id) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/pairing/qr", get(generate_qr))
        .route("/pairing/devices", get(list_devices))
        .route("/pairing/devices/{id}", delete(unpair_device))
}
