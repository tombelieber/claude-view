//! Shared response types for sync endpoints.

use serde::Serialize;
use ts_rs::TS;

/// Status value for accepted sync responses.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Accepted,
}

/// Response for successful sync initiation.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SyncAcceptedResponse {
    pub message: String,
    pub status: SyncStatus,
}
