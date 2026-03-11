//! WebSocket client that connects to the relay server and forwards
//! encrypted session updates to paired mobile devices.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use base64::{engine::general_purpose::STANDARD, Engine};

use super::manager::LiveSessionMap;
use super::state::{AgentStateGroup, LiveSession, SessionEvent};
use crate::crypto::{
    add_paired_device, box_secret_key, decrypt_from_device, encrypt_for_device,
    find_and_verify_hmac, load_or_create_identity, load_paired_devices, sign_auth_challenge,
    DeviceIdentity, PairedDevice,
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
            relay_url: std::env::var("RELAY_URL")
                .ok()
                .or_else(|| option_env!("RELAY_URL").map(str::to_string)),
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
            // Always connect — even with no paired devices.
            // The relay connection must be open to receive pair_complete
            // messages for the first-ever pairing (bootstrap).
            // Session-sending loops are naturally guarded by iterating
            // over paired_devices (empty list = no sends).
            let paired_devices = load_paired_devices();

            match connect_and_stream(
                &identity,
                &paired_devices,
                &tx,
                &sessions,
                &relay_url,
                &config,
            )
            .await
            {
                Ok(()) => {
                    info!("relay connection closed cleanly");
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    warn!(
                        backoff_secs = backoff.as_secs(),
                        "relay connection failed: {e}"
                    );
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(config.max_reconnect_delay);
        }
    });
}

/// Build an envelope JSON for sending to the relay.
/// Includes unencrypted `push_hint` and `push_title` when the session
/// is in the NeedsYou group so the relay can trigger push notifications
/// without decrypting the payload.
fn build_envelope(
    device_id: &str,
    encrypted: &str,
    session: Option<&LiveSession>,
) -> serde_json::Value {
    let mut envelope = serde_json::json!({
        "to": device_id,
        "payload": encrypted,
    });
    if let Some(s) = session {
        if s.agent_state.group == AgentStateGroup::NeedsYou {
            envelope["push_hint"] = serde_json::Value::String(s.agent_state.label.clone());
            envelope["push_title"] = serde_json::Value::String(s.project_display_name.clone());
        }
    }
    envelope
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
            let Ok(json) = serde_json::to_vec(session) else {
                tracing::error!("failed to serialize session for relay");
                continue;
            };
            for device in paired_devices {
                if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret)
                {
                    let envelope = build_envelope(&device.device_id, &encrypted, Some(session));
                    if let Err(e) = sink.send(Message::Text(envelope.to_string().into())).await {
                        tracing::warn!(error = %e, "failed to send WebSocket message to relay");
                    }
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
                        let Ok(json) = serde_json::to_vec(&session) else {
                            tracing::error!("failed to serialize session for relay");
                            continue;
                        };
                        for device in paired_devices {
                            if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                let envelope = build_envelope(&device.device_id, &encrypted, Some(&session));
                                if sink.send(Message::Text(envelope.to_string().into())).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Ok(SessionEvent::SessionClosed { session }) => {
                        let Ok(json) = serde_json::to_vec(&session) else {
                            tracing::error!("failed to serialize session for relay");
                            continue;
                        };
                        for device in paired_devices {
                            if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                let envelope = build_envelope(&device.device_id, &encrypted, Some(&session));
                                if sink.send(Message::Text(envelope.to_string().into())).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Ok(SessionEvent::SessionCompleted { session_id }) => {
                        let msg = serde_json::json!({"type": "session_completed", "sessionId": session_id});
                        let Ok(json) = serde_json::to_vec(&msg) else {
                            tracing::error!("failed to serialize session for relay");
                            continue;
                        };
                        for device in paired_devices {
                            if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                let envelope = build_envelope(&device.device_id, &encrypted, None);
                                if let Err(e) = sink.send(Message::Text(envelope.to_string().into())).await {
                                    tracing::warn!(error = %e, "failed to send WebSocket message to relay");
                                }
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
                            let Ok(json) = serde_json::to_vec(session) else {
                                tracing::error!("failed to serialize session for relay");
                                continue;
                            };
                            for device in paired_devices {
                                if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                    let envelope = build_envelope(&device.device_id, &encrypted, Some(session));
                                    if let Err(e) = sink.send(Message::Text(envelope.to_string().into())).await {
                                        tracing::warn!(error = %e, "failed to send WebSocket message to relay");
                                    }
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
                                let phone_device_id = val.get("device_id").and_then(|v| v.as_str());
                                let phone_x25519_b64 = val.get("x25519_pubkey").and_then(|v| v.as_str());
                                let encrypted_blob = val.get("pubkey_encrypted_blob").and_then(|v| v.as_str());

                                let verification_hmac = val.get("verification_hmac").and_then(|v| v.as_str());

                                match (phone_device_id, phone_x25519_b64, encrypted_blob) {
                                    (Some(did), Some(x_pub), Some(blob)) => {
                                        // Decrypt blob to verify phone owns the X25519 key
                                        match decrypt_from_device(blob, x_pub, &box_secret) {
                                            Ok(decrypted) => {
                                                let claimed_pubkey = STANDARD.encode(&decrypted);
                                                if claimed_pubkey != x_pub {
                                                    warn!("pair_complete: decrypted pubkey doesn't match claimed x25519_pubkey");
                                                    continue;
                                                }

                                                // HMAC anti-MITM verification: if the phone
                                                // sent an HMAC, verify it against our stored
                                                // verification secret. This proves the phone
                                                // scanned our QR code directly (the relay
                                                // cannot forge this without the secret).
                                                match verification_hmac {
                                                    Some(hmac) => {
                                                        match find_and_verify_hmac(x_pub, hmac) {
                                                            Ok(true) => {
                                                                info!(phone = %did, "pair_complete: HMAC verified, anti-MITM binding confirmed");
                                                            }
                                                            Ok(false) => {
                                                                warn!(phone = %did, "pair_complete: HMAC verification failed — possible relay key substitution, rejecting");
                                                                continue;
                                                            }
                                                            Err(e) => {
                                                                warn!(phone = %did, "pair_complete: HMAC verification error: {e}, rejecting");
                                                                continue;
                                                            }
                                                        }
                                                    }
                                                    None => {
                                                        // Backwards compatibility: older phone
                                                        // clients may not send HMAC yet.
                                                        warn!(phone = %did, "pair_complete: no verification_hmac provided (legacy client), accepting without HMAC binding");
                                                    }
                                                }

                                                info!(phone = %did, "pair_complete: verified and storing paired device");
                                                let device = PairedDevice {
                                                    device_id: did.to_string(),
                                                    x25519_pubkey: x_pub.to_string(),
                                                    name: format!("Phone {}", &did[..did.len().min(12)]),
                                                    paired_at: std::time::SystemTime::now()
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .expect("system clock before Unix epoch")
                                                        .as_secs(),
                                                };
                                                if let Err(e) = add_paired_device(device) {
                                                    error!("failed to store paired device: {e}");
                                                }
                                            }
                                            Err(e) => {
                                                warn!("pair_complete: failed to decrypt phone pubkey blob: {e}");
                                            }
                                        }
                                    }
                                    _ => {
                                        warn!("pair_complete: missing required fields (device_id, x25519_pubkey, pubkey_encrypted_blob)");
                                    }
                                }
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
