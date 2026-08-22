//! GET /api/profiles — session counts per Claude config directory.
//!
//! Drives the profile filter in the history view. Claude Code reads its
//! config from `~/.claude` unless `CLAUDE_CONFIG_DIR` says otherwise, and
//! people who wrap the CLI in per-context launchers end up with sessions
//! spread across several config dirs. This endpoint reports which ones
//! actually have sessions, so the UI can offer a filter only when there is
//! something to filter between.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use ts_rs::TS;

use crate::error::ApiResult;
use crate::state::AppState;

/// One Claude config dir with at least one indexed session.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    /// Short name used in the `profiles` filter param: `default` for
    /// `~/.claude`, otherwise the suffix (`~/.claude-work` -> `work`).
    pub id: String,
    /// Absolute config dir path, for a tooltip.
    pub config_dir: String,
    pub count: usize,
}

/// Response for GET /api/profiles.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProfilesResponse {
    pub profiles: Vec<ProfileSummary>,
}

#[utoipa::path(
    get,
    path = "/api/profiles",
    tag = "profiles",
    responses((status = 200, description = "Session counts per Claude config dir", body = ProfilesResponse))
)]
pub async fn list_profiles(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ProfilesResponse>> {
    let counts = state.db.count_sessions_by_config_dir().await?;

    let profiles = counts
        .into_iter()
        // Sessions indexed before attribution existed carry an empty
        // config_dir. They belong to no profile, so offering them as one
        // would produce a checkbox that filters to "unknown".
        .filter(|(config_dir, _)| !config_dir.is_empty())
        .map(|(config_dir, count)| ProfileSummary {
            id: claude_view_core::discovery::profile_name(std::path::Path::new(&config_dir)),
            config_dir,
            count,
        })
        .collect();

    Ok(Json(ProfilesResponse { profiles }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/profiles", get(list_profiles))
}
