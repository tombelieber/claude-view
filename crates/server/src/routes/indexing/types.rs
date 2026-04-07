//! Request/response types for indexing endpoints.

use serde::Serialize;
use ts_rs::TS;

/// JSON snapshot of current indexing progress (for polling).
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IndexingStatusResponse {
    pub phase: String,
    pub indexed: usize,
    pub total: usize,
    #[ts(type = "number")]
    pub bytes_processed: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
