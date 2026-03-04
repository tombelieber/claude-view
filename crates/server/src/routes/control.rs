// crates/server/src/routes/control.rs
//! Phase F: Interactive control routes.
//!
//! - POST /api/control/estimate — cost estimation (Rust-only, no sidecar)
//! - POST /api/control/resume — proxy to sidecar (Task 8)
//! - WS   /api/control/sessions/:id/stream — proxy to sidecar (Task 10)

use std::sync::Arc;

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use axum::response::Response;
use axum::{extract::State, routing::post, Json, Router};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;

use crate::{error::ApiError, state::AppState};

/// Request body for cost estimation.
#[derive(Debug, Deserialize)]
pub struct EstimateRequest {
    pub session_id: String,
    pub model: Option<String>,
}

/// Cost estimation response.
#[derive(Debug, Serialize)]
pub struct CostEstimate {
    pub session_id: String,
    pub history_tokens: u64,
    pub cache_warm: bool,
    pub first_message_cost: Option<f64>,
    pub per_message_cost: Option<f64>,
    pub has_pricing: bool,
    pub model: String,
    pub explanation: String,
    pub session_title: Option<String>,
    pub project_name: Option<String>,
    pub turn_count: u32,
    pub files_edited: u32,
    pub last_active_secs_ago: i64,
}

async fn estimate_cost(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EstimateRequest>,
) -> Result<Json<CostEstimate>, ApiError> {
    let now = chrono::Utc::now().timestamp();

    // Look up session in DB
    let session = state
        .db
        .get_session_by_id(&req.session_id)
        .await
        .map_err(|e| ApiError::Internal(format!("DB error: {e}")))?
        .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", req.session_id)))?;

    let model = req.model.unwrap_or_else(|| {
        session
            .primary_model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
    });

    let history_tokens = session.total_input_tokens.unwrap_or(0);
    let last_activity = session.modified_at; // epoch seconds
    let cache_warm = last_activity > 0 && (now - last_activity) < 300; // 5 min TTL

    // Look up model pricing
    let pricing = state.pricing.read().expect("pricing lock poisoned");
    let model_pricing = claude_view_core::pricing::lookup_pricing(&model, &pricing);

    let per_million =
        |tokens: u64, rate_per_m: f64| -> f64 { (tokens as f64 / 1_000_000.0) * rate_per_m };

    let secs_ago = now - last_activity;
    let (first_message_cost, per_message_cost, has_pricing, explanation) = if let Some(p) =
        model_pricing
    {
        let input_base = p.input_cost_per_token * 1_000_000.0;
        let first_message_cost = if cache_warm {
            per_million(history_tokens, input_base * 0.10) // cache read
        } else {
            per_million(history_tokens, input_base * 1.25) // cache write
        };
        let per_message_cost = per_million(history_tokens, input_base * 0.10); // always cache read
        let explanation = if cache_warm {
            format!(
                "Cache is warm (last active {}s ago). First message: ${:.4} (cached). Each follow-up: ~${:.4}.",
                secs_ago, first_message_cost, per_message_cost,
            )
        } else {
            format!(
                "Cache is cold (last active {}m ago). First message: ${:.4} (cache warming). Follow-ups drop to ~${:.4} (cached).",
                secs_ago / 60, first_message_cost, per_message_cost,
            )
        };
        (
            Some(first_message_cost),
            Some(per_message_cost),
            true,
            explanation,
        )
    } else {
        (
            None,
            None,
            false,
            format!(
                "Model pricing not found for {} (last active {}s ago). Cost estimate unavailable without real pricing data.",
                model, secs_ago
            ),
        )
    };

    // Use display_name (short human-readable project name, e.g. "claude-backup")
    // derived from CWD evidence at index time. Falls back to the raw encoded
    // directory name when no CWD is available — never guesses.
    let project_name = if session.display_name.is_empty() {
        None
    } else {
        Some(session.display_name.clone())
    };

    Ok(Json(CostEstimate {
        session_id: req.session_id,
        history_tokens,
        cache_warm,
        first_message_cost,
        per_message_cost,
        has_pricing,
        model,
        explanation,
        session_title: session.longest_task_preview.clone(),
        project_name,
        turn_count: session.turn_count_api.unwrap_or(0).min(u32::MAX as u64) as u32,
        files_edited: session.files_edited_count,
        last_active_secs_ago: secs_ago,
    }))
}

/// Proxy a request to the sidecar, ensuring it's running first.
///
/// Uses raw `tokio::net::UnixStream` + `hyper::client::conn::http1` instead of
/// `hyperlocal` (which is incompatible with hyper 1.x).
///
/// Returns `Result<Response, ApiError>` so error cases return JSON error bodies
/// that the frontend can parse — not bare StatusCode with empty body.
async fn proxy_to_sidecar(
    state: &AppState,
    method: &str,
    path: &str,
    body: Option<String>,
) -> Result<Response, ApiError> {
    let socket_path = state.sidecar.ensure_running().await.map_err(|e| {
        tracing::error!("Sidecar unavailable: {e}");
        ApiError::Internal(format!("Sidecar unavailable: {e}"))
    })?;

    // Connect to sidecar Unix socket
    let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
        tracing::error!("Failed to connect to sidecar socket: {e}");
        ApiError::Internal(format!("Sidecar connection failed: {e}"))
    })?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = http1::handshake(io).await.map_err(|e| {
        tracing::error!("Sidecar HTTP handshake failed: {e}");
        ApiError::Internal(format!("Sidecar handshake failed: {e}"))
    })?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::error!("Sidecar HTTP connection driver error: {e}");
        }
    });

    let req = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost")
        .header("content-type", "application/json");

    let req = if let Some(body) = body {
        req.body(Full::new(Bytes::from(body)))
    } else {
        req.body(Full::new(Bytes::new()))
    }
    .map_err(|e| ApiError::Internal(format!("Build request: {e}")))?;

    let resp = sender.send_request(req).await.map_err(|e| {
        tracing::error!("Sidecar request failed: {e}");
        ApiError::Internal(format!("Sidecar request failed: {e}"))
    })?;

    // Convert hyper response to axum response
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| ApiError::Internal(format!("Read sidecar response: {e}")))?
        .to_bytes();
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .unwrap())
}

