// crates/db/src/snapshots/types.rs
//! Type definitions for contribution snapshots and analytics.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Time range for contribution queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    /// Today only (real-time query, not from snapshots)
    Today,
    /// Last 7 days (includes today)
    Week,
    /// Last 30 days (includes today)
    Month,
    /// Last 90 days (includes today)
    NinetyDays,
    /// All time
    All,
    /// Custom date range (from, to)
    Custom,
}

impl TimeRange {
    /// Parse from query string parameter.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "today" => Some(TimeRange::Today),
            "week" => Some(TimeRange::Week),
            "month" => Some(TimeRange::Month),
            "90days" => Some(TimeRange::NinetyDays),
            "all" => Some(TimeRange::All),
            "custom" => Some(TimeRange::Custom),
            _ => None,
        }
    }

    /// Get the number of days to look back (None for All or Custom).
    pub fn days_back(&self) -> Option<i64> {
        match self {
            TimeRange::Today => Some(0),
            TimeRange::Week => Some(7),
            TimeRange::Month => Some(30),
            TimeRange::NinetyDays => Some(90),
            TimeRange::All => None,
            TimeRange::Custom => None,
        }
    }

    /// Cache duration in seconds for this time range.
    pub fn cache_seconds(&self) -> u64 {
        match self {
            TimeRange::Today => 60,        // 1 minute for real-time data
            TimeRange::Week => 300,        // 5 minutes
            TimeRange::Month => 900,       // 15 minutes
            TimeRange::NinetyDays => 1800, // 30 minutes
            TimeRange::All => 1800,        // 30 minutes
            TimeRange::Custom => 900,      // 15 minutes
        }
    }
}

/// A single contribution snapshot row.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ContributionSnapshot {
    #[ts(type = "number")]
    pub id: i64,
    pub date: String,
    pub project_id: Option<String>,
    pub branch: Option<String>,
    #[ts(type = "number")]
    pub sessions_count: i64,
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    #[ts(type = "number")]
    pub commit_insertions: i64,
    #[ts(type = "number")]
    pub commit_deletions: i64,
    #[ts(type = "number")]
    pub tokens_used: i64,
    #[ts(type = "number")]
    pub cost_cents: i64,
}

/// Aggregated contribution metrics for a time period.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AggregatedContributions {
    /// Total sessions in the period
    #[ts(type = "number")]
    pub sessions_count: i64,
    /// Total AI lines added
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    /// Total AI lines removed
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    /// Total commits linked
    #[ts(type = "number")]
    pub commits_count: i64,
    /// Total commit insertions
    #[ts(type = "number")]
    pub commit_insertions: i64,
    /// Total commit deletions
    #[ts(type = "number")]
    pub commit_deletions: i64,
    /// Total tokens used
    #[ts(type = "number")]
    pub tokens_used: i64,
    /// Total cost in cents
    #[ts(type = "number")]
    pub cost_cents: i64,
    /// Total files edited across all sessions
    #[ts(type = "number")]
    pub files_edited_count: i64,
}

/// Daily trend data point for charts.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DailyTrendPoint {
    pub date: String,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub commits: i64,
    #[ts(type = "number")]
    pub sessions: i64,
    #[ts(type = "number")]
    pub tokens_used: i64,
    #[ts(type = "number")]
    pub cost_cents: i64,
}

/// Model usage breakdown.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ModelBreakdown {
    pub model: String,
    #[ts(type = "number")]
    pub sessions: i64,
    #[ts(type = "number")]
    pub lines: i64,
    #[ts(type = "number")]
    pub tokens: i64,
    #[ts(type = "number")]
    pub cost_cents: i64,
    pub reedit_rate: Option<f64>,
}

/// Branch contribution breakdown.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BranchBreakdown {
    pub branch: String,
    #[ts(type = "number")]
    pub sessions_count: i64,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    pub ai_share: Option<f64>,
    #[ts(type = "number | null")]
    pub last_activity: Option<i64>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
}

/// Session contribution detail for the drill-down view.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionContribution {
    pub session_id: String,
    pub work_type: Option<String>,
    #[ts(type = "number")]
    pub duration_seconds: i64,
    #[ts(type = "number")]
    pub prompt_count: i64,
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    #[ts(type = "number")]
    pub files_edited_count: i64,
    #[ts(type = "number")]
    pub reedited_files_count: i64,
    #[ts(type = "number")]
    pub commit_count: i64,
}

/// Linked commit for session drill-down.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct LinkedCommit {
    pub hash: String,
    pub message: String,
    #[ts(type = "number | null")]
    pub insertions: Option<i64>,
    #[ts(type = "number | null")]
    pub deletions: Option<i64>,
    #[ts(type = "number")]
    pub tier: i64,
}

/// Model statistics for the byModel breakdown.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ModelStats {
    pub model: String,
    #[ts(type = "number")]
    pub sessions: i64,
    #[ts(type = "number")]
    pub lines: i64,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
    #[ts(type = "number")]
    pub cache_read_tokens: i64,
    #[ts(type = "number")]
    pub cache_creation_tokens: i64,
    pub reedit_rate: Option<f64>,
    pub cost_per_line: Option<f64>,
    pub insight: String,
}

/// Learning curve data point.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct LearningCurvePeriod {
    pub period: String,
    pub reedit_rate: f64,
}

/// Learning curve metrics.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct LearningCurve {
    pub periods: Vec<LearningCurvePeriod>,
    pub current_avg: f64,
    pub improvement: f64,
    pub insight: String,
}

/// Skill effectiveness statistics.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SkillStats {
    pub skill: String,
    #[ts(type = "number")]
    pub sessions: i64,
    #[ts(type = "number")]
    pub avg_loc: i64,
    pub commit_rate: f64,
    pub reedit_rate: f64,
}

/// Uncommitted work tracker entry.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct UncommittedWork {
    pub project_id: String,
    pub project_name: String,
    pub branch: Option<String>,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub files_count: i64,
    pub last_session_id: String,
    pub last_session_preview: String,
    #[ts(type = "number")]
    pub last_activity_at: i64,
    pub insight: String,
}

/// File impact for session detail view.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FileImpact {
    pub path: String,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    pub action: String, // "created", "modified", "deleted"
}

/// Lightweight session summary for branch expansion.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BranchSession {
    pub session_id: String,
    pub work_type: Option<String>,
    #[ts(type = "number")]
    pub duration_seconds: i64,
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    #[ts(type = "number")]
    pub commit_count: i64,
    #[ts(type = "number")]
    pub last_message_at: i64,
}

/// Snapshot retention statistics for monitoring.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStats {
    /// Number of daily snapshots
    pub daily_count: i64,
    /// Number of weekly rollup snapshots
    pub weekly_count: i64,
    /// Oldest daily snapshot date (YYYY-MM-DD)
    pub oldest_daily: Option<String>,
    /// Oldest weekly snapshot date (YYYY-MM-DD, without W: prefix)
    pub oldest_weekly: Option<String>,
}
