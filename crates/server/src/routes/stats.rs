//! Dashboard statistics endpoint.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use ts_rs::TS;
use vibe_recall_core::{claude_projects_dir, DashboardStats};
use vibe_recall_db::trends::{TrendMetric, WeekTrends};

use crate::error::ApiResult;
use crate::metrics::record_request;
use crate::state::AppState;

/// Current week metrics for dashboard (Step 22).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CurrentWeekMetrics {
    #[ts(type = "number")]
    pub session_count: u64,
    #[ts(type = "number")]
    pub total_tokens: u64,
    #[ts(type = "number")]
    pub total_files_edited: u64,
    #[ts(type = "number")]
    pub commit_count: u64,
}

/// Extended dashboard stats with current week and trends.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ExtendedDashboardStats {
    /// Base dashboard stats
    #[serde(flatten)]
    pub base: DashboardStats,
    /// Current week metrics
    pub current_week: CurrentWeekMetrics,
    /// Week-over-week trends
    pub trends: DashboardTrends,
}

/// Simplified trends for dashboard display.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct DashboardTrends {
    /// Session count trend
    pub sessions: TrendMetric,
    /// Token usage trend
    pub tokens: TrendMetric,
    /// Files edited trend
    pub files_edited: TrendMetric,
    /// Commits linked trend
    pub commits: TrendMetric,
    /// Avg tokens per prompt trend
    pub avg_tokens_per_prompt: TrendMetric,
    /// Avg re-edit rate trend (percentage 0-100)
    pub avg_reedit_rate: TrendMetric,
}

impl From<WeekTrends> for DashboardTrends {
    fn from(t: WeekTrends) -> Self {
        Self {
            sessions: t.session_count,
            tokens: t.total_tokens,
            files_edited: t.total_files_edited,
            commits: t.commit_link_count,
            avg_tokens_per_prompt: t.avg_tokens_per_prompt,
            avg_reedit_rate: t.avg_reedit_rate,
        }
    }
}

/// Storage statistics for the settings page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    /// Size of JSONL session files in bytes.
    pub jsonl_bytes: u64,
    /// Size of SQLite database in bytes.
    pub sqlite_bytes: u64,
    /// Size of search index in bytes (Tantivy - not implemented yet, returns 0).
    pub index_bytes: u64,
    /// Total number of sessions.
    pub session_count: i64,
    /// Total number of projects.
    pub project_count: i64,
    /// Total number of linked commits.
    pub commit_count: i64,
    /// Unix timestamp of oldest session.
    pub oldest_session_date: Option<i64>,
    /// Unix timestamp of last index completion.
    pub last_index_at: Option<i64>,
    /// Duration of last index in milliseconds.
    pub last_index_duration_ms: Option<i64>,
    /// Number of sessions indexed in last run.
    pub last_index_session_count: i64,
    /// Unix timestamp of last git sync.
    pub last_git_sync_at: Option<i64>,
    /// Duration of last git sync in milliseconds (not currently tracked, returns None).
    pub last_git_sync_duration_ms: Option<i64>,
    /// Number of repos scanned in last git sync (not currently tracked, returns 0).
    pub last_git_sync_repo_count: i64,
}