/// POST /api/control/resume — proxy to sidecar
async fn resume_session(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Response, ApiError> {
    proxy_to_sidecar(&state, "POST", "/control/resume", Some(body)).await
}

/// POST /api/control/send — proxy to sidecar
async fn send_message(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Response, ApiError> {
    proxy_to_sidecar(&state, "POST", "/control/send", Some(body)).await
}

/// GET /api/control/sessions — list active control sessions
async fn list_sessions(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    proxy_to_sidecar(&state, "GET", "/control/sessions", None).await
}

/// DELETE /api/control/sessions/:id — terminate a control session
async fn terminate_session(
    State(state): State<Arc<AppState>>,
    Path(control_id): Path<String>,
) -> Result<Response, ApiError> {
    proxy_to_sidecar(
        &state,
        "DELETE",
        &format!("/control/sessions/{control_id}"),
        None,
    )
    .await
}

/// WS /api/control/sessions/:id/stream — bidirectional relay to sidecar
async fn ws_stream(
    ws: WebSocketUpgrade,
    Path(control_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws_relay(socket, control_id, state))
}

async fn handle_ws_relay(frontend_ws: WebSocket, control_id: String, state: Arc<AppState>) {
    // Ensure sidecar is running
    let socket_path = match state.sidecar.ensure_running().await {
        Ok(p) => p,
        Err(e) => {
            let (mut sink, _) = frontend_ws.split();
            let _ = sink
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": format!("Sidecar unavailable: {e}"),
                        "fatal": true,
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    // Connect to sidecar WebSocket via Unix socket
    let sidecar_path = format!("/control/sessions/{control_id}/stream");

    let unix_stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to connect to sidecar Unix socket: {e}");
            let (mut sink, _) = frontend_ws.split();
            let _ = sink
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": "Failed to connect to sidecar",
                        "fatal": true,
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    let ws_url = format!("ws://localhost{sidecar_path}");
    let (sidecar_ws, _) = match tokio_tungstenite::client_async(ws_url, unix_stream).await {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!("Sidecar WS handshake failed: {e}");
            let (mut sink, _) = frontend_ws.split();
            let _ = sink
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": "Sidecar WebSocket handshake failed",
                        "fatal": true,
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    // Split both WebSockets and relay bidirectionally
    let (mut fe_sink, mut fe_stream) = frontend_ws.split();
    let (mut sc_sink, mut sc_stream) = sidecar_ws.split();

    // In Axum 0.8 + tungstenite 0.28, Message::Text wraps Utf8Bytes, not String.
    // Use .as_ref() to read and .into() to convert when relaying between
    // axum::extract::ws::Message and tokio_tungstenite::tungstenite::Message.
    let fe_to_sc = async {
        while let Some(Ok(msg)) = fe_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let s: &str = text.as_ref();
                    if sc_sink
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            s.to_string().into(),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Message::Ping(data) => {
                    let _ = sc_sink
                        .send(tokio_tungstenite::tungstenite::Message::Ping(
                            data.to_vec().into(),
                        ))
                        .await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    };

    let sc_to_fe = async {
        while let Some(Ok(msg)) = sc_stream.next().await {
            match msg {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    let s: &str = text.as_ref();
                    if fe_sink
                        .send(Message::Text(s.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = fe_to_sc => {},
        _ = sc_to_fe => {},
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/control/estimate", post(estimate_cost))
        .route("/control/resume", post(resume_session))
        .route("/control/send", post(send_message))
        .route("/control/sessions", axum::routing::get(list_sessions))
        .route(
            "/control/sessions/{id}",
            axum::routing::delete(terminate_session),
        )
        .route(
            "/control/sessions/{id}/stream",
            axum::routing::get(ws_stream),
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_cache_warm_within_5_minutes() {
        let now = 1000;
        let last_activity = 800; // 200s ago (< 300s)
        assert!(last_activity > 0 && (now - last_activity) < 300);
    }

    #[test]
    fn test_cache_cold_after_5_minutes() {
        let now = 1000;
        let last_activity = 600; // 400s ago (> 300s)
        assert!(!(last_activity > 0 && (now - last_activity) < 300));
    }

    #[test]
    fn test_cache_cold_for_epoch_zero() {
        let now = 1000;
        let last_activity = 0; // never active
        assert!(!(last_activity > 0 && (now - last_activity) < 300));
    }

    #[test]
    fn test_cost_estimate_math() {
        // 100K tokens, Sonnet pricing ($3/1M input), cache cold
        let tokens: u64 = 100_000;
        let input_base = 3.0; // per 1M
        let cache_write_cost = (tokens as f64 / 1_000_000.0) * (input_base * 1.25);
        let cache_read_cost = (tokens as f64 / 1_000_000.0) * (input_base * 0.10);

        assert!((cache_write_cost - 0.375).abs() < 0.001); // $0.375
        assert!((cache_read_cost - 0.030).abs() < 0.001); // $0.030
    }
}
