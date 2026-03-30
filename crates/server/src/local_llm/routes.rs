use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

/// Mount: `.nest("/api/local-llm", local_llm_routes())`
pub fn local_llm_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(handle_status))
        .route("/enable", post(handle_enable))
        .route("/disable", post(handle_disable))
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
        Ok(()) => Json(serde_json::json!({ "status": "disabled" })),
        Err(e) => Json(serde_json::json!({ "error": e })),
    }
}
