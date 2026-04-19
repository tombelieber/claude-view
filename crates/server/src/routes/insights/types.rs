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

/// Query parameters shared by `/api/insights/models` and
/// `/api/insights/projects`. Both endpoints aggregate rollup rows
/// over `[from, to)` and return per-dimension totals.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct InsightsAggregateQuery {
    /// Period start (unix seconds). Pair with `to`; omit both for
    /// all-time.
    pub from: Option<i64>,
    /// Period end (unix seconds). Pair with `from`.
    pub to: Option<i64>,
    /// Rollup bucket granularity. Valid: `daily`, `weekly`, `monthly`.
    /// Defaults to `daily`.
    ///
    /// Smaller buckets scan more rows; for ranges >= 180 days prefer
    /// `weekly` or `monthly` to keep row count bounded. The aggregation
    /// sum is identical across granularities.
    pub bucket: Option<String>,
    /// Max rows to return after descending-by-total-tokens sort.
    /// Defaults to 100. Hard-capped at 500 server-side to bound payload
    /// size.
    pub limit: Option<u32>,
}

// ============================================================================
// Phase 4 PR 4.5 — /api/insights/models + /api/insights/projects
//
// Both endpoints sum rollup rows from `daily_*_stats` (or weekly /
// monthly) by dimension and return per-dimension totals. They do NOT
// expose fields that Phase 5 will fill (`lines_added`, `lines_removed`,
// `commit_count`, `commit_insertions`, `commit_deletions`) unless PR
// 4.8 has folded snapshot data — see the field docs for the exact
// populated vs zero semantics per field.
// ============================================================================

/// One model's aggregated usage for the requested time range.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ModelInsight {
    /// Model identifier (e.g. `claude-opus-4-7`, `claude-sonnet-4-6`).
    pub model_id: String,
    /// Sessions that had this as their primary model.
    pub session_count: u64,
    /// Sum of input + output + cache-read + cache-creation tokens.
    pub total_tokens: u64,
    /// Sum of user-prompt counts across sessions.
    pub prompt_count: u64,
    /// Mean session duration in seconds (computed from
    /// `duration_sum_ms / duration_count / 1000`). Zero when no
    /// sessions in the range contributed a duration.
    pub avg_duration_seconds: f64,
}

/// Response for `/api/insights/models`.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsModelsResponse {
    /// Per-model totals, sorted by `total_tokens` descending.
    pub models: Vec<ModelInsight>,
    /// Response metadata.
    pub meta: InsightsAggregateMeta,
}

/// One project's aggregated usage for the requested time range.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProjectInsight {
    /// Project identifier (directory-mangled form, matching
    /// `session_stats.project_id`).
    pub project_id: String,
    /// Sessions whose `project_id` matched this row.
    pub session_count: u64,
    pub total_tokens: u64,
    pub prompt_count: u64,
    pub avg_duration_seconds: f64,
    /// AI-attributed lines added — populated from the PR 4.8
    /// contribution_snapshots fold for history, and from Phase 5
    /// SessionFlags going forward.
    pub lines_added: u64,
    pub lines_removed: u64,
    /// Distinct commit count attributed to this project.
    pub commit_count: u64,
}

/// Response for `/api/insights/projects`.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsProjectsResponse {
    /// Per-project totals, sorted by `total_tokens` descending.
    pub projects: Vec<ProjectInsight>,
    /// Response metadata.
    pub meta: InsightsAggregateMeta,
}

/// Shared metadata for `/api/insights/models` and
/// `/api/insights/projects`.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsAggregateMeta {
    /// Resolved `[from, to)` unix range that was queried.
    pub effective_range: EffectiveRangeMeta,
    /// Bucket granularity that was actually used (input clamped).
    pub bucket: String,
    /// Rows read from the rollup table before aggregation.
    pub rows_read: u64,
    /// Dimension rows returned (after sort + limit).
    pub rows_returned: u64,
    /// `true` when the handler fell back to the legacy GROUP BY path
    /// (`CLAUDE_VIEW_USE_LEGACY_STATS_READ=1`).
    pub legacy_path: bool,
}
