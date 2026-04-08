//! GET /api/sessions — List all sessions with filter/sort.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::types::{SessionsListQuery, SessionsListResponse, VALID_FILTERS, VALID_SORTS};

/// GET /api/sessions - List all sessions with filter/sort (Step 20).
///
/// Filters:
/// - `all` (default): no filter
/// - `has_commits`: commit_count > 0
/// - `high_reedit`: reedit_rate > 0.2
/// - `long_session`: duration_seconds > 1800 (30 minutes)
///
/// Sorts:
/// - `recent` (default): first_message_at DESC
/// - `tokens`: (total_input + total_output) DESC
/// - `prompts`: user_prompt_count DESC
/// - `files_edited`: files_edited_count DESC
/// - `duration`: duration_seconds DESC
///
/// Returns 400 with valid options list for invalid filter/sort.
#[utoipa::path(get, path = "/api/sessions", tag = "sessions",
    params(SessionsListQuery),
    responses(
        (status = 200, description = "Paginated session list", body = SessionsListResponse),
        (status = 400, description = "Invalid filter or sort parameter"),
    )
)]
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SessionsListQuery>,
) -> ApiResult<Json<SessionsListResponse>> {
    let filter = query.filter.unwrap_or_else(|| "all".to_string());
    let sort = query.sort.unwrap_or_else(|| "recent".to_string());
    let limit = query.limit.unwrap_or(30);
    let offset = query.offset.unwrap_or(0);

    // Validate filter (kept for backward compat — legacy single-value filter)
    if !VALID_FILTERS.contains(&filter.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid filter '{}'. Valid options: {}",
            filter,
            VALID_FILTERS.join(", ")
        )));
    }

    // Validate sort
    if !VALID_SORTS.contains(&sort.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid sort '{}'. Valid options: {}",
            sort,
            VALID_SORTS.join(", ")
        )));
    }

    // Map legacy filter param to the new structured params
    let has_commits = match (query.has_commits, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "has_commits") => Some(true),
        _ => None,
    };
    let high_reedit = match (query.high_reedit, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "high_reedit") => Some(true),
        _ => None,
    };
    let min_duration = match (query.min_duration, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "long_session") => Some(1800),
        _ => None,
    };

    // Resolve text query via unified search (same engine as /api/search).
    let search_session_ids = if let Some(ref q_text) = query.q {
        let q_trimmed = q_text.trim();
        if q_trimmed.is_empty() {
            None
        } else {
            let filters = crate::search_service::SearchFilters {
                project: query.project.clone(),
                branch: query
                    .branches
                    .as_deref()
                    .and_then(|b| b.split(',').next().map(|s| s.trim().to_string())),
                model: query
                    .models
                    .as_deref()
                    .and_then(|m| m.split(',').next().map(|s| s.trim().to_string())),
                after: query.time_after.and_then(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                }),
                before: query.time_before.and_then(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                }),
            };
            // skip_snippets=true: we only need session IDs, not highlighted text.
            match crate::search_service::execute_search(
                &state,
                q_trimmed,
                &filters,
                usize::MAX,
                0,
                true,
            )
            .await
            {
                Ok(response) => {
                    let ids: Vec<String> = response
                        .sessions
                        .into_iter()
                        .map(|s| s.session_id)
                        .collect();
                    if ids.is_empty() {
                        None
                    } else {
                        Some(ids)
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Unified search failed, falling back to LIKE");
                    None
                }
            }
        }
    } else {
        None
    };

    let params = claude_view_db::SessionFilterParams {
        q: query.q,
        search_session_ids,
        branches: query
            .branches
            .map(|s| s.split(',').map(|b| b.trim().to_string()).collect()),
        models: query
            .models
            .map(|s| s.split(',').map(|m| m.trim().to_string()).collect()),
        has_commits,
        has_skills: query.has_skills,
        min_duration,
        min_files: query.min_files,
        min_tokens: query.min_tokens,
        high_reedit,
        time_after: query.time_after,
        time_before: query.time_before,
        project: query.project,
        show_archived: query.show_archived,
        sort: sort.clone(),
        limit,
        offset,
    };

    let (sessions, total) = state.db.query_sessions_filtered(&params).await?;
    let has_more = (offset + limit) < total as i64;

    Ok(Json(SessionsListResponse {
        sessions,
        total,
        has_more,
        filter,
        sort,
    }))
}
