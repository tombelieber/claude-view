// crates/server/src/routes/contributions.rs
//! Contributions API endpoints for Theme 3.
//!
//! This module provides:
//! - `GET /api/contributions` - Main contributions page data
//! - `GET /api/contributions/sessions/:id` - Session contribution detail
//! - `GET /api/contributions/branches/:name/sessions` - Sessions for a branch
//!
//! ## Caching
//!
//! Responses include Cache-Control headers based on the time range:
//! - Today: 1 minute (real-time data)
//! - Week: 5 minutes
//! - Month: 15 minutes
//! - 90 days / All: 30 minutes

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use vibe_recall_db::{
    calculate_cost_usd, lookup_pricing, AggregatedContributions, BranchBreakdown, BranchSession,
    DailyTrendPoint, FileImpact, LearningCurve, LinkedCommit, ModelStats, SkillStats, TimeRange,
    TokenBreakdown, UncommittedWork, FALLBACK_COST_PER_TOKEN_USD,
};

use crate::error::{ApiError, ApiResult};
use crate::insights::{
    effectiveness_insight, efficiency_insight, fluency_insight, output_insight, Insight,
};
use crate::state::AppState;

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for GET /api/contributions.
#[derive(Debug, Deserialize)]
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
}

fn default_range() -> String {
    "week".to_string()
}

/// Query parameters for GET /api/contributions/branches/:name/sessions.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct FluencyMetrics {
    #[ts(type = "number")]
    pub sessions: i64,
    pub prompts_per_session: f64,
    pub trend: Option<f64>,
    pub insight: Insight,
}

/// Output metrics for the overview card.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct EffectivenessMetrics {
    pub commit_rate: Option<f64>,
    pub reedit_rate: Option<f64>,
    pub insight: Insight,
}

/// Overview section combining all three metric cards.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct OverviewMetrics {
    pub fluency: FluencyMetrics,
    pub output: OutputMetrics,
    pub effectiveness: EffectivenessMetrics,
}

/// Efficiency metrics section.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyMetrics {
    pub total_cost: f64,
    #[ts(type = "number")]
    pub total_lines: i64,
    pub cost_per_line: Option<f64>,
    pub cost_per_commit: Option<f64>,
    pub cost_trend: Vec<f64>,
    pub cost_is_estimated: bool,
    pub insight: Insight,
}

/// Warning attached to response when data is incomplete.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ContributionWarning {
    pub code: String,
    pub message: String,
}

/// Main response for GET /api/contributions.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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
}

/// Response for GET /api/contributions/sessions/:id.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
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
}

/// Response for GET /api/contributions/branches/:name/sessions.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BranchSessionsResponse {
    /// Branch name
    pub branch: String,
    /// Sessions for this branch
    pub sessions: Vec<BranchSession>,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// GET /api/contributions - Main contributions page data.
