// crates/server/src/routes/teams.rs
//! Teams API routes.
//!
//! - GET /teams              — List all teams (summaries)
//! - GET /teams/:name        — Get team detail (config + members)
//! - GET /teams/:name/inbox  — Get team inbox messages
//! - GET /teams/:name/cost   — Get team cost breakdown (resolves member sessions)
//! - GET /teams/:name/sidechains?session_id=xxx — Get team member sidechains

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::routes::sessions::resolve_session_file_path;
use crate::state::AppState;
use crate::teams::{InboxMessage, TeamCostBreakdown, TeamDetail, TeamMemberSidechain, TeamSummary};

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
///
/// Enriches inbox-augmented members with model info from the lead JSONL.
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
    let mut team = state
        .teams
        .get(&name)
        .ok_or_else(|| ApiError::NotFound(format!("Team '{}' not found", name)))?;

    // Enrich members that have empty model (inbox-augmented) from JSONL spawn data
    let has_empty_model = team.members.iter().any(|m| m.model.is_empty());
    if has_empty_model {
        if let Ok(lead_path) = resolve_session_file_path(&state, &team.lead_session_id).await {
            let resolved = crate::teams::resolve_team_member_sessions(&lead_path, &team.name);
            for member in &mut team.members {
                if member.model.is_empty() {
                    if let Some(info) = resolved.get(&member.name) {
                        if let Some(model) = &info.model {
                            member.model.clone_from(model);
                        }
                    }
                }
            }
        }
    }

    Ok(Json(team))
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
    for info in member_sessions.values() {
        if !info.in_process {
            if let Ok(path) = resolve_session_file_path(&state, &info.agent_id).await {
                session_paths.insert(info.agent_id.clone(), path);
            }
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

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TeamSidechainsQuery {
    pub session_id: String,
}

/// GET /api/teams/:name/sidechains?session_id=xxx — Get team member sidechains.
///
/// Resolves sidechain `.meta.json` / `.jsonl` pairs inside the session directory
/// to enumerate each member's spawned sub-conversations.
#[utoipa::path(get, path = "/api/teams/{name}/sidechains", tag = "teams",
    params(
        ("name" = String, Path, description = "Team name"),
        ("session_id" = String, Query, description = "Lead session ID"),
    ),
    responses(
        (status = 200, description = "Team member sidechains", body = serde_json::Value),
        (status = 404, description = "Team not found"),
    )
)]
pub async fn get_team_sidechains(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(query): Query<TeamSidechainsQuery>,
) -> ApiResult<Json<Vec<TeamMemberSidechain>>> {
    // Verify team exists
    let _team = state
        .teams
        .get(&name)
        .ok_or_else(|| ApiError::NotFound(format!("Team '{}' not found", name)))?;

    // Resolve session JSONL path, then derive the session directory.
    // Layout: {project_dir}/{session_id}.jsonl → subagents at {project_dir}/{session_id}/subagents/
    // Same convention as resolve_subagent_path in live/subagent_file.rs.
    let session_path = resolve_session_file_path(&state, &query.session_id).await?;
    let parent_dir = session_path
        .parent()
        .ok_or_else(|| ApiError::Internal("Session path has no parent directory".into()))?;
    let session_stem = session_path
        .file_stem()
        .ok_or_else(|| ApiError::Internal("Session path has no file stem".into()))?;
    let session_dir = parent_dir.join(session_stem);

    // Heavy I/O: read meta.json + count JSONL lines + compute cost on blocking thread
    let pricing = state.pricing.clone();
    let sidechains = tokio::task::spawn_blocking(move || {
        crate::teams::resolve_team_sidechains(&session_dir, &pricing)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?;

    Ok(Json(sidechains))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/teams", get(list_teams))
        .route("/teams/{name}", get(get_team))
        .route("/teams/{name}/inbox", get(get_team_inbox))
        .route("/teams/{name}/cost", get(get_team_cost))
        .route("/teams/{name}/sidechains", get(get_team_sidechains))
}
