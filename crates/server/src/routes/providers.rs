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
    Router::new().route("/providers", get(list_providers))
}