pub async fn get_contributions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ContributionsQuery>,
) -> ApiResult<impl IntoResponse> {
    let range = TimeRange::parse_str(&params.range).unwrap_or(TimeRange::Week);
    let from_date = params.from.as_deref();
    let to_date = params.to.as_deref();
    let project_id = params.project_id.as_deref();

    // Get aggregated contributions
    let agg = state
        .db
        .get_aggregated_contributions(range, from_date, to_date, project_id)
        .await?;

    // Get previous period for comparison (for fluency trend)
    let prev_agg = get_previous_period_contributions(&state, range, project_id).await?;

    // Get trend data
    let trend = state
        .db
        .get_contribution_trend(range, from_date, to_date, project_id)
        .await?;

    // Get branch breakdown
    let by_branch = state
        .db
        .get_branch_breakdown(range, from_date, to_date, project_id)
        .await?;

    // Get model breakdown (mutable — handler fills in cost_per_line per model)
    let mut by_model = state
        .db
        .get_model_breakdown(range, from_date, to_date, project_id)
        .await?;

    // Get learning curve
    let learning_curve = state.db.get_learning_curve(project_id).await?;

    // Get skill breakdown
    let by_skill = state
        .db
        .get_skill_breakdown(range, from_date, to_date, project_id)
        .await?;

    // Get uncommitted work
    let uncommitted = state.db.get_uncommitted_work().await?;

    // Get rates
    let commit_rate = state
        .db
        .get_commit_rate(range, from_date, to_date, project_id)
        .await?;
    let reedit_rate = state
        .db
        .get_reedit_rate(range, from_date, to_date, project_id)
        .await?;

    // Calculate derived metrics using real prompt counts from sessions
    let total_prompts = state
        .db
        .get_total_prompts(range, from_date, to_date, project_id)
        .await?;
    let prompts_per_session = if agg.sessions_count > 0 {
        total_prompts as f64 / agg.sessions_count as f64
    } else {
        0.0
    };

    let fluency_trend = if prev_agg.sessions_count > 0 {
        Some(
            ((agg.sessions_count - prev_agg.sessions_count) as f64
                / prev_agg.sessions_count as f64)
                * 100.0,
        )
    } else {
        None
    };

    // Find peak day from trend
    let peak_day = trend
        .iter()
        .max_by_key(|t| t.lines_added)
        .map(|t| (t.date.as_str(), t.lines_added));

    // Build overview
    let overview = OverviewMetrics {
        fluency: FluencyMetrics {
            sessions: agg.sessions_count,
            prompts_per_session,
            trend: fluency_trend,
            insight: fluency_insight(agg.sessions_count, prev_agg.sessions_count),
        },
        output: OutputMetrics {
            lines_added: agg.ai_lines_added,
            lines_removed: agg.ai_lines_removed,
            files_count: agg.files_edited_count,
            commits_count: agg.commits_count,
            insight: output_insight(agg.ai_lines_added, peak_day),
        },
        effectiveness: EffectivenessMetrics {
            commit_rate,
            reedit_rate,
            insight: effectiveness_insight(commit_rate, reedit_rate),
        },
    };

    // Compute per-model cost from ModelStats token data + pricing table
    let mut total_cost_usd = 0.0;
    for ms in &mut by_model {
        let tokens = TokenBreakdown {
            input_tokens: ms.input_tokens,
            output_tokens: ms.output_tokens,
            cache_read_tokens: ms.cache_read_tokens,
            cache_creation_tokens: ms.cache_creation_tokens,
        };
        let model_cost = match lookup_pricing(&ms.model, &state.pricing) {
            Some(p) => calculate_cost_usd(&tokens, p),
            None => {
                let total = (ms.input_tokens + ms.output_tokens
                    + ms.cache_read_tokens + ms.cache_creation_tokens) as f64;
                total * FALLBACK_COST_PER_TOKEN_USD
            }
        };
        total_cost_usd += model_cost;
        ms.cost_per_line = if ms.lines > 0 {
            Some(model_cost / ms.lines as f64)
        } else {
            None
        };
    }

    let total_lines = agg.ai_lines_added + agg.ai_lines_removed;
    let cost_per_line = if total_lines > 0 {
        Some(total_cost_usd / total_lines as f64)
    } else {
        None
    };
    let cost_per_commit = if agg.commits_count > 0 {
        Some(total_cost_usd / agg.commits_count as f64)
    } else {
        None
    };

    // Cost trend from daily snapshot data (still uses blended rate)
    let cost_trend: Vec<f64> = trend
        .iter()
        .map(|t| t.cost_cents as f64 / 100.0)
        .collect();

    let efficiency = EfficiencyMetrics {
        total_cost: total_cost_usd,
        total_lines,
        cost_per_line,
        cost_per_commit,
        cost_trend: cost_trend.clone(),
        cost_is_estimated: true,
        insight: efficiency_insight(cost_per_line, &cost_trend),
    };

    // Generate skill insight
    let skill_insight = generate_skill_insight(&by_skill);

    // Generate uncommitted insight
    let uncommitted_insight = generate_uncommitted_insight(&uncommitted, total_lines);

    // Detect warnings
    let warnings = detect_warnings(&agg, &uncommitted, &trend, range);

    // Build response
    let response = ContributionsResponse {
        overview,
        trend,
        efficiency,
        by_model,
        learning_curve,
        by_branch,
        by_skill,
        skill_insight,
        uncommitted,
        uncommitted_insight,
        warnings,
    };

    // Build cache headers
    // Don't cache empty responses — the server may still be generating snapshots
    // and a cached empty response would hide data for up to 30 minutes.
    let mut headers = HeaderMap::new();
    if agg.sessions_count == 0 {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store"),
        );
    } else {
        let cache_seconds = range.cache_seconds();
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_str(&format!("max-age={}", cache_seconds)).unwrap(),
        );
    }

    Ok((headers, Json(response)))
}

