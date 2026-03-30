use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

#[derive(serde::Deserialize)]
struct SwitchRequest {
    model_id: String,
}

/// Mount: `.nest("/api/local-llm", local_llm_routes())`
pub fn local_llm_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(handle_status))
        .route("/enable", post(handle_enable))
        .route("/disable", post(handle_disable))
        .route("/models", get(handle_models))
        .route("/switch", post(handle_switch))
}

async fn handle_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.local_llm.status_snapshot())
}

async fn handle_enable(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.local_llm.enable().await {
        Ok(Some(rx)) => {
            let stream = ReceiverStream::new(rx).map(|progress| {
                Event::default()
                    .json_data(&progress)
                    .map_err(axum::Error::new)
            });
            Sse::new(stream).into_response()
        }
        Ok(None) => {
            Json(serde_json::json!({ "status": "enabling", "download": false })).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn handle_disable(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.local_llm.disable() {
        Ok(()) => Json(serde_json::json!({ "status": "disabled" })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn handle_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.local_llm.models_list())
}

async fn handle_switch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SwitchRequest>,
) -> impl IntoResponse {
    match state.local_llm.switch_model(&body.model_id).await {
        Ok(Some(rx)) => {
            // Download needed — stream progress as SSE
            let stream = ReceiverStream::new(rx).map(|progress| {
                Event::default()
                    .json_data(&progress)
                    .map_err(axum::Error::new)
            });
            Sse::new(stream).into_response()
        }
        Ok(None) => {
            // Already downloaded — switch is immediate
            Json(serde_json::json!({ "status": "switched" })).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
