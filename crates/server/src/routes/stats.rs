//! Dashboard statistics endpoint.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use ts_rs::TS;
use vibe_recall_core::DashboardStats;
use vibe_recall_db::trends::{TrendMetric, WeekTrends};

use crate::error::ApiResult;
use crate::state::AppState;

/// Current week metrics for dashboard (Step 22).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CurrentWeekMetrics {
    pub session_count: u64,
    pub total_tokens: u64,
    pub total_files_edited: u64,
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

/// GET /api/stats/dashboard - Pre-computed dashboard statistics (Step 22 extended).
///
/// Returns:
/// - Base stats: total_sessions, total_projects, heatmap, top_skills, top_projects, tool_totals
/// - Current week: session_count, total_tokens, total_files_edited, commit_count
/// - Trends: week-over-week changes for key metrics
pub async fn dashboard_stats(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ExtendedDashboardStats>> {
    // Get base dashboard stats
    let base = state.db.get_dashboard_stats().await?;

    // Get week trends
    let week_trends = state.db.get_week_trends().await?;

    // Build current week metrics from trends
    let current_week = CurrentWeekMetrics {
        session_count: week_trends.session_count.current as u64,
        total_tokens: week_trends.total_tokens.current as u64,
        total_files_edited: week_trends.total_files_edited.current as u64,
        commit_count: week_trends.commit_link_count.current as u64,
    };

    let trends = DashboardTrends::from(week_trends);

    Ok(Json(ExtendedDashboardStats {
        base,
        current_week,
        trends,
    }))
}

/// Create the stats routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/stats/dashboard", get(dashboard_stats))
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
}
