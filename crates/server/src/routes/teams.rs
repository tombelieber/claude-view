// crates/server/src/routes/teams.rs
//! Teams API routes.
//!
//! - GET /teams          — List all teams (summaries)
//! - GET /teams/:name    — Get team detail (config + members)
//! - GET /teams/:name/inbox — Get team inbox messages

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::teams::{InboxMessage, TeamDetail, TeamSummary};

/// GET /api/teams — List all teams.
#[utoipa::path(get, path = "/api/teams", tag = "teams",
    responses(
        (status = 200, description = "All team summaries", body = serde_json::Value),
    )
)]
pub async fn list_teams(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<TeamSummary>>> {
    Ok(Json(state.teams.summaries()))
}

/// GET /api/teams/:name — Get team detail.
async fn get_team(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<Json<TeamDetail>> {
    state
        .teams
        .get(&name)
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Team '{}' not found", name)))
}

/// GET /api/teams/:name/inbox — Get team inbox messages.
async fn get_team_inbox(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<Json<Vec<InboxMessage>>> {
    state
        .teams
        .inbox(&name)
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Team '{}' not found", name)))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/teams", get(list_teams))
        .route("/teams/{name}", get(get_team))
        .route("/teams/{name}/inbox", get(get_team_inbox))
}
