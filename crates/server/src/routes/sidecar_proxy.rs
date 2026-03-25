//! Reverse proxy for sidecar HTTP + WebSocket routes.
//!
//! In development, Vite proxies `/api/sidecar/*` and `/ws/chat/*` to the
//! sidecar on localhost:3001. In production the Rust server serves the
//! frontend directly, so it must forward these requests itself.
//!
//! Pattern: Backend-for-Frontend (BFF) — the Rust server aggregates all
//! backend services behind a single origin. Next.js `rewrites`, Cloudflare
//! Workers, and API gateways all use this same pattern.
//!
//! Routes:
//!   - ANY /api/sidecar/*  → HTTP proxy to sidecar
//!   - WS  /ws/chat/*      → WebSocket relay to sidecar

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::{
    body::Body,
    extract::{ws::WebSocket, Request, State, WebSocketUpgrade},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use futures_util::{SinkExt, StreamExt};

use crate::state::AppState;

/// Shared reqwest client — connection pooling + timeout, created once.
fn shared_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(4)
            .build()
            .expect("reqwest client")
    })
}

/// Build the sidecar proxy router.
///
/// Mounted at the ROOT level (not under /api) because the WebSocket
/// route `/ws/chat/{id}` sits outside the /api prefix.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ws/chat/{session_id}", get(ws_proxy_handler))
        .route("/api/sidecar/{*rest}", any(http_proxy_handler))
}

// ── HTTP reverse proxy ──────────────────────────────────────────────

/// Forward any HTTP request under `/api/sidecar/*` to the sidecar.
///
/// Calls `ensure_running()` to auto-start the sidecar if it crashed.
async fn http_proxy_handler(State(state): State<Arc<AppState>>, req: Request) -> Response {
    // Ensure sidecar is running (idempotent, fast when already alive)
    let sidecar_base = match state.sidecar.ensure_running().await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!(error = %e, "Sidecar not available for proxy");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &format!("Sidecar not available: {e}"),
            );
        }
    };

    // Reconstruct the target URL: sidecar_base + original path + query
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(req.uri().path());

    let target_url = format!("{sidecar_base}{path_and_query}");

    let method = req.method().clone();
    let headers = req.headers().clone();
    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Failed to read proxy request body: {e}");
            return error_response(StatusCode::BAD_REQUEST, "Failed to read request body");
        }
    };

    let mut builder = shared_client().request(to_reqwest_method(&method), &target_url);

    // Forward relevant headers (skip hop-by-hop headers)
    for (name, value) in headers.iter() {
        let n = name.as_str();
        if matches!(n, "host" | "connection" | "transfer-encoding") {
            continue;
        }
        if let Ok(v) = value.to_str() {
            builder = builder.header(n, v);
        }
    }

    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes);
    }

    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(url = %target_url, error = %e, "Sidecar proxy request failed");
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Sidecar unreachable: {e}"),
            );
        }
    };

    // Forward status + headers + body back to the client.
    // Body is buffered (not streamed) — sidecar responses are small JSON.
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response_headers = HeaderMap::new();
    for (name, value) in resp.headers().iter() {
        if matches!(name.as_str(), "transfer-encoding" | "connection") {
            continue;
        }
        response_headers.insert(name.clone(), value.clone());
    }

    let body_bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Failed to read sidecar response: {e}");
            return error_response(StatusCode::BAD_GATEWAY, "Failed to read sidecar response");
        }
    };

    let mut response = Response::new(Body::from(body_bytes));
    *response.status_mut() = status;
    *response.headers_mut() = response_headers;
    response
}

