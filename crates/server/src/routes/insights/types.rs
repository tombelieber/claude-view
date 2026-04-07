//! Response and query types for the insights API.

use claude_view_core::insights::generator::GeneratedInsight;
use claude_view_core::{AnalyticsScopeMeta, BenchmarksResponse, EffectiveRangeMeta};
use claude_view_db::insights_trends::{CategoryDataPoint, HeatmapCell, MetricDataPoint};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Categories response types (Phase 6)
// ============================================================================

/// Top-level category breakdown percentages.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CategoryBreakdown {
    pub code_work: CategorySummary,
    pub support_work: CategorySummary,
    pub thinking_work: CategorySummary,
    pub uncategorized: CategorySummary,
}

/// Count and percentage for a single L1 category.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    pub count: u32,
    pub percentage: f64,
}

/// Hierarchical category node for treemap.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CategoryNode {
    /// Hierarchical ID: "code_work", "code_work/feature", "code_work/feature/new-component"
    pub id: String,
    /// Category level: 1, 2, or 3
    pub level: u8,
    /// Display name
    pub name: String,
    /// Number of sessions
    pub count: u32,
    /// Percentage of total sessions
    pub percentage: f64,
    /// Average re-edit rate (files re-edited / files edited)
    pub avg_reedit_rate: f64,
    /// Average session duration in seconds
    pub avg_duration: u32,
    /// Average prompts per session
    pub avg_prompts: f64,
    /// Percentage of sessions with commits
    pub commit_rate: f64,
    /// AI-generated insight/recommendation (nullable)
    pub insight: Option<String>,
    /// Child categories (empty for L3)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schema(no_recursion)]
    pub children: Vec<CategoryNode>,
}

/// Full categories response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CategoriesResponse {
    /// High-level breakdown percentages
    pub breakdown: CategoryBreakdown,
    /// Hierarchical category tree
    pub categories: Vec<CategoryNode>,
    /// User's overall averages for comparison
    pub overall_averages: OverallAverages,
    /// Response metadata.
    pub meta: CategoriesMeta,
}

/// Categories response metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CategoriesMeta {
    pub effective_range: EffectiveRangeMeta,
    #[serde(flatten)]
    pub analytics_scope: AnalyticsScopeMeta,
}

/// Overall averages across all sessions for comparison.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct OverallAverages {
    pub avg_reedit_rate: f64,
    pub avg_duration: u32,
    pub avg_prompts: f64,
    pub commit_rate: f64,
}

// ============================================================================
// Insights response types
// ============================================================================

/// Full insights API response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsResponse {
    /// Hero insight (highest impact).
    pub top_insight: Option<GeneratedInsight>,
    /// Overview statistics.
    pub overview: InsightsOverview,
    /// Patterns grouped by impact tier.
    pub patterns: PatternGroups,
    /// Classification coverage statistics.
    pub classification_status: ClassificationCoverage,
    /// Response metadata.
    pub meta: InsightsMeta,
}

/// Overview statistics for the insights page.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsOverview {
    pub work_breakdown: WorkBreakdown,
    pub efficiency: EfficiencyStats,
    pub best_time: BestTimeStats,
}

/// Work type breakdown.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkBreakdown {
    pub total_sessions: u32,
    pub with_commits: u32,
    pub exploration: u32,
    pub avg_session_minutes: f64,
}

/// Efficiency trend stats.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyStats {
    pub avg_reedit_rate: f64,
    pub avg_edit_velocity: f64,
    pub trend: String,
    pub trend_pct: f64,
}

/// Best time of day/week stats.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BestTimeStats {
    pub day_of_week: String,
    pub time_slot: String,
    pub improvement_pct: f64,
}

/// Patterns grouped by impact tier.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PatternGroups {
    pub high: Vec<GeneratedInsight>,
    pub medium: Vec<GeneratedInsight>,
    pub observations: Vec<GeneratedInsight>,
}

/// Classification coverage status.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClassificationCoverage {
    pub classified: u32,
    pub total: u32,
    pub pending_classification: u32,
    pub classification_pct: f64,
}

/// Response metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsMeta {
    #[ts(type = "number")]
    pub computed_at: i64,
    #[ts(type = "number")]
    pub time_range_start: i64,
    #[ts(type = "number")]
    pub time_range_end: i64,
    pub effective_range: EffectiveRangeMeta,
    pub patterns_evaluated: u32,
    pub patterns_returned: u32,
    #[serde(flatten)]
    pub analytics_scope: AnalyticsScopeMeta,
}

/// Trends response metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsTrendsMeta {
    pub effective_range: EffectiveRangeMeta,
    #[serde(flatten)]
    pub analytics_scope: AnalyticsScopeMeta,
}

/// Full trends response wrapper with additive metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsTrendsResponse {
    pub metric: String,
    pub data_points: Vec<MetricDataPoint>,
    pub average: f64,
    pub trend: f64,
    pub trend_direction: String,
    pub insight: String,
    pub category_evolution: Option<Vec<CategoryDataPoint>>,
    pub category_insight: Option<String>,
    pub classification_required: bool,
    pub activity_heatmap: Vec<HeatmapCell>,
    pub heatmap_insight: String,
    pub period_start: String,
    pub period_end: String,
    #[ts(type = "number")]
    pub total_sessions: i64,
    pub meta: InsightsTrendsMeta,
}

/// Benchmarks response wrapper with additive metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BenchmarksResponseWithMeta {
    #[serde(flatten)]
    pub base: BenchmarksResponse,
    pub meta: AnalyticsScopeMeta,
}

// ============================================================================
// Query parameters
// ============================================================================

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct InsightsQuery {
    /// Period start (unix timestamp).
    pub from: Option<i64>,
    /// Period end (unix timestamp).
    pub to: Option<i64>,
    /// Minimum impact score (0.0-1.0). Defaults to 0.3.
    pub min_impact: Option<f64>,
    /// Comma-separated pattern categories to include.
    pub categories: Option<String>,
    /// Max patterns to return. Defaults to 50.
    pub limit: Option<u32>,
}

/// Query parameters for GET /api/insights/categories.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct CategoriesQuery {
    /// Period start (unix timestamp).
    pub from: Option<i64>,
    /// Period end (unix timestamp).
    pub to: Option<i64>,
}

/// Query parameters for GET /api/insights/trends.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TrendsQuery {
    #[serde(default = "default_metric")]
    pub metric: String,
    pub range: Option<String>,
    #[serde(default = "default_granularity")]
    pub granularity: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

fn default_metric() -> String {
    "reedit_rate".to_string()
}
fn default_granularity() -> String {
    "week".to_string()
}

/// Query parameters for the benchmarks endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct BenchmarksQuery {
    /// Time range: all, 30d, 90d, 1y. Defaults to all.
    pub range: Option<String>,
}
