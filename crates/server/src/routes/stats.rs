//! Dashboard statistics endpoint.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use vibe_recall_core::{claude_projects_dir, DashboardStats};
use vibe_recall_db::trends::{TrendMetric, WeekTrends};
use vibe_recall_db::AIGenerationStats;

use crate::error::ApiResult;
use crate::metrics::record_request;
use crate::state::AppState;

/// Query parameters for dashboard stats endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DashboardQuery {
    /// Period start timestamp (Unix seconds, inclusive).
    /// If omitted along with `to`, returns all-time stats with no trends.
    pub from: Option<i64>,
    /// Period end timestamp (Unix seconds, inclusive).
    /// If omitted along with `from`, returns all-time stats with no trends.
    pub to: Option<i64>,
    /// Optional project filter (matches sessions.project_id).
    pub project: Option<String>,
    /// Optional branch filter (matches sessions.git_branch).
    pub branch: Option<String>,
}

/// Current period metrics for dashboard (adapts to selected time range).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CurrentPeriodMetrics {
    #[ts(type = "number")]
    pub session_count: u64,
    #[ts(type = "number")]
    pub total_tokens: u64,
    #[ts(type = "number")]
    pub total_files_edited: u64,
    #[ts(type = "number")]
    pub commit_count: u64,
}

/// Extended dashboard stats with current period and trends.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ExtendedDashboardStats {
    /// Base dashboard stats
    #[serde(flatten)]
    pub base: DashboardStats,
    /// Current period metrics (adapts to selected time range)
    pub current_week: CurrentPeriodMetrics,
    /// Period-over-period trends (None if viewing all-time)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trends: Option<DashboardTrends>,
    /// Start of the requested period (Unix timestamp).
    #[ts(type = "number | null")]
    pub period_start: Option<i64>,
    /// End of the requested period (Unix timestamp).
    #[ts(type = "number | null")]
    pub period_end: Option<i64>,
    /// Start of the comparison period (Unix timestamp).
    #[ts(type = "number | null")]
    pub comparison_period_start: Option<i64>,
    /// End of the comparison period (Unix timestamp).
    #[ts(type = "number | null")]
    pub comparison_period_end: Option<i64>,
    /// Earliest session date in the database (Unix timestamp).
    /// Used to display "since [date]" in the UI.
    #[ts(type = "number | null")]
    pub data_start_date: Option<i64>,
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
    #[ts(type = "number")]
    pub jsonl_bytes: u64,
    /// Size of SQLite database in bytes.
    #[ts(type = "number")]
    pub sqlite_bytes: u64,
    /// Size of search index in bytes (deep index - not implemented yet, returns 0).
    #[ts(type = "number")]
    pub index_bytes: u64,
    /// Total number of sessions.
    #[ts(type = "number")]
    pub session_count: i64,
    /// Total number of projects.
    #[ts(type = "number")]
    pub project_count: i64,
    /// Total number of linked commits.
    #[ts(type = "number")]
    pub commit_count: i64,
    /// Unix timestamp of oldest session.
    #[ts(type = "number | null")]
    pub oldest_session_date: Option<i64>,
    /// Unix timestamp of last index completion.
    #[ts(type = "number | null")]
    pub last_index_at: Option<i64>,
    /// Duration of last index in milliseconds.
    #[ts(type = "number | null")]
    pub last_index_duration_ms: Option<i64>,
    /// Number of sessions indexed in last run.
    #[ts(type = "number")]
    pub last_index_session_count: i64,
    /// Unix timestamp of last git sync.
    #[ts(type = "number | null")]
    pub last_git_sync_at: Option<i64>,
    /// Duration of last git sync in milliseconds (not currently tracked, returns None).
    #[ts(type = "number | null")]
    pub last_git_sync_duration_ms: Option<i64>,
    /// Number of repos scanned in last git sync (not currently tracked, returns 0).
    #[ts(type = "number")]
    pub last_git_sync_repo_count: i64,
}

