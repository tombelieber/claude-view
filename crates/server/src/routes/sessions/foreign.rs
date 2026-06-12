//! Foreign-provider session integration for the sessions routes.
//!
//! Foreign sessions (Codex, Cursor, OpenCode, …) come from the
//! `ForeignCatalog`, never from the CC pipeline. Their ids are namespaced
//! `<provider>:<raw>`, so route handlers dispatch on
//! `ProviderKind::from_session_id` before touching CC path resolution.

use std::sync::Arc;

use axum::Json;
use claude_view_core::SessionInfo;
use claude_view_providers::{ForeignSessionMeta, ProviderKind};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::types::{
    DerivedMetrics, PaginatedBlocks, SessionDetail, SessionMessagesQuery, SessionsListQuery,
};

/// Provider id representing the native Claude Code pipeline in the
/// `providers` query filter.
pub const CLAUDE_PROVIDER_ID: &str = "claude-code";

/// Parse the comma-separated `providers` query param. `None` = no filter.
pub fn parse_providers_param(raw: Option<&str>) -> Option<Vec<String>> {
    let list: Vec<String> = raw?
        .split(',')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

/// Total session cost in USD — `Some` only when EVERY token in the session
/// is attributable to a priced model (per-model coverage must equal the
/// session totals). A partial cost is never emitted (trust gate).
fn foreign_cost_usd(
    meta: &ForeignSessionMeta,
    pricing: &claude_view_core::pricing::PricingTable,
) -> Option<f64> {
    let usage = &meta.usage;
    if !usage.has_usage || usage.per_model.is_empty() {
        return None;
    }
    let mut covered = claude_view_providers::UsageTotals::default();
    let mut total = 0.0f64;
    for (model, t) in &usage.per_model {
        let p = claude_view_core::pricing::lookup_foreign_pricing(model, pricing)?;
        total += claude_view_core::pricing::cost_for_totals(
            &p,
            t.input_tokens,
            t.output_tokens,
            t.cache_read_input_tokens,
            t.cache_creation_input_tokens,
        );
        covered.add(t);
    }
    // Unattributed usage (e.g. tokens recorded without a model name) means
    // the sum would understate the real cost — show nothing instead.
    (covered == usage.totals).then_some(total)
}

/// Convert parsed foreign metadata into the list/detail `SessionInfo` shape.
///
/// Fields the foreign formats genuinely don't carry stay at their truthful
/// defaults (`None` / 0) — the cards hide those sections.
pub fn meta_to_session_info(
    meta: &ForeignSessionMeta,
    size_bytes: u64,
    pricing: &claude_view_core::pricing::PricingTable,
) -> SessionInfo {
    let modified_at = meta.ended_at.or(meta.started_at).unwrap_or(0.0) as i64;
    let duration_seconds = match (meta.started_at, meta.ended_at) {
        (Some(s), Some(e)) if e > s => (e - s) as u32,
        _ => 0,
    };
    let usage = &meta.usage;
    let preview = meta
        .title
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| meta.first_message.clone());

    SessionInfo {
        id: meta.id.clone(),
        project: meta.project.clone(),
        project_path: meta.cwd.clone().unwrap_or_default(),
        display_name: meta.project.clone(),
        file_path: meta.source_path.display().to_string(),
        modified_at,
        size_bytes,
        preview,
        last_message: String::new(),
        message_count: meta.message_count as usize,
        turn_count: meta.user_message_count as usize,
        user_prompt_count: meta.user_message_count,
        git_branch: meta.git_branch.clone(),
        primary_model: meta.models.first().cloned(),
        total_input_tokens: usage.has_usage.then_some(usage.totals.input_tokens),
        total_output_tokens: usage.has_usage.then_some(usage.totals.output_tokens),
        total_cache_read_tokens: usage
            .has_usage
            .then_some(usage.totals.cache_read_input_tokens),
        total_cache_creation_tokens: usage
            .has_usage
            .then_some(usage.totals.cache_creation_input_tokens),
        duration_seconds,
        first_message_at: meta.started_at.map(|ts| ts as i64),
        total_cost_usd: foreign_cost_usd(meta, pricing),
        provider: Some(meta.provider.as_str().to_string()),
        ..Default::default()
    }
}

