//! Active Claude Code sessions from ~/.claude/sessions/*.json.
//!
//! - GET /api/active-sessions — all currently alive session files

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use claude_view_core::session_files;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// GET /api/active-sessions — returns all active session files from ~/.claude/sessions/.
#[utoipa::path(get, path = "/api/active-sessions", tag = "sessions",
    responses(
        (status = 200, description = "Active Claude Code sessions", body = Vec<session_files::ActiveSession>),
    )
)]
pub async fn get_active_sessions(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<session_files::ActiveSession>>> {
    let sessions = tokio::task::spawn_blocking(|| match session_files::claude_sessions_dir() {
        Some(dir) => session_files::scan_active_sessions(&dir),
        None => Vec::new(),
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Task join error: {e}")))?;

    Ok(Json(sessions))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/active-sessions", get(get_active_sessions))
}