/// GET /api/stats/dashboard - Pre-computed dashboard statistics with time range filtering.
///
/// Query params:
/// - `from`: Period start (Unix timestamp, optional)
/// - `to`: Period end (Unix timestamp, optional)
///
/// If `from` and `to` are omitted, returns all-time stats with no trends.
/// If provided, returns stats filtered to that period with comparison to the equivalent previous period.
///
/// Returns:
/// - Base stats: total_sessions, total_projects, heatmap, top_skills, top_projects, tool_totals
/// - Current period: session_count, total_tokens, total_files_edited, commit_count
/// - Trends: period-over-period changes for key metrics (None if viewing all-time)
/// - Period bounds: periodStart, periodEnd, comparisonPeriodStart, comparisonPeriodEnd
/// - dataStartDate: earliest session in database
pub async fn dashboard_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DashboardQuery>,
) -> ApiResult<Json<ExtendedDashboardStats>> {
    let start = Instant::now();

    // Reject half-specified ranges
    if query.from.is_some() != query.to.is_some() {
        return Err(crate::error::ApiError::BadRequest(
            "Both 'from' and 'to' must be provided together".to_string(),
        ));
    }
    // Reject inverted ranges
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if from >= to {
            return Err(crate::error::ApiError::BadRequest(
                "'from' must be less than 'to'".to_string(),
            ));
        }
    }

    // Get earliest session date for "since [date]" display
    let data_start_date = match state.db.get_oldest_session_date(query.project.as_deref(), query.branch.as_deref()).await {
        Ok(date) => date,
        Err(e) => {
            tracing::warn!(endpoint = "dashboard_stats", error = %e, "Failed to fetch oldest session date");
            None
        }
    };

    // Determine if we have a time range filter
    let has_time_range = query.from.is_some() && query.to.is_some();

    // Get base dashboard stats (always includes heatmap which is fixed at 90 days)
    let base = match if has_time_range {
        state.db.get_dashboard_stats_with_range(query.from, query.to, query.project.as_deref(), query.branch.as_deref()).await
    } else {
        state.db.get_dashboard_stats(query.project.as_deref(), query.branch.as_deref()).await
    } {
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

    // Calculate period bounds and comparison period
    let (period_start, period_end, comparison_start, comparison_end) = if let (Some(from), Some(to)) = (query.from, query.to) {
        let duration = to - from;
        // Previous period is the same duration immediately before
        let comp_end = from - 1;
        let comp_start = comp_end - duration;
        (Some(from), Some(to), Some(comp_start), Some(comp_end))
    } else {
        (None, None, None, None)
    };

    // Get trends (either for custom period or default week-over-week)
    let (current_week, trends) = if let (Some(from), Some(to)) = (query.from, query.to) {
        // Get trends for the specified period
        match state.db.get_trends_with_range(from, to, query.project.as_deref(), query.branch.as_deref()).await {
            Ok(period_trends) => {
                let current = CurrentPeriodMetrics {
                    session_count: period_trends.session_count.current as u64,
                    total_tokens: period_trends.total_tokens.current as u64,
                    total_files_edited: period_trends.total_files_edited.current as u64,
                    commit_count: period_trends.commit_link_count.current as u64,
                };
                let trends = DashboardTrends::from(period_trends);
                (current, Some(trends))
            }
            Err(e) => {
                tracing::error!(
                    endpoint = "dashboard_stats",
                    error = %e,
                    "Failed to fetch period trends"
                );
                record_request("dashboard_stats", "500", start.elapsed());
                return Err(e.into());
            }
        }
    } else {
        // All-time view: show aggregate stats but no trends
        match state.db.get_all_time_metrics(query.project.as_deref(), query.branch.as_deref()).await {
            Ok((session_count, total_tokens, total_files_edited, commit_count)) => {
                let current = CurrentPeriodMetrics {
                    session_count,
                    total_tokens,
                    total_files_edited,
                    commit_count,
                };
                // No trends for all-time view
                (current, None)
            }
            Err(e) => {
                tracing::error!(
                    endpoint = "dashboard_stats",
                    error = %e,
                    "Failed to fetch all-time metrics"
                );
                record_request("dashboard_stats", "500", start.elapsed());
                return Err(e.into());
            }
        }
    };

    // Record successful request metrics
    record_request("dashboard_stats", "200", start.elapsed());

    Ok(Json(ExtendedDashboardStats {
        base,
        current_week,
        trends,
        period_start,
        period_end,
        comparison_period_start: comparison_start,
        comparison_period_end: comparison_end,
        data_start_date,
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

    // Get all counts in a single query (replaces 4 separate queries)
    let (session_count, project_count, commit_count, oldest_session_date) =
        match state.db.get_storage_counts().await {
            Ok(counts) => counts,
            Err(e) => {
                tracing::error!(endpoint = "storage_stats", error = %e, "Failed to get storage counts");
                record_request("storage_stats", "500", start.elapsed());
                return Err(e.into());
            }
        };

    // Calculate JSONL storage size
    let jsonl_bytes = calculate_jsonl_size().await;

    // Calculate SQLite database size
    let sqlite_bytes = match state.db.get_database_size().await {
        Ok(size) => size as u64,
        Err(e) => {
            tracing::error!(endpoint = "storage_stats", error = %e, "Failed to get database size");
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Search index size (deep index not implemented yet)
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
        Err(e) => {
            tracing::warn!(error = %e, "Failed to locate Claude projects directory for JSONL size calculation");
            return 0;
        }
    };

    calculate_directory_jsonl_size(&projects_dir).await
}

/// Recursively calculate the total size of .jsonl files in a directory.
async fn calculate_directory_jsonl_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, dir = %dir.display(), "Failed to read directory for JSONL size calculation");
            return 0;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "Failed to get file type during JSONL size calculation");
                continue;
            }
        };

        if file_type.is_dir() {
            // Recurse into subdirectories (project directories)
            total += Box::pin(calculate_directory_jsonl_size(&path)).await;
        } else if file_type.is_file() {
            // Only count .jsonl files
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => {
                        total += metadata.len();
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, path = %path.display(), "Failed to get metadata for JSONL file");
                    }
                }
            }
        }
    }

    total
}

