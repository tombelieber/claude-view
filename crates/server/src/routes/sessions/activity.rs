//! Activity, branches, and hook event endpoints.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;

use crate::error::ApiResult;
use crate::state::AppState;

use super::types::{RichActivityParams, SessionActivityResponse, SparklineActivityParams};

/// GET /api/branches - Get distinct list of branch names across all sessions.
///
/// Returns a sorted array of unique branch names found in the database.
/// Excludes sessions without a branch (NULL git_branch).
#[utoipa::path(get, path = "/api/branches", tag = "sessions",
    responses(
        (status = 200, description = "Sorted list of unique branch names", body = Vec<String>),
    )
)]
pub async fn list_branches(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<String>>> {
    // Fetch all projects with sessions
    let projects = state.db.list_projects().await?;

    // Collect all unique branch names
    let mut branches: Vec<String> = projects
        .into_iter()
        .flat_map(|p| p.sessions)
        .filter_map(|s| s.git_branch)
        .collect();

    // Sort and deduplicate
    branches.sort();
    branches.dedup();

    Ok(Json(branches))
}

/// GET /api/sessions/:id/hook-events — Fetch hook events for a session.
///
/// For live sessions, hook events are in memory (not flushed to SQLite until
/// SessionEnd). Check the live state first, then fall back to SQLite.
///
/// **Phase 3 PR 3.5 — no cutover.** Hook events live on the
/// `hook_events` table (Channel B per design decision D8 of
/// `2026-04-17-cqrs-phase-1-7-design.md` §1). Channel B is
/// authoritative for real-time agent state, and `session_stats` is
/// authoritative for parsed message stats — they carry different data
/// and never dedup against each other (see CLAUDE.md "Separate
/// Channels = Separate Data"). This handler is therefore unchanged by
/// Phase 3; the openapi_compatibility test pins the response shape.
#[utoipa::path(get, path = "/api/sessions/{id}/hook-events", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Hook events for the session", body = serde_json::Value),
    )
)]
pub async fn get_session_hook_events(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    // Check live sessions first — hooks accumulate in memory and are only
    // flushed to SQLite on SessionEnd. Without this, the history view shows
    // hook=0 for any session that hasn't ended yet.
    {
        let sessions = state.live_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            if !session.hook.hook_events.is_empty() {
                let json_events: Vec<serde_json::Value> = session
                    .hook
                    .hook_events
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "timestamp": e.timestamp,
                            "eventName": e.event_name,
                            "toolName": e.tool_name,
                            "label": e.label,
                            "group": e.group,
                            "context": e.context,
                            "source": e.source,
                        })
                    })
                    .collect();
                return Json(serde_json::json!({ "hookEvents": json_events }));
            }
        }
    }

    // Fall back to SQLite for completed sessions
    match claude_view_db::hook_events_queries::get_hook_events(&state.db, &session_id).await {
        Ok(events) => {
            let json_events: Vec<serde_json::Value> = events
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "timestamp": e.timestamp,
                        "eventName": e.event_name,
                        "toolName": e.tool_name,
                        "label": e.label,
                        "group": e.group_name,
                        "context": e.context,
                        "source": e.source,
                    })
                })
                .collect();
            Json(serde_json::json!({ "hookEvents": json_events }))
        }
        Err(e) => Json(serde_json::json!({ "hookEvents": [], "error": e.to_string() })),
    }
}

/// GET /api/sessions/activity — Activity histogram for sparkline chart.
#[utoipa::path(get, path = "/api/sessions/activity", tag = "sessions",
    params(
        ("time_after" = Option<i64>, Query, description = "Unix timestamp lower bound"),
        ("time_before" = Option<i64>, Query, description = "Unix timestamp upper bound"),
    ),
    responses(
        (status = 200, description = "Activity histogram with date buckets and session count", body = SessionActivityResponse),
    )
)]
pub async fn session_activity(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SparklineActivityParams>,
) -> ApiResult<Json<SessionActivityResponse>> {
    let (activity, bucket) = state
        .db
        .session_activity_histogram(params.time_after, params.time_before)
        .await?;
    // When time-filtered, total = sum of histogram counts (matches the chart).
    // When unfiltered, use the full DB count (includes sessions with last_message_at=0
    // that can't appear on the chart axis).
    let total = if params.time_after.is_some() || params.time_before.is_some() {
        activity.iter().map(|a| a.count as usize).sum()
    } else {
        state.db.get_session_count().await? as usize
    };
    Ok(Json(SessionActivityResponse {
        activity,
        bucket,
        total,
    }))
}

/// GET /api/sessions/activity/rich — Full server-side activity aggregation.
///
/// Returns histogram, project breakdown, and summary stats in a single request.
/// Replaces the client-side `useActivityData` pagination loop.
#[utoipa::path(get, path = "/api/sessions/activity/rich", tag = "sessions",
    params(
        ("time_after" = Option<i64>, Query, description = "Unix timestamp lower bound"),
        ("time_before" = Option<i64>, Query, description = "Unix timestamp upper bound"),
        ("project" = Option<String>, Query, description = "Filter by project path"),
        ("branch" = Option<String>, Query, description = "Filter by git branch"),
    ),
    responses(
        (status = 200, description = "Rich activity data", body = claude_view_db::RichActivityResponse),
    )
)]
pub async fn session_activity_rich(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RichActivityParams>,
) -> ApiResult<Json<claude_view_db::RichActivityResponse>> {
    let result = state
        .db
        .rich_activity(
            params.time_after,
            params.time_before,
            params.project.as_deref(),
            params.branch.as_deref(),
        )
        .await?;
    Ok(Json(result))
}
