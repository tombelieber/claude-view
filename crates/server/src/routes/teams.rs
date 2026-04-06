// crates/server/src/routes/teams.rs
//! Teams API routes.
//!
//! - GET /teams              — List all teams (summaries)
//! - GET /teams/:name        — Get team detail (config + members)
//! - GET /teams/:name/inbox  — Get team inbox messages
//! - GET /teams/:name/cost   — Get team cost breakdown (resolves member sessions)

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::routes::sessions::resolve_session_file_path;
use crate::state::AppState;
use crate::teams::{InboxMessage, TeamCostBreakdown, TeamDetail, TeamSummary};

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
#[utoipa::path(get, path = "/api/teams/{name}", tag = "teams",
    params(("name" = String, Path, description = "Team name")),
    responses(
        (status = 200, description = "Team detail", body = serde_json::Value),
        (status = 404, description = "Team not found"),
    )
)]
pub async fn get_team(
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
#[utoipa::path(get, path = "/api/teams/{name}/inbox", tag = "teams",
    params(("name" = String, Path, description = "Team name")),
    responses(
        (status = 200, description = "Inbox messages", body = serde_json::Value),
        (status = 404, description = "Team not found"),
    )
)]
pub async fn get_team_inbox(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<Json<Vec<InboxMessage>>> {
    state
        .teams
        .inbox(&name)
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Team '{}' not found", name)))
}

/// GET /api/teams/:name/cost — Get team cost breakdown.
///
/// Resolves team member session IDs from the lead session's JSONL, then runs
/// `SessionAccumulator::from_file` on each member session to compute per-member costs.
#[utoipa::path(get, path = "/api/teams/{name}/cost", tag = "teams",
    params(("name" = String, Path, description = "Team name")),
    responses(
        (status = 200, description = "Team cost breakdown", body = serde_json::Value),
        (status = 404, description = "Team not found"),
    )
)]
pub async fn get_team_cost(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<Json<TeamCostBreakdown>> {
    let team = state
        .teams
        .get(&name)
        .ok_or_else(|| ApiError::NotFound(format!("Team '{}' not found", name)))?;

    // Resolve lead session JSONL path
    let lead_path = resolve_session_file_path(&state, &team.lead_session_id).await?;

    // Collect all member session paths (need to resolve while we have async context)
    let member_sessions = crate::teams::resolve_team_member_sessions(&lead_path, &team.name);
    let mut session_paths: std::collections::HashMap<String, std::path::PathBuf> =
        std::collections::HashMap::new();
    for session_id in member_sessions.values() {
        if let Ok(path) = resolve_session_file_path(&state, session_id).await {
            session_paths.insert(session_id.clone(), path);
        }
    }

    // Heavy I/O: parse JSONL files on blocking thread
    let pricing = state.pricing.clone();
    let cost = tokio::task::spawn_blocking(move || {
        crate::teams::build_team_cost(&team, &lead_path, &pricing, |sid| {
            session_paths.get(sid).cloned()
        })
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?;

    Ok(Json(cost))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/teams", get(list_teams))
        .route("/teams/{name}", get(get_team))
        .route("/teams/{name}/inbox", get(get_team_inbox))
        .route("/teams/{name}/cost", get(get_team_cost))
}
