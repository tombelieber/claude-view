//! GET /api/stats/dashboard — Pre-computed dashboard statistics with time range filtering.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Query, State};
use axum::Json;
use claude_view_core::{AnalyticsScopeMeta, EffectiveRangeMeta, EffectiveRangeSource};
use claude_view_stats_rollup::{sum_global_stats_in_range, Bucket};

use crate::error::ApiResult;
use crate::metrics::{
    record_request, record_time_range_resolution, record_time_range_resolution_error,
};
use crate::state::AppState;
use crate::time_range::{resolve_from_to_or_all_time, ResolveFromToInput};

use super::helpers::fetch_session_breakdown;
use super::types::{
    CurrentPeriodMetrics, DashboardMeta, DashboardQuery, DashboardRangesMeta, DashboardTrends,
    ExtendedDashboardStats,
};

use crate::routes::insights::rollup_read::legacy_stats_read_enabled;

/// GET /api/stats/dashboard - Pre-computed dashboard statistics with time range filtering.
///
/// Query params:
/// - `from`: Period start (Unix timestamp, optional)
/// - `to`: Period end (Unix timestamp, optional)
///
/// If `from` and `to` are omitted, returns all-time stats with no trends.
/// If provided, returns stats filtered to that period with comparison to the equivalent previous period.
///
/// Returns:
/// - Base stats: total_sessions, total_projects, heatmap, top_skills, top_projects, tool_totals
/// - Current period: session_count, total_tokens, total_files_edited, commit_count
/// - Trends: period-over-period changes for key metrics (None if viewing all-time)
/// - Period bounds: periodStart, periodEnd, comparisonPeriodStart, comparisonPeriodEnd
/// - dataStartDate: earliest session in database
#[utoipa::path(get, path = "/api/stats/dashboard", tag = "stats",
    params(DashboardQuery),
    responses(
        (status = 200, description = "Dashboard statistics with trends and heatmap", body = serde_json::Value),
    )
)]
pub async fn dashboard_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DashboardQuery>,
) -> ApiResult<Json<ExtendedDashboardStats>> {
    let start = Instant::now();
    let now = chrono::Utc::now().timestamp();

    // Reject half-specified ranges
    if query.from.is_some() != query.to.is_some() {
        record_time_range_resolution_error("stats_dashboard_current_period", "one_sided_input");
        tracing::warn!(
            endpoint = "stats_dashboard_current_period",
            requested_from = query.from,
            requested_to = query.to,
            "Rejected request time range"
        );
        return Err(crate::error::ApiError::BadRequest(
            "Both 'from' and 'to' must be provided together".to_string(),
        ));
    }
    // Reject inverted ranges
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if from > to {
            record_time_range_resolution_error("stats_dashboard_current_period", "inverted_range");
            tracing::warn!(
                endpoint = "stats_dashboard_current_period",
                requested_from = query.from,
                requested_to = query.to,
                "Rejected request time range"
            );
            return Err(crate::error::ApiError::BadRequest(
                "'from' must be <= 'to'".to_string(),
            ));
        }
    }

    // Get earliest session date for "since [date]" display
    let data_start_date = match state
        .db
        .get_oldest_session_date(query.project.as_deref(), query.branch.as_deref())
        .await
    {
        Ok(date) => date,
        Err(e) => {
            tracing::warn!(endpoint = "dashboard_stats", error = %e, "Failed to fetch oldest session date");
            None
        }
    };
    let current_period_range = match resolve_from_to_or_all_time(ResolveFromToInput {
        endpoint: "stats_dashboard_current_period",
        from: query.from,
        to: query.to,
        now,
        oldest_timestamp: data_start_date,
    }) {
        Ok(resolved) => {
            record_time_range_resolution("stats_dashboard_current_period", resolved.source);
            tracing::info!(
                endpoint = "stats_dashboard_current_period",
                from = resolved.from,
                to = resolved.to,
                source = resolved.source.as_str(),
                requested_from = query.from,
                requested_to = query.to,
                "Resolved request time range"
            );
            resolved
        }
        Err(err) => {
            record_time_range_resolution_error(
                "stats_dashboard_current_period",
                err.reason.as_str(),
            );
            tracing::warn!(
                endpoint = "stats_dashboard_current_period",
                reason = err.reason.as_str(),
                requested_from = query.from,
                requested_to = query.to,
                "Rejected request time range"
            );
            return Err(crate::error::ApiError::BadRequest(err.message));
        }
    };
    // Heatmap now respects the caller's time range — no longer hardcoded to 90 days.
    let heatmap_from = query.from.unwrap_or(1);
    let heatmap_to = query.to.unwrap_or(now);
    let heatmap_range = EffectiveRangeMeta {
        from: heatmap_from,
        to: heatmap_to,
        source: if query.from.is_some() {
            EffectiveRangeSource::ExplicitRangeParam
        } else {
            EffectiveRangeSource::DefaultAllTime
        },
    };
    record_time_range_resolution("stats_dashboard_heatmap", heatmap_range.source);
    tracing::info!(
        endpoint = "stats_dashboard_heatmap",
        from = heatmap_range.from,
        to = heatmap_range.to,
        source = heatmap_range.source.as_str(),
        "Resolved section range"
    );

    // Determine if we have a time range filter
    let has_time_range = query.from.is_some() && query.to.is_some();

    // Get base dashboard stats — heatmap respects the time range filter
    let base = match if has_time_range {
        state
            .db
            .get_dashboard_stats_with_range(
                query.from,
                query.to,
                query.project.as_deref(),
                query.branch.as_deref(),
            )
            .await
    } else {
        state
            .db
            .get_dashboard_stats(query.project.as_deref(), query.branch.as_deref())
            .await
    } {
        Ok(stats) => stats,
        Err(e) => {
            tracing::error!(
                endpoint = "dashboard_stats",
                error = %e,
                "Failed to fetch dashboard stats"
            );
            record_request("dashboard_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Calculate period bounds and comparison period
    let (period_start, period_end, comparison_start, comparison_end) =
        if let (Some(from), Some(to)) = (query.from, query.to) {
            let duration = to - from;
            // Previous period is the same duration immediately before
            let comp_end = from - 1;
            // H5: Clamp to 0 to prevent negative timestamps for new users
            let comp_start = (comp_end - duration).max(0);
            (Some(from), Some(to), Some(comp_start), Some(comp_end))
        } else {
            (None, None, None, None)
        };

    // Get trends (either for custom period or default week-over-week)
    let (current_week, trends) = if let (Some(from), Some(to)) = (query.from, query.to) {
        // Get trends for the specified period
        match state
            .db
            .get_trends_with_range(from, to, query.project.as_deref(), query.branch.as_deref())
            .await
        {
            Ok(period_trends) => {
                let current = CurrentPeriodMetrics {
                    session_count: period_trends.session_count.current as u64,
                    total_tokens: period_trends.total_tokens.current as u64,
                    total_files_edited: period_trends.total_files_edited.current as u64,
                    commit_count: period_trends.commit_link_count.current as u64,
                };
                let trends = DashboardTrends::from(period_trends);
                (current, Some(trends))
            }
            Err(e) => {
                tracing::error!(
                    endpoint = "dashboard_stats",
                    error = %e,
                    "Failed to fetch period trends"
                );
                record_request("dashboard_stats", "500", start.elapsed());
                return Err(e.into());
            }
        }
    } else {
        // All-time view: show aggregate stats but no trends.
        //
        // CQRS Phase 4 PR 4.3 — when there is no project/branch filter and
        // the legacy-stats env var is unset, read session_count + total_tokens
        // from the `daily_global_stats` rollup. The remaining two fields
        // (files_edited_count, commit_count) stay on the legacy path until
        // Phase 5 `SessionFlags` fold populates them on rollup rows.
        // Filtered requests continue to use the legacy GROUP BY over
        // valid_sessions — no project/branch rollup for unified metrics yet.
        let use_rollup =
            !legacy_stats_read_enabled() && query.project.is_none() && query.branch.is_none();
        match state
            .db
            .get_all_time_metrics(query.project.as_deref(), query.branch.as_deref())
            .await
        {
            Ok((session_count_legacy, total_tokens_legacy, total_files_edited, commit_count)) => {
                let (session_count, total_tokens) = if use_rollup {
                    match sum_global_stats_in_range(state.db.pool(), Bucket::Daily, 0, i64::MAX)
                        .await
                    {
                        Ok(stats) => (stats.session_count, stats.total_tokens),
                        Err(e) => {
                            tracing::warn!(
                                endpoint = "dashboard_stats",
                                error = %e,
                                "daily_global_stats rollup read failed — falling back to legacy values"
                            );
                            (session_count_legacy, total_tokens_legacy)
                        }
                    }
                } else {
                    (session_count_legacy, total_tokens_legacy)
                };
                let current = CurrentPeriodMetrics {
                    session_count,
                    total_tokens,
                    total_files_edited,
                    commit_count,
                };
                // No trends for all-time view
                (current, None)
            }
            Err(e) => {
                tracing::error!(
                    endpoint = "dashboard_stats",
                    error = %e,
                    "Failed to fetch all-time metrics"
                );
                record_request("dashboard_stats", "500", start.elapsed());
                return Err(e.into());
            }
        }
    };

    let session_breakdown = fetch_session_breakdown(
        &state,
        if has_time_range { query.from } else { None },
        if has_time_range { query.to } else { None },
        query.project.as_deref(),
        query.branch.as_deref(),
    )
    .await?;

    // Record successful request metrics
    record_request("dashboard_stats", "200", start.elapsed());

    Ok(Json(ExtendedDashboardStats {
        base,
        current_week,
        trends,
        period_start,
        period_end,
        comparison_period_start: comparison_start,
        comparison_period_end: comparison_end,
        data_start_date,
        meta: DashboardMeta {
            ranges: DashboardRangesMeta {
                current_period: current_period_range,
                heatmap: heatmap_range,
            },
            analytics_scope: AnalyticsScopeMeta::new(session_breakdown),
        },
    }))
}