fn to_reqwest_method(m: &Method) -> reqwest::Method {
    match *m {
        Method::GET => reqwest::Method::GET,
        Method::POST => reqwest::Method::POST,
        Method::PUT => reqwest::Method::PUT,
        Method::DELETE => reqwest::Method::DELETE,
        Method::PATCH => reqwest::Method::PATCH,
        Method::HEAD => reqwest::Method::HEAD,
        Method::OPTIONS => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({ "error": message });
    (status, axum::Json(body)).into_response()
}

// ── WebSocket reverse proxy ─────────────────────────────────────────

/// Upgrade to WebSocket and relay to sidecar's `/ws/chat/:session_id`.
async fn ws_proxy_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    // Ensure sidecar is running before upgrading
    let sidecar_base = match state.sidecar.ensure_running().await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!(error = %e, "Sidecar not available for WS proxy");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &format!("Sidecar not available: {e}"),
            );
        }
    };

    let ws_url = sidecar_base
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let target = format!("{ws_url}/ws/chat/{session_id}");

    ws.on_upgrade(move |client_ws| relay_websocket(client_ws, target))
}

/// Bidirectional WebSocket relay between the browser and the sidecar.
async fn relay_websocket(client_ws: WebSocket, target_url: String) {
    let sidecar_ws = match tokio_tungstenite::connect_async(&target_url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            tracing::warn!(url = %target_url, error = %e, "Failed to connect to sidecar WS");
            return;
        }
    };

    let (mut client_tx, mut client_rx) = client_ws.split();
    let (mut sidecar_tx, mut sidecar_rx) = sidecar_ws.split();

    // Client → Sidecar
    let client_to_sidecar = async {
        while let Some(msg) = client_rx.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    tracing::debug!(error = %e, "Client WS read error");
                    break;
                }
            };
            if let Some(tung_msg) = axum_to_tungstenite(msg) {
                if sidecar_tx.send(tung_msg).await.is_err() {
                    break;
                }
            }
        }
    };

    // Sidecar → Client
    let sidecar_to_client = async {
        while let Some(msg) = sidecar_rx.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    tracing::debug!(error = %e, "Sidecar WS read error");
                    break;
                }
            };
            if let Some(axum_msg) = tungstenite_to_axum(msg) {
                if client_tx.send(axum_msg).await.is_err() {
                    break;
                }
            }
        }
    };

    // Run both directions concurrently; when either side closes, stop both.
    tokio::select! {
        _ = client_to_sidecar => {},
        _ = sidecar_to_client => {},
    }
}

/// Convert an axum WebSocket message to a tungstenite message.
fn axum_to_tungstenite(
    msg: axum::extract::ws::Message,
) -> Option<tokio_tungstenite::tungstenite::Message> {
    use axum::extract::ws::Message as A;
    use tokio_tungstenite::tungstenite::Message as T;
    Some(match msg {
        A::Text(t) => T::Text(t.to_string().into()),
        A::Binary(b) => T::Binary(Vec::from(b.as_ref()).into()),
        A::Ping(p) => T::Ping(Vec::from(p.as_ref()).into()),
        A::Pong(p) => T::Pong(Vec::from(p.as_ref()).into()),
        A::Close(Some(cf)) => {
            T::Close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
                code: cf.code.into(),
                reason: cf.reason.to_string().into(),
            }))
        }
        A::Close(None) => T::Close(None),
    })
}

/// Convert a tungstenite WebSocket message to an axum message.
fn tungstenite_to_axum(
    msg: tokio_tungstenite::tungstenite::Message,
) -> Option<axum::extract::ws::Message> {
    use axum::extract::ws::Message as A;
    use tokio_tungstenite::tungstenite::Message as T;
    Some(match msg {
        T::Text(t) => A::Text(t.to_string().into()),
        T::Binary(b) => A::Binary(b),
        T::Ping(p) => A::Ping(p),
        T::Pong(p) => A::Pong(p),
        T::Close(Some(cf)) => A::Close(Some(axum::extract::ws::CloseFrame {
            code: cf.code.into(),
            reason: cf.reason.to_string().into(),
        })),
        T::Close(None) => A::Close(None),
        T::Frame(_) => return None,
    })
}
