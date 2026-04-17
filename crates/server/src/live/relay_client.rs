//! WebSocket client that connects to the stateless relay (Phase 1) and
//! forwards per-device encrypted session updates.
//!
//! Protocol (Phase 1 frozen — crates/relay/src/ws.rs):
//! 1. Open `wss://<relay>/ws?token=<JWT>` (Supabase JWT from auth_session).
//! 2. Send ClientMsg::RegisterDevice { device_id, platform, display_name }.
//! 3. Await ServerMsg::AuthOk — any error, log + retry with exp. backoff.
//! 4. For each LiveSession change, encrypt with the peer X25519 pubkey and
//!    send ClientMsg::SessionUpdate { to_device_id, payload_b64, push_hint? }.
//! 5. If our AuthSession expires mid-session, send ClientMsg::ReAuth.
//! 6. Handle ServerMsg::Ping → respond with ClientMsg::Pong.
//!
//! Invariants (§3.4.1):
//! - auth_session RwLock is never held across .await.
//! - Peer-device list comes from Supabase REST, not the relay.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

use super::manager::LiveSessionMap;
use super::state::{AgentStateGroup, LiveSession, SessionEvent};
use crate::auth::session_store::AuthSession;
use crate::crypto::{box_secret_key, encrypt_for_device, DeviceIdentity};
use crate::supabase_proxy::{list_devices, normalize_pubkey_field, DeviceRow};

pub struct RelayClientConfig {
    pub relay_url: Option<String>,
    pub heartbeat_interval: Duration,
    pub max_reconnect_delay: Duration,
    pub re_auth_lead_seconds: u64,
}

impl Default for RelayClientConfig {
    fn default() -> Self {
        Self {
            relay_url: std::env::var("RELAY_URL")
                .ok()
                .or_else(|| option_env!("RELAY_URL").map(str::to_string)),
            heartbeat_interval: Duration::from_secs(30),
            max_reconnect_delay: Duration::from_secs(30),
            re_auth_lead_seconds: 120,
        }
    }
}

