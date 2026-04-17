//! GET /api/sessions — JSONL-first list handler with DB enrichment.
//!
//! Pipeline:
//!   1. SessionCatalog filter (project, time window) — in-memory, cheap.
//!   2. Optional `q` full-text search — intersects IDs from the tantivy
//!      index via `search_service::execute_search`.
//!   3. `session_stats::extract_stats` for every surviving candidate —
//!      ~0.28ms p95 per session (JSONL parse).
//!   4. `enrichment::fetch_enrichments` — bulk DB lookup for archived_at,
//!      commit_count, skills_used, reedit_rate (single query via json_each).
//!   5. Apply filters/sort/pagination in Rust, return `SessionsListResponse`.
//!
//! The DB is no longer the primary source for session metadata — it is
//! only consulted for user state (archive), git correlation (commit_count),
//! and classifier output (skills_used). Every field derivable from the
//! JSONL file comes from `session_stats`.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use claude_view_core::session_catalog::{Filter as CatFilter, Sort as CatSort};
use claude_view_core::{session_stats, SessionInfo};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::enrichment::{fetch_enrichments, SessionEnrichment};
use super::helpers::build_session_info;
use super::types::{SessionsListQuery, SessionsListResponse, VALID_FILTERS, VALID_SORTS};

/// GET /api/sessions — list sessions with filter/sort/search.
///
/// See the module docstring for the pipeline. See
/// [`SessionsListQuery`](super::types::SessionsListQuery) for the full set
/// of supported parameters.
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
    let filter = query.filter.clone().unwrap_or_else(|| "all".to_string());
    let sort = query.sort.clone().unwrap_or_else(|| "recent".to_string());
    let limit = query.limit.unwrap_or(30).max(1);
    let offset = query.offset.unwrap_or(0).max(0);

    if !VALID_FILTERS.contains(&filter.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid filter '{}'. Valid options: {}",
            filter,
            VALID_FILTERS.join(", ")
        )));
    }
    if !VALID_SORTS.contains(&sort.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid sort '{}'. Valid options: {}",
            sort,
            VALID_SORTS.join(", ")
        )));
    }

    // Map legacy single-value `filter` into the structured boolean filters.
    let has_commits_param = match (query.has_commits, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "has_commits") => Some(true),
        _ => None,
    };
    let high_reedit_param = match (query.high_reedit, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "high_reedit") => Some(true),
        _ => None,
    };
    let min_duration_param = match (query.min_duration, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "long_session") => Some(1800),
        _ => None,
    };

    // 1. Catalog filter (project, time window). Catalog rows are in memory.
    let cat_filter = CatFilter {
        project_id: query.project.clone(),
        min_last_ts: query.time_after,
        max_last_ts: query.time_before,
    };
    let mut candidate_rows =
        state
            .session_catalog
            .list(&cat_filter, CatSort::LastTsDesc, usize::MAX);

    // 2. Optional full-text search via tantivy — intersect IDs with catalog.
    if let Some(q_trimmed) = query.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let search_filters = crate::search_service::SearchFilters {
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
                chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.format("%Y-%m-%d").to_string())
            }),
            before: query.time_before.and_then(|ts| {
                chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.format("%Y-%m-%d").to_string())
            }),
        };
        match crate::search_service::execute_search(
            &state,
            q_trimmed,
            &search_filters,
            usize::MAX,
            0,
            true,
        )
        .await
        {
            Ok(resp) => {
                let hits: std::collections::HashSet<String> =
                    resp.sessions.into_iter().map(|s| s.session_id).collect();
                candidate_rows.retain(|row| hits.contains(&row.id));
            }
            Err(e) => {
                tracing::warn!(error = %e, "Unified search failed; returning empty set for q");
                candidate_rows.clear();
            }
        }
    }

    // 3. Extract JSONL stats for every candidate. Skips rows whose file
    //    can't be parsed (deleted/corrupt JSONL) — those never reach the UI.
    let pricing = &state.pricing;
    let mut enriched: Vec<SessionInfo> = candidate_rows
        .iter()
        .filter_map(|row| {
            session_stats::extract_stats(&row.file_path, row.is_compressed)
                .ok()
                .map(|stats| build_session_info(row, &stats, pricing))
        })
        .collect();

    // 4. Layer DB-only fields (archive, commits, skills, reedit_rate).
    let candidate_ids: Vec<String> = enriched.iter().map(|s| s.id.clone()).collect();
    let enrichment_map = fetch_enrichments(&state.db, &candidate_ids)
        .await
        .map_err(ApiError::from)?;

    // 5. Filter + sort + paginate in Rust.
    let models_filter: Vec<String> = query
        .models
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // Branches filter splits the `~` NO_BRANCH_SENTINEL out of the named list:
    // matches sessions whose git_branch is NULL. Named entries match the
    // stored branch string exactly. Mixed ("~,main") means NULL OR main.
    let branches_filter_raw: Vec<String> = query
        .branches
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|b| b.trim().to_string())
                .filter(|b| !b.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let branches_include_none = branches_filter_raw
        .iter()
        .any(|b| b == claude_view_core::NO_BRANCH_SENTINEL);
    let branches_named: Vec<String> = branches_filter_raw
        .iter()
        .filter(|b| b.as_str() != claude_view_core::NO_BRANCH_SENTINEL)
        .cloned()
        .collect();
    let branches_filter_active = !branches_filter_raw.is_empty();
    let show_archived = query.show_archived.unwrap_or(false);
    let has_skills_param = query.has_skills;
    let min_files_param = query.min_files;
    let min_tokens_param = query.min_tokens;

    let filtered: Vec<SessionInfo> = enriched
        .drain(..)
        .filter(|info| {
            let default_enrichment = SessionEnrichment::default();
            let enr = enrichment_map.get(&info.id).unwrap_or(&default_enrichment);

            // Archive mode: show_archived=true → only archived; false/None → exclude archived.
            let is_archived = enr.archived_at.is_some();
            if show_archived {
                if !is_archived {
                    return false;
                }
            } else if is_archived {
                return false;
            }

            if let Some(true) = has_commits_param {
                if enr.commit_count == 0 {
                    return false;
                }
            }
            if let Some(false) = has_commits_param {
                if enr.commit_count > 0 {
                    return false;
                }
            }
            if let Some(true) = has_skills_param {
                if enr.skills_used.is_empty() {
                    return false;
                }
            }
            if let Some(false) = has_skills_param {
                if !enr.skills_used.is_empty() {
                    return false;
                }
            }
            if let Some(true) = high_reedit_param {
                if enr.reedit_rate <= 0.2 {
                    return false;
                }
            }
            if let Some(min) = min_duration_param {
                if (info.duration_seconds as i64) < min {
                    return false;
                }
            }
            if let Some(min) = min_files_param {
                if ((info.files_read_count + info.files_edited_count) as i64) < min {
                    return false;
                }
            }
            if let Some(min) = min_tokens_param {
                let total =
                    info.total_input_tokens.unwrap_or(0) + info.total_output_tokens.unwrap_or(0);
                if (total as i64) < min {
                    return false;
                }
            }
            if !models_filter.is_empty() {
                let primary = info.primary_model.as_deref().unwrap_or("");
                if !models_filter.iter().any(|m| m == primary) {
                    return false;
                }
            }
            if branches_filter_active {
                let matched = match &info.git_branch {
                    None => branches_include_none,
                    Some(b) => branches_named.iter().any(|n| n == b),
                };
                if !matched {
                    return false;
                }
            }

            true
        })
        .collect();

    let mut sorted = filtered;
    match sort.as_str() {
        "tokens" => sorted.sort_by(|a, b| {
            let ka = a.total_input_tokens.unwrap_or(0) + a.total_output_tokens.unwrap_or(0);
            let kb = b.total_input_tokens.unwrap_or(0) + b.total_output_tokens.unwrap_or(0);
            kb.cmp(&ka)
        }),
        "prompts" => sorted.sort_by(|a, b| b.user_prompt_count.cmp(&a.user_prompt_count)),
        "files_edited" => sorted.sort_by(|a, b| b.files_edited_count.cmp(&a.files_edited_count)),
        "duration" => sorted.sort_by(|a, b| b.duration_seconds.cmp(&a.duration_seconds)),
        _ => sorted.sort_by(|a, b| b.modified_at.cmp(&a.modified_at)), // recent
    }

    let total = sorted.len();
    let sessions: Vec<SessionInfo> = sorted
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    let has_more = (offset as usize + limit as usize) < total;

    Ok(Json(SessionsListResponse {
        sessions,
        total,
        has_more,
        filter,
        sort,
    }))
}
