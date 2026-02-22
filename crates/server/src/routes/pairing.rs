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
    verifying_key_bytes,
};
use crate::state::AppState;

/// Relay base URL for HTTP API calls (Mac server → relay).
/// Derived from RELAY_URL env var (e.g. wss://host/ws → https://host).
fn relay_http_url() -> Option<String> {
    std::env::var("RELAY_URL").ok().map(|u| {
        u.replace("wss://", "https://")
            .replace("ws://", "http://")
            .trim_end_matches("/ws")
            .to_string()
    })
}

/// Relay WebSocket URL for QR code (phone → relay). Read from RELAY_URL env var.
fn relay_ws_url() -> Option<String> {
    std::env::var("RELAY_URL").ok()
}

#[derive(Serialize)]
struct QrPayload {
    /// Relay WebSocket URL.
    r: String,
    /// Mac X25519 public key (base64).
    k: String,
    /// One-time pairing token.
    t: String,
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
async fn generate_qr(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<QrPayload>, StatusCode> {
    let identity = load_or_create_identity().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let box_secret =
        box_secret_key(&identity).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let box_public = box_secret.public_key();

    let token = uuid::Uuid::new_v4().to_string();

    let relay_ws = relay_ws_url().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let relay_http = relay_http_url().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let ed25519_pubkey =
        verifying_key_bytes(&identity).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let client = reqwest::Client::new();
    let _ = client
        .post(format!("{relay_http}/pair"))
        .json(&serde_json::json!({
            "device_id": identity.device_id,
            "pubkey": STANDARD.encode(&ed25519_pubkey),
            "one_time_token": &token,
        }))
        .send()
        .await;

    Ok(Json(QrPayload {
        r: relay_ws,
        k: STANDARD.encode(box_public.as_bytes()),
        t: token,
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
