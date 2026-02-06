// crates/server/src/routes/system.rs
//! System status and management endpoints for the System page.
//!
//! - GET  /system           — Comprehensive system status
//! - POST /system/reindex   — Trigger full re-index
//! - POST /system/clear-cache — Clear search index and cache
//! - POST /system/git-resync — Trigger full git re-sync (stub -- not yet implemented)
//! - POST /system/reset     — Factory reset (requires confirmation)

use std::sync::Arc;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use vibe_recall_core::ClaudeCliStatus;
use vibe_recall_db::{ClassificationStatus, HealthStats, HealthStatus, StorageStats};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ============================================================================
// Response Types
// ============================================================================

/// Full system status response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SystemResponse {
    pub storage: StorageInfo,
    pub performance: PerformanceInfo,
    pub health: HealthInfo,
    pub index_history: Vec<IndexRunInfo>,
    pub classification: ClassificationInfo,
    pub claude_cli: ClaudeCliStatus,
}

/// Storage section of system response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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

impl From<StorageStats> for StorageInfo {
    fn from(s: StorageStats) -> Self {
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
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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

/// Index history entry in system response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ActionResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Clear cache response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/system - Get comprehensive system status.
///
/// Runs storage, health, and classification queries in parallel
/// using tokio::join!, then detects Claude CLI status.
pub async fn get_system_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SystemResponse>> {
    // Run independent queries in parallel
    let (storage_result, health_result, metadata_result, index_runs_result, classification_result) =
        tokio::join!(
            state.db.get_storage_stats(),
            state.db.get_health_stats(),
            state.db.get_index_metadata(),
            state.db.get_recent_index_runs(),
            state.db.get_classification_status(),
        );

    let storage_stats = storage_result?;
    let health_stats = health_result?;
    let metadata = metadata_result?;
    let index_runs = index_runs_result?;
    let classification = classification_result?;

    // Detect Claude CLI (runs shell commands - fast enough for API call)
    let claude_cli = tokio::task::spawn_blocking(ClaudeCliStatus::detect)
        .await
        .unwrap_or_else(|_| ClaudeCliStatus::default());

    // Calculate performance metrics from index metadata
    let performance = calculate_performance(&metadata, &storage_stats);

    // Convert index runs to response format
    let index_history: Vec<IndexRunInfo> = index_runs
        .into_iter()
        .map(|run| IndexRunInfo {
            timestamp: run.started_at,
            run_type: run.run_type.as_db_str().to_string(),
            sessions_count: run.sessions_after,
            duration_ms: run.duration_ms,
            status: run.status.as_db_str().to_string(),
            error_message: run.error_message,
        })
        .collect();

    let response = SystemResponse {
        storage: storage_stats.into(),
        performance,
        health: health_stats.into(),
        index_history,
        classification: classification.into(),
        claude_cli,
    };

    Ok(Json(response))
}

/// Calculate performance metrics from index metadata and storage stats.
fn calculate_performance(
    metadata: &vibe_recall_db::IndexMetadata,
    storage: &StorageStats,
) -> PerformanceInfo {
    let last_index_duration_ms = metadata.last_index_duration_ms;

    let throughput_bytes_per_sec = match (last_index_duration_ms, storage.jsonl_bytes) {
        (Some(duration_ms), bytes) if duration_ms > 0 => {
            Some((bytes as f64 / (duration_ms as f64 / 1000.0)) as u64)
        }
        _ => None,
    };

    let sessions_per_sec = match (last_index_duration_ms, metadata.sessions_indexed) {
        (Some(duration_ms), sessions) if duration_ms > 0 => {
            Some(sessions as f64 / (duration_ms as f64 / 1000.0))
        }
        _ => None,
    };

    PerformanceInfo {
        last_index_duration_ms,
        throughput_bytes_per_sec,
        sessions_per_sec,
    }
}

/// POST /api/system/reindex - Trigger a full re-index.
///
/// This is a lightweight endpoint that signals the server to start
/// a background re-index. The actual work happens asynchronously.
pub async fn trigger_reindex(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ActionResponse>> {
    // Record the index run in the database
    let _run_id = state
        .db
        .create_index_run("full", None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create index run: {}", e)))?;

    // Signal the indexing state to trigger a re-index
    state.indexing.trigger_reindex();

    Ok(Json(ActionResponse {
        status: "started".to_string(),
        message: Some("Full re-index started".to_string()),
    }))
}

/// POST /api/system/clear-cache - Clear search index and cached data.
pub async fn clear_cache(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<ClearCacheResponse>> {
    // Calculate size of cache before clearing
    let cache_dir = dirs::cache_dir()
        .map(|d| d.join("vibe-recall").join("index"));

    let cleared_bytes = if let Some(ref dir) = cache_dir {
        if dir.exists() {
            // Calculate size first
            let size = calculate_dir_size(dir);
            // Remove the directory
            if let Err(e) = std::fs::remove_dir_all(dir) {
                tracing::warn!("Failed to clear cache directory: {}", e);
                0
            } else {
                size
            }
        } else {
            0
        }
    } else {
        0
    };

    Ok(Json(ClearCacheResponse {
        status: "success".to_string(),
        cleared_bytes,
    }))
}

/// POST /api/system/git-resync - Trigger full git re-sync.
pub async fn trigger_git_resync(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<ActionResponse>> {
    Ok(Json(ActionResponse {
        status: "not_implemented".to_string(),
        message: Some("Git re-sync is not yet available".to_string()),
    }))
}

/// POST /api/system/reset - Factory reset all data.
///
/// Requires a confirmation string "RESET_ALL_DATA" in the request body.
pub async fn reset_all(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ResetRequest>,
) -> ApiResult<Json<ActionResponse>> {
    // Require exact confirmation string
    if body.confirm != "RESET_ALL_DATA" {
        return Err(ApiError::BadRequest(
            "Invalid confirmation. Send {\"confirm\": \"RESET_ALL_DATA\"} to confirm."
                .to_string(),
        ));
    }

    state
        .db
        .reset_all_data()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reset data: {}", e)))?;

    Ok(Json(ActionResponse {
        status: "success".to_string(),
        message: Some("All data has been reset".to_string()),
    }))
}

/// Calculate the size of a directory recursively.
fn calculate_dir_size(dir: &std::path::Path) -> u64 {
    if !dir.exists() {
        return 0;
    }

    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                total += calculate_dir_size(&path);
            }
        }
    }
    total
}

// ============================================================================
// Router
// ============================================================================

/// Create the system routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system", get(get_system_status))
        .route("/system/reindex", post(trigger_reindex))
        .route("/system/clear-cache", post(clear_cache))
        .route("/system/git-resync", post(trigger_git_resync))
        .route("/system/reset", post(reset_all))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn do_post_json(
        app: axum::Router,
        uri: &str,
        json_body: &str,
    ) -> (StatusCode, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(json_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn do_post(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    // ========================================================================
    // GET /api/system tests
    // ========================================================================

    #[tokio::test]
    async fn test_system_endpoint_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/system").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Storage should exist with zeros
        assert!(json["storage"].is_object());
        assert_eq!(json["storage"]["jsonlBytes"], 0);
        assert_eq!(json["storage"]["dbBytes"], 0);

        // Performance should have null values
        assert!(json["performance"].is_object());
        assert!(json["performance"]["lastIndexDurationMs"].is_null());

        // Health should show 0 counts, healthy status
        assert!(json["health"].is_object());
        assert_eq!(json["health"]["sessionsCount"], 0);
        assert_eq!(json["health"]["commitsCount"], 0);
        assert_eq!(json["health"]["projectsCount"], 0);
        assert_eq!(json["health"]["status"], "healthy");

        // Index history should be empty
        assert!(json["indexHistory"].is_array());
        assert_eq!(json["indexHistory"].as_array().unwrap().len(), 0);

        // Classification should show zeros
        assert!(json["classification"].is_object());
        assert_eq!(json["classification"]["classifiedCount"], 0);
        assert_eq!(json["classification"]["unclassifiedCount"], 0);
        assert!(!json["classification"]["isRunning"].as_bool().unwrap());

        // Claude CLI should be present (may or may not be installed)
        assert!(json["claudeCli"].is_object());
    }

    #[tokio::test]
    async fn test_system_endpoint_with_index_metadata() {
        let db = test_db().await;

        // Set some index metadata
        db.update_index_metadata_on_success(2800, 6712, 47)
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/system").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Performance should reflect the metadata
        assert_eq!(json["performance"]["lastIndexDurationMs"], 2800);
        assert!(json["performance"]["sessionsPerSec"].is_number());

        // Health should show healthy with a recent sync
        assert_eq!(json["health"]["status"], "healthy");
        assert!(json["health"]["lastSyncAt"].is_string());
    }

    #[tokio::test]
    async fn test_system_endpoint_with_sessions() {
        let db = test_db().await;

        // Insert a session
        db.insert_session_from_index(
            "sess-1",
            "project-a",
            "Project A",
            "/tmp/project-a",
            "/tmp/sess1.jsonl",
            "Test session",
            None,
            5,
            chrono::Utc::now().timestamp(),
            None,
            false,
            1000,
        )
        .await
        .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/system").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(json["health"]["sessionsCount"], 1);
        assert_eq!(json["health"]["projectsCount"], 1);

        // Unclassified should be 1 since no classification done
        assert_eq!(json["classification"]["unclassifiedCount"], 1);
    }

    #[tokio::test]
    async fn test_system_endpoint_with_index_runs() {
        let db = test_db().await;

        // Create an index run
        let run_id = db.create_index_run("full", Some(0)).await.unwrap();
        db.complete_index_run(run_id, Some(100), 2500, Some(5.2))
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/system").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        let history = json["indexHistory"].as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["type"], "full");
        assert_eq!(history[0]["status"], "completed");
        assert_eq!(history[0]["sessionsCount"], 100);
        assert_eq!(history[0]["durationMs"], 2500);
    }

    // ========================================================================
    // POST /api/system/reindex tests
    // ========================================================================

    #[tokio::test]
    async fn test_reindex_endpoint() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_post(app, "/api/system/reindex").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "started");
        assert!(json["message"].as_str().unwrap().contains("re-index"));
    }

    // ========================================================================
    // POST /api/system/clear-cache tests
    // ========================================================================

    #[tokio::test]
    async fn test_clear_cache_endpoint() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_post(app, "/api/system/clear-cache").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "success");
        assert!(json["clearedBytes"].is_number());
    }

