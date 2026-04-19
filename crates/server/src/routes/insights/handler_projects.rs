//! GET /api/insights/projects — per-project rollup aggregation (PR 4.5).
//!
//! Reads from `daily_project_stats` / `weekly_project_stats` /
//! `monthly_project_stats` and groups in-memory by `project_id`. Adds
//! line and commit fields to the response when they are populated
//! (PR 4.8 fold + Phase 5 flag fold populate them).
//!
//! Legacy fallback (`CLAUDE_VIEW_USE_LEGACY_STATS_READ=1`) runs a
//! direct GROUP BY on `valid_sessions` joined against `session_commits`
//! + `commits` so the line / commit view still works without rollups.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;

use claude_view_stats_rollup::stats_core::{
    select_range_daily_project_stats, select_range_monthly_project_stats,
    select_range_weekly_project_stats,
};
use claude_view_stats_rollup::Bucket;

use crate::error::{ApiError, ApiResult};
use crate::metrics::{record_time_range_resolution, record_time_range_resolution_error};
use crate::state::AppState;
use crate::time_range::{resolve_from_to_or_all_time, ResolveFromToInput};

use super::rollup_read::{
    bucket_label, clamp_limit, legacy_stats_read_enabled, parse_bucket, resolved_range_to_unix,
    DimAggregate,
};
use super::types::{
    InsightsAggregateMeta, InsightsAggregateQuery, InsightsProjectsResponse, ProjectInsight,
};

/// GET /api/insights/projects — per-project usage aggregated from
/// rollup tables.
#[utoipa::path(get, path = "/api/insights/projects", tag = "insights",
    params(InsightsAggregateQuery),
    responses(
        (status = 200, description = "Per-project aggregated usage for the requested range", body = InsightsProjectsResponse),
    )
)]
pub async fn get_insights_projects(
    State(state): State<Arc<AppState>>,
    Query(query): Query<InsightsAggregateQuery>,
) -> ApiResult<Json<InsightsProjectsResponse>> {
    let now = chrono::Utc::now().timestamp();
    let oldest_timestamp = match state.db.get_oldest_session_date(None, None).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                endpoint = "insights_projects",
                error = %e,
                "oldest_session_date lookup failed — defaulting to empty all-time"
            );
            None
        }
    };
    let effective_range = match resolve_from_to_or_all_time(ResolveFromToInput {
        endpoint: "insights_projects",
        from: query.from,
        to: query.to,
        now,
        oldest_timestamp,
    }) {
        Ok(r) => {
            record_time_range_resolution("insights_projects", r.source);
            r
        }
        Err(err) => {
            record_time_range_resolution_error("insights_projects", err.reason.as_str());
            return Err(ApiError::BadRequest(err.message));
        }
    };

    let bucket = parse_bucket(query.bucket.as_deref());
    let limit = clamp_limit(query.limit);
    let (range_start, range_end) = resolved_range_to_unix(&effective_range);
    let legacy = legacy_stats_read_enabled();

    let (aggregates, rows_read) = if legacy {
        legacy_group_by_project(state.db.pool(), effective_range.from, effective_range.to).await?
    } else {
        aggregate_from_rollup(state.db.pool(), bucket, range_start, range_end).await?
    };

    let mut projects: Vec<ProjectInsight> = aggregates
        .into_iter()
        .map(|(project_id, agg)| ProjectInsight {
            project_id,
            session_count: agg.session_count,
            total_tokens: agg.total_tokens,
            prompt_count: agg.prompt_count,
            avg_duration_seconds: agg.avg_duration_seconds(),
            lines_added: agg.lines_added,
            lines_removed: agg.lines_removed,
            commit_count: agg.commit_count,
        })
        .collect();
    projects.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    let rows_before_limit = projects.len() as u64;
    projects.truncate(limit);

    Ok(Json(InsightsProjectsResponse {
        projects,
        meta: InsightsAggregateMeta {
            effective_range,
            bucket: bucket_label(bucket).to_string(),
            rows_read,
            rows_returned: rows_before_limit.min(limit as u64),
            legacy_path: legacy,
        },
    }))
}