/// GET /api/stats/ai-generation - AI generation statistics with time range filtering.
///
/// Query params:
/// - `from`: Period start (Unix timestamp, optional)
/// - `to`: Period end (Unix timestamp, optional)
///
/// Returns:
/// - linesAdded, linesRemoved: Currently not tracked, returns 0 (future migration needed)
/// - filesCreated: Files edited/created by AI
/// - totalInputTokens, totalOutputTokens: Aggregate token usage
/// - tokensByModel: Token breakdown by AI model
/// - tokensByProject: Top 5 projects by token usage + "Others"
pub async fn ai_generation_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DashboardQuery>,
) -> ApiResult<Json<AIGenerationStats>> {
    let start = Instant::now();

    // Reject half-specified ranges
    if query.from.is_some() != query.to.is_some() {
        return Err(crate::error::ApiError::BadRequest(
            "Both 'from' and 'to' must be provided together".to_string(),
        ));
    }
    // Reject inverted ranges
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if from >= to {
            return Err(crate::error::ApiError::BadRequest(
                "'from' must be less than 'to'".to_string(),
            ));
        }
    }

    match state.db.get_ai_generation_stats(query.from, query.to, query.project.as_deref(), query.branch.as_deref()).await {
        Ok(stats) => {
            record_request("ai_generation_stats", "200", start.elapsed());
            Ok(Json(stats))
        }
        Err(e) => {
            tracing::error!(
                endpoint = "ai_generation_stats",
                error = %e,
                "Failed to fetch AI generation stats"
            );
            record_request("ai_generation_stats", "500", start.elapsed());
            Err(e.into())
        }
    }
}

