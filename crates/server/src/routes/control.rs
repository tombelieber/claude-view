// crates/server/src/routes/control.rs
//! Phase F: Interactive control routes.
//!
//! - POST /api/control/estimate — cost estimation (Rust-only, no sidecar)
//! - POST /api/control/resume — proxy to sidecar (Task 8)
//! - WS   /api/control/sessions/:id/stream — proxy to sidecar (Task 10)

use std::sync::Arc;

use axum::body::Body;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use axum::response::{IntoResponse, Response};
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
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .map_err(|e| ApiError::Internal(format!("Build response: {e}")))
}

/// POST /api/control/resume — proxy to sidecar
async fn resume_session(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Response, ApiError> {
    proxy_to_sidecar(&state, "POST", "/control/resume", Some(body)).await
}

#[derive(Debug, Deserialize)]
pub struct ConnectQuery {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub model: Option<String>,
}

/// Call sidecar POST /control/resume via the EXISTING proxy_to_sidecar helper.
/// Reuses the hyper UDS plumbing already in control.rs — no duplication.
///
/// IMPORTANT: proxy_to_sidecar takes `body: Option<String>` (confirmed: line 159).
/// Use serde_json::to_string(), NOT to_vec(). The helper sends body as UTF-8.
async fn proxy_resume(
    state: &AppState,
    session_id: &str,
    model: Option<&str>,
) -> Result<String, ApiError> {
    let body = serde_json::json!({
        "sessionId": session_id,
        "model": model.unwrap_or("claude-sonnet-4-20250514"),
    });
    let body_string = serde_json::to_string(&body)
        .map_err(|e| ApiError::Internal(format!("Serialize resume request: {e}")))?;

    let resp = proxy_to_sidecar(state, "POST", "/control/resume", Some(body_string)).await?;

    let bytes = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| ApiError::Internal(format!("Read resume body: {e}")))?
        .to_bytes();
    let data: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::Internal(format!("Parse resume response: {e}")))?;
    let control_id = data["controlId"]
        .as_str()
        .ok_or_else(|| ApiError::Internal("No controlId in resume response".into()))?
        .to_string();
    Ok(control_id)
}

/// GET /api/control/connect?sessionId=xxx — merged resume + WS endpoint.
///
/// Performs resume BEFORE the WS upgrade. If resume fails, returns HTTP error
/// (not a WS that opens then errors). Eliminates the TOCTOU race.
async fn ws_connect(
    ws: WebSocketUpgrade,
    axum::extract::Query(query): axum::extract::Query<ConnectQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    // 1. Validate sessionId format — simple check, no regex crate needed
    //    UUID v4: 8-4-4-4-12 hex chars = 36 chars total
    let is_valid_uuid = query.session_id.len() == 36
        && query.session_id.bytes().enumerate().all(|(i, b)| {
            if i == 8 || i == 13 || i == 18 || i == 23 {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        });
    if !is_valid_uuid {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid session ID format",
        )
            .into_response();
    }

    // 2. Validate session exists in LiveSessionManager (if available)
    if let Some(ref live_manager) = state.live_manager {
        let sessions = state.live_sessions.read().await;
        let session = sessions.get(&query.session_id);
        match session {
            None => {
                return (
                    axum::http::StatusCode::NOT_FOUND,
                    format!("Session {} not found in live monitor", query.session_id),
                )
                    .into_response();
            }
            Some(s)
                if s.pid.is_none() || !crate::live::process::is_pid_alive(s.pid.unwrap_or(0)) =>
            {
                return (
                    axum::http::StatusCode::GONE,
                    format!("Session {} process is no longer alive", query.session_id),
                )
                    .into_response();
            }
            Some(s) if s.control.is_some() => {
                // Already controlled — reuse existing binding (idempotent reconnect)
                let control_id = s.control.as_ref().unwrap().control_id.clone();
                let cancel = s.control.as_ref().unwrap().cancel.clone();
                drop(sessions);
                let socket_path = match state.sidecar.ensure_running().await {
                    Ok(p) => p,
                    Err(e) => {
                        return (
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            format!("Sidecar unavailable: {e}"),
                        )
                            .into_response();
                    }
                };
                return ws.on_upgrade(move |socket| {
                    handle_ws_relay_with_close_codes(socket, control_id, socket_path, state, cancel)
                });
            }
            _ => {} // Session exists, PID alive, no control — proceed to resume
        }
        drop(sessions);
        let _ = live_manager; // suppress unused warning after drop
    }

    // 3. Resume session via sidecar (reject with HTTP 502 on failure)
    //    proxy_resume internally calls ensure_running() via proxy_to_sidecar
    let control_id = match proxy_resume(&state, &query.session_id, query.model.as_deref()).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Resume failed for connect: {e}");
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                format!("Resume failed: {e}"),
            )
                .into_response();
        }
    };

    // CAS bind: only succeeds if no one else bound between our check and now
    if let Some(ref live_manager) = state.live_manager {
        let bound = live_manager
            .bind_control(&query.session_id, control_id.clone(), None)
            .await;
        if !bound {
            // Lost the race — terminate orphaned SDK session
            let _ = proxy_to_sidecar(
                &state,
                "DELETE",
                &format!("/control/sessions/{control_id}"),
                None,
            )
            .await;
            return (
                axum::http::StatusCode::CONFLICT,
                format!(
                    "Session {} is already controlled by another connection",
                    query.session_id
                ),
            )
                .into_response();
        }
        live_manager.request_snapshot_save();
    }

    // 4. Get socket_path for the WS relay (sidecar guaranteed running after resume)
    let socket_path = match state.sidecar.ensure_running().await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Sidecar unavailable after resume: {e}");
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                format!("Sidecar unavailable: {e}"),
            )
                .into_response();
        }
    };

    // Get cancel token from the binding we just created
    let cancel = if state.live_manager.is_some() {
        let sessions = state.live_sessions.read().await;
        sessions
            .get(&query.session_id)
            .and_then(|s| s.control.as_ref())
            .map(|c| c.cancel.clone())
            .unwrap_or_default()
    } else {
        tokio_util::sync::CancellationToken::new()
    };

    // 5. Only NOW upgrade — session is guaranteed to exist
    ws.on_upgrade(move |socket| {
        handle_ws_relay_with_close_codes(socket, control_id, socket_path, state, cancel)
    })
}

