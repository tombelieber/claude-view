//! MCP server configuration API endpoint.
//!
//! - GET /api/mcp-servers — deduplicated MCP server configs from plugin cache

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use claude_view_core::mcp_files::{self, McpServerIndex};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// GET /api/mcp-servers — returns all deduplicated MCP server configurations.
#[utoipa::path(get, path = "/api/mcp-servers", tag = "mcp",
    responses(
        (status = 200, description = "Deduplicated MCP server configurations", body = McpServerIndex),
    )
)]
pub async fn get_mcp_servers(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<McpServerIndex>> {
    let index = tokio::task::spawn_blocking(mcp_files::discover_mcp_servers)
        .await
        .map_err(|e| ApiError::Internal(format!("Task join error: {e}")))?;

    Ok(Json(index))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/mcp-servers", get(get_mcp_servers))
}
