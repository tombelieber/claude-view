//! Trend metric types and index metadata structs.

use serde::Serialize;
use ts_rs::TS;

/// A single trend metric comparing current vs previous period.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TrendMetric {
    /// Current period value.
    #[ts(type = "number")]
    pub current: i64,
    /// Previous period value.
    #[ts(type = "number")]
    pub previous: i64,
    /// Absolute change (current - previous).
    #[ts(type = "number")]
    pub delta: i64,
    /// Percentage change, rounded to 1 decimal place.
    /// None if previous == 0 (cannot calculate percentage).
    pub delta_percent: Option<f64>,
}

impl TrendMetric {
    /// Create a new TrendMetric from current and previous values.
    ///
    /// Calculates delta and delta_percent automatically.
    /// delta_percent is None if previous is 0.
    pub fn new(current: i64, previous: i64) -> Self {
        let delta = current - previous;
        let delta_percent = if previous == 0 {
            None
        } else {
            // Round to 1 decimal place
            let percent = (delta as f64 / previous as f64) * 100.0;
            Some((percent * 10.0).round() / 10.0)
        };
        Self {
            current,
            previous,
            delta,
            delta_percent,
        }
    }
}

/// Collection of all week-over-week trend metrics.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WeekTrends {
    /// Session count trend.
    pub session_count: TrendMetric,
    /// Total tokens (input + output) trend.
    pub total_tokens: TrendMetric,
    /// Average tokens per prompt (weighted average).
    /// None if no prompts in either period.
    pub avg_tokens_per_prompt: TrendMetric,
    /// Total files edited trend.
    pub total_files_edited: TrendMetric,
    /// Average re-edit rate (weighted average) * 100 for percentage display.
    /// None if no files edited in either period.
    pub avg_reedit_rate: TrendMetric,
    /// Commit link count trend.
    pub commit_link_count: TrendMetric,
}

/// Index metadata for data freshness tracking.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IndexMetadata {
    /// Unix timestamp of last successful index completion.
    #[ts(type = "number | null")]
    pub last_indexed_at: Option<i64>,
    /// Duration of last successful index in milliseconds.
    #[ts(type = "number | null")]
    pub last_index_duration_ms: Option<i64>,
    /// Number of sessions indexed in last run.
    #[ts(type = "number")]
    pub sessions_indexed: i64,
    /// Number of projects indexed in last run.
    #[ts(type = "number")]
    pub projects_indexed: i64,
    /// Unix timestamp of last successful git sync.
    #[ts(type = "number | null")]
    pub last_git_sync_at: Option<i64>,
    /// Number of commits found in last git sync.
    #[ts(type = "number")]
    pub commits_found: i64,
    /// Number of session-commit links created in last git sync.
    #[ts(type = "number")]
    pub links_created: i64,
    /// Unix timestamp of last metadata update.
    #[ts(type = "number")]
    pub updated_at: i64,
    /// User-configurable git sync interval in seconds (default 60).
    #[ts(type = "number")]
    pub git_sync_interval_secs: i64,
}
