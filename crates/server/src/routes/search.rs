//! Full-text search endpoint.
//!
//! GET /search?q=...&limit=...&offset=...&project=...&branch=...&model=...&after=...&before=...
//!
//! Thin wrapper around `search_service::execute_search()`.

use crate::error::{ApiError, ApiResult};
use crate::search_service::{execute_search, SearchFilters};
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use claude_view_search::types::SearchResponse;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/search", get(search_handler))
}

async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let q = query.q.as_deref().unwrap_or("").trim();
    if q.is_empty() {
        return Err(ApiError::BadRequest(
            "query parameter 'q' is required".to_string(),
        ));
    }

    let limit = query.limit.unwrap_or(20);
    let offset = query.offset.unwrap_or(0);

    let filters = SearchFilters {
        project: query.project,
        branch: query.branch,
        model: query.model,
        after: query.after,
        before: query.before,
    };

    let response = execute_search(&state, q, &filters, limit, offset, false).await?;
    Ok(Json(response))
}
