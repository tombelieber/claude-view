//! Request / response types for the classification API.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request body for POST /api/classify.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyRequest {
    /// Which sessions to classify: "unclassified" or "all"
    pub mode: String,
    /// Dry run: calculate cost without executing
    #[serde(default)]
    pub dry_run: bool,
}

/// Response for POST /api/classify (202 Accepted).
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassifyResponse {
    #[ts(type = "number")]
    pub job_id: i64,
    #[ts(type = "number")]
    pub total_sessions: i64,
    pub status: String,
}

/// Response for POST /api/classify/cancel.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CancelResponse {
    #[ts(type = "number")]
    pub job_id: i64,
    #[ts(type = "number")]
    pub classified: u64,
    pub status: String,
}

/// Response for GET /api/classify/status.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassifyStatusResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub job_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<ClassifyProgressInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<ClassifyLastRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ClassifyErrorInfo>,
    #[ts(type = "number")]
    pub total_sessions: i64,
    #[ts(type = "number")]
    pub classified_sessions: i64,
    #[ts(type = "number")]
    pub unclassified_sessions: i64,
}

/// Progress information for a running classification.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassifyProgressInfo {
    #[ts(type = "number")]
    pub classified: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub percentage: f64,
    pub eta: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_batch: Option<String>,
}

/// Information about the last completed classification run.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassifyLastRun {
    #[ts(type = "number")]
    pub job_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[ts(type = "number")]
    pub sessions_classified: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub cost_cents: Option<i64>,
    #[ts(type = "number")]
    pub error_count: i64,
    pub status: String,
}

/// Error information for failed classification.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassifyErrorInfo {
    pub message: String,
    pub retryable: bool,
}

/// Response for POST /api/classify/single/:session_id.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassifySingleResponse {
    pub session_id: String,
    pub category_l1: String,
    pub category_l2: String,
    pub category_l3: String,
    pub confidence: f64,
    /// true if result was already cached (previously classified)
    pub was_cached: bool,
}

/// SSE progress event data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SseProgressData {
    pub classified: u64,
    pub total: u64,
    pub percentage: f64,
    pub eta: String,
}

/// SSE complete event data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SseCompleteData {
    pub job_id: i64,
    pub classified: u64,
}
