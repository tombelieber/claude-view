//! Shared types for dashboard statistics endpoints.

use claude_view_core::{AnalyticsScopeMeta, EffectiveRangeMeta};
use claude_view_db::trends::{TrendMetric, WeekTrends};
use claude_view_db::AIGenerationStats;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Query parameters for dashboard stats endpoint.
#[derive(Debug, Clone, Default, Deserialize, utoipa::IntoParams)]
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
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
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
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ExtendedDashboardStats {
    /// Base dashboard stats
    #[serde(flatten)]
    pub base: claude_view_core::DashboardStats,
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
    /// Additive section-specific effective range metadata.
    pub meta: DashboardMeta,
}

/// Dashboard metadata wrapper.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DashboardMeta {
    pub ranges: DashboardRangesMeta,
    #[serde(flatten)]
    pub analytics_scope: AnalyticsScopeMeta,
}

/// Section-specific range metadata for dashboard.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DashboardRangesMeta {
    pub current_period: EffectiveRangeMeta,
    pub heatmap: EffectiveRangeMeta,
}

/// Simplified trends for dashboard display.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
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

/// AI generation response wrapper with additive metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AIGenerationStatsResponse {
    #[serde(flatten)]
    pub base: AIGenerationStats,
    pub meta: AnalyticsScopeMeta,
}

/// Storage statistics for the settings page.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    /// Size of JSONL session files in bytes.
    #[ts(type = "number")]
    pub jsonl_bytes: u64,
    /// Size of SQLite database in bytes.
    #[ts(type = "number")]
    pub sqlite_bytes: u64,
    /// Size of obsolete session search cache in bytes.
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
    /// Path to JSONL session files (Claude Code data, read-only).
    pub jsonl_path: Option<String>,
    /// Path to SQLite database file.
    pub sqlite_path: Option<String>,
    /// Path to obsolete session search cache directory.
    pub index_path: Option<String>,
    /// Parent app data directory — safe to delete, rebuilt on next launch.
    pub app_data_path: Option<String>,
}
