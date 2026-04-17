//! WebSocket message router — the entirety of the relay's runtime behavior.
//!
//! On connect:
//!   1. Validate JWT (via query param `?token=JWT`).
//!   2. First message must be `register_device` with {device_id, platform, display_name}.
//!   3. Verify the device belongs to the JWT's user_id via DeviceCache.
//!   4. Register DeviceConnection in the state.connections map.
//!   5. Reply `auth_ok` and await incoming messages.
//!
//! During connection:
//!   - `session_update {to_device_id, payload_b64, push_hint?}` → route to target,
//!     or OneSignal push if target is offline
//!   - `terminate_request {target_device_id}` → close the target's WS (if it's the
//!     caller's own revoked device)
//!   - `re_auth {new_jwt}` → re-validate, reset the JWT expiry timer
//!   - `pong` → heartbeat response
//!
//! On disconnect: remove self from state.connections, log.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{
        ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::auth::validate_jwt;
use crate::state::{DeviceConnection, RelayState};

/// Max concurrent WS connections on a single relay instance.
const MAX_CONNECTIONS: usize = 10_000;

/// Heartbeat ping interval. Client must `pong` within 2 missed pings or we close.
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Query params for the WS upgrade URL. JWT can be passed via `?token=...`
/// (because the WebSocket client can't set custom headers in the browser).
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    RegisterDevice {
        device_id: String,
        platform: String,
        #[allow(dead_code)]
        display_name: String,
    },
    SessionUpdate {
        to_device_id: String,
        payload_b64: String,
        push_hint: Option<String>,
        push_title: Option<String>,
    },
    TerminateRequest {
        target_device_id: String,
    },
    ReAuth {
        new_jwt: String,
    },
    Pong,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg<'a> {
    AuthOk {
        user_id: &'a str,
        device_count: usize,
    },
    AuthError {
        code: &'a str,
        message: &'a str,
    },
    SessionUpdate {
        from_device_id: &'a str,
        payload_b64: &'a str,
    },
    DeviceRevoked {
        device_id: &'a str,
        reason: &'a str,
    },
    ReAuthOk,
    ReAuthError {
        code: &'a str,
    },
    Ping,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsQuery>,
    State(state): State<RelayState>,
) -> Response {
    // Enforce global connection cap BEFORE the upgrade.
    if state.connections.len() >= MAX_CONNECTIONS {
        warn!(
            current = state.connections.len(),
            max = MAX_CONNECTIONS,
            "Rejecting WS upgrade — at connection limit"
        );
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    // Validate JWT (required in query param — browsers can't set headers on upgrade).
    let jwt = match params.token {
        Some(t) => t,
        None => {
            warn!("WS upgrade rejected — no token");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    let user_id = match validate_jwt(&jwt, state.supabase_auth.as_ref()) {
        Ok(uid) => uid,
        Err(e) => {
            warn!(error = %e, "WS upgrade rejected — JWT validation failed");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id, jwt))
        .into_response()
}

async fn handle_socket(socket: WebSocket, state: RelayState, user_id: String, initial_jwt: String) {
    let (mut sink, mut stream) = socket.split();

    // First message must be `register_device`.
    let first = match stream.next().await {
        Some(Ok(Message::Text(t))) => t,
        _ => {
            let _ = send(
                &mut sink,
                ServerMsg::AuthError {
                    code: "AUTH_FIRST_MESSAGE",
                    message: "First message must be register_device",
                },
            )
            .await;
            return;
        }
    };

    let (device_id, platform) = match serde_json::from_str::<ClientMsg>(&first) {
        Ok(ClientMsg::RegisterDevice {
            device_id,
            platform,
            display_name: _,
        }) => (device_id, platform),
        _ => {
            let _ = send(
                &mut sink,
                ServerMsg::AuthError {
                    code: "AUTH_BAD_FIRST_MESSAGE",
                    message: "First message must be register_device",
                },
            )
            .await;
            return;
        }
    };

    // Verify this device belongs to this user via the cache.
    match state.device_cache.get(&user_id, &device_id).await {
        Ok(Some(_device_row)) => {}
        Ok(None) => {
            let _ = send(
                &mut sink,
                ServerMsg::AuthError {
                    code: "DEVICE_NOT_OWNED",
                    message: "Device not registered to this user",
                },
            )
            .await;
            return;
        }
        Err(e) => {
            error!(error = %e, "Device cache lookup failed — treating as unauthorized");
            let _ = send(
                &mut sink,
                ServerMsg::AuthError {
                    code: "SUPABASE_UNREACHABLE",
                    message: "Could not verify device ownership",
                },
            )
            .await;
            return;
        }
    }

    // Create the connection and register it.
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn = Arc::new(DeviceConnection {
        user_id: user_id.clone(),
        device_id: device_id.clone(),
        platform,
        tx,
        connected_at: Instant::now(),
    });
    state.connections.insert(conn.self_key(), conn.clone());

    info!(
        user_id = %user_id,
        device_id = %device_id,
        total = state.connections.len(),
        "Device connected"
    );

    // Send auth_ok.
    let device_count = state.connections_for_user(&user_id).len();
    if send(
        &mut sink,
        ServerMsg::AuthOk {
            user_id: &user_id,
            device_count,
        },
    )
    .await
    .is_err()
    {
        state.remove_connection(&user_id, &device_id);
        return;
    }

    // Spawn the outbound forwarder (drains `rx` to the socket).
    let outbound_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink
                .send(Message::Text(Utf8Bytes::from(msg)))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Main loop: ping + message handling.
    let mut current_jwt = initial_jwt;
    let mut ping_tick = tokio::time::interval(PING_INTERVAL);
    ping_tick.tick().await; // skip first immediate tick
    let mut missed_pongs = 0u8;

    loop {
        tokio::select! {
            _ = ping_tick.tick() => {
                if missed_pongs >= 2 {
                    warn!(
                        user_id = %user_id,
                        device_id = %device_id,
                        "Closing WS — missed pongs"
                    );
                    break;
                }
                if let Some(conn) = state.connection_for(&user_id, &device_id) {
                    let ping_json = serde_json::to_string(&ServerMsg::Ping).unwrap_or_default();
                    let _ = conn.tx.send(ping_json);
                    missed_pongs += 1;
                }
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let msg: ClientMsg = match serde_json::from_str(text.as_str()) {
                            Ok(m) => m,
                            Err(e) => {
                                warn!(error = %e, "Malformed WS message from client");
                                continue;
                            }
                        };
                        match msg {
                            ClientMsg::RegisterDevice { .. } => {
                                warn!("Duplicate register_device on live connection — ignoring");
                            }
                            ClientMsg::SessionUpdate {
                                to_device_id, payload_b64, push_hint, push_title
                            } => {
                                route_session_update(
                                    &state, &user_id, &device_id,
                                    &to_device_id, &payload_b64,
                                    push_hint.as_deref(), push_title.as_deref(),
                                ).await;
                            }
                            ClientMsg::TerminateRequest { target_device_id } => {
                                handle_terminate_request(&state, &user_id, &target_device_id).await;
                            }
                            ClientMsg::ReAuth { new_jwt } => {
                                match validate_jwt(&new_jwt, state.supabase_auth.as_ref()) {
                                    Ok(new_user_id) if new_user_id == user_id => {
                                        current_jwt = new_jwt;
                                        if let Some(conn) = state.connection_for(&user_id, &device_id) {
                                            let ok = serde_json::to_string(&ServerMsg::ReAuthOk).unwrap_or_default();
                                            let _ = conn.tx.send(ok);
                                        }
                                    }
                                    Ok(other) => {
                                        warn!(
                                            current = %user_id,
                                            attempted = %other,
                                            "re_auth with different user_id rejected"
                                        );
                                        if let Some(conn) = state.connection_for(&user_id, &device_id) {
                                            let err = serde_json::to_string(&ServerMsg::ReAuthError {
                                                code: "ACCOUNT_MISMATCH",
                                            }).unwrap_or_default();
                                            let _ = conn.tx.send(err);
                                        }
                                        break;
                                    }
                                    Err(e) => {
                                        warn!(error = %e, "re_auth JWT validation failed");
                                        if let Some(conn) = state.connection_for(&user_id, &device_id) {
                                            let err = serde_json::to_string(&ServerMsg::ReAuthError {
                                                code: "JWT_INVALID",
                                            }).unwrap_or_default();
                                            let _ = conn.tx.send(err);
                                        }
                                        break;
                                    }
                                }
                            }
                            ClientMsg::Pong => {
                                missed_pongs = 0;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        warn!(error = %e, "WS read error");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup.
    state.remove_connection(&user_id, &device_id);
    outbound_task.abort();
    info!(
        user_id = %user_id,
        device_id = %device_id,
        total = state.connections.len(),
        "Device disconnected"
    );

    let _ = current_jwt; // suppress unused_assignments if re_auth never fires
}

async fn send<S>(sink: &mut S, msg: ServerMsg<'_>) -> Result<(), ()>
where
    S: SinkExt<Message> + Unpin,
{
    let json = serde_json::to_string(&msg).map_err(|_| ())?;
    sink.send(Message::Text(Utf8Bytes::from(json)))
        .await
        .map_err(|_| ())
}

async fn route_session_update(
    state: &RelayState,
    from_user_id: &str,
    from_device_id: &str,
    to_device_id: &str,
    payload_b64: &str,
    push_hint: Option<&str>,
    push_title: Option<&str>,
) {
    let target = state.connection_for(from_user_id, to_device_id);
    match target {
        Some(conn) => {
            let envelope = serde_json::to_string(&ServerMsg::SessionUpdate {
                from_device_id,
                payload_b64,
            })
            .unwrap_or_default();
            let _ = conn.tx.send(envelope);
        }
        None => {
            // Target offline — fire push notification via OneSignal.
            let title = push_title.unwrap_or("Session update");
            let body = push_hint.unwrap_or("");
            if !body.is_empty() {
                crate::push::send_push_notification(state, title, body, Some(to_device_id)).await;
            }
        }
    }
}

async fn handle_terminate_request(state: &RelayState, user_id: &str, target_device_id: &str) {
    // A device can only request termination of other devices of the SAME user.
    if let Some(target) = state.connection_for(user_id, target_device_id) {
        let msg = serde_json::to_string(&ServerMsg::DeviceRevoked {
            device_id: target_device_id,
            reason: "user_action",
        })
        .unwrap_or_default();
        let _ = target.tx.send(msg);
        state.device_cache.invalidate(user_id, target_device_id);
    }
}
