// crates/server/src/routes/reports/mod.rs
//! Report API routes.
//!
//! - POST /reports/generate — Stream-generate a report via Claude CLI (SSE)
//! - GET  /reports           — List all saved reports
//! - GET  /reports/:id       — Get a single report
//! - DELETE /reports/:id     — Delete a report
//! - GET  /reports/preview   — Aggregate preview stats for a date range

mod digest;
mod handlers;
mod types;

#[cfg(test)]
mod tests;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

// Re-export all public types
pub use types::{GenerateRequest, PreviewQuery};

// Re-export all public handlers
pub use handlers::{delete_report, generate_report, get_preview, get_report, list_reports};

// Re-export utoipa-generated __path_ types for OpenAPI schema registration
pub use handlers::{
    __path_delete_report, __path_generate_report, __path_get_preview, __path_get_report,
    __path_list_reports,
};

/// Build the reports router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/reports", get(list_reports))
        .route("/reports/preview", get(get_preview))
        .route("/reports/generate", post(generate_report))
        .route("/reports/{id}", get(get_report).delete(delete_report))
}