    // ========================================================================
    // POST /api/system/git-resync tests
    // ========================================================================

    #[tokio::test]
    async fn test_git_resync_endpoint() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_post(app, "/api/system/git-resync").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "not_implemented");
        assert!(json["message"].as_str().unwrap().contains("not yet available"));
    }

    // ========================================================================
    // POST /api/system/reset tests
    // ========================================================================

    #[tokio::test]
    async fn test_reset_requires_confirmation() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) = do_post_json(
            app,
            "/api/system/reset",
            r#"{"confirm": "wrong"}"#,
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_reset_with_correct_confirmation() {
        let db = test_db().await;

        // Insert some data first
        db.insert_session_from_index(
            "sess-1",
            "project-a",
            "Project A",
            "/tmp/project-a",
            "/tmp/sess1.jsonl",
            "Test session",
            None,
            5,
            chrono::Utc::now().timestamp(),
            None,
            false,
            1000,
        )
        .await
        .unwrap();

        // Verify data exists
        let health = db.get_health_stats().await.unwrap();
        assert_eq!(health.sessions_count, 1);

        let app = build_app(db.clone());
        let (status, body) = do_post_json(
            app,
            "/api/system/reset",
            r#"{"confirm": "RESET_ALL_DATA"}"#,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "success");

        // Verify data is cleared
        let health = db.get_health_stats().await.unwrap();
        assert_eq!(health.sessions_count, 0);
        assert_eq!(health.commits_count, 0);
    }

    #[tokio::test]
    async fn test_reset_without_body_fails() {
        let db = test_db().await;
        let app = build_app(db);

        // POST without a JSON body should fail
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/system/reset")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should get an error status (400, 415, or 422 depending on framework)
        assert!(
            response.status().is_client_error(),
            "Expected 4xx client error, got {}",
            response.status()
        );
    }

    // ========================================================================
    // Performance calculation tests
    // ========================================================================

    #[test]
    fn test_calculate_performance_with_data() {
        let metadata = vibe_recall_db::IndexMetadata {
            last_indexed_at: Some(1000),
            last_index_duration_ms: Some(2000),
            sessions_indexed: 1000,
            projects_indexed: 10,
            last_git_sync_at: None,
            commits_found: 0,
            links_created: 0,
            updated_at: 1000,
            git_sync_interval_secs: 60,
        };
        let storage = StorageStats {
            jsonl_bytes: 10_000_000,
            index_bytes: 0,
            db_bytes: 0,
            cache_bytes: 0,
            total_bytes: 10_000_000,
        };

        let perf = calculate_performance(&metadata, &storage);
        assert_eq!(perf.last_index_duration_ms, Some(2000));
        // 10MB in 2 seconds = 5MB/s = 5_000_000 bytes/sec
        assert_eq!(perf.throughput_bytes_per_sec, Some(5_000_000));
        // 1000 sessions in 2 seconds = 500 sessions/sec
        assert!((perf.sessions_per_sec.unwrap() - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_performance_empty() {
        let metadata = vibe_recall_db::IndexMetadata {
            last_indexed_at: None,
            last_index_duration_ms: None,
            sessions_indexed: 0,
            projects_indexed: 0,
            last_git_sync_at: None,
            commits_found: 0,
            links_created: 0,
            updated_at: 0,
            git_sync_interval_secs: 60,
        };
        let storage = StorageStats {
            jsonl_bytes: 0,
            index_bytes: 0,
            db_bytes: 0,
            cache_bytes: 0,
            total_bytes: 0,
        };

        let perf = calculate_performance(&metadata, &storage);
        assert!(perf.last_index_duration_ms.is_none());
        assert!(perf.throughput_bytes_per_sec.is_none());
        assert!(perf.sessions_per_sec.is_none());
    }

    #[test]
    fn test_calculate_performance_zero_duration() {
        let metadata = vibe_recall_db::IndexMetadata {
            last_indexed_at: Some(1000),
            last_index_duration_ms: Some(0),
            sessions_indexed: 100,
            projects_indexed: 5,
            last_git_sync_at: None,
            commits_found: 0,
            links_created: 0,
            updated_at: 1000,
            git_sync_interval_secs: 60,
        };
        let storage = StorageStats {
            jsonl_bytes: 1000,
            index_bytes: 0,
            db_bytes: 0,
            cache_bytes: 0,
            total_bytes: 1000,
        };

        let perf = calculate_performance(&metadata, &storage);
        // With 0 duration, throughput should be None (div by zero guard)
        assert!(perf.throughput_bytes_per_sec.is_none());
        assert!(perf.sessions_per_sec.is_none());
    }

    // ========================================================================
    // Health status tests
    // ========================================================================

    #[tokio::test]
    async fn test_health_status_healthy() {
        let db = test_db().await;
        let health = db.get_health_stats().await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_status_warning_on_failed_runs() {
        let db = test_db().await;

        // Create one failed index run
        let run_id = db.create_index_run("full", None).await.unwrap();
        db.fail_index_run(run_id, "test error").await.unwrap();

        let health = db.get_health_stats().await.unwrap();
        assert_eq!(health.errors_count, 1);
        assert_eq!(health.status, HealthStatus::Warning);
    }

    #[tokio::test]
    async fn test_health_status_error_on_many_failed_runs() {
        let db = test_db().await;

        // Create 10+ failed index runs
        for _ in 0..10 {
            let run_id = db.create_index_run("full", None).await.unwrap();
            db.fail_index_run(run_id, "test error").await.unwrap();
        }

        let health = db.get_health_stats().await.unwrap();
        assert_eq!(health.errors_count, 10);
        assert_eq!(health.status, HealthStatus::Error);
    }

    // ========================================================================
    // Storage stats tests
    // ========================================================================

    #[tokio::test]
    async fn test_storage_stats_empty_db() {
        let db = test_db().await;
        let stats = db.get_storage_stats().await.unwrap();
        assert_eq!(stats.jsonl_bytes, 0);
        assert_eq!(stats.index_bytes, 0);
        // In-memory DB has empty path, so db_bytes = 0
        assert_eq!(stats.db_bytes, 0);
        assert_eq!(stats.cache_bytes, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    // ========================================================================
    // Classification status tests
    // ========================================================================

    #[tokio::test]
    async fn test_classification_status_empty() {
        let db = test_db().await;
        let status = db.get_classification_status().await.unwrap();
        assert_eq!(status.classified_count, 0);
        assert_eq!(status.unclassified_count, 0);
        assert!(!status.is_running);
        assert!(status.progress.is_none());
    }

    #[tokio::test]
    async fn test_classification_status_with_active_job() {
        let db = test_db().await;

        // Insert a session
        db.insert_session_from_index(
            "sess-1",
            "project-a",
            "Project A",
            "/tmp/project-a",
            "/tmp/sess1.jsonl",
            "Test session",
            None,
            5,
            chrono::Utc::now().timestamp(),
            None,
            false,
            1000,
        )
        .await
        .unwrap();

        // Create a running classification job
        let _job_id = db
            .create_classification_job(1, "claude-cli", "haiku", None)
            .await
            .unwrap();

        let status = db.get_classification_status().await.unwrap();
        assert!(status.is_running);
        assert_eq!(status.unclassified_count, 1);
    }

    // ========================================================================
    // Reset tests
    // ========================================================================

    #[tokio::test]
    async fn test_reset_all_data() {
        let db = test_db().await;

        // Insert data
        db.insert_session_from_index(
            "sess-1",
            "project-a",
            "Project A",
            "/tmp/project-a",
            "/tmp/sess1.jsonl",
            "Test",
            None,
            5,
            chrono::Utc::now().timestamp(),
            None,
            false,
            1000,
        )
        .await
        .unwrap();

        // Create an index run
        let run_id = db.create_index_run("full", Some(0)).await.unwrap();
        db.complete_index_run(run_id, Some(1), 100, None)
            .await
            .unwrap();

        // Update metadata
        db.update_index_metadata_on_success(100, 1, 1)
            .await
            .unwrap();

        // Verify data exists
        let health = db.get_health_stats().await.unwrap();
        assert_eq!(health.sessions_count, 1);

        // Reset
        db.reset_all_data().await.unwrap();

        // Verify data is gone
        let health = db.get_health_stats().await.unwrap();
        assert_eq!(health.sessions_count, 0);
        assert_eq!(health.commits_count, 0);

        // Index runs should be gone
        let runs = db.get_recent_index_runs().await.unwrap();
        assert!(runs.is_empty());

        // Metadata should be reset
        let metadata = db.get_index_metadata().await.unwrap();
        assert!(metadata.last_indexed_at.is_none());
        assert_eq!(metadata.sessions_indexed, 0);
    }

    // ========================================================================
    // Router tests
    // ========================================================================

    #[test]
    fn test_system_router_creation() {
        let _router = router();
    }
}
