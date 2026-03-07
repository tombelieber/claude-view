use axum::{extract::State, http::StatusCode, Json};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;
use tracing::info;

use crate::state::{PairingOffer, RegisteredDevice, RelayState};

#[derive(Deserialize)]
pub struct PairRequest {
    pub device_id: String,
    #[serde(with = "base64_bytes")]
    pub pubkey: Vec<u8>,
    pub one_time_token: String,
}

#[derive(Deserialize)]
pub struct ClaimRequest {
    pub one_time_token: String,
    pub device_id: String,
    #[serde(with = "base64_bytes")]
    pub pubkey: Vec<u8>,
    /// Phone's X25519 pubkey encrypted with Mac's X25519 pubkey (NaCl box).
    pub pubkey_encrypted_blob: String,
    /// Phone's plaintext X25519 pubkey (base64). Mac uses this to decrypt
    /// the blob and verify ownership. Forwarded in pair_complete message.
    pub x25519_pubkey: String,
    /// HMAC-SHA256(verification_secret, phone_x25519_pubkey) for anti-MITM binding.
    /// Optional for backwards compatibility. Relay forwards this to Mac without inspection.
    #[serde(default)]
    pub verification_hmac: Option<String>,
}

#[derive(Serialize)]
pub struct PairResponse {
    pub ok: bool,
}

fn extract_ip(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("fly-client-ip")
        .or_else(|| headers.get("cf-connecting-ip"))
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .unwrap_or("unknown")
        .trim()
        .to_string()
}

/// POST /pair — Mac creates a pairing offer.
pub async fn create_pair(
    State(state): State<RelayState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PairRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    // Rate limiting
    let ip = extract_ip(&headers);
    if !state.pair_rate_limiter.check(&ip).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Validate Ed25519 pubkey
    let verifying_key = VerifyingKey::from_bytes(
        req.pubkey
            .as_slice()
            .try_into()
            .map_err(|_| StatusCode::BAD_REQUEST)?,
    )
    .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Register device if not already registered
    if !state.devices.contains_key(&req.device_id) {
        state.devices.insert(
            req.device_id.clone(),
            RegisteredDevice {
                device_id: req.device_id.clone(),
                verifying_key,
                paired_devices: HashSet::new(),
            },
        );
    }

    // Store pairing offer with TTL (cleanup handled by background task)
    state.pairing_offers.insert(
        req.one_time_token.clone(),
        PairingOffer {
            device_id: req.device_id,
            pubkey: req.pubkey,
            created_at: Instant::now(),
        },
    );

    info!(token = %req.one_time_token, "pairing offer created");
    Ok(Json(PairResponse { ok: true }))
}

/// POST /pair/claim — Phone claims a pairing offer.
pub async fn claim_pair(
    State(state): State<RelayState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    // JWT auth — require valid Supabase token if auth is configured
    if let Some(auth) = &state.supabase_auth {
        let jwt = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if auth.validate(jwt).is_err() {
            tracing::warn!(endpoint = "pair/claim", "JWT validation failed");
            sentry::capture_message("JWT validation failed", sentry::Level::Warning);
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    // Rate limiting
    let ip = extract_ip(&headers);
    if !state.claim_rate_limiter.check(&ip).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Look up and consume the one-time token
    let (_, offer) = state
        .pairing_offers
        .remove(&req.one_time_token)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check TTL (5 minutes)
    if offer.created_at.elapsed().as_secs() > 300 {
        return Err(StatusCode::GONE);
    }

    // Register phone device
    let phone_verifying_key = VerifyingKey::from_bytes(
        req.pubkey
            .as_slice()
            .try_into()
            .map_err(|_| StatusCode::BAD_REQUEST)?,
    )
    .map_err(|_| StatusCode::BAD_REQUEST)?;

    state.devices.insert(
        req.device_id.clone(),
        RegisteredDevice {
            device_id: req.device_id.clone(),
            verifying_key: phone_verifying_key,
            paired_devices: {
                let mut s = HashSet::new();
                s.insert(offer.device_id.clone());
                s
            },
        },
    );

    // Update Mac's paired devices
    if let Some(mut mac_device) = state.devices.get_mut(&offer.device_id) {
        mac_device.paired_devices.insert(req.device_id.clone());
    }

    // Forward encrypted phone pubkey blob + plaintext X25519 key to Mac via WS.
    // Include verification_hmac if present (relay is a passthrough, cannot forge it).
    if let Some(mac_conn) = state.connections.get(&offer.device_id) {
        let mut msg = serde_json::json!({
            "type": "pair_complete",
            "device_id": req.device_id,
            "pubkey_encrypted_blob": req.pubkey_encrypted_blob,
            "x25519_pubkey": req.x25519_pubkey,
        });
        if let Some(ref hmac) = req.verification_hmac {
            msg["verification_hmac"] = serde_json::Value::String(hmac.clone());
        }
        let _ = mac_conn.tx.send(msg.to_string());
    }

    // PostHog: track successful pairing
    if let Some(ref client) = state.posthog_client {
        let client = client.clone();
        let api_key = state.posthog_api_key.clone();
        let device_id = req.device_id.clone();
        tokio::spawn(async move {
            crate::posthog::track(
                &client,
                &api_key,
                "relay_paired",
                &device_id,
                serde_json::json!({}),
            )
            .await;
        });
    }

    info!(mac = %offer.device_id, phone = %req.device_id, "pairing complete");
    Ok(Json(PairResponse { ok: true }))
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}