/// Create the stats routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/stats/dashboard", get(dashboard_stats))
        .route("/stats/storage", get(storage_stats))
        .route("/stats/ai-generation", get(ai_generation_stats))
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
        // All-time view (no time range params) should NOT include trends
        assert!(json.get("trends").is_none() || json["trends"].is_null());
        // dataStartDate should be null for empty DB
        assert!(json["dataStartDate"].is_null());
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_time_range() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session = SessionInfo {
            id: "sess-range-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            file_path: "/path/sess-range-1.jsonl".to_string(),
            modified_at: now - 86400, // 1 day ago
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
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
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 10,
            files_edited_count: 3,
            reedited_files_count: 1,
            duration_seconds: 300,
            commit_count: 1,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        };
        db.insert_session(&session, "project-a", "Project A").await.unwrap();

        let app = build_app(db);

        // Query with time range (7 days)
        let seven_days_ago = now - (7 * 86400);
        let uri = format!("/api/stats/dashboard?from={}&to={}", seven_days_ago, now);
        let (status, body) = do_get(app, &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // With time range params, trends should be present
        assert!(json["trends"].is_object());
        assert!(json["trends"]["sessions"].is_object());
        assert!(json["trends"]["sessions"]["current"].is_number());
        assert!(json["trends"]["sessions"]["previous"].is_number());

        // Period bounds should be present
        assert!(json["periodStart"].is_number());
        assert!(json["periodEnd"].is_number());
        assert!(json["comparisonPeriodStart"].is_number());
        assert!(json["comparisonPeriodEnd"].is_number());

        // dataStartDate should be set
        assert!(json["dataStartDate"].is_number());
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

            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        };
        db.insert_session(&session, "project-a", "Project A").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);
        assert_eq!(json["totalProjects"], 1);
        assert!(!json["heatmap"].as_array().unwrap().is_empty());

        // Check current week metrics (all-time view)
        assert!(json["currentWeek"]["sessionCount"].is_number());

        // All-time view should not include trends
        assert!(json.get("trends").is_none() || json["trends"].is_null());

        // dataStartDate should be set when there's data
        assert!(json["dataStartDate"].is_number());
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
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
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

    #[tokio::test]
    async fn test_ai_generation_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All values should be 0 for empty DB
        assert_eq!(json["linesAdded"], 0);
        assert_eq!(json["linesRemoved"], 0);
        assert_eq!(json["filesCreated"], 0);
        assert_eq!(json["totalInputTokens"], 0);
        assert_eq!(json["totalOutputTokens"], 0);

        // Arrays should be empty
        assert!(json["tokensByModel"].as_array().unwrap().is_empty());
        assert!(json["tokensByProject"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session with token data
        // Use update_session_deep_fields to set token data since insert_session doesn't handle tokens
        let session = SessionInfo {
            id: "sess-ai-1".to_string(),
            project: "project-ai".to_string(),
            project_path: "/home/user/project-ai".to_string(),
            file_path: "/path/sess-ai-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "AI Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec![],
            tool_counts: ToolCounts { edit: 5, read: 10, bash: 3, write: 2 },
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
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec!["src/main.rs".to_string()],
            files_read_count: 15,
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
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        };
        db.insert_session(&session, "project-ai", "Project AI").await.unwrap();

        // Update with token data and first_message_at
        db.update_session_deep_fields(
            "sess-ai-1",
            "Test msg",
            8,   // turn_count
            5,   // tool_edit
            10,  // tool_read
            3,   // tool_bash
            2,   // tool_write
            r#"["src/main.rs"]"#,  // files_touched
            "[]",                   // skills_used
            10,  // user_prompt_count
            20,  // api_call_count
            50,  // tool_call_count
            "[]",                   // files_read
            r#"["src/main.rs"]"#,  // files_edited
            15,  // files_read_count
            5,   // files_edited_count
            2,   // reedited_files_count
            600, // duration_seconds
            3,   // commit_count
            Some(now - 86400),      // first_message_at
            150000,  // total_input_tokens
            250000,  // total_output_tokens
            10000,   // cache_read_tokens
            5000,    // cache_creation_tokens
            2,       // thinking_block_count
            Some(500),  // turn_duration_avg_ms
            Some(2000), // turn_duration_max_ms
            Some(4000), // turn_duration_total_ms
            0,       // api_error_count
            0,       // api_retry_count
            0,       // compaction_count
            0,       // hook_blocked_count
            0,       // agent_spawn_count
            0,       // bash_progress_count
            0,       // hook_progress_count
            0,       // mcp_progress_count
            None,    // summary_text
            1,       // parse_version
            2048,    // file_size
            now - 86400,  // file_mtime
            0, 0, 0, // lines_added, lines_removed, loc_source
            0, 0,    // ai_lines_added, ai_lines_removed
            None,    // work_type
            None,    // git_branch
            None,    // primary_model
            None,    // last_message_at
            None,    // first_user_prompt
        ).await.unwrap();

        // Update the primary_model column using the db pool directly
        db.set_session_primary_model("sess-ai-1", "claude-3-5-sonnet-20241022")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Lines are not tracked yet, should be 0
        assert_eq!(json["linesAdded"], 0);
        assert_eq!(json["linesRemoved"], 0);

        // Files created should match files_edited_count
        assert_eq!(json["filesCreated"], 5);

        // Token totals
        assert_eq!(json["totalInputTokens"], 150000);
        assert_eq!(json["totalOutputTokens"], 250000);

        // Token by model should have our model
        let models = json["tokensByModel"].as_array().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(models[0]["inputTokens"], 150000);
        assert_eq!(models[0]["outputTokens"], 250000);

        // Token by project should have our project
        let projects = json["tokensByProject"].as_array().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["project"], "Project AI");
        assert_eq!(projects[0]["inputTokens"], 150000);
        assert_eq!(projects[0]["outputTokens"], 250000);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_time_range() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session with a known first_message_at
        let session = SessionInfo {
            id: "sess-range".to_string(),
            project: "project-range".to_string(),
            project_path: "/home/user/project-range".to_string(),
            file_path: "/path/sess-range.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Range Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
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
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 10,
            files_edited_count: 3,
            reedited_files_count: 1,
            duration_seconds: 300,
            commit_count: 1,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        };
        db.insert_session(&session, "project-range", "Project Range").await.unwrap();

        // Update with token data and first_message_at
        db.update_session_deep_fields(
            "sess-range",
            "Test msg",
            5, 0, 0, 0, 0,
            "[]", "[]",
            5, 10, 20,
            "[]", "[]",
            10, 3, 1, 300, 1,
            Some(now - 86400),  // first_message_at: 1 day ago
            100000, 200000, 0, 0,
            0, None, None, None,
            0, 0, 0, 0, 0, 0, 0, 0,
            None, 1, 2048, now - 86400,
            0, 0, 0, // lines_added, lines_removed, loc_source
            0, 0,    // ai_lines_added, ai_lines_removed
            None,    // work_type
            None,    // git_branch
            None, // primary_model
            None, // last_message_at
            None, // first_user_prompt
        ).await.unwrap();

        let app = build_app(db);

        // Query with time range that includes the session
        let seven_days_ago = now - (7 * 86400);
        let uri = format!("/api/stats/ai-generation?from={}&to={}", seven_days_ago, now);
        let (status, body) = do_get(app.clone(), &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalInputTokens"], 100000);
        assert_eq!(json["totalOutputTokens"], 200000);

        // Query with time range that excludes the session (future)
        let uri = format!("/api/stats/ai-generation?from={}&to={}", now + 86400, now + 172800);
        let (status, body) = do_get(app, &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalInputTokens"], 0);
        assert_eq!(json["totalOutputTokens"], 0);
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_project_filter() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session_a = SessionInfo {
            id: "sess-proj-a".to_string(),
            project: "project-alpha".to_string(),
            project_path: "/home/user/project-alpha".to_string(),
            file_path: "/path/sess-proj-a.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Alpha session".to_string(),
            last_message: "Test msg A".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts { edit: 5, read: 10, bash: 3, write: 2 },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: Some("main".to_string()),
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
            files_read_count: 15,
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
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        };
        db.insert_session(&session_a, "project-alpha", "Project Alpha").await.unwrap();

        let mut session_b = session_a.clone();
        session_b.id = "sess-proj-b".to_string();
        session_b.project = "project-beta".to_string();
        session_b.project_path = "/home/user/project-beta".to_string();
        session_b.file_path = "/path/sess-proj-b.jsonl".to_string();
        session_b.preview = "Beta session".to_string();
        session_b.git_branch = Some("develop".to_string());
        db.insert_session(&session_b, "project-beta", "Project Beta").await.unwrap();

        let app = build_app(db);

        // Filter by project
        let (status, body) = do_get(app.clone(), "/api/stats/dashboard?project=project-alpha").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1, "should only count project-alpha sessions");
        assert_eq!(json["totalProjects"], 1);

        // Filter by project + branch
        let (status, body) = do_get(app.clone(), "/api/stats/dashboard?project=project-alpha&branch=main").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);

        // Filter by project + wrong branch = 0 sessions
        let (status, body) = do_get(app.clone(), "/api/stats/dashboard?project=project-alpha&branch=develop").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 0);

        // No filter — both sessions
        let (status, body) = do_get(app, "/api/stats/dashboard").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 2);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_project_filter() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session_a = SessionInfo {
            id: "sess-aigen-a".to_string(),
            project: "project-alpha".to_string(),
            project_path: "/home/user/project-alpha".to_string(),
            file_path: "/path/sess-aigen-a.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Alpha AI".to_string(),
            last_message: "msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: Some("main".to_string()),
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 5,
            files_edited_count: 3,
            reedited_files_count: 0,
            duration_seconds: 300,
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
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        };
        db.insert_session(&session_a, "project-alpha", "Project Alpha").await.unwrap();

        let app = build_app(db);

        // Filter by project
        let (status, body) = do_get(app.clone(), "/api/stats/ai-generation?project=project-alpha").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["filesCreated"], 3);

        // Filter by non-existent project = 0
        let (status, body) = do_get(app, "/api/stats/ai-generation?project=project-nope").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["filesCreated"], 0);
    }
}
