//! GET /api/stats/ai-generation — AI generation statistics with time range filtering.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Query, State};
use axum::Json;
use claude_view_core::pricing::{self as pricing_engine};
use claude_view_core::AnalyticsScopeMeta;
use claude_view_db::AggregateCostBreakdown;

use crate::error::ApiResult;
use crate::metrics::record_request;
use crate::state::AppState;

use super::helpers::fetch_session_breakdown;
use super::types::{AIGenerationStatsResponse, DashboardQuery};

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
#[utoipa::path(get, path = "/api/stats/ai-generation", tag = "stats",
    params(DashboardQuery),
    responses(
        (status = 200, description = "AI generation statistics (tokens, cost, tool usage)", body = serde_json::Value),
    )
)]
pub async fn ai_generation_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DashboardQuery>,
) -> ApiResult<Json<AIGenerationStatsResponse>> {
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

    let mut stats = match state
        .db
        .get_ai_generation_stats(
            query.from,
            query.to,
            query.project.as_deref(),
            query.branch.as_deref(),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(endpoint = "ai_generation_stats", error = %e, "Failed to fetch AI generation stats");
            record_request("ai_generation_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Compute aggregate cost breakdown.
    // total_cost_usd comes from SUM(sessions.total_cost_usd) — the authoritative
    // per-turn tiered cost already stored by the indexer. Component breakdown
    // (input/output/cache) uses base rates as proportional estimates since we don't
    // store per-component costs at session level.
    if let Ok(model_tokens) = state
        .db
        .get_per_model_token_breakdown(
            query.from,
            query.to,
            query.project.as_deref(),
            query.branch.as_deref(),
        )
        .await
    {
        let pricing = &*state.pricing;
        let mut cost = AggregateCostBreakdown::default();

        let mut priced_tokens_total: i64 = 0;
        let mut all_tokens_total: i64 = 0;

        // Component breakdown uses base rates for proportional display.
        // The authoritative total comes from SUM(sessions.total_cost_usd) below.
        for (model_id, input, output, cache_read, cache_create) in &model_tokens {
            let model_token_total = *input + *output + *cache_read + *cache_create;
            all_tokens_total += model_token_total;
            if let Some(mp) = pricing_engine::lookup_pricing(model_id, pricing) {
                cost.priced_model_count += 1;
                priced_tokens_total += model_token_total;
                cost.input_cost_usd += *input as f64 * mp.input_cost_per_token;
                cost.output_cost_usd += *output as f64 * mp.output_cost_per_token;
                let cr_cost = *cache_read as f64 * mp.cache_read_cost_per_token;
                cost.cache_read_cost_usd += cr_cost;
                cost.cache_creation_cost_usd +=
                    *cache_create as f64 * mp.cache_creation_cost_per_token;
                cost.cache_savings_usd += *cache_read as f64 * mp.input_cost_per_token - cr_cost;
            } else {
                cost.unpriced_model_count += 1;
                cost.unpriced_input_tokens += *input;
                cost.unpriced_output_tokens += *output;
                cost.unpriced_cache_read_tokens += *cache_read;
                cost.unpriced_cache_creation_tokens += *cache_create;
                tracing::warn!(
                    model_id = %model_id,
                    input,
                    output,
                    cache_read,
                    cache_create,
                    "Missing pricing for model in ai-generation stats; cost left unpriced"
                );
            }
        }

        let computed_priced_total = cost.input_cost_usd
            + cost.output_cost_usd
            + cost.cache_read_cost_usd
            + cost.cache_creation_cost_usd;
        cost.computed_priced_total_cost_usd = computed_priced_total;
        cost.has_unpriced_usage = cost.unpriced_model_count > 0;
        cost.priced_token_coverage = if all_tokens_total > 0 {
            priced_tokens_total as f64 / all_tokens_total as f64
        } else {
            1.0
        };

        // Authoritative total: SUM of per-session costs (computed with per-turn
        // tiered pricing by the indexer). This is the same number shown in session
        // detail views, reports, and snapshots — one cost, everywhere.
        let session_cost_sum: (Option<f64>,) = sqlx::query_as(
            r#"SELECT SUM(total_cost_usd) FROM valid_sessions
               WHERE last_message_at >= ?1 AND last_message_at <= ?2
                 AND (?3 IS NULL OR project_id = ?3
                      OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3)
                      OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3))
                 AND (?4 IS NULL OR git_branch = ?4)"#,
        )
        .bind(query.from.unwrap_or(1))
        .bind(query.to.unwrap_or(i64::MAX))
        .bind(query.project.as_deref())
        .bind(query.branch.as_deref())
        .fetch_one(state.db.pool())
        .await
        .unwrap_or((None,));

        cost.total_cost_usd = session_cost_sum.0.unwrap_or(computed_priced_total);
        cost.total_cost_source = if cost.has_unpriced_usage {
            "computed_priced_tokens_partial".to_string()
        } else {
            "computed_priced_tokens_full".to_string()
        };
        stats.cost = cost;
    }

    let session_breakdown = fetch_session_breakdown(
        &state,
        Some(query.from.unwrap_or(1)),
        Some(query.to.unwrap_or(i64::MAX)),
        query.project.as_deref(),
        query.branch.as_deref(),
    )
    .await?;

    record_request("ai_generation_stats", "200", start.elapsed());
    Ok(Json(AIGenerationStatsResponse {
        base: stats,
        meta: AnalyticsScopeMeta::new(session_breakdown),
    }))
}
