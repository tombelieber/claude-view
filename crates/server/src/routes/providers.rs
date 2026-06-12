//! GET /api/providers — session counts per source agent.
//!
//! Drives the provider filter in the history view: the UI only offers
//! providers that actually have sessions on this machine (zero-count
//! providers would be 20+ useless checkboxes).

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use ts_rs::TS;

use crate::state::AppState;

/// One provider with at least one discovered session.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProviderSummary {
    /// Kebab id used in the `providers` filter param ("claude-code", "codex", …).
    pub id: String,
    pub display_name: String,
    pub count: usize,
}

/// Response for GET /api/providers.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderSummary>,
}

/// Per-provider usage aggregates for the analytics "By agent" surface.
/// Cost semantics (trust gate): `cost_usd` sums ONLY sessions whose every
/// token resolved to a priced model; `priced_sessions`/`usage_sessions`
/// make the coverage explicit so the UI never implies a complete total.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub id: String,
    pub display_name: String,
    pub sessions: usize,
    #[ts(type = "number")]
    pub input_tokens: u64,
    #[ts(type = "number")]
    pub output_tokens: u64,
    #[ts(type = "number")]
    pub cache_read_tokens: u64,
    #[ts(type = "number")]
    pub cache_creation_tokens: u64,
    /// Sessions that carried any token accounting at all.
    pub usage_sessions: usize,
    /// Sessions whose full usage resolved to priced models.
    pub priced_sessions: usize,
    /// Sum over priced sessions only; `None` when no session priced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

/// Response for GET /api/providers/usage.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProvidersUsageResponse {
    pub days: u32,
    pub providers: Vec<ProviderUsage>,
}

/// Query for GET /api/providers/usage.
#[derive(Debug, serde::Deserialize, Default, utoipa::IntoParams)]
#[serde(default)]
pub struct ProvidersUsageQuery {
    /// Time window in days (default 30, max 365).
    pub days: Option<u32>,
}

/// GET /api/providers/usage — token/cost aggregates per foreign provider
/// within a trailing window. Claude Code is NOT included: CC usage comes
/// from the existing rollup pipeline; this endpoint reads the foreign
/// catalog's cached metadata (no files parsed beyond the cache fill).
#[utoipa::path(get, path = "/api/providers/usage", tag = "providers",
    params(ProvidersUsageQuery),
    responses((status = 200, description = "Per-provider usage aggregates", body = ProvidersUsageResponse))
)]
pub async fn providers_usage(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ProvidersUsageQuery>,
) -> Json<ProvidersUsageResponse> {
    let days = query.days.unwrap_or(30).clamp(1, 365);
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        - i64::from(days) * 86_400;

    let state_for_task = Arc::clone(&state);
    let providers = tokio::task::spawn_blocking(move || {
        let mut acc: HashMap<claude_view_providers::ProviderKind, ProviderUsage> = HashMap::new();
        for row in state_for_task.foreign_catalog.rows() {
            let Some(meta) = state_for_task.foreign_catalog.meta_for(&row) else {
                continue;
            };
            let modified = meta.ended_at.or(meta.started_at).unwrap_or(row.mtime) as i64;
            if modified < cutoff {
                continue;
            }
            let entry = acc.entry(meta.provider).or_insert_with(|| ProviderUsage {
                id: meta.provider.as_str().to_string(),
                display_name: meta.provider.display_name().to_string(),
                sessions: 0,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                usage_sessions: 0,
                priced_sessions: 0,
                cost_usd: None,
            });
            entry.sessions += 1;
            if meta.usage.has_usage {
                entry.usage_sessions += 1;
                let t = &meta.usage.totals;
                entry.input_tokens += t.input_tokens;
                entry.output_tokens += t.output_tokens;
                entry.cache_read_tokens += t.cache_read_input_tokens;
                entry.cache_creation_tokens += t.cache_creation_input_tokens;
            }
            if let Some(cost) =
                super::sessions::foreign::foreign_cost_usd(&meta, &state_for_task.pricing)
            {
                entry.priced_sessions += 1;
                *entry.cost_usd.get_or_insert(0.0) += cost;
            }
        }
        let mut providers: Vec<ProviderUsage> = acc.into_values().collect();
        providers.sort_by(|a, b| {
            (b.input_tokens + b.output_tokens)
                .cmp(&(a.input_tokens + a.output_tokens))
                .then_with(|| b.sessions.cmp(&a.sessions))
                .then_with(|| a.id.cmp(&b.id))
        });
        providers
    })
    .await
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "providers usage aggregation panicked");
        Vec::new()
    });

    Json(ProvidersUsageResponse { days, providers })
}

/// GET /api/providers — providers with session counts (count > 0 only,
/// Claude Code always first).
#[utoipa::path(get, path = "/api/providers", tag = "providers",
    responses((status = 200, description = "Providers with session counts", body = ProvidersResponse))
)]
pub async fn list_providers(State(state): State<Arc<AppState>>) -> Json<ProvidersResponse> {
    let claude_count = state
        .session_catalog
        .list(
            &claude_view_core::session_catalog::Filter::default(),
            claude_view_core::session_catalog::Sort::LastTsDesc,
            usize::MAX,
        )
        .len();

    let mut counts: HashMap<claude_view_providers::ProviderKind, usize> = HashMap::new();
    for row in state.foreign_catalog.rows() {
        *counts.entry(row.provider).or_default() += 1;
    }

    let mut providers = vec![ProviderSummary {
        id: super::sessions::foreign::CLAUDE_PROVIDER_ID.to_string(),
        display_name: "Claude Code".to_string(),
        count: claude_count,
    }];
    let mut foreign: Vec<ProviderSummary> = counts
        .into_iter()
        .map(|(kind, count)| ProviderSummary {
            id: kind.as_str().to_string(),
            display_name: kind.display_name().to_string(),
            count,
        })
        .collect();
    foreign.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
    providers.extend(foreign);

    Json(ProvidersResponse { providers })
}

/// Create the providers routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/providers", get(list_providers))
        .route("/providers/usage", get(providers_usage))
}
