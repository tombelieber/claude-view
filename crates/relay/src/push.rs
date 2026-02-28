use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::state::RelayState;

#[derive(Deserialize)]
pub struct RegisterPushTokenRequest {
    pub device_id: String,
    pub onesignal_player_id: String,
}

#[derive(Serialize)]
pub struct RegisterPushTokenResponse {
    pub ok: bool,
}

/// POST /push-tokens — Register a OneSignal push token for a device.
///
/// Rate limited to 10 requests/min per device_id.
pub async fn register_push_token(
    State(state): State<RelayState>,
    Json(req): Json<RegisterPushTokenRequest>,
) -> Result<Json<RegisterPushTokenResponse>, StatusCode> {
    // Validate the device_id is non-empty before using it as a rate-limiter key
    if req.device_id.is_empty() || req.onesignal_player_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Rate limiting: 10 req/min per device_id (key is now guaranteed non-empty)
    if !state.push_rate_limiter.check(&req.device_id).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let (app_id, api_key, client) = match (
        &state.onesignal_app_id,
        &state.onesignal_api_key,
        &state.onesignal_client,
    ) {
        (Some(a), Some(k), Some(c)) => (a.clone(), k.clone(), c.clone()),
        _ => {
            // OneSignal not configured — accept silently (no-op)
            return Ok(Json(RegisterPushTokenResponse { ok: true }));
        }
    };

    // Link the OneSignal player to this device_id via the external_user_id alias
    let url = format!(
        "https://api.onesignal.com/apps/{app_id}/users/by/onesignal_id/{player_id}",
        app_id = app_id,
        player_id = req.onesignal_player_id
    );
    let payload = serde_json::json!({
        "identity": { "external_id": req.device_id }
    });

    match client
        .patch(&url)
        .header("Authorization", format!("Basic {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if !resp.status().is_success() => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!("OneSignal token registration failed ({status}): {body}");
            // Still return ok=true — the relay accepted the request. OneSignal
            // errors are non-fatal; the device can retry on the next app launch.
        }
        Err(e) => {
            warn!("OneSignal token registration request error: {e}");
        }
        _ => {} // success
    }

    Ok(Json(RegisterPushTokenResponse { ok: true }))
}

/// Send a push notification via OneSignal REST API.
///
/// If `target_device_id` is Some, targets that specific device (by external_user_id).
/// If None, sends to all subscribed users.
pub async fn send_push_notification(
    state: &RelayState,
    title: &str,
    body: &str,
    target_device_id: Option<&str>,
) {
    let (app_id, api_key, client) = match (
        &state.onesignal_app_id,
        &state.onesignal_api_key,
        &state.onesignal_client,
    ) {
        (Some(a), Some(k), Some(c)) => (a.as_str(), k.as_str(), c.clone()),
        _ => return, // OneSignal not configured — skip silently
    };

    let mut payload = serde_json::json!({
        "app_id": app_id,
        "headings": { "en": title },
        "contents": { "en": body },
    });

    if let Some(did) = target_device_id {
        payload["include_aliases"] = serde_json::json!({ "external_id": [did] });
        payload["target_channel"] = serde_json::json!("push");
    } else {
        payload["included_segments"] = serde_json::json!(["Subscribed Users"]);
    }

    match client
        .post("https://api.onesignal.com/api/v1/notifications")
        .header("Authorization", format!("Basic {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if !resp.status().is_success() => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!("OneSignal push failed ({status}): {body}");
        }
        Err(e) => {
            warn!("OneSignal push request error: {e}");
        }
        _ => {} // success
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
