use axum::{extract::State, http::StatusCode, Json};
use base64::engine::{general_purpose::STANDARD, Engine};
use serde::Deserialize;
use tracing::warn;

use crate::auth::{verify_auth, AuthMessage};
use crate::state::RelayState;

#[derive(Deserialize)]
pub struct RegisterToken {
    pub device_id: String,
    pub token: String,
    /// Ed25519 signature — same auth scheme as WS connections.
    pub timestamp: u64,
    pub signature: String,
}

pub async fn register_push_token(
    State(state): State<RelayState>,
    Json(body): Json<RegisterToken>,
) -> StatusCode {
    // Rate limit: 10 req/min per device_id
    if !state.push_rate_limiter.check(&body.device_id).await {
        return StatusCode::TOO_MANY_REQUESTS;
    }

    // Look up registered device's verifying key
    let verifying_key = match state.devices.get(&body.device_id) {
        Some(d) => d.verifying_key,
        None => return StatusCode::UNAUTHORIZED,
    };

    let sig_bytes = match STANDARD.decode(&body.signature) {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    // Construct AuthMessage manually — signature field is Vec<u8> (base64
    // deserialization is only used when going through serde JSON path).
    let auth_msg = AuthMessage {
        msg_type: "auth".to_string(),
        device_id: body.device_id.clone(),
        timestamp: body.timestamp,
        signature: sig_bytes,
    };

    if !verify_auth(&auth_msg, &verifying_key) {
        return StatusCode::UNAUTHORIZED;
    }

    state.push_tokens.insert(body.device_id, body.token);
    StatusCode::OK
}

/// Send a push notification to all registered Expo push tokens.
pub async fn send_push_notification(state: &RelayState, title: &str, body: &str) {
    let tokens: Vec<String> = state
        .push_tokens
        .iter()
        .map(|entry| entry.value().clone())
        .collect();

    if tokens.is_empty() {
        return;
    }

    let client = reqwest::Client::new();
    let messages: Vec<serde_json::Value> = tokens
        .into_iter()
        .map(|token| {
            serde_json::json!({
                "to": token,
                "title": title,
                "body": body,
                "sound": "default",
            })
        })
        .collect();

    if let Err(e) = client
        .post("https://exp.host/--/api/v2/push/send")
        .json(&messages)
        .send()
        .await
    {
        warn!("failed to send push notification: {e}");
    }

    // PostHog: track push notification sent
    if let Some(ref ph_client) = state.posthog_client {
        let ph_client = ph_client.clone();
        let api_key = state.posthog_api_key.clone();
        tokio::spawn(async move {
            crate::posthog::track(
                &ph_client,
                &api_key,
                "push_notification_sent",
                "relay_server",
                serde_json::json!({}),
            )
            .await;
        });
    }
}
