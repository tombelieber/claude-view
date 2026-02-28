use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::auth::{verify_auth, AuthMessage};
use crate::push;
use crate::state::{DeviceConnection, RelayState};

#[derive(Deserialize)]
struct RelayEnvelope {
    to: String,
    #[allow(dead_code)]
    payload: String,
    /// Unencrypted push hint from Mac — relay uses this to trigger push
    /// notifications without needing to decrypt the payload.
    push_hint: Option<String>,
    push_title: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<RelayState>,
) -> Response {
    // JWT auth — require valid token in query param if auth is configured
    if let Some(auth) = &state.supabase_auth {
        let jwt = match params.get("token") {
            Some(t) => t,
            None => return StatusCode::UNAUTHORIZED.into_response(),
        };
        if auth.validate(jwt).is_err() {
            tracing::warn!(endpoint = "ws", "JWT validation failed on WS connect");
            sentry::capture_message(
                "JWT validation failed on WS connect",
                sentry::Level::Warning,
            );
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    // Check global connection limit
    if state.connections.len() >= 1000 {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    // PostHog: track WS connection
    if let Some(ref client) = state.posthog_client {
        let client = client.clone();
        let api_key = state.posthog_api_key.clone();
        tokio::spawn(async move {
            crate::posthog::track(
                &client,
                &api_key,
                "relay_connected",
                "ws_anonymous",
                serde_json::json!({}),
            )
            .await;
        });
    }

    ws.on_upgrade(|socket| handle_socket(socket, state))
        .into_response()
}

async fn handle_socket(socket: WebSocket, state: RelayState) {
    let (mut sink, mut stream) = socket.split();

    // First message must be auth
    let Some(Ok(Message::Text(first_msg))) = stream.next().await else {
        return;
    };
    let Ok(auth) = serde_json::from_str::<AuthMessage>(&first_msg) else {
        let _ = sink
            .send(Message::Text(r#"{"error":"invalid auth format"}"#.into()))
            .await;
        return;
    };
    if auth.msg_type != "auth" {
        let _ = sink
            .send(Message::Text(
                r#"{"error":"first message must be auth"}"#.into(),
            ))
            .await;
        return;
    }

    // Verify against registered device
    let device_id = auth.device_id.clone();
    let verified = state
        .devices
        .get(&device_id)
        .map(|d| verify_auth(&auth, &d.verifying_key))
        .unwrap_or(false);

    if !verified {
        let _ = sink
            .send(Message::Text(r#"{"error":"auth failed"}"#.into()))
            .await;
        return;
    }

    info!(device_id = %device_id, "device authenticated");
    let _ = sink
        .send(Message::Text(r#"{"type":"auth_ok"}"#.into()))
        .await;

    // Create channel for forwarding messages TO this device
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    state.connections.insert(
        device_id.clone(),
        DeviceConnection {
            device_id: device_id.clone(),
            tx,
            connected_at: std::time::Instant::now(),
        },
    );

    // Spawn task to forward queued messages to WS sink
    let device_id_clone = device_id.clone();
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Read incoming messages and forward to recipients
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(envelope) = serde_json::from_str::<RelayEnvelope>(&text) {
                    // Check if sender and recipient are paired
                    let is_paired = state
                        .devices
                        .get(&device_id)
                        .map(|d| d.paired_devices.contains(&envelope.to))
                        .unwrap_or(false);

                    if is_paired {
                        if let Some(conn) = state.connections.get(&envelope.to) {
                            let _ = conn.tx.send(text.to_string());
                        }
                        // If recipient offline, message is dropped (M1: no queuing)

                        // Trigger push notification if Mac included a push hint
                        if let Some(ref hint) = envelope.push_hint {
                            let title = envelope.push_title.as_deref().unwrap_or("Session update");
                            let state = state.clone();
                            let title = title.to_string();
                            let hint = hint.clone();
                            tokio::spawn(async move {
                                push::send_push_notification(&state, &title, &hint).await;
                            });
                        }
                    } else {
                        warn!(from = %device_id, to = %envelope.to, "unpaired device, dropping");
                    }
                }
            }
            Message::Ping(_) => {
                // Pong is handled automatically by axum
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup: remove connection and notify paired devices that this device went offline.
    state.connections.remove(&device_id);
    forward_task.abort();

    // Notify all paired devices that this device is offline.
    // This allows phones to show a "Mac offline" indicator immediately
    // rather than waiting for a heartbeat timeout.
    if let Some(device_entry) = state.devices.get(&device_id) {
        for paired_id in &device_entry.paired_devices {
            if let Some(conn) = state.connections.get(paired_id.as_str()) {
                let _ = conn.tx.send(r#"{"type":"mac_offline"}"#.to_string());
            }
        }
    }

    info!(device_id = %device_id_clone, "device disconnected");
}
