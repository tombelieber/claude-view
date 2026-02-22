use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::auth::{verify_auth, AuthMessage};
use crate::state::{DeviceConnection, RelayState};

#[derive(Deserialize)]
struct RelayEnvelope {
    to: String,
    #[allow(dead_code)]
    payload: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<RelayState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
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

    // Cleanup
    state.connections.remove(&device_id);
    forward_task.abort();
    info!(device_id = %device_id_clone, "device disconnected");
}
