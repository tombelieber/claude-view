use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

#[derive(serde::Deserialize)]
struct ConnectRequest {
    url: String,
}

#[derive(serde::Deserialize)]
struct ToggleRequest {
    enabled: bool,
}

#[derive(serde::Deserialize)]
struct SetModelRequest {
    model_id: String,
}

pub fn local_llm_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(handle_status))
        .route("/connect", post(handle_connect))
        .route("/disconnect", post(handle_disconnect))
        .route("/toggle", post(handle_toggle))
        .route("/model", post(handle_set_model))
}

async fn handle_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.local_llm.status_snapshot())
}

async fn handle_connect(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConnectRequest>,
) -> impl IntoResponse {
    match state.local_llm.connect(body.url) {
        Ok(()) => Json(serde_json::json!({ "status": "connecting" })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn handle_disconnect(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.local_llm.disconnect() {
        Ok(()) => Json(serde_json::json!({ "status": "disconnected" })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn handle_set_model(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetModelRequest>,
) -> impl IntoResponse {
    match state.local_llm.set_model(&body.model_id) {
        Ok(()) => Json(serde_json::json!({ "model_id": body.model_id })).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn handle_toggle(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ToggleRequest>,
) -> impl IntoResponse {
    let result = if body.enabled {
        state.local_llm.enable()
    } else {
        state.local_llm.disable()
    };
    match result {
        Ok(()) => Json(serde_json::json!({ "enabled": body.enabled })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
