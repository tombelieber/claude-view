//! Claude Code settings API endpoint.
//!
//! - GET /api/claude-code-settings — merged, redacted settings

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use claude_view_core::settings_files::{self, ClaudeCodeSettings};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// GET /api/claude-code-settings — returns merged, redacted Claude Code settings.
#[utoipa::path(get, path = "/api/claude-code-settings", tag = "settings",
    responses(
        (status = 200, description = "Claude Code settings", body = ClaudeCodeSettings),
    )
)]
pub async fn get_claude_code_settings(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<ClaudeCodeSettings>> {
    let settings = tokio::task::spawn_blocking(settings_files::read_claude_code_settings)
        .await
        .map_err(|e| ApiError::Internal(format!("Task join error: {e}")))?;

    Ok(Json(settings))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/claude-code-settings", get(get_claude_code_settings))
}