/// GET /api/contributions/sessions/:id - Session contribution detail.
pub async fn get_session_contribution(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Get session contribution data
    let contribution = state
        .db
        .get_session_contribution(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    // Get linked commits
    let commits = state.db.get_session_commits(&session_id).await?;

    // Get file impacts
    let files = state.db.get_session_file_impacts(&session_id).await?;

    // Calculate rates
    let commit_rate = if contribution.commit_count > 0 {
        Some(1.0) // This session has commits
    } else {
        Some(0.0) // No commits
    };

    let reedit_rate = if contribution.files_edited_count > 0 {
        Some(contribution.reedited_files_count as f64 / contribution.files_edited_count as f64)
    } else {
        None
    };

    // Generate insight
    let insight = effectiveness_insight(commit_rate, reedit_rate);

    let response = SessionContributionResponse {
        session_id: contribution.session_id,
        work_type: contribution.work_type,
        duration: contribution.duration_seconds,
        prompt_count: contribution.prompt_count,
        ai_lines_added: contribution.ai_lines_added,
        ai_lines_removed: contribution.ai_lines_removed,
        files_edited_count: contribution.files_edited_count,
        files,
        commits,
        commit_rate,
        reedit_rate,
        insight,
    };

    // Cache for 5 minutes (session data doesn't change frequently)
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("max-age=300"),
    );

    Ok((headers, Json(response)))
}

/// GET /api/contributions/branches/:name/sessions - Sessions for a branch.
pub async fn get_branch_sessions(
    State(state): State<Arc<AppState>>,
    Path(branch_name): Path<String>,
    Query(params): Query<BranchSessionsQuery>,
) -> ApiResult<impl IntoResponse> {
    let range = TimeRange::parse_str(&params.range).unwrap_or(TimeRange::Week);
    let from_date = params.from.as_deref();
    let to_date = params.to.as_deref();
    let project_id = params.project_id.as_deref();
    let limit = params.limit.unwrap_or(10).clamp(1, 50);

    // URL decode the branch name (e.g., "feature%2Ftest" -> "feature/test")
    let branch = urlencoding::decode(&branch_name)
        .map(|s| s.into_owned())
        .unwrap_or(branch_name);

    let sessions = state
        .db
        .get_branch_sessions(&branch, range, from_date, to_date, project_id, limit)
        .await?;

    let response = BranchSessionsResponse {
        branch,
        sessions,
    };

    // Cache for 5 minutes
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("max-age=300"),
    );

    Ok((headers, Json(response)))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get contributions for the previous period (for trend comparison).
async fn get_previous_period_contributions(
    state: &Arc<AppState>,
    range: TimeRange,
    project_id: Option<&str>,
) -> ApiResult<AggregatedContributions> {
    // Calculate the previous period based on current range
    let (prev_from, prev_to) = match range {
        TimeRange::Today => {
            // Yesterday
            let yesterday = chrono::Utc::now() - chrono::Duration::days(1);
            let date = yesterday.format("%Y-%m-%d").to_string();
            (date.clone(), date)
        }
        TimeRange::Week => {
            // Previous 7 days
            let now = chrono::Utc::now();
            let from = (now - chrono::Duration::days(14)).format("%Y-%m-%d").to_string();
            let to = (now - chrono::Duration::days(8)).format("%Y-%m-%d").to_string();
            (from, to)
        }
        TimeRange::Month => {
            // Previous 30 days
            let now = chrono::Utc::now();
            let from = (now - chrono::Duration::days(60)).format("%Y-%m-%d").to_string();
            let to = (now - chrono::Duration::days(31)).format("%Y-%m-%d").to_string();
            (from, to)
        }
        TimeRange::NinetyDays => {
            // Previous 90 days
            let now = chrono::Utc::now();
            let from = (now - chrono::Duration::days(180)).format("%Y-%m-%d").to_string();
            let to = (now - chrono::Duration::days(91)).format("%Y-%m-%d").to_string();
            (from, to)
        }
        TimeRange::All | TimeRange::Custom => {
            // For all/custom, return empty previous period
            return Ok(AggregatedContributions::default());
        }
    };

    state
        .db
        .get_aggregated_contributions(
            TimeRange::Custom,
            Some(&prev_from),
            Some(&prev_to),
            project_id,
        )
        .await
        .map_err(Into::into)
}

