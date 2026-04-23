//! GET /api/insights/models — per-model rollup aggregation (PR 4.5).
//!
//! Reads from `daily_model_stats` / `weekly_model_stats` /
//! `monthly_model_stats` (selected by the `bucket` query param) and
//! groups in-memory by `model_id`. Returns per-model totals sorted by
//! `total_tokens` descending, capped at `limit` (default 100, max 500).
//!
//! The legacy GROUP BY fallback was retired in CQRS Phase 7.g once
//! rollup coverage was complete.
//!
//! ## Fields populated from rollup
//!
//! - `session_count`, `total_tokens`, `prompt_count`,
//!   `avg_duration_seconds` — Stage C-owned; accurate today.
//!
//! This handler intentionally does not expose line / commit fields —
//! the model dimension in snapshot fold is not populated (snapshots
//! key by project+branch, not model), so there's no historical data
//! worth surfacing there until Phase 5.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;

use claude_view_stats_rollup::stats_core::{
    select_range_daily_model_stats, select_range_monthly_model_stats,
    select_range_weekly_model_stats,
};
use claude_view_stats_rollup::Bucket;

use crate::error::{ApiError, ApiResult};
use crate::metrics::{record_time_range_resolution, record_time_range_resolution_error};
use crate::state::AppState;
use crate::time_range::{resolve_from_to_or_all_time, ResolveFromToInput};

use super::rollup_read::{
    bucket_label, clamp_limit, parse_bucket, resolved_range_to_unix, DimAggregate,
};
use super::types::{
    InsightsAggregateMeta, InsightsAggregateQuery, InsightsModelsResponse, ModelInsight,
};

/// GET /api/insights/models — per-model usage aggregated from rollup
/// tables.
#[utoipa::path(get, path = "/api/insights/models", tag = "insights",
    params(InsightsAggregateQuery),
    responses(
        (status = 200, description = "Per-model aggregated usage for the requested range", body = InsightsModelsResponse),
    )
)]
pub async fn get_insights_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<InsightsAggregateQuery>,
) -> ApiResult<Json<InsightsModelsResponse>> {
    let now = chrono::Utc::now().timestamp();
    let oldest_timestamp = match state.db.get_oldest_session_date(None, None).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                endpoint = "insights_models",
                error = %e,
                "oldest_session_date lookup failed — defaulting to empty all-time"
            );
            None
        }
    };
    let effective_range = match resolve_from_to_or_all_time(ResolveFromToInput {
        endpoint: "insights_models",
        from: query.from,
        to: query.to,
        now,
        oldest_timestamp,
    }) {
        Ok(r) => {
            record_time_range_resolution("insights_models", r.source);
            r
        }
        Err(err) => {
            record_time_range_resolution_error("insights_models", err.reason.as_str());
            return Err(ApiError::BadRequest(err.message));
        }
    };

    let bucket = parse_bucket(query.bucket.as_deref());
    let limit = clamp_limit(query.limit);
    let (range_start, range_end) = resolved_range_to_unix(&effective_range);

    let (aggregates, rows_read) =
        aggregate_from_rollup(state.db.pool(), bucket, range_start, range_end).await?;

    let mut models: Vec<ModelInsight> = aggregates
        .into_iter()
        .map(|(model_id, agg)| ModelInsight {
            model_id,
            session_count: agg.session_count,
            total_tokens: agg.total_tokens,
            prompt_count: agg.prompt_count,
            avg_duration_seconds: agg.avg_duration_seconds(),
        })
        .collect();
    models.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    let rows_before_limit = models.len() as u64;
    models.truncate(limit);

    Ok(Json(InsightsModelsResponse {
        models,
        meta: InsightsAggregateMeta {
            effective_range,
            bucket: bucket_label(bucket).to_string(),
            rows_read,
            rows_returned: rows_before_limit.min(limit as u64),
            legacy_path: false,
        },
    }))
}

/// Scan the appropriate rollup table and fold rows into a per-model
/// aggregate map.
async fn aggregate_from_rollup(
    pool: &sqlx::SqlitePool,
    bucket: Bucket,
    range_start: i64,
    range_end: i64,
) -> ApiResult<(HashMap<String, DimAggregate>, u64)> {
    let mut agg: HashMap<String, DimAggregate> = HashMap::new();
    let rows_read = match bucket {
        Bucket::Daily => {
            let rows = select_range_daily_model_stats(pool, range_start, range_end)
                .await
                .map_err(|e| ApiError::Internal(format!("daily_model_stats read failed: {e}")))?;
            let n = rows.len() as u64;
            for row in rows {
                let entry = agg.entry(row.model_id).or_default();
                entry.session_count += row.session_count;
                entry.total_tokens += row.total_tokens;
                entry.prompt_count += row.prompt_count;
                entry.duration_sum_ms += row.duration_sum_ms;
                entry.duration_count += row.duration_count;
            }
            n
        }
        Bucket::Weekly => {
            let rows = select_range_weekly_model_stats(pool, range_start, range_end)
                .await
                .map_err(|e| ApiError::Internal(format!("weekly_model_stats read failed: {e}")))?;
            let n = rows.len() as u64;
            for row in rows {
                let entry = agg.entry(row.model_id).or_default();
                entry.session_count += row.session_count;
                entry.total_tokens += row.total_tokens;
                entry.prompt_count += row.prompt_count;
                entry.duration_sum_ms += row.duration_sum_ms;
                entry.duration_count += row.duration_count;
            }
            n
        }
        Bucket::Monthly => {
            let rows = select_range_monthly_model_stats(pool, range_start, range_end)
                .await
                .map_err(|e| ApiError::Internal(format!("monthly_model_stats read failed: {e}")))?;
            let n = rows.len() as u64;
            for row in rows {
                let entry = agg.entry(row.model_id).or_default();
                entry.session_count += row.session_count;
                entry.total_tokens += row.total_tokens;
                entry.prompt_count += row.prompt_count;
                entry.duration_sum_ms += row.duration_sum_ms;
                entry.duration_count += row.duration_count;
            }
            n
        }
    };
    Ok((agg, rows_read))
}
