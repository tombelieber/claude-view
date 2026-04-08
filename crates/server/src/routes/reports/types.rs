// crates/server/src/routes/reports/types.rs
//! Request/response types and concurrency guard for report routes.

use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};

/// Guard to prevent concurrent report generation.
pub(super) static GENERATING: AtomicBool = AtomicBool::new(false);

/// RAII guard that resets GENERATING to false on drop.
/// Ensures the lock is released even if the SSE stream is dropped (client disconnect).
pub(super) struct GeneratingGuard;

impl Drop for GeneratingGuard {
    fn drop(&mut self) {
        GENERATING.store(false, Ordering::SeqCst);
    }
}

/// Request body for POST /api/reports/generate.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub report_type: String,
    pub date_start: String,
    pub date_end: String,
    /// Unix timestamp for range start (from frontend, uses local midnight).
    pub start_ts: i64,
    /// Unix timestamp for range end (from frontend, uses local midnight + 86399).
    pub end_ts: i64,
}

/// Query params for GET /api/reports/preview.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PreviewQuery {
    pub start_ts: i64,
    pub end_ts: i64,
}