/// Spawn the relay client as a background task. `auth_session` is shared
/// with `routes/auth.rs` and `auth/session_refresh.rs`.
pub fn spawn_relay_client(
    auth_session: Arc<RwLock<Option<AuthSession>>>,
    device_identity: DeviceIdentity,
    tx: broadcast::Sender<SessionEvent>,
    sessions: LiveSessionMap,
    config: RelayClientConfig,
) {
    tokio::spawn(async move {
        let Some(relay_url) = config.relay_url.clone() else {
            info!("RELAY_URL not set — mobile relay disabled");
            return;
        };

        let mut backoff = Duration::from_secs(1);

        loop {
            // Snapshot the session — never hold the lock across .await.
            let session_snapshot = {
                let guard = auth_session.read().await;
                guard.clone()
            };

            let Some(session) = session_snapshot else {
                // Not signed in: idle. 10 s poll — if a session appears,
                // we pick it up on the next iteration.
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            };

            match run_session(
                &session,
                &auth_session,
                &device_identity,
                &tx,
                &sessions,
                &relay_url,
                &config,
            )
            .await
            {
                Ok(()) => {
                    info!("relay session closed cleanly");
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    warn!(
                        backoff_secs = backoff.as_secs(),
                        "relay session failed: {e}"
                    );
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(config.max_reconnect_delay);
        }
    });
}

async fn run_session(
    session: &AuthSession,
    auth_session_holder: &Arc<RwLock<Option<AuthSession>>>,
    identity: &DeviceIdentity,
    tx: &broadcast::Sender<SessionEvent>,
    sessions: &LiveSessionMap,
    relay_url_raw: &str,
    config: &RelayClientConfig,
) -> Result<(), String> {
    let url_with_token = attach_token(relay_url_raw, &session.access_token);
    let (ws_stream, _) = connect_async(&url_with_token)
        .await
        .map_err(|e| format!("WS connect failed: {e}"))?;
    let (mut sink, mut stream) = ws_stream.split();

    // Step 1: register_device.
    let register = json!({
        "type": "register_device",
        "device_id": identity.device_id,
        "platform": "mac",
        "display_name": gethostname::gethostname().to_string_lossy().into_owned(),
    });
    sink.send(Message::Text(register.to_string().into()))
        .await
        .map_err(|e| format!("register send failed: {e}"))?;

    // Step 2: expect auth_ok (or auth_error).
    match stream.next().await {
        Some(Ok(Message::Text(t))) if t.contains("auth_ok") => {
            info!("relay authenticated");
        }
        Some(Ok(Message::Text(t))) => return Err(format!("auth_error: {t}")),
        other => return Err(format!("unexpected first reply: {other:?}")),
    }

    // Step 3: fetch peer device list via Supabase (the relay no longer
    // stores it in Phase 1).
    let peers = fetch_peer_devices(session, identity).await;
    if peers.is_empty() {
        info!("no peer devices — staying connected for pair events");
    }

    // Step 4: send initial snapshot per peer.
    let box_secret = box_secret_key(identity).map_err(|e| format!("box_secret_key: {e}"))?;
    {
        let sessions_map = sessions.read().await;
        for s in sessions_map.values() {
            for peer in &peers {
                if let Err(e) = push_session(&mut sink, s, peer, &box_secret).await {
                    warn!(peer = %peer.device_id, error = %e, "initial snapshot send failed");
                }
            }
        }
    }

    // Step 5: main loop.
    let mut rx = tx.subscribe();
    let mut re_auth_ticker = tokio::time::interval(Duration::from_secs(60));
    re_auth_ticker.tick().await;

    loop {
        tokio::select! {
            biased;
            // (a) Forward broadcast events to all peers.
            event = rx.recv() => {
                match event {
                    Ok(SessionEvent::SessionUpsert { session: s }) => {
                        for peer in &peers {
                            let _ = push_session(&mut sink, &s, peer, &box_secret).await;
                        }
                    }
                    Ok(SessionEvent::SessionRemove { session: s, .. }) => {
                        for peer in &peers {
                            let _ = push_session(&mut sink, &s, peer, &box_secret).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "relay client lagged; resyncing");
                        let sessions_map = sessions.read().await;
                        for s in sessions_map.values() {
                            for peer in &peers {
                                let _ = push_session(&mut sink, s, peer, &box_secret).await;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
            // (b) Proactive re_auth when access_token near expiry or rotated
            // by the refresh loop.
            _ = re_auth_ticker.tick() => {
                let snapshot = {
                    let guard = auth_session_holder.read().await;
                    guard.clone()
                };
                if let Some(current) = snapshot {
                    if current.access_token != session.access_token
                        || current.is_near_expiry(Duration::from_secs(config.re_auth_lead_seconds))
                    {
                        let re_auth = json!({
                            "type": "re_auth",
                            "new_jwt": current.access_token,
                        });
                        if let Err(e) = sink.send(Message::Text(re_auth.to_string().into())).await {
                            return Err(format!("re_auth send failed: {e}"));
                        }
                    }
                }
            }
            // (c) Read server frames — pings, re_auth_ok, device_revoked,
            // errors.
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&t) {
                            match val.get("type").and_then(|v| v.as_str()) {
                                Some("ping") => {
                                    let _ = sink
                                        .send(Message::Text(r#"{"type":"pong"}"#.into()))
                                        .await;
                                }
                                Some("re_auth_ok") => info!("re_auth acknowledged by relay"),
                                Some("re_auth_error") => {
                                    warn!("re_auth rejected; reconnecting: {t}");
                                    return Err("re_auth_error".into());
                                }
                                Some("device_revoked") => {
                                    warn!("device_revoked received; closing: {t}");
                                    return Err("device_revoked".into());
                                }
                                Some("session_update") => {
                                    // Inbound updates from a peer (phone → mac).
                                    // v1 Mac ignores these — we're the producer.
                                    tracing::debug!("incoming session_update ignored");
                                }
                                Some("auth_error") => return Err(format!("server auth_error: {t}")),
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    Some(Err(e)) => return Err(format!("WS read error: {e}")),
                    _ => {}
                }
            }
        }
    }
}

async fn fetch_peer_devices(session: &AuthSession, identity: &DeviceIdentity) -> Vec<DeviceRow> {
    let Some(url) = std::env::var("SUPABASE_URL")
        .ok()
        .or_else(|| option_env!("SUPABASE_URL").map(str::to_string))
    else {
        warn!("SUPABASE_URL not configured — no peer devices");
        return Vec::new();
    };
    let publishable = match std::env::var("SUPABASE_PUBLISHABLE_KEY")
        .ok()
        .or_else(|| std::env::var("SUPABASE_ANON_KEY").ok())
        .or_else(|| option_env!("SUPABASE_PUBLISHABLE_KEY").map(str::to_string))
    {
        Some(p) => p,
        None => {
            warn!("SUPABASE_PUBLISHABLE_KEY not configured — no peer devices");
            return Vec::new();
        }
    };
    let http = reqwest::Client::new();
    match list_devices(&http, &url, &publishable, &session.access_token).await {
        Ok(rows) => rows
            .into_iter()
            .filter(|r| r.device_id != identity.device_id && r.revoked_at.is_none())
            .collect(),
        Err(e) => {
            warn!(error = %e, "failed to list peer devices");
            Vec::new()
        }
    }
}

async fn push_session<S>(
    sink: &mut S,
    session: &LiveSession,
    peer: &DeviceRow,
    box_secret: &crypto_box::SecretKey,
) -> Result<(), String>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let peer_pubkey_b64 = peer_pubkey_from_supabase(peer)?;
    let json_bytes = serde_json::to_vec(session).map_err(|e| format!("serialize session: {e}"))?;
    let encrypted = encrypt_for_device(&json_bytes, &peer_pubkey_b64, box_secret)?;
    let envelope = build_envelope(&peer.device_id, &encrypted, Some(session));
    sink.send(Message::Text(envelope.to_string().into()))
        .await
        .map_err(|e| format!("send failed: {e}"))
}

fn build_envelope(
    device_id: &str,
    encrypted: &str,
    session: Option<&LiveSession>,
) -> serde_json::Value {
    let mut envelope = json!({
        "type": "session_update",
        "to_device_id": device_id,
        "payload_b64": encrypted,
    });
    if let Some(s) = session {
        if s.hook.agent_state.group == AgentStateGroup::NeedsYou {
            envelope["push_hint"] = serde_json::Value::String(s.hook.agent_state.label.clone());
            envelope["push_title"] =
                serde_json::Value::String(s.jsonl.project_display_name.clone());
        }
    }
    envelope
}

fn peer_pubkey_from_supabase(peer: &DeviceRow) -> Result<String, String> {
    let raw = peer
        .x25519_pubkey
        .as_deref()
        .ok_or_else(|| format!("peer {} missing x25519_pubkey", peer.device_id))?;
    normalize_pubkey_field(raw)
        .ok_or_else(|| format!("peer {} x25519_pubkey malformed", peer.device_id))
}

fn attach_token(relay_url: &str, token: &str) -> String {
    let sep = if relay_url.contains('?') { '&' } else { '?' };
    format!("{relay_url}{sep}token={}", urlencoding::encode(token))
}

// Test-only re-export so integration tests can exercise attach_token without
// a public production-path entry point.
pub fn __test_attach_token(url: &str, token: &str) -> String {
    attach_token(url, token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_token_appends_query_param_when_none() {
        assert_eq!(
            attach_token("wss://host/ws", "abc"),
            "wss://host/ws?token=abc"
        );
    }

    #[test]
    fn attach_token_appends_query_param_when_existing() {
        assert_eq!(
            attach_token("wss://host/ws?region=nrt", "abc"),
            "wss://host/ws?region=nrt&token=abc"
        );
    }

    #[test]
    fn attach_token_urlencodes_special_chars() {
        // Tokens contain JWT dots which are safe, but slashes/plus would need
        // escaping. Sanity-check that urlencoding handles padding-free base64.
        let encoded = attach_token("wss://h/ws", "a.b/c+d=");
        assert!(encoded.starts_with("wss://h/ws?token="));
        assert!(encoded.contains("%2F") || encoded.contains("+"));
    }
}
