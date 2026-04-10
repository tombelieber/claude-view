//! Webhook CRUD API routes.
//!
//! Manages webhook configurations in `~/.claude-view/notifications.json` and
//! signing secrets in `~/.claude-view/webhook-secrets.json`.
//!
//! - GET    /api/webhooks          — List all webhooks (secrets excluded)
//! - POST   /api/webhooks          — Create a new webhook (returns signing secret once)
//! - GET    /api/webhooks/{id}     — Get a single webhook
//! - PUT    /api/webhooks/{id}     — Update a webhook (partial update)
//! - DELETE /api/webhooks/{id}     — Delete a webhook
//! - POST   /api/webhooks/{id}/test — Test-send a webhook (501 placeholder)

pub mod handlers;
pub mod types;

#[cfg(test)]
mod tests;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::state::AppState;

/// Webhook management API router.
///
/// ## Dependencies
/// - `state.webhook_config_path` — path to `notifications.json`
/// - `state.webhook_secrets_path` — path to `webhook-secrets.json`
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/webhooks",
            get(handlers::list_webhooks).post(handlers::create_webhook),
        )
        .route(
            "/webhooks/{id}",
            get(handlers::get_webhook)
                .put(handlers::update_webhook)
                .delete(handlers::delete_webhook),
        )
        .route("/webhooks/{id}/test", post(handlers::test_send))
}
