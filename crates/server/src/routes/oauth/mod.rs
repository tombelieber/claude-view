//! OAuth usage + identity endpoints.
//!
//! Wraps the undocumented Anthropic upstream `GET /api/oauth/usage` and the
//! local `claude auth status --json` subprocess, projecting both onto stable
//! frontend-facing response shapes that survive upstream evolution.
//!
//! Layout (one concern per file, per repo CLAUDE.md):
//!
//! - [`anthropic`] — upstream API types + fetcher
//! - [`tiers`] — frontend response + tier registry + trust gate
//! - [`identity`] — `/api/oauth/identity` handler + subprocess
//! - [`handlers`] — `/api/oauth/usage` + `/api/oauth/usage/refresh` handlers

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

mod anthropic;
mod handlers;
mod identity;
mod tiers;

// Re-exports for `state.rs` (cache type) and `openapi.rs` (path/schema refs).
//
// `utoipa::path` expands to a sibling `__path_<fn>` zero-sized type that the
// `OpenApi` derive looks up at `crate::routes::oauth::__path_<fn>`. We re-export
// both the handler and that companion so the lookup resolves.
pub use handlers::{
    __path_get_oauth_usage, __path_post_oauth_usage_refresh, get_oauth_usage,
    post_oauth_usage_refresh,
};
pub use identity::{__path_get_auth_identity, get_auth_identity, AuthIdentityResponse};
pub use tiers::{OAuthUsageResponse, TierKind, UsageTier};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/oauth/usage", get(get_oauth_usage))
        .route("/oauth/usage/refresh", post(post_oauth_usage_refresh))
        .route("/oauth/identity", get(get_auth_identity))
}
