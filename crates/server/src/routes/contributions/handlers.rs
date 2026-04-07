//! Route handlers for the Contributions API endpoints.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::IntoResponse,
    Json,
};
use claude_view_db::{calculate_cost_usd, lookup_pricing, TimeRange, TokenBreakdown};

use crate::error::{ApiError, ApiResult};
use crate::insights::{effectiveness_insight, efficiency_insight, fluency_insight, output_insight};
use crate::state::AppState;

use super::helpers::{
    detect_warnings, generate_skill_insight, generate_uncommitted_insight,
    get_previous_period_contributions,
};
use super::scope_meta::{
    fetch_branch_sessions_scope_meta, fetch_contributions_scope_meta,
    fetch_session_contribution_scope_meta,
};
use super::types::*;

/// GET /api/contributions - Main contributions page data.
#[utoipa::path(get, path = "/api/contributions", tag = "contributions",
    params(ContributionsQuery),
    responses(
        (status = 200, description = "Contribution metrics for the specified period", body = ContributionsResponse),
    )
)]
pub async fn get_contributions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ContributionsQuery>,
) -> ApiResult<impl IntoResponse> {
    let range = TimeRange::parse_str(&params.range).unwrap_or(TimeRange::Week);
    let from_date = params.from.as_deref();
    let to_date = params.to.as_deref();
    let project_id = params.project_id.as_deref();
    let branch = params.branch.as_deref();

    // Get aggregated contributions
    let agg = state
        .db
        .get_aggregated_contributions(range, from_date, to_date, project_id, branch)
        .await?;

    // Get previous period for comparison (for fluency trend)
    let prev_agg = get_previous_period_contributions(&state, range, project_id, branch).await?;

    // Get trend data (cost values are passed through from DB snapshots as-is)
    let trend = state
        .db
        .get_contribution_trend(range, from_date, to_date, project_id, branch)
        .await?;

    // Get branch breakdown
    let by_branch = state
        .db
        .get_branch_breakdown(range, from_date, to_date, project_id, branch)
        .await?;

    // Get model breakdown (mutable — handler fills in cost_per_line per model)
    let mut by_model = state
        .db
        .get_model_breakdown(range, from_date, to_date, project_id, branch)
        .await?;

    // Get learning curve
    let learning_curve = state.db.get_learning_curve(project_id, branch).await?;

    // Get skill breakdown
    let by_skill = state
        .db
        .get_skill_breakdown(range, from_date, to_date, project_id, branch)
        .await?;

    // Get uncommitted work
    let uncommitted = state.db.get_uncommitted_work().await?;

    // Get rates
    let commit_rate = state
        .db
        .get_commit_rate(range, from_date, to_date, project_id, branch)
        .await?;
    let reedit_rate = state
        .db
        .get_reedit_rate(range, from_date, to_date, project_id, branch)
        .await?;

    // Calculate derived metrics using real prompt counts from sessions
    let total_prompts = state
        .db
        .get_total_prompts(range, from_date, to_date, project_id, branch)
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

    // Compute per-model cost from ModelStats token data + pricing table.
    // Strict mode: unknown models are surfaced as unpriced; no synthetic fallback dollars.
    let (
        total_cost_usd,
        priced_lines,
        priced_model_count,
        unpriced_model_count,
        unpriced_input_tokens,
        unpriced_output_tokens,
        unpriced_cache_read_tokens,
        unpriced_cache_creation_tokens,
        priced_token_coverage,
        mut unpriced_models,
    ) = compute_model_costs(&mut by_model, &state);

    let has_unpriced_usage = unpriced_model_count > 0;

    let total_lines = agg.ai_lines_added + agg.ai_lines_removed;
    let cost_per_line = if priced_lines > 0 {
        Some(total_cost_usd / priced_lines as f64)
    } else {
        None
    };
    let cost_per_commit = if agg.commits_count > 0 {
        Some(total_cost_usd / agg.commits_count as f64)
    } else {
        None
    };

    // Keep per-day costs from DB snapshots; do not synthesize redistributed USD.
    let cost_trend: Vec<f64> = trend.iter().map(|t| t.cost_cents as f64 / 100.0).collect();

    let efficiency = EfficiencyMetrics {
        total_cost: total_cost_usd,
        total_lines,
        priced_lines,
        cost_per_line,
        cost_per_commit,
        cost_trend: cost_trend.clone(),
        has_unpriced_usage,
        priced_model_count,
        unpriced_model_count,
        unpriced_input_tokens,
        unpriced_output_tokens,
        unpriced_cache_read_tokens,
        unpriced_cache_creation_tokens,
        priced_token_coverage,
        cost_scope: if has_unpriced_usage {
            "priced_models_only_partial".to_string()
        } else {
            "priced_models_only_full".to_string()
        },
        insight: efficiency_insight(cost_per_line, &cost_trend),
    };

    // Generate skill insight
    let skill_insight = generate_skill_insight(&by_skill);

    // Generate uncommitted insight
    let uncommitted_insight = generate_uncommitted_insight(&uncommitted, total_lines);

    // Detect warnings
    let mut warnings = detect_warnings(&agg, &uncommitted);
    if has_unpriced_usage {
        unpriced_models.sort();
        unpriced_models.dedup();
        let sample: Vec<String> = unpriced_models.iter().take(5).cloned().collect();
        let more = unpriced_models.len().saturating_sub(sample.len());
        let suffix = if more > 0 {
            format!(" (+{} more)", more)
        } else {
            String::new()
        };
        warnings.push(ContributionWarning {
            code: "UnpricedModels".to_string(),
            message: format!(
                "Cost excludes usage from {} model(s) without pricing: {}{}",
                unpriced_model_count,
                sample.join(", "),
                suffix
            ),
        });
    }

    let meta =
        fetch_contributions_scope_meta(&state, range, from_date, to_date, project_id, branch)
            .await?;

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
        meta,
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

/// Compute per-model cost from ModelStats token data + pricing table.
///
/// Returns a tuple of all cost-related aggregates. Mutates `by_model` in place
/// to fill in `cost_per_line` for each model.
fn compute_model_costs(
    by_model: &mut [claude_view_db::ModelStats],
    state: &Arc<AppState>,
) -> (f64, i64, i64, i64, i64, i64, i64, i64, f64, Vec<String>) {
    let mut total_cost_usd = 0.0;
    let mut priced_lines: i64 = 0;
    let mut priced_model_count: i64 = 0;
    let mut unpriced_model_count: i64 = 0;
    let mut unpriced_input_tokens: i64 = 0;
    let mut unpriced_output_tokens: i64 = 0;
    let mut unpriced_cache_read_tokens: i64 = 0;
    let mut unpriced_cache_creation_tokens: i64 = 0;
    let mut priced_tokens_total: i64 = 0;
    let mut all_tokens_total: i64 = 0;
    let mut unpriced_models: Vec<String> = Vec::new();
    let pricing = &*state.pricing;

    for ms in by_model.iter_mut() {
        let tokens = TokenBreakdown {
            input_tokens: ms.input_tokens,
            output_tokens: ms.output_tokens,
            cache_read_tokens: ms.cache_read_tokens,
            cache_creation_tokens: ms.cache_creation_tokens,
        };
        let model_token_total =
            ms.input_tokens + ms.output_tokens + ms.cache_read_tokens + ms.cache_creation_tokens;
        all_tokens_total += model_token_total;

        match lookup_pricing(&ms.model, pricing) {
            Some(p) => {
                let model_cost = calculate_cost_usd(&tokens, p);
                total_cost_usd += model_cost;
                priced_lines += ms.lines;
                priced_model_count += 1;
                priced_tokens_total += model_token_total;
                ms.cost_per_line = if ms.lines > 0 {
                    Some(model_cost / ms.lines as f64)
                } else {
                    None
                };
            }
            None => {
                unpriced_model_count += 1;
                unpriced_input_tokens += ms.input_tokens;
                unpriced_output_tokens += ms.output_tokens;
                unpriced_cache_read_tokens += ms.cache_read_tokens;
                unpriced_cache_creation_tokens += ms.cache_creation_tokens;
                unpriced_models.push(ms.model.clone());
                ms.cost_per_line = None;
                tracing::warn!(
                    model = %ms.model,
                    input_tokens = ms.input_tokens,
                    output_tokens = ms.output_tokens,
                    cache_read_tokens = ms.cache_read_tokens,
                    cache_creation_tokens = ms.cache_creation_tokens,
                    "Missing pricing for model in contributions; model cost left unpriced"
                );
            }
        }
    }

    let priced_token_coverage = if all_tokens_total > 0 {
        priced_tokens_total as f64 / all_tokens_total as f64
    } else {
        1.0
    };

    (
        total_cost_usd,
        priced_lines,
        priced_model_count,
        unpriced_model_count,
        unpriced_input_tokens,
        unpriced_output_tokens,
        unpriced_cache_read_tokens,
        unpriced_cache_creation_tokens,
        priced_token_coverage,
        unpriced_models,
    )
}

/// GET /api/contributions/sessions/:id - Session contribution detail.
#[utoipa::path(get, path = "/api/contributions/sessions/{id}", tag = "contributions",
    params(
        ("id" = String, Path, description = "Session ID"),
        ContributionsQuery,
    ),
    responses(
        (status = 200, description = "Contribution metrics for a single session", body = SessionContributionResponse),
        (status = 404, description = "Session not found"),
    )
)]
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
    let meta = fetch_session_contribution_scope_meta(&state, &session_id).await?;

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
        meta,
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
#[utoipa::path(get, path = "/api/contributions/branches/{name}/sessions", tag = "contributions",
    params(
        ("name" = String, Path, description = "Branch name"),
        BranchSessionsQuery,
    ),
    responses(
        (status = 200, description = "Sessions for a specific branch", body = BranchSessionsResponse),
    )
)]
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

    let branch_filter = if branch == "(no branch)" {
        None
    } else {
        Some(branch.as_str())
    };
    let meta = fetch_branch_sessions_scope_meta(
        &state,
        branch_filter,
        range,
        from_date,
        to_date,
        project_id,
    )
    .await?;

    let response = BranchSessionsResponse {
        branch,
        sessions,
        meta,
    };

    // Cache for 5 minutes
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("max-age=300"),
    );

    Ok((headers, Json(response)))
}