/// Generate skill insight comparing sessions with and without skills.
fn generate_skill_insight(by_skill: &[SkillStats]) -> String {
    // Find "(no skill)" entry and compare with best skill
    let no_skill = by_skill.iter().find(|s| s.skill == "(no skill)");
    let best_skill = by_skill
        .iter()
        .filter(|s| s.skill != "(no skill)" && s.sessions >= 2)
        .min_by(|a, b| {
            a.reedit_rate
                .partial_cmp(&b.reedit_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    match (no_skill, best_skill) {
        (Some(ns), Some(bs)) if ns.reedit_rate > 0.0 => {
            let improvement = ((ns.reedit_rate - bs.reedit_rate) / ns.reedit_rate) * 100.0;
            if improvement > 30.0 {
                format!(
                    "Sessions using {} skill have {:.0}% lower re-edit rate than sessions without skills - structured workflows produce better results",
                    bs.skill, improvement
                )
            } else if improvement > 10.0 {
                format!(
                    "{} skill provides {:.0}% improvement in output quality",
                    bs.skill, improvement
                )
            } else {
                "Similar quality with or without skills".to_string()
            }
        }
        (None, Some(bs)) => {
            format!("{} is your most effective skill ({:.0}% re-edit rate)", bs.skill, bs.reedit_rate * 100.0)
        }
        _ => "Skill usage patterns not yet established".to_string(),
    }
}

/// Detect warnings based on data quality indicators.
fn detect_warnings(
    agg: &AggregatedContributions,
    uncommitted: &[UncommittedWork],
    trend: &[DailyTrendPoint],
    range: TimeRange,
) -> Vec<ContributionWarning> {
    let mut warnings = Vec::new();

    // GitSyncIncomplete: Sessions exist but no commits were correlated
    // This suggests git sync hasn't run or failed
    if agg.sessions_count > 0 && agg.commits_count == 0 && !uncommitted.is_empty() {
        warnings.push(ContributionWarning {
            code: "GitSyncIncomplete".to_string(),
            message: "Some commit data unavailable - run sync to update git history".to_string(),
        });
    }

    // CostUnavailable: No cost data when we have sessions
    // Cost is estimated from tokens, so if cost_cents is 0 but we have sessions, token data is missing
    if agg.sessions_count > 0 && agg.cost_cents == 0 && agg.tokens_used == 0 {
        warnings.push(ContributionWarning {
            code: "CostUnavailable".to_string(),
            message: "Cost metrics unavailable - token data missing from some sessions".to_string(),
        });
    }

    // PartialData: Trend data has fewer days than expected for the range
    let expected_days = match range {
        TimeRange::Today => 1,
        TimeRange::Week => 7,
        TimeRange::Month => 30,
        TimeRange::NinetyDays => 90,
        TimeRange::All | TimeRange::Custom => 0, // No expected minimum for these
    };
    if expected_days > 0 && trend.len() < expected_days && agg.sessions_count > 0 {
        // Only warn if we have sessions but gaps in trend data
        warnings.push(ContributionWarning {
            code: "PartialData".to_string(),
            message: "Showing partial data - some days have no recorded sessions".to_string(),
        });
    }

    warnings
}

/// Generate uncommitted insight from uncommitted work data.
fn generate_uncommitted_insight(uncommitted: &[UncommittedWork], total_lines: i64) -> String {
    if uncommitted.is_empty() {
        return "All AI work has been committed".to_string();
    }

    let total_uncommitted: i64 = uncommitted.iter().map(|u| u.lines_added).sum();
    let project_count = uncommitted.len();

    if total_lines > 0 {
        let pct = (total_uncommitted as f64 / total_lines as f64) * 100.0;
        if pct > 20.0 {
            format!(
                "You have {} uncommitted AI lines across {} projects - {:.0}% of recent output. Commit often to avoid losing work.",
                total_uncommitted, project_count, pct
            )
        } else {
            format!(
                "{} uncommitted lines across {} projects - small amount of work in progress",
                total_uncommitted, project_count
            )
        }
    } else {
        format!(
            "{} uncommitted lines across {} projects",
            total_uncommitted, project_count
        )
    }
}

// ============================================================================
// Router
// ============================================================================

/// Create the contributions routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/contributions", get(get_contributions))
        .route("/contributions/sessions/{id}", get(get_session_contribution))
        .route(
            "/contributions/branches/{name}/sessions",
            get(get_branch_sessions),
        )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String, HeaderMap) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap(), headers)
    }

    #[tokio::test]
    async fn test_get_contributions_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body, headers) = do_get(app, "/api/contributions").await;

        assert_eq!(status, StatusCode::OK);

        // Check response structure
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["overview"].is_object());
        assert!(json["overview"]["fluency"].is_object());
        assert!(json["overview"]["output"].is_object());
        assert!(json["overview"]["effectiveness"].is_object());
        assert!(json["trend"].is_array());
        assert!(json["efficiency"].is_object());
        assert!(json["byBranch"].is_array());

        // Empty responses should not be cached (server may still be generating snapshots)
        let cache_control = headers.get("cache-control").unwrap().to_str().unwrap();
        assert!(cache_control.contains("no-cache"));
    }

    #[tokio::test]
    async fn test_get_contributions_with_range() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _, headers) = do_get(app, "/api/contributions?range=today").await;

        assert_eq!(status, StatusCode::OK);

        // Empty DB: should get no-cache (not range-based caching)
        let cache_control = headers.get("cache-control").unwrap().to_str().unwrap();
        assert!(cache_control.contains("no-cache"));
    }

    #[tokio::test]
    async fn test_get_contributions_week_cache() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _, headers) = do_get(app, "/api/contributions?range=week").await;

        assert_eq!(status, StatusCode::OK);

        // Empty DB: should get no-cache (not range-based caching)
        let cache_control = headers.get("cache-control").unwrap().to_str().unwrap();
        assert!(cache_control.contains("no-cache"));
    }

    #[tokio::test]
    async fn test_get_contributions_custom_range() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body, _) =
            do_get(app, "/api/contributions?range=custom&from=2026-01-01&to=2026-02-01").await;

        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["overview"].is_object());
    }

    #[tokio::test]
    async fn test_get_session_contribution_not_found() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body, _) =
            do_get(app, "/api/contributions/sessions/nonexistent-session").await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["error"].is_string());
    }

    #[tokio::test]
    async fn test_contributions_response_serialization() {
        let response = ContributionsResponse {
            overview: OverviewMetrics {
                fluency: FluencyMetrics {
                    sessions: 10,
                    prompts_per_session: 5.5,
                    trend: Some(15.0),
                    insight: crate::insights::Insight::info("Test"),
                },
                output: OutputMetrics {
                    lines_added: 500,
                    lines_removed: 100,
                    files_count: 12,
                    commits_count: 5,
                    insight: crate::insights::Insight::info("Test"),
                },
                effectiveness: EffectivenessMetrics {
                    commit_rate: Some(0.8),
                    reedit_rate: Some(0.15),
                    insight: crate::insights::Insight::success("Excellent"),
                },
            },
            trend: vec![],
            efficiency: EfficiencyMetrics {
                total_cost: 2.50,
                total_lines: 600,
                cost_per_line: Some(0.004),
                cost_per_commit: Some(0.50),
                cost_trend: vec![0.5, 0.4, 0.3],
                cost_is_estimated: true,
                insight: crate::insights::Insight::info("Good"),
            },
            by_model: vec![],
            learning_curve: LearningCurve {
                periods: vec![],
                current_avg: 0.2,
                improvement: 10.0,
                insight: "Steady improvement".to_string(),
            },
            by_branch: vec![],
            by_skill: vec![],
            skill_insight: "Skill insight".to_string(),
            uncommitted: vec![],
            uncommitted_insight: "No uncommitted work".to_string(),
            warnings: vec![],
        };

        let json = serde_json::to_string(&response).unwrap();

        // Verify camelCase
        assert!(json.contains("promptsPerSession"));
        assert!(json.contains("linesAdded"));
        assert!(json.contains("commitRate"));
        assert!(json.contains("reeditRate"));
        assert!(json.contains("totalCost"));
        assert!(json.contains("costPerLine"));
        // Verify new fields
        assert!(json.contains("byModel"));
        assert!(json.contains("learningCurve"));
        assert!(json.contains("bySkill"));
        assert!(json.contains("skillInsight"));
        assert!(json.contains("uncommitted"));
        assert!(json.contains("uncommittedInsight"));
    }
}