async fn aggregate_from_rollup(
    pool: &sqlx::SqlitePool,
    bucket: Bucket,
    range_start: i64,
    range_end: i64,
) -> ApiResult<(HashMap<String, DimAggregate>, u64)> {
    let mut agg: HashMap<String, DimAggregate> = HashMap::new();
    let rows_read = match bucket {
        Bucket::Daily => {
            let rows = select_range_daily_project_stats(pool, range_start, range_end)
                .await
                .map_err(|e| ApiError::Internal(format!("daily_project_stats read failed: {e}")))?;
            let n = rows.len() as u64;
            for row in rows {
                let entry = agg.entry(row.project_id).or_default();
                entry.session_count += row.session_count;
                entry.total_tokens += row.total_tokens;
                entry.prompt_count += row.prompt_count;
                entry.duration_sum_ms += row.duration_sum_ms;
                entry.duration_count += row.duration_count;
                entry.lines_added += row.lines_added;
                entry.lines_removed += row.lines_removed;
                entry.commit_count += row.commit_count;
            }
            n
        }
        Bucket::Weekly => {
            let rows = select_range_weekly_project_stats(pool, range_start, range_end)
                .await
                .map_err(|e| {
                    ApiError::Internal(format!("weekly_project_stats read failed: {e}"))
                })?;
            let n = rows.len() as u64;
            for row in rows {
                let entry = agg.entry(row.project_id).or_default();
                entry.session_count += row.session_count;
                entry.total_tokens += row.total_tokens;
                entry.prompt_count += row.prompt_count;
                entry.duration_sum_ms += row.duration_sum_ms;
                entry.duration_count += row.duration_count;
                entry.lines_added += row.lines_added;
                entry.lines_removed += row.lines_removed;
                entry.commit_count += row.commit_count;
            }
            n
        }
        Bucket::Monthly => {
            let rows = select_range_monthly_project_stats(pool, range_start, range_end)
                .await
                .map_err(|e| {
                    ApiError::Internal(format!("monthly_project_stats read failed: {e}"))
                })?;
            let n = rows.len() as u64;
            for row in rows {
                let entry = agg.entry(row.project_id).or_default();
                entry.session_count += row.session_count;
                entry.total_tokens += row.total_tokens;
                entry.prompt_count += row.prompt_count;
                entry.duration_sum_ms += row.duration_sum_ms;
                entry.duration_count += row.duration_count;
                entry.lines_added += row.lines_added;
                entry.lines_removed += row.lines_removed;
                entry.commit_count += row.commit_count;
            }
            n
        }
    };
    Ok((agg, rows_read))
}

async fn legacy_group_by_project(
    pool: &sqlx::SqlitePool,
    from: i64,
    to: i64,
) -> ApiResult<(HashMap<String, DimAggregate>, u64)> {
    // Session-level aggregates: count + tokens + prompts + duration.
    let session_rows: Vec<(String, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
        r"SELECT project_id,
                 COUNT(*),
                 COALESCE(SUM(total_input_tokens + total_output_tokens +
                              COALESCE(cache_read_tokens,0) +
                              COALESCE(cache_creation_tokens,0)), 0),
                 COALESCE(SUM(user_prompt_count), 0),
                 COALESCE(SUM(duration_seconds), 0),
                 COALESCE(SUM(ai_lines_added), 0),
                 COALESCE(SUM(ai_lines_removed), 0)
            FROM valid_sessions
           WHERE last_message_at >= ?1 AND last_message_at <= ?2
             AND project_id IS NOT NULL AND project_id <> ''
           GROUP BY project_id",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("legacy project GROUP BY (sessions) failed: {e}")))?;

    let rows_read = session_rows.len() as u64;
    let mut agg = HashMap::with_capacity(session_rows.len());
    for (project_id, cnt, tokens, prompts, dur_s, lines_add, lines_rem) in session_rows {
        agg.insert(
            project_id,
            DimAggregate {
                session_count: cnt.max(0) as u64,
                total_tokens: tokens.max(0) as u64,
                prompt_count: prompts.max(0) as u64,
                duration_sum_ms: (dur_s.max(0) as u64) * 1000,
                duration_count: cnt.max(0) as u64,
                lines_added: lines_add.max(0) as u64,
                lines_removed: lines_rem.max(0) as u64,
                commit_count: 0,
            },
        );
    }

    // Commits sub-query: join session_commits → commits → sessions so
    // we count distinct commits per project. Skipping this when the
    // previous GROUP BY returned nothing avoids a pointless scan.
    if !agg.is_empty() {
        let commit_rows: Vec<(String, i64)> = sqlx::query_as(
            r"SELECT s.project_id,
                     COUNT(DISTINCT sc.commit_hash)
                FROM session_commits sc
                JOIN valid_sessions s ON sc.session_id = s.id
               WHERE s.last_message_at >= ?1 AND s.last_message_at <= ?2
                 AND s.project_id IS NOT NULL AND s.project_id <> ''
               GROUP BY s.project_id",
        )
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("legacy project GROUP BY (commits) failed: {e}"))
        })?;
        for (project_id, commits) in commit_rows {
            if let Some(entry) = agg.get_mut(&project_id) {
                entry.commit_count = commits.max(0) as u64;
            }
        }
    }

    Ok((agg, rows_read))
}
