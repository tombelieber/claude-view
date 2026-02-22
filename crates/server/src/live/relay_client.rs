//! WebSocket client that connects to the relay server and forwards
//! encrypted session updates to paired mobile devices.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use super::manager::LiveSessionMap;
use super::state::SessionEvent;
use crate::crypto::{
    box_secret_key, encrypt_for_device, load_or_create_identity, load_paired_devices,
    sign_auth_challenge, DeviceIdentity, PairedDevice,
};

/// Configuration for the relay client.
pub struct RelayClientConfig {
    /// RELAY_URL env var (e.g. wss://host/ws). None = relay disabled.
    pub relay_url: Option<String>,
    pub heartbeat_interval: Duration,
    pub max_reconnect_delay: Duration,
}

impl Default for RelayClientConfig {
    fn default() -> Self {
        Self {
            relay_url: std::env::var("RELAY_URL").ok(),
            heartbeat_interval: Duration::from_secs(30),
            max_reconnect_delay: Duration::from_secs(30),
        }
    }
}

/// Spawn the relay client as a background task.
/// Subscribes to the broadcast channel and forwards encrypted session updates.
pub fn spawn_relay_client(
    tx: broadcast::Sender<SessionEvent>,
    sessions: LiveSessionMap,
    config: RelayClientConfig,
) {
    tokio::spawn(async move {
        let relay_url = match config.relay_url {
            Some(ref url) => url.clone(),
            None => {
                info!("RELAY_URL not set — mobile relay disabled");
                return;
            }
        };

        // Load identity (or create on first run)
        let identity = match load_or_create_identity() {
            Ok(id) => id,
            Err(e) => {
                error!("failed to load device identity: {e}");
                return;
            }
        };

        info!(device_id = %identity.device_id, %relay_url, "relay client starting");

        let mut backoff = Duration::from_secs(1);

        loop {
            let paired_devices = load_paired_devices();
            if paired_devices.is_empty() {
                // No devices paired — sleep and check again
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }

            match connect_and_stream(&identity, &paired_devices, &tx, &sessions, &relay_url, &config).await {
                Ok(()) => {
                    info!("relay connection closed cleanly");
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    warn!(backoff_secs = backoff.as_secs(), "relay connection failed: {e}");
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(config.max_reconnect_delay);
        }
    });
}

async fn connect_and_stream(
    identity: &DeviceIdentity,
    paired_devices: &[PairedDevice],
    tx: &broadcast::Sender<SessionEvent>,
    sessions: &LiveSessionMap,
    relay_url: &str,
    config: &RelayClientConfig,
) -> Result<(), String> {
    // Connect to relay
    let (ws_stream, _) = connect_async(relay_url)
        .await
        .map_err(|e| format!("WS connect failed: {e}"))?;

    let (mut sink, mut stream) = ws_stream.split();

    // Authenticate
    let (timestamp, signature) = sign_auth_challenge(identity)?;
    let auth_msg = serde_json::json!({
        "type": "auth",
        "device_id": identity.device_id,
        "timestamp": timestamp,
        "signature": signature,
    });
    sink.send(Message::Text(auth_msg.to_string().into()))
        .await
        .map_err(|e| format!("auth send failed: {e}"))?;

    // Wait for auth_ok
    match stream.next().await {
        Some(Ok(Message::Text(text))) => {
            if text.contains("error") {
                return Err(format!("auth rejected: {text}"));
            }
        }
        other => return Err(format!("unexpected auth response: {other:?}")),
    }

    info!("relay authenticated, sending initial snapshot");

    let box_secret = box_secret_key(identity)?;

    // Send initial snapshot of all current sessions
    {
        let sessions_map = sessions.read().await;
        for session in sessions_map.values() {
            let json = serde_json::to_vec(session).unwrap_or_default();
            for device in paired_devices {
                if let Ok(encrypted) =
                    encrypt_for_device(&json, &device.x25519_pubkey, &box_secret)
                {
                    let envelope = serde_json::json!({
                        "to": device.device_id,
                        "payload": encrypted,
                    });
                    let _ = sink.send(Message::Text(envelope.to_string().into())).await;
                }
            }
        }
    }

    // Subscribe to broadcast and forward events
    let mut rx = tx.subscribe();
    let heartbeat_interval = config.heartbeat_interval;

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(SessionEvent::SessionDiscovered { session } |
                       SessionEvent::SessionUpdated { session }) => {
                        let json = serde_json::to_vec(&session).unwrap_or_default();
                        for device in paired_devices {
                            if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                let envelope = serde_json::json!({
                                    "to": device.device_id,
                                    "payload": encrypted,
                                });
                                if sink.send(Message::Text(envelope.to_string().into())).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Ok(SessionEvent::SessionCompleted { session_id }) => {
                        let msg = serde_json::json!({"type": "session_completed", "session_id": session_id});
                        let json = serde_json::to_vec(&msg).unwrap_or_default();
                        for device in paired_devices {
                            if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                let envelope = serde_json::json!({
                                    "to": device.device_id,
                                    "payload": encrypted,
                                });
                                let _ = sink.send(Message::Text(envelope.to_string().into())).await;
                            }
                        }
                    }
                    Ok(SessionEvent::Summary { .. }) => {
                        // Skip summary events for mobile (phone computes locally)
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "relay client lagged, will resync");
                        let sessions_map = sessions.read().await;
                        for session in sessions_map.values() {
                            let json = serde_json::to_vec(session).unwrap_or_default();
                            for device in paired_devices {
                                if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                    let envelope = serde_json::json!({
                                        "to": device.device_id,
                                        "payload": encrypted,
                                    });
                                    let _ = sink.send(Message::Text(envelope.to_string().into())).await;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Ok(());
                    }
                }
            }
            _ = tokio::time::sleep(heartbeat_interval) => {
                if sink.send(Message::Ping(vec![].into())).await.is_err() {
                    return Ok(());
                }
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                            if val.get("type").and_then(|t| t.as_str()) == Some("pair_complete") {
                                info!("received pair_complete from relay");
                                // TODO: decrypt phone pubkey and store in Keychain
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    _ => {}
                }
            }
        }
    }
}