/// GET /api/stats/dashboard - Pre-computed dashboard statistics (Step 22 extended).
///
/// Returns:
/// - Base stats: total_sessions, total_projects, heatmap, top_skills, top_projects, tool_totals
/// - Current week: session_count, total_tokens, total_files_edited, commit_count
/// - Trends: week-over-week changes for key metrics
pub async fn dashboard_stats(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ExtendedDashboardStats>> {
    let start = Instant::now();

    // Get base dashboard stats
    let base = match state.db.get_dashboard_stats().await {
        Ok(stats) => stats,
        Err(e) => {
            tracing::error!(
                endpoint = "dashboard_stats",
                error = %e,
                "Failed to fetch dashboard stats"
            );
            record_request("dashboard_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Get week trends
    let week_trends = match state.db.get_week_trends().await {
        Ok(trends) => trends,
        Err(e) => {
            tracing::error!(
                endpoint = "dashboard_stats",
                error = %e,
                "Failed to fetch week trends"
            );
            record_request("dashboard_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Build current week metrics from trends
    let current_week = CurrentWeekMetrics {
        session_count: week_trends.session_count.current as u64,
        total_tokens: week_trends.total_tokens.current as u64,
        total_files_edited: week_trends.total_files_edited.current as u64,
        commit_count: week_trends.commit_link_count.current as u64,
    };

    let trends = DashboardTrends::from(week_trends);

    // Record successful request metrics
    record_request("dashboard_stats", "200", start.elapsed());

    Ok(Json(ExtendedDashboardStats {
        base,
        current_week,
        trends,
    }))
}

/// GET /api/stats/storage - Storage statistics for the settings page.
///
/// Returns:
/// - Storage sizes: JSONL files, SQLite database, search index
/// - Counts: sessions, projects, commits
/// - Timing: oldest session, last index, last git sync
pub async fn storage_stats(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<StorageStats>> {
    let start = Instant::now();

    // Get index metadata for timing info
    let metadata = match state.db.get_index_metadata().await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(
                endpoint = "storage_stats",
                error = %e,
                "Failed to fetch index metadata"
            );
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Get counts from database
    let session_count = state.db.get_session_count().await.unwrap_or(0);
    let project_count = state.db.get_project_count().await.unwrap_or(0);
    let commit_count = state.db.get_commit_count().await.unwrap_or(0);
    let oldest_session_date = state.db.get_oldest_session_date().await.ok().flatten();

    // Calculate JSONL storage size
    let jsonl_bytes = calculate_jsonl_size().await;

    // Calculate SQLite database size
    let sqlite_bytes = state.db.get_database_size().await.unwrap_or(0) as u64;

    // Search index size (Tantivy not implemented yet)
    let index_bytes: u64 = 0;

    record_request("storage_stats", "200", start.elapsed());

    Ok(Json(StorageStats {
        jsonl_bytes,
        sqlite_bytes,
        index_bytes,
        session_count,
        project_count,
        commit_count,
        oldest_session_date,
        last_index_at: metadata.last_indexed_at,
        last_index_duration_ms: metadata.last_index_duration_ms,
        last_index_session_count: metadata.sessions_indexed,
        last_git_sync_at: metadata.last_git_sync_at,
        last_git_sync_duration_ms: None, // Not tracked currently
        last_git_sync_repo_count: 0,     // Not tracked currently
    }))
}

/// Calculate total size of JSONL session files in ~/.claude/projects/
async fn calculate_jsonl_size() -> u64 {
    let projects_dir = match claude_projects_dir() {
        Ok(dir) => dir,
        Err(_) => return 0,
    };

    calculate_directory_jsonl_size(&projects_dir).await
}

/// Recursively calculate the total size of .jsonl files in a directory.
async fn calculate_directory_jsonl_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return 0,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            // Recurse into subdirectories (project directories)
            total += Box::pin(calculate_directory_jsonl_size(&path)).await;
        } else if file_type.is_file() {
            // Only count .jsonl files
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Ok(metadata) = tokio::fs::metadata(&path).await {
                    total += metadata.len();
                }
            }
        }
    }

    total
}

/// Create the stats routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/stats/dashboard", get(dashboard_stats))
        .route("/stats/storage", get(storage_stats))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_core::{SessionInfo, ToolCounts};
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

    #[tokio::test]
    async fn test_dashboard_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 0);
        assert_eq!(json["totalProjects"], 0);
        assert!(json["heatmap"].is_array());
        assert!(json["topSkills"].is_array());
        assert!(json["topProjects"].is_array());
        assert!(json["toolTotals"].is_object());

        // Check extended fields
        assert!(json["currentWeek"].is_object());
        assert_eq!(json["currentWeek"]["sessionCount"], 0);
        assert!(json["trends"].is_object());
        assert!(json["trends"]["sessions"].is_object());
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            file_path: "/path/sess-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec!["/commit".to_string()],
            tool_counts: ToolCounts { edit: 5, read: 10, bash: 3, write: 2 },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            summary_text: None,
            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
        };
        db.insert_session(&session, "project-a", "Project A").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);
        assert_eq!(json["totalProjects"], 1);
        assert!(!json["heatmap"].as_array().unwrap().is_empty());

        // Check current week metrics
        assert!(json["currentWeek"]["sessionCount"].is_number());

        // Check trends structure
        assert!(json["trends"]["sessions"]["current"].is_number());
        assert!(json["trends"]["sessions"]["previous"].is_number());
        assert!(json["trends"]["sessions"]["delta"].is_number());
    }

    #[tokio::test]
    async fn test_storage_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/storage").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All counts should be 0
        assert_eq!(json["sessionCount"], 0);
        assert_eq!(json["projectCount"], 0);
        assert_eq!(json["commitCount"], 0);

        // Storage sizes should be present (even if 0)
        assert!(json["jsonlBytes"].is_number());
        assert!(json["sqliteBytes"].is_number());
        assert!(json["indexBytes"].is_number());

        // Oldest session should be null for empty DB
        assert!(json["oldestSessionDate"].is_null());

        // Last index/sync should be null for fresh DB
        assert!(json["lastIndexAt"].is_null());
        assert!(json["lastGitSyncAt"].is_null());
    }

    #[tokio::test]
    async fn test_storage_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session
        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            file_path: "/path/sess-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            summary_text: None,
            parse_version: 0,
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        // Update index metadata
        db.update_index_metadata_on_success(1500, 1, 1)
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/storage").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have 1 session and 1 project
        assert_eq!(json["sessionCount"], 1);
        assert_eq!(json["projectCount"], 1);

        // Oldest session should be set
        assert!(json["oldestSessionDate"].is_number());

        // Last index info should be present
        assert!(json["lastIndexAt"].is_number());
        assert_eq!(json["lastIndexDurationMs"], 1500);
        assert_eq!(json["lastIndexSessionCount"], 1);

        // SQLite size should be > 0 for non-empty db
        assert!(json["sqliteBytes"].as_u64().unwrap() > 0);
    }
}
