//! Route handler and router for the per-turn breakdown endpoint.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::scanner::scan_turns;
use super::types::TurnInfo;

/// GET /api/sessions/{id}/turns -- Per-turn breakdown for a historical session.
#[utoipa::path(get, path = "/api/sessions/{id}/turns", tag = "turns",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Per-turn breakdown with wall-clock and CC durations", body = Vec<crate::routes::turns::TurnInfo>),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_turns(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<Vec<TurnInfo>>> {
    // Resolve JSONL file path via DB
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    // Read + parse in blocking thread (file I/O)
    let turns = tokio::task::spawn_blocking(move || {
        let data = std::fs::read(&path)
            .map_err(|e| ApiError::Internal(format!("Failed to read session file: {}", e)))?;
        Ok::<Vec<TurnInfo>, ApiError>(scan_turns(&data))
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Task join error: {}", e)))??;

    Ok(Json(turns))
}

/// Create the turns routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/sessions/{id}/turns", get(get_session_turns))
}