async fn handle_ws_relay_with_close_codes(
    frontend_ws: WebSocket,
    control_id: String,
    socket_path: String,
    state: Arc<AppState>,
    cancel: tokio_util::sync::CancellationToken,
) {
    let _ = state; // kept for future use (e.g. metrics), suppress unused warning
    let sidecar_path = format!("/control/sessions/{control_id}/stream");
    let unix_stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Sidecar Unix socket connect failed: {e}");
            let (mut sink, _) = frontend_ws.split();
            let _ = sink
                .send(Message::Close(Some(CloseFrame {
                    code: 4100,
                    reason: "sidecar_connect_failed".into(),
                })))
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
                .send(Message::Close(Some(CloseFrame {
                    code: 4101,
                    reason: "sidecar_ws_failed".into(),
                })))
                .await;
            return;
        }
    };

    // Bidirectional relay with close code mapping
    let (mut fe_sink, mut fe_stream) = frontend_ws.split();
    let (mut sc_sink, mut sc_stream) = sidecar_ws.split();

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
        let mut got_close = false;
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
                tokio_tungstenite::tungstenite::Message::Close(frame) => {
                    let close_frame = frame.map(|f| {
                        let code_u16: u16 = f.code.into();
                        CloseFrame {
                            code: code_u16,
                            reason: f.reason.to_string().into(),
                        }
                    });
                    let _ = fe_sink.send(Message::Close(close_frame)).await;
                    got_close = true;
                    break;
                }
                _ => {}
            }
        }
        // ONLY send 4102 if sidecar stream ended WITHOUT a proper close frame.
        if !got_close {
            let _ = fe_sink
                .send(Message::Close(Some(CloseFrame {
                    code: 4102,
                    reason: "sidecar_stream_ended".into(),
                })))
                .await;
        }
    };

    tokio::select! {
        _ = cancel.cancelled() => {
            // Session was unbound (stale detection, terminate, or re-bind)
            let _ = fe_sink
                .send(Message::Close(Some(CloseFrame {
                    code: 4001,
                    reason: "session_rebound".into(),
                })))
                .await;
        }
        _ = fe_to_sc => {},
        _ = sc_to_fe => {},
    }
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
    // Unbind from LiveSessionManager before telling sidecar to terminate
    if let Some(ref live_manager) = state.live_manager {
        let controlled = live_manager.controlled_session_ids().await;
        for (session_id, cid) in &controlled {
            if cid == &control_id {
                live_manager
                    .unbind_control_if(session_id, &control_id)
                    .await;
                live_manager.request_snapshot_save();
                break;
            }
        }
    }
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
        .route("/control/connect", axum::routing::get(ws_connect))
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
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_core::{SessionInfo, ToolCounts};
    use claude_view_db::Database;
    use tower::ServiceExt;

    fn make_session_with_model(id: &str, model: &str, modified_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            display_name: "project-a".to_string(),
            git_root: None,
            file_path: format!("/tmp/{id}.jsonl"),
            modified_at,
            size_bytes: 2048,
            preview: "Preview".to_string(),
            last_message: "Last message".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 5,
            turn_count: 3,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: Some(10_000),
            total_output_tokens: Some(2_000),
            total_cache_read_tokens: Some(0),
            total_cache_creation_tokens: Some(0),
            turn_count_api: Some(3),
            primary_model: Some(model.to_string()),
            user_prompt_count: 2,
            api_call_count: 1,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 60,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: Some(modified_at),
            total_cost_usd: None,
        }
    }

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

    #[tokio::test]
    async fn test_estimate_cost_without_pricing_returns_null_cost_fields() {
        let db = Database::new_in_memory().await.unwrap();
        let now = chrono::Utc::now().timestamp();
        let session =
            make_session_with_model("sess-no-pricing", "claude-sonnet-4-20250514", now - 120);
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = crate::create_app(db);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/control/estimate")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"session_id":"sess-no-pricing","model":"zzzz-unpriced-model"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["has_pricing"], false);
        assert!(json["first_message_cost"].is_null());
        assert!(json["per_message_cost"].is_null());
    }
}
