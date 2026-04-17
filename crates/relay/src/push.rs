//! OneSignal push notification helper + `/push-tokens` legacy alias registration.
//!
//! The relay's only push responsibility is firing notifications when
//! `session_update` arrives for a device that is offline. Mobile apps
//! register their OneSignal player_id → device_id alias directly with
//! OneSignal during onboarding (Phase 4), but we keep the `/push-tokens`
//! HTTP shim so devices built against the old relay can still register.
//! Once Phase 4 ships and all mobile builds have migrated, this endpoint
//! can be deleted.

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
/// Rate limited to 10 requests/min per device_id.
pub async fn register_push_token(
    State(state): State<RelayState>,
    Json(req): Json<RegisterPushTokenRequest>,
) -> Result<Json<RegisterPushTokenResponse>, StatusCode> {
    if req.device_id.is_empty() || req.onesignal_player_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if !state.push_rate_limiter.check(&req.device_id).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let (app_id, api_key, client) = match (
        &state.onesignal_app_id,
        &state.onesignal_rest_api_key,
        &state.onesignal_http,
    ) {
        (Some(a), Some(k), Some(c)) => (a.clone(), k.clone(), c.clone()),
        _ => {
            // OneSignal not configured — accept silently (no-op)
            return Ok(Json(RegisterPushTokenResponse { ok: true }));
        }
    };

    // Link the OneSignal player to this device_id via the external_id alias.
    let url = format!(
        "https://api.onesignal.com/apps/{app_id}/users/by/onesignal_id/{player_id}",
        app_id = app_id,
        player_id = req.onesignal_player_id,
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
            let body = match resp.text().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!(error = %e, "failed to read push notification response body");
                    String::new()
                }
            };
            warn!("OneSignal token registration failed ({status}): {body}");
        }
        Err(e) => {
            warn!("OneSignal token registration request error: {e}");
        }
        _ => {}
    }

    Ok(Json(RegisterPushTokenResponse { ok: true }))
}

/// Send a push notification via OneSignal REST API.
///
/// If `target_device_id` is Some, targets that specific device by external_id alias.
/// If None, sends to all subscribed users (unused but kept for future broadcast).
pub async fn send_push_notification(
    state: &RelayState,
    title: &str,
    body: &str,
    target_device_id: Option<&str>,
) {
    let (app_id, api_key, client) = match (
        &state.onesignal_app_id,
        &state.onesignal_rest_api_key,
        &state.onesignal_http,
    ) {
        (Some(a), Some(k), Some(c)) => (a.as_str(), k.as_str(), c.clone()),
        _ => return,
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
            let resp_body = match resp.text().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!(error = %e, "failed to read push notification response body");
                    String::new()
                }
            };
            warn!("OneSignal push failed ({status}): {resp_body}");
        }
        Err(e) => {
            warn!("OneSignal push request error: {e}");
        }
        _ => {}
    }

    // PostHog: track push notification sent (best-effort).
    if let (Some(ref ph_client), Some(ref api_key)) = (&state.posthog_http, &state.posthog_api_key)
    {
        let ph_client = ph_client.clone();
        let api_key = api_key.clone();
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