/// List foreign sessions as `SessionInfo`, applying the list-endpoint
/// filters that foreign sessions can honestly answer:
/// - `providers`: keep only listed provider ids.
/// - `project`: foreign sessions are EXCLUDED when a CC project filter is
///   active (CC project ids don't address foreign projects).
/// - `time_after` / `time_before`: against modified_at.
/// - `q`: case-insensitive substring over title/first message/project
///   (metadata search — foreign JSONL is not grep-indexed).
pub fn list_foreign(
    state: &AppState,
    query: &SessionsListQuery,
    providers_filter: Option<&[String]>,
) -> Vec<SessionInfo> {
    if query.project.is_some() {
        return Vec::new();
    }
    let q_lower = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);

    let mut out = Vec::new();
    for row in state.foreign_catalog.rows() {
        let provider_id = row.provider.as_str();
        if let Some(filter) = providers_filter {
            if !filter.iter().any(|p| p == provider_id) {
                continue;
            }
        }
        // Cheap stat-level time gate before parsing.
        if let Some(after) = query.time_after {
            if (row.mtime as i64) < after {
                continue;
            }
        }
        if let Some(before) = query.time_before {
            if (row.mtime as i64) > before {
                continue;
            }
        }
        let Some(meta) = state.foreign_catalog.meta_for(&row) else {
            continue;
        };
        if let Some(q) = &q_lower {
            let haystack = format!(
                "{} {} {}",
                meta.title.as_deref().unwrap_or(""),
                meta.first_message,
                meta.project
            )
            .to_lowercase();
            if !haystack.contains(q) {
                continue;
            }
        }
        out.push(meta_to_session_info(&meta, row.size_bytes, &state.pricing));
    }
    out
}

/// GET /api/sessions/{id}/messages for a foreign session.
/// Only `format=block` is supported — foreign sessions have no legacy
/// parsed format.
pub async fn foreign_messages(
    state: &Arc<AppState>,
    session_id: &str,
    query: &SessionMessagesQuery,
) -> ApiResult<Json<serde_json::Value>> {
    if query.format.as_deref() != Some("block") {
        return Err(ApiError::BadRequest(
            "foreign sessions support format=block only".to_string(),
        ));
    }
    let catalog = Arc::clone(&state.foreign_catalog);
    let id = session_id.to_string();
    let session = tokio::task::spawn_blocking(move || catalog.parse_session(&id))
        .await
        .map_err(|e| ApiError::Internal(format!("join error: {e}")))?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.to_string()))?;

    let blocks = session.blocks;
    let total = blocks.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(50);
    let end = std::cmp::min(offset + limit, total);
    let page: Vec<_> = if offset < total {
        blocks.into_iter().skip(offset).take(limit).collect()
    } else {
        vec![]
    };

    let result = PaginatedBlocks {
        blocks: page,
        total,
        offset,
        limit,
        has_more: end < total,
        forked_from: None,
        entrypoint: None,
    };
    Ok(Json(serde_json::to_value(result).unwrap()))
}

/// GET /api/sessions/{id} for a foreign session: SessionInfo + empty
/// CC-only enrichments (no commits/tasks/todos/plans for foreign agents).
pub async fn foreign_detail(
    state: &Arc<AppState>,
    session_id: &str,
) -> ApiResult<Json<SessionDetail>> {
    let row = state
        .foreign_catalog
        .get(session_id)
        .ok_or_else(|| ApiError::SessionNotFound(session_id.to_string()))?;
    let catalog = Arc::clone(&state.foreign_catalog);
    let row_for_meta = row.clone();
    let meta = tokio::task::spawn_blocking(move || catalog.meta_for(&row_for_meta))
        .await
        .map_err(|e| ApiError::Internal(format!("join error: {e}")))?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.to_string()))?;

    let info = meta_to_session_info(&meta, row.size_bytes, &state.pricing);
    let derived_metrics = DerivedMetrics::from(&info);
    Ok(Json(SessionDetail {
        info,
        commits: Vec::new(),
        derived_metrics,
        tasks: Vec::new(),
        todos: Vec::new(),
        has_plans: false,
        warnings: Vec::new(),
    }))
}

/// True when the id belongs to a foreign provider.
pub fn is_foreign_id(session_id: &str) -> bool {
    ProviderKind::from_session_id(session_id).is_some()
}
