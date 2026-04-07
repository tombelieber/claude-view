//! GET /api/insights/trends handler.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;

use claude_view_core::EffectiveRangeSource;

use crate::error::{ApiError, ApiResult};
use crate::metrics::{record_time_range_resolution, record_time_range_resolution_error};
use crate::state::AppState;
use crate::time_range::{resolve_range_param_or_all_time, ResolveFromToInput};

use super::db::fetch_analytics_scope_meta_for_range;
use super::types::{InsightsTrendsMeta, InsightsTrendsResponse, TrendsQuery};

const VALID_METRICS: &[&str] = &[
    "reedit_rate",
    "sessions",
    "lines",
    "cost_per_line",
    "prompts",
];
const VALID_TREND_RANGES: &[&str] = &["3mo", "6mo", "1yr", "all"];
const VALID_GRANULARITIES: &[&str] = &["day", "week", "month"];

/// GET /api/insights/trends - Get time-series trend data for charts.
#[utoipa::path(get, path = "/api/insights/trends", tag = "insights",
    params(TrendsQuery),
    responses(
        (status = 200, description = "Time-series trend data for metrics and heatmap", body = InsightsTrendsResponse),
    )
)]
pub async fn get_insights_trends(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrendsQuery>,
) -> ApiResult<Json<InsightsTrendsResponse>> {
    use claude_view_db::insights_trends::{
        calculate_trend_stats, generate_category_insight, generate_heatmap_insight,
        generate_metric_insight,
    };

    // Validate inputs
    if !VALID_METRICS.contains(&query.metric.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid metric. Must be one of: {}",
            VALID_METRICS.join(", ")
        )));
    }
    if !VALID_GRANULARITIES.contains(&query.granularity.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid granularity. Must be one of: {}",
            VALID_GRANULARITIES.join(", ")
        )));
    }

    let now = chrono::Utc::now().timestamp();
    let oldest_timestamp = match state.db.get_oldest_session_date(None, None).await {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(
                endpoint = "insights_trends",
                error = %e,
                "Failed to fetch oldest session date for default all-time range"
            );
            None
        }
    };
    let effective_range = match resolve_range_param_or_all_time(
        ResolveFromToInput {
            endpoint: "insights_trends",
            from: query.from,
            to: query.to,
            now,
            oldest_timestamp,
        },
        query.range.as_deref(),
        VALID_TREND_RANGES,
        |range| match range {
            "3mo" => Some(90 * 86400_i64),
            "6mo" => Some(180 * 86400_i64),
            "1yr" => Some(365 * 86400_i64),
            "all" => Some(365 * 10 * 86400_i64),
            _ => None,
        },
    ) {
        Ok(resolved) => {
            record_time_range_resolution("insights_trends", resolved.source);
            tracing::info!(
                endpoint = "insights_trends",
                from = resolved.from,
                to = resolved.to,
                source = resolved.source.as_str(),
                requested_from = query.from,
                requested_to = query.to,
                requested_range = query.range.as_deref().unwrap_or(""),
                "Resolved request time range"
            );
            resolved
        }
        Err(err) => {
            record_time_range_resolution_error("insights_trends", err.reason.as_str());
            tracing::warn!(
                endpoint = "insights_trends",
                reason = err.reason.as_str(),
                requested_from = query.from,
                requested_to = query.to,
                requested_range = query.range.as_deref().unwrap_or(""),
                "Rejected request time range"
            );
            return Err(ApiError::BadRequest(err.message));
        }
    };
    let from = effective_range.from;
    let to = effective_range.to;

    // Fetch all data
    let data_points = state
        .db
        .get_metric_timeseries(&query.metric, from, to, &query.granularity)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metric timeseries: {}", e)))?;

    let category_evolution = state
        .db
        .get_category_evolution(from, to, &query.granularity)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch category evolution: {}", e)))?;

    let activity_heatmap = state
        .db
        .get_activity_heatmap(from, to)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch activity heatmap: {}", e)))?;

    let total_sessions = state
        .db
        .get_session_count_in_range(from, to)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count sessions: {}", e)))?;

    // Calculate statistics
    let (average, trend, trend_direction) = calculate_trend_stats(&data_points, &query.metric);
    let range_label_for_insight = match effective_range.source {
        EffectiveRangeSource::ExplicitRangeParam => query.range.as_deref().unwrap_or("all"),
        EffectiveRangeSource::DefaultAllTime => "all",
        _ => "custom",
    };
    let insight = generate_metric_insight(&query.metric, trend, range_label_for_insight);
    let category_insight = category_evolution
        .as_ref()
        .map(|data| generate_category_insight(data));
    let heatmap_insight = generate_heatmap_insight(&activity_heatmap);

    let classification_required = category_evolution.is_none();
    let analytics_scope = fetch_analytics_scope_meta_for_range(&state, from, to).await?;

    Ok(Json(InsightsTrendsResponse {
        metric: query.metric,
        data_points,
        average,
        trend,
        trend_direction,
        insight,
        category_evolution,
        category_insight,
        classification_required,
        activity_heatmap,
        heatmap_insight,
        period_start: chrono::DateTime::from_timestamp(from, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        period_end: chrono::DateTime::from_timestamp(to, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        total_sessions,
        meta: InsightsTrendsMeta {
            effective_range,
            analytics_scope,
        },
    }))
}
