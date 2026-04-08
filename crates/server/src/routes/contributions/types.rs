//! Query parameters and response types for the Contributions API.

use claude_view_core::AnalyticsScopeMeta;
use claude_view_db::{
    BranchBreakdown, BranchSession, DailyTrendPoint, FileImpact, LearningCurve, LinkedCommit,
    ModelStats, SkillStats, UncommittedWork,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::insights::Insight;

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for GET /api/contributions.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ContributionsQuery {
    /// Time range: today, week, month, 90days, all, custom
    #[serde(default = "default_range")]
    pub range: String,
    /// Start date for custom range (YYYY-MM-DD)
    pub from: Option<String>,
    /// End date for custom range (YYYY-MM-DD)
    pub to: Option<String>,
    /// Optional project filter
    pub project_id: Option<String>,
    /// Optional branch filter (requires project_id)
    pub branch: Option<String>,
}

pub(crate) fn default_range() -> String {
    "week".to_string()
}

/// Query parameters for GET /api/contributions/branches/:name/sessions.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct BranchSessionsQuery {
    /// Time range: today, week, month, 90days, all, custom
    #[serde(default = "default_range")]
    pub range: String,
    /// Start date for custom range (YYYY-MM-DD)
    pub from: Option<String>,
    /// End date for custom range (YYYY-MM-DD)
    pub to: Option<String>,
    /// Optional project filter
    pub project_id: Option<String>,
    /// Maximum number of sessions to return (default: 10, max: 50)
    pub limit: Option<i64>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Fluency metrics for the overview card.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FluencyMetrics {
    #[ts(type = "number")]
    pub sessions: i64,
    pub prompts_per_session: f64,
    pub trend: Option<f64>,
    pub insight: Insight,
}

/// Output metrics for the overview card.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct OutputMetrics {
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub files_count: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    pub insight: Insight,
}

/// Effectiveness metrics for the overview card.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct EffectivenessMetrics {
    pub commit_rate: Option<f64>,
    pub reedit_rate: Option<f64>,
    pub insight: Insight,
}

/// Overview section combining all three metric cards.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct OverviewMetrics {
    pub fluency: FluencyMetrics,
    pub output: OutputMetrics,
    pub effectiveness: EffectivenessMetrics,
}

/// Efficiency metrics section.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyMetrics {
    pub total_cost: f64,
    #[ts(type = "number")]
    pub total_lines: i64,
    #[ts(type = "number")]
    pub priced_lines: i64,
    pub cost_per_line: Option<f64>,
    pub cost_per_commit: Option<f64>,
    pub cost_trend: Vec<f64>,
    /// True when any tokens were excluded from cost because model pricing was missing.
    pub has_unpriced_usage: bool,
    #[ts(type = "number")]
    pub priced_model_count: i64,
    #[ts(type = "number")]
    pub unpriced_model_count: i64,
    #[ts(type = "number")]
    pub unpriced_input_tokens: i64,
    #[ts(type = "number")]
    pub unpriced_output_tokens: i64,
    #[ts(type = "number")]
    pub unpriced_cache_read_tokens: i64,
    #[ts(type = "number")]
    pub unpriced_cache_creation_tokens: i64,
    /// Fraction of tokens priced with real model rates [0.0, 1.0].
    pub priced_token_coverage: f64,
    /// `priced_models_only_full` | `priced_models_only_partial`.
    pub cost_scope: String,
    pub insight: Insight,
}

/// Warning attached to response when data is incomplete.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ContributionWarning {
    pub code: String,
    pub message: String,
}

/// Main response for GET /api/contributions.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ContributionsResponse {
    /// Overview cards (fluency, output, effectiveness)
    pub overview: OverviewMetrics,
    /// Daily trend data for charting
    pub trend: Vec<DailyTrendPoint>,
    /// Efficiency metrics
    pub efficiency: EfficiencyMetrics,
    /// Model breakdown
    pub by_model: Vec<ModelStats>,
    /// Learning curve data
    pub learning_curve: LearningCurve,
    /// Branch breakdown
    pub by_branch: Vec<BranchBreakdown>,
    /// Skill effectiveness breakdown
    pub by_skill: Vec<SkillStats>,
    /// Global skill insight
    pub skill_insight: String,
    /// Uncommitted work tracker
    pub uncommitted: Vec<UncommittedWork>,
    /// Global uncommitted insight
    pub uncommitted_insight: String,
    /// Warnings if data is incomplete
    pub warnings: Vec<ContributionWarning>,
    /// Additive analytics scope metadata.
    pub meta: AnalyticsScopeMeta,
}

/// Response for GET /api/contributions/sessions/:id.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionContributionResponse {
    /// Session ID
    pub session_id: String,
    /// Work type classification
    pub work_type: Option<String>,
    /// Duration in seconds
    #[ts(type = "number")]
    pub duration: i64,
    /// Number of prompts
    #[ts(type = "number")]
    pub prompt_count: i64,
    /// AI lines added
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    /// AI lines removed
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    /// Files edited count
    #[ts(type = "number")]
    pub files_edited_count: i64,
    /// Per-file breakdown
    pub files: Vec<FileImpact>,
    /// Linked commits
    pub commits: Vec<LinkedCommit>,
    /// Commit rate for this session
    pub commit_rate: Option<f64>,
    /// Re-edit rate for this session
    pub reedit_rate: Option<f64>,
    /// Insight about this session
    pub insight: Insight,
    /// Additive analytics scope metadata.
    pub meta: AnalyticsScopeMeta,
}

/// Response for GET /api/contributions/branches/:name/sessions.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BranchSessionsResponse {
    /// Branch name
    pub branch: String,
    /// Sessions for this branch
    pub sessions: Vec<BranchSession>,
    /// Additive analytics scope metadata.
    pub meta: AnalyticsScopeMeta,
}
