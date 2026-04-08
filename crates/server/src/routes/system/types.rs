//! Request and response types for system endpoints.

use claude_view_db::{
    ClassificationStatus, HealthStats, HealthStatus, IndexRunIntegrityCounters, SystemStorageStats,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Response Types
// ============================================================================

/// Full system status response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SystemResponse {
    pub storage: StorageInfo,
    pub performance: PerformanceInfo,
    pub health: HealthInfo,
    pub integrity: IntegrityInfo,
    pub index_history: Vec<IndexRunInfo>,
    pub classification: ClassificationInfo,
    pub claude_cli: claude_view_core::ClaudeCliStatus,
}

/// Storage section of system response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    #[ts(type = "number")]
    pub jsonl_bytes: u64,
    #[ts(type = "number")]
    pub index_bytes: u64,
    #[ts(type = "number")]
    pub db_bytes: u64,
    #[ts(type = "number")]
    pub cache_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

impl From<SystemStorageStats> for StorageInfo {
    fn from(s: SystemStorageStats) -> Self {
        Self {
            jsonl_bytes: s.jsonl_bytes,
            index_bytes: s.index_bytes,
            db_bytes: s.db_bytes,
            cache_bytes: s.cache_bytes,
            total_bytes: s.total_bytes,
        }
    }
}

/// Performance section of system response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PerformanceInfo {
    /// Duration of last successful index in milliseconds.
    #[ts(type = "number | null")]
    pub last_index_duration_ms: Option<i64>,
    /// Throughput: bytes processed per second during last index.
    #[ts(type = "number | null")]
    pub throughput_bytes_per_sec: Option<u64>,
    /// Sessions indexed per second during last index.
    pub sessions_per_sec: Option<f64>,
}

/// Health section of system response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct HealthInfo {
    #[ts(type = "number")]
    pub sessions_count: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    #[ts(type = "number")]
    pub projects_count: i64,
    #[ts(type = "number")]
    pub errors_count: i64,
    pub last_sync_at: Option<String>,
    pub status: HealthStatus,
}

impl From<HealthStats> for HealthInfo {
    fn from(h: HealthStats) -> Self {
        let last_sync_at = h.last_sync_at.map(|ts| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| ts.to_string())
        });
        Self {
            sessions_count: h.sessions_count,
            commits_count: h.commits_count,
            projects_count: h.projects_count,
            errors_count: h.errors_count,
            last_sync_at,
            status: h.status,
        }
    }
}

/// Integrity section of system response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IntegrityInfo {
    pub counters: IntegrityCounterInfo,
}

/// Integrity counter values from the latest index run.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IntegrityCounterInfo {
    #[ts(type = "number")]
    pub unknown_top_level_type_count: i64,
    #[ts(type = "number")]
    pub unknown_required_path_count: i64,
    #[ts(type = "number")]
    pub imaginary_path_access_count: i64,
    #[ts(type = "number")]
    pub legacy_fallback_path_count: i64,
    #[ts(type = "number")]
    pub dropped_line_invalid_json_count: i64,
    #[ts(type = "number")]
    pub schema_mismatch_count: i64,
    #[ts(type = "number")]
    pub unknown_source_role_count: i64,
    #[ts(type = "number")]
    pub derived_source_message_doc_count: i64,
    #[ts(type = "number")]
    pub source_message_non_source_provenance_count: i64,
}

impl From<IndexRunIntegrityCounters> for IntegrityCounterInfo {
    fn from(c: IndexRunIntegrityCounters) -> Self {
        Self {
            unknown_top_level_type_count: c.unknown_top_level_type_count,
            unknown_required_path_count: c.unknown_required_path_count,
            imaginary_path_access_count: c.imaginary_path_access_count,
            legacy_fallback_path_count: c.legacy_fallback_path_count,
            dropped_line_invalid_json_count: c.dropped_line_invalid_json_count,
            schema_mismatch_count: c.schema_mismatch_count,
            unknown_source_role_count: c.unknown_source_role_count,
            derived_source_message_doc_count: c.derived_source_message_doc_count,
            source_message_non_source_provenance_count: c
                .source_message_non_source_provenance_count,
        }
    }
}

/// Index history entry in system response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IndexRunInfo {
    pub timestamp: String,
    #[serde(rename = "type")]
    pub run_type: String,
    #[ts(type = "number | null")]
    pub sessions_count: Option<i64>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Classification section of system response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassificationInfo {
    #[ts(type = "number")]
    pub classified_count: i64,
    #[ts(type = "number")]
    pub unclassified_count: i64,
    pub last_run_at: Option<String>,
    #[ts(type = "number | null")]
    pub last_run_duration_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub last_run_cost_cents: Option<i64>,
    pub provider: String,
    pub model: String,
    pub is_running: bool,
    #[ts(type = "number | null")]
    pub progress: Option<i64>,
}

impl From<ClassificationStatus> for ClassificationInfo {
    fn from(c: ClassificationStatus) -> Self {
        Self {
            classified_count: c.classified_count,
            unclassified_count: c.unclassified_count,
            last_run_at: c.last_run_at,
            last_run_duration_ms: c.last_run_duration_ms,
            last_run_cost_cents: c.last_run_cost_cents,
            provider: c.provider,
            model: c.model,
            is_running: c.is_running,
            progress: c.progress,
        }
    }
}

/// Generic action response for POST endpoints.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ActionResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Clear cache response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClearCacheResponse {
    pub status: String,
    #[ts(type = "number")]
    pub cleared_bytes: u64,
}

/// Reset request body.
#[derive(Debug, Deserialize)]
pub struct ResetRequest {
    pub confirm: String,
}

/// Query parameters for the check-path endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct CheckPathQuery {
    pub path: String,
}

/// Response for the check-path endpoint.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CheckPathResponse {
    pub exists: bool,
}
