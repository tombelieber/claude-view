//! Dashboard statistics endpoint.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use claude_view_core::pricing::{self as pricing_engine};
use claude_view_core::{
    claude_projects_dir, AnalyticsScopeMeta, AnalyticsSessionBreakdown, DashboardStats,
    EffectiveRangeMeta, EffectiveRangeSource,
};
use claude_view_db::trends::{TrendMetric, WeekTrends};
use claude_view_db::{AIGenerationStats, AggregateCostBreakdown};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ApiResult;
use crate::metrics::{
    record_request, record_time_range_resolution, record_time_range_resolution_error,
};
use crate::state::AppState;
use crate::time_range::{resolve_from_to_or_all_time, ResolveFromToInput};

/// Query parameters for dashboard stats endpoint.
#[derive(Debug, Clone, Default, Deserialize, utoipa::IntoParams)]
pub struct DashboardQuery {
    /// Period start timestamp (Unix seconds, inclusive).
    /// If omitted along with `to`, returns all-time stats with no trends.
    pub from: Option<i64>,
    /// Period end timestamp (Unix seconds, inclusive).
    /// If omitted along with `from`, returns all-time stats with no trends.
    pub to: Option<i64>,
    /// Optional project filter (matches sessions.project_id).
    pub project: Option<String>,
    /// Optional branch filter (matches sessions.git_branch).
    pub branch: Option<String>,
}

/// Current period metrics for dashboard (adapts to selected time range).
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CurrentPeriodMetrics {
    #[ts(type = "number")]
    pub session_count: u64,
    #[ts(type = "number")]
    pub total_tokens: u64,
    #[ts(type = "number")]
    pub total_files_edited: u64,
    #[ts(type = "number")]
    pub commit_count: u64,
}

/// Extended dashboard stats with current period and trends.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ExtendedDashboardStats {
    /// Base dashboard stats
    #[serde(flatten)]
    pub base: DashboardStats,
    /// Current period metrics (adapts to selected time range)
    pub current_week: CurrentPeriodMetrics,
    /// Period-over-period trends (None if viewing all-time)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trends: Option<DashboardTrends>,
    /// Start of the requested period (Unix timestamp).
    #[ts(type = "number | null")]
    pub period_start: Option<i64>,
    /// End of the requested period (Unix timestamp).
    #[ts(type = "number | null")]
    pub period_end: Option<i64>,
    /// Start of the comparison period (Unix timestamp).
    #[ts(type = "number | null")]
    pub comparison_period_start: Option<i64>,
    /// End of the comparison period (Unix timestamp).
    #[ts(type = "number | null")]
    pub comparison_period_end: Option<i64>,
    /// Earliest session date in the database (Unix timestamp).
    /// Used to display "since [date]" in the UI.
    #[ts(type = "number | null")]
    pub data_start_date: Option<i64>,
    /// Additive section-specific effective range metadata.
    pub meta: DashboardMeta,
}

/// Dashboard metadata wrapper.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DashboardMeta {
    pub ranges: DashboardRangesMeta,
    #[serde(flatten)]
    pub analytics_scope: AnalyticsScopeMeta,
}

/// Section-specific range metadata for dashboard.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DashboardRangesMeta {
    pub current_period: EffectiveRangeMeta,
    pub heatmap: EffectiveRangeMeta,
}

/// Simplified trends for dashboard display.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DashboardTrends {
    /// Session count trend
    pub sessions: TrendMetric,
    /// Token usage trend
    pub tokens: TrendMetric,
    /// Files edited trend
    pub files_edited: TrendMetric,
    /// Commits linked trend
    pub commits: TrendMetric,
    /// Avg tokens per prompt trend
    pub avg_tokens_per_prompt: TrendMetric,
    /// Avg re-edit rate trend (percentage 0-100)
    pub avg_reedit_rate: TrendMetric,
}

/// AI generation response wrapper with additive metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AIGenerationStatsResponse {
    #[serde(flatten)]
    pub base: AIGenerationStats,
    pub meta: AnalyticsScopeMeta,
}

impl From<WeekTrends> for DashboardTrends {
    fn from(t: WeekTrends) -> Self {
        Self {
            sessions: t.session_count,
            tokens: t.total_tokens,
            files_edited: t.total_files_edited,
            commits: t.commit_link_count,
            avg_tokens_per_prompt: t.avg_tokens_per_prompt,
            avg_reedit_rate: t.avg_reedit_rate,
        }
    }
}

/// Storage statistics for the settings page.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    /// Size of JSONL session files in bytes.
    #[ts(type = "number")]
    pub jsonl_bytes: u64,
    /// Size of SQLite database in bytes.
    #[ts(type = "number")]
    pub sqlite_bytes: u64,
    /// Size of search index in bytes.
    #[ts(type = "number")]
    pub index_bytes: u64,
    /// Total number of sessions.
    #[ts(type = "number")]
    pub session_count: i64,
    /// Total number of projects.
    #[ts(type = "number")]
    pub project_count: i64,
    /// Total number of linked commits.
    #[ts(type = "number")]
    pub commit_count: i64,
    /// Unix timestamp of oldest session.
    #[ts(type = "number | null")]
    pub oldest_session_date: Option<i64>,
    /// Unix timestamp of last index completion.
    #[ts(type = "number | null")]
    pub last_index_at: Option<i64>,
    /// Duration of last index in milliseconds.
    #[ts(type = "number | null")]
    pub last_index_duration_ms: Option<i64>,
    /// Number of sessions indexed in last run.
    #[ts(type = "number")]
    pub last_index_session_count: i64,
    /// Unix timestamp of last git sync.
    #[ts(type = "number | null")]
    pub last_git_sync_at: Option<i64>,
    /// Duration of last git sync in milliseconds (not currently tracked, returns None).
    #[ts(type = "number | null")]
    pub last_git_sync_duration_ms: Option<i64>,
    /// Number of repos scanned in last git sync (not currently tracked, returns 0).
    #[ts(type = "number")]
    pub last_git_sync_repo_count: i64,
    /// Path to JSONL session files (Claude Code data, read-only).
    pub jsonl_path: Option<String>,
    /// Path to SQLite database file.
    pub sqlite_path: Option<String>,
    /// Path to Tantivy search index directory.
    pub index_path: Option<String>,
    /// Parent app data directory — safe to delete, rebuilt on next launch.
    pub app_data_path: Option<String>,
}

async fn fetch_session_breakdown(
    state: &Arc<AppState>,
    from: Option<i64>,
    to: Option<i64>,
    project: Option<&str>,
    branch: Option<&str>,
) -> ApiResult<AnalyticsSessionBreakdown> {
    let (primary_sessions, sidechain_sessions): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN is_sidechain = 0 THEN 1 ELSE 0 END), 0) AS primary_sessions,
            COALESCE(SUM(CASE WHEN is_sidechain = 1 THEN 1 ELSE 0 END), 0) AS sidechain_sessions
        FROM sessions
        WHERE (?1 IS NULL OR last_message_at >= ?1)
          AND (?2 IS NULL OR last_message_at <= ?2)
          AND (?3 IS NULL OR project_id = ?3)
          AND (?4 IS NULL OR git_branch = ?4)
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(project)
    .bind(branch)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        crate::error::ApiError::Internal(format!("Failed to fetch session breakdown: {e}"))
    })?;

    Ok(AnalyticsSessionBreakdown::new(
        primary_sessions,
        sidechain_sessions,
    ))
}

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
    let heatmap_range = EffectiveRangeMeta {
        from: now - 90 * 86400,
        to: now,
        source: EffectiveRangeSource::ExplicitRangeParam,
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

    // Get base dashboard stats (always includes heatmap which is fixed at 90 days)
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
            let comp_start = comp_end - duration;
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
        // All-time view: show aggregate stats but no trends
        match state
            .db
            .get_all_time_metrics(query.project.as_deref(), query.branch.as_deref())
            .await
        {
            Ok((session_count, total_tokens, total_files_edited, commit_count)) => {
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

/// GET /api/stats/storage - Storage statistics for the settings page.
///
/// Returns:
/// - Storage sizes: JSONL files, SQLite database, search index
/// - Counts: sessions, projects, commits
/// - Timing: oldest session, last index, last git sync
#[utoipa::path(get, path = "/api/stats/storage", tag = "stats",
    responses(
        (status = 200, description = "Storage usage statistics (JSONL, SQLite, search index)", body = StorageStats),
    )
)]
pub async fn storage_stats(State(state): State<Arc<AppState>>) -> ApiResult<Json<StorageStats>> {
    let start = Instant::now();

    // Get index metadata for timing info
    let metadata = match state.db.get_index_metadata().await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(
                endpoint = "storage_stats",
                error = %e,
                "Failed to fetch index metadata"
            );
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Get all counts in a single query (replaces 4 separate queries)
    let (session_count, project_count, commit_count, oldest_session_date) = match state
        .db
        .get_storage_counts()
        .await
    {
        Ok(counts) => counts,
        Err(e) => {
            tracing::error!(endpoint = "storage_stats", error = %e, "Failed to get storage counts");
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Calculate JSONL storage size
    let jsonl_bytes = calculate_jsonl_size().await;

    // Calculate SQLite database size
    let sqlite_bytes = match state.db.get_database_size().await {
        Ok(size) => size as u64,
        Err(e) => {
            tracing::error!(endpoint = "storage_stats", error = %e, "Failed to get database size");
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Search index size — measured from actual directory on disk
    let index_bytes = match claude_view_core::paths::search_index_dir() {
        Some(dir) if dir.exists() => calculate_directory_size(&dir).await,
        _ => 0,
    };

    // Resolve display paths (replace $HOME with ~ for readability)
    let home = dirs::home_dir().map(|h| h.to_string_lossy().to_string());
    let shorten = |p: Option<std::path::PathBuf>| -> Option<String> {
        p.map(|path| {
            let s = path.to_string_lossy().to_string();
            match &home {
                Some(h) if s.starts_with(h.as_str()) => format!("~{}", &s[h.len()..]),
                _ => s,
            }
        })
    };

    let jsonl_path = shorten(claude_projects_dir().ok());
    let sqlite_path = shorten(claude_view_core::paths::db_path());
    let index_path = shorten(claude_view_core::paths::search_index_dir());
    let app_data_path = shorten(Some(claude_view_core::paths::data_dir()));

    record_request("storage_stats", "200", start.elapsed());

    Ok(Json(StorageStats {
        jsonl_bytes,
        sqlite_bytes,
        index_bytes,
        session_count,
        project_count,
        commit_count,
        oldest_session_date,
        last_index_at: metadata.last_indexed_at,
        last_index_duration_ms: metadata.last_index_duration_ms,
        last_index_session_count: metadata.sessions_indexed,
        last_git_sync_at: metadata.last_git_sync_at,
        last_git_sync_duration_ms: None, // Not tracked currently
        last_git_sync_repo_count: 0,     // Not tracked currently
        jsonl_path,
        sqlite_path,
        index_path,
        app_data_path,
    }))
}

/// Calculate total size of JSONL session files in ~/.claude/projects/
async fn calculate_jsonl_size() -> u64 {
    let projects_dir = match claude_projects_dir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to locate Claude projects directory for JSONL size calculation");
            return 0;
        }
    };

    calculate_directory_jsonl_size(&projects_dir).await
}

/// Recursively calculate the total size of .jsonl files in a directory.
async fn calculate_directory_jsonl_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, dir = %dir.display(), "Failed to read directory for JSONL size calculation");
            return 0;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "Failed to get file type during JSONL size calculation");
                continue;
            }
        };

        if file_type.is_dir() {
            // Recurse into subdirectories (project directories)
            total += Box::pin(calculate_directory_jsonl_size(&path)).await;
        } else if file_type.is_file() {
            // Only count .jsonl files
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => {
                        total += metadata.len();
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, path = %path.display(), "Failed to get metadata for JSONL file");
                    }
                }
            }
        }
    }

    total
}

/// Recursively calculate the total size of all files in a directory.
async fn calculate_directory_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return 0,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            total += Box::pin(calculate_directory_size(&path)).await;
        } else if file_type.is_file() {
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                total += metadata.len();
            }
        }
    }

    total
}

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

/// Create the stats routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/stats/dashboard", get(dashboard_stats))
        .route("/stats/storage", get(storage_stats))
        .route("/stats/ai-generation", get(ai_generation_stats))
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_core::{SessionInfo, ToolCounts};
    use claude_view_db::{AggregateCostBreakdown, Database};
    use sqlx::Executor;
    use tower::ServiceExt;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    fn session_fixture(id: &str, modified_at: i64, is_sidechain: bool) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: "project-meta".to_string(),
            project_path: "/home/user/project-meta".to_string(),
            display_name: "project-meta".to_string(),
            git_root: None,
            file_path: format!("/path/{}.jsonl", id),
            modified_at,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: Some("main".to_string()),
            is_sidechain,
            deep_indexed: false,
            total_input_tokens: Some(100),
            total_output_tokens: Some(200),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 2,
            api_call_count: 4,
            tool_call_count: 6,
            files_read: vec![],
            files_edited: vec!["src/main.rs".to_string()],
            files_read_count: 1,
            files_edited_count: 1,
            reedited_files_count: 0,
            duration_seconds: 120,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        }
    }

    #[test]
    fn test_aggregate_cost_breakdown_defaults_are_explicit() {
        let cost = AggregateCostBreakdown::default();
        assert_eq!(cost.total_cost_usd, 0.0);
        assert_eq!(cost.computed_priced_total_cost_usd, 0.0);
        assert_eq!(cost.total_cost_source, "");
        assert!(!cost.has_unpriced_usage);
        assert_eq!(cost.priced_token_coverage, 0.0);
    }

    #[tokio::test]
    async fn test_dashboard_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 0);
        assert_eq!(json["totalProjects"], 0);
        assert!(json["heatmap"].is_array());
        assert!(json["topSkills"].is_array());
        assert!(json["topProjects"].is_array());
        assert!(json["toolTotals"].is_object());

        // Check extended fields
        assert!(json["currentWeek"].is_object());
        assert_eq!(json["currentWeek"]["sessionCount"], 0);
        // All-time view (no time range params) should NOT include trends
        assert!(json.get("trends").is_none() || json["trends"].is_null());
        // dataStartDate should be null for empty DB
        assert!(json["dataStartDate"].is_null());
        assert!(json["meta"]["ranges"]["currentPeriod"]["from"].is_number());
        assert!(json["meta"]["ranges"]["currentPeriod"]["to"].is_number());
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "default_all_time"
        );
        assert!(json["meta"]["ranges"]["heatmap"]["from"].is_number());
        assert!(json["meta"]["ranges"]["heatmap"]["to"].is_number());
        assert_eq!(
            json["meta"]["ranges"]["heatmap"]["source"],
            "explicit_range_param"
        );
    }

    #[tokio::test]
    async fn test_dashboard_stats_includes_data_scope_meta() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["meta"]["dataScope"]["sessions"],
            "primary_sessions_only"
        );
        assert_eq!(
            json["meta"]["dataScope"]["workload"],
            "primary_plus_subagent_work"
        );
    }

    #[tokio::test]
    async fn test_dashboard_stats_includes_session_breakdown_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        let primary = session_fixture("dash-primary", now - 120, false);
        let sidechain = session_fixture("dash-sidechain", now - 60, true);
        db.insert_session(&primary, "project-meta", "Project Meta")
            .await
            .unwrap();
        db.insert_session(&sidechain, "project-meta", "Project Meta")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["sessionBreakdown"]["primarySessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["sidechainSessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["otherSessions"], 0);
        assert_eq!(json["meta"]["sessionBreakdown"]["totalObservedSessions"], 2);
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_time_range() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session = SessionInfo {
            id: "sess-range-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            display_name: "project-a".to_string(),
            git_root: None,
            file_path: "/path/sess-range-1.jsonl".to_string(),
            modified_at: now - 86400, // 1 day ago
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 10,
            files_edited_count: 3,
            reedited_files_count: 1,
            duration_seconds: 300,
            commit_count: 1,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);

        // Query with time range (7 days)
        let seven_days_ago = now - (7 * 86400);
        let uri = format!("/api/stats/dashboard?from={}&to={}", seven_days_ago, now);
        let (status, body) = do_get(app, &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // With time range params, trends should be present
        assert!(json["trends"].is_object());
        assert!(json["trends"]["sessions"].is_object());
        assert!(json["trends"]["sessions"]["current"].is_number());
        assert!(json["trends"]["sessions"]["previous"].is_number());

        // Period bounds should be present
        assert!(json["periodStart"].is_number());
        assert!(json["periodEnd"].is_number());
        assert!(json["comparisonPeriodStart"].is_number());
        assert!(json["comparisonPeriodEnd"].is_number());
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "explicit_from_to"
        );
        assert_eq!(
            json["meta"]["ranges"]["heatmap"]["source"],
            "explicit_range_param"
        );

        // dataStartDate should be set
        assert!(json["dataStartDate"].is_number());
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            display_name: "project-a".to_string(),
            git_root: None,
            file_path: "/path/sess-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec!["/commit".to_string()],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,

            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);
        assert_eq!(json["totalProjects"], 1);
        assert!(!json["heatmap"].as_array().unwrap().is_empty());

        // Check current week metrics (all-time view)
        assert!(json["currentWeek"]["sessionCount"].is_number());

        // All-time view should not include trends
        assert!(json.get("trends").is_none() || json["trends"].is_null());

        // dataStartDate should be set when there's data
        assert!(json["dataStartDate"].is_number());
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "default_all_time"
        );
        assert_eq!(
            json["meta"]["ranges"]["heatmap"]["source"],
            "explicit_range_param"
        );
    }

    #[tokio::test]
    async fn test_dashboard_stats_rejects_one_sided_ranges() {
        let db = test_db().await;
        let app = build_app(db);

        let (from_status, from_body) =
            do_get(app.clone(), "/api/stats/dashboard?from=1700000000").await;
        assert_eq!(from_status, StatusCode::BAD_REQUEST);
        let from_json: serde_json::Value = serde_json::from_str(&from_body).unwrap();
        assert!(from_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));

        let (to_status, to_body) = do_get(app, "/api/stats/dashboard?to=1700000000").await;
        assert_eq!(to_status, StatusCode::BAD_REQUEST);
        let to_json: serde_json::Value = serde_json::from_str(&to_body).unwrap();
        assert!(to_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));
    }

    #[tokio::test]
    async fn test_dashboard_stats_rejects_inverted_range() {
        let db = test_db().await;
        let app = build_app(db);

        let (status, body) =
            do_get(app, "/api/stats/dashboard?from=1700100000&to=1700000000").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"]
            .as_str()
            .unwrap()
            .contains("'from' must be <= 'to'"));
    }

    #[tokio::test]
    async fn test_dashboard_stats_accepts_equal_bounds() {
        let db = test_db().await;
        let app = build_app(db);

        let ts = chrono::Utc::now().timestamp();
        let (status, body) =
            do_get(app, &format!("/api/stats/dashboard?from={}&to={}", ts, ts)).await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["periodStart"], ts);
        assert_eq!(json["periodEnd"], ts);
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "explicit_from_to"
        );
    }

    #[tokio::test]
    async fn test_storage_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/storage").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All counts should be 0
        assert_eq!(json["sessionCount"], 0);
        assert_eq!(json["projectCount"], 0);
        assert_eq!(json["commitCount"], 0);

        // Storage sizes should be present (even if 0)
        assert!(json["jsonlBytes"].is_number());
        assert!(json["sqliteBytes"].is_number());
        assert!(json["indexBytes"].is_number());

        // Oldest session should be null for empty DB
        assert!(json["oldestSessionDate"].is_null());

        // Last index/sync should be null for fresh DB
        assert!(json["lastIndexAt"].is_null());
        assert!(json["lastGitSyncAt"].is_null());
    }

    #[tokio::test]
    async fn test_storage_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session
        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            display_name: "project-a".to_string(),
            git_root: None,
            file_path: "/path/sess-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        // Update index metadata
        db.update_index_metadata_on_success(1500, 1, 1)
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/storage").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have 1 session and 1 project
        assert_eq!(json["sessionCount"], 1);
        assert_eq!(json["projectCount"], 1);

        // Oldest session should be set
        assert!(json["oldestSessionDate"].is_number());

        // Last index info should be present
        assert!(json["lastIndexAt"].is_number());
        assert_eq!(json["lastIndexDurationMs"], 1500);
        assert_eq!(json["lastIndexSessionCount"], 1);

        // SQLite size should be > 0 for non-empty db
        assert!(json["sqliteBytes"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All values should be 0 for empty DB
        assert_eq!(json["linesAdded"], 0);
        assert_eq!(json["linesRemoved"], 0);
        assert_eq!(json["filesCreated"], 0);
        assert_eq!(json["totalInputTokens"], 0);
        assert_eq!(json["totalOutputTokens"], 0);

        // Arrays should be empty
        assert!(json["tokensByModel"].as_array().unwrap().is_empty());
        assert!(json["tokensByProject"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_ai_generation_includes_data_scope_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        let primary = session_fixture("ai-primary", now - 120, false);
        let sidechain = session_fixture("ai-sidechain", now - 60, true);
        db.insert_session(&primary, "project-meta", "Project Meta")
            .await
            .unwrap();
        db.insert_session(&sidechain, "project-meta", "Project Meta")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["meta"]["dataScope"]["sessions"],
            "primary_sessions_only"
        );
        assert_eq!(
            json["meta"]["dataScope"]["workload"],
            "primary_plus_subagent_work"
        );
        assert_eq!(json["meta"]["sessionBreakdown"]["primarySessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["sidechainSessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["otherSessions"], 0);
        assert_eq!(json["meta"]["sessionBreakdown"]["totalObservedSessions"], 2);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_marks_partial_when_unpriced_model_present() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        let session = session_fixture("sess-aigen-unpriced", now - 60, false);
        db.insert_session(&session, "project-meta", "Project Meta")
            .await
            .unwrap();

        // Unknown model exists in DB but has no pricing entry.
        db.pool()
            .execute(sqlx::query(
                r#"
                INSERT OR IGNORE INTO models (id, provider, family, first_seen, last_seen)
                VALUES ('unknown-model-without-pricing', 'unknown', 'unknown', 0, 0)
                "#,
            ))
            .await
            .unwrap();
        db.pool()
            .execute(
                sqlx::query(
                    r#"
                    INSERT INTO turns (
                        session_id, uuid, seq, model_id, input_tokens, output_tokens,
                        cache_read_tokens, cache_creation_tokens, timestamp
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind("sess-aigen-unpriced")
                .bind("turn-unpriced-1")
                .bind(1)
                .bind("unknown-model-without-pricing")
                .bind(5_000)
                .bind(1_000)
                .bind(0)
                .bind(0)
                .bind(now - 60),
            )
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["cost"]["hasUnpricedUsage"], true);
        assert_eq!(
            json["cost"]["totalCostSource"],
            "computed_priced_tokens_partial"
        );
        assert_eq!(json["cost"]["unpricedModelCount"], 1);
        assert_eq!(json["cost"]["pricedTokenCoverage"], 0.0);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session with token data
        // Use update_session_deep_fields to set token data since insert_session doesn't handle tokens
        let session = SessionInfo {
            id: "sess-ai-1".to_string(),
            project: "project-ai".to_string(),
            project_path: "/home/user/project-ai".to_string(),
            display_name: "project-ai".to_string(),
            git_root: None,
            file_path: "/path/sess-ai-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "AI Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec![],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec!["src/main.rs".to_string()],
            files_read_count: 15,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-ai", "Project AI")
            .await
            .unwrap();

        // Update with token data and first_message_at
        db.update_session_deep_fields(
            "sess-ai-1",
            "Test msg",
            8,                    // turn_count
            5,                    // tool_edit
            10,                   // tool_read
            3,                    // tool_bash
            2,                    // tool_write
            r#"["src/main.rs"]"#, // files_touched
            "[]",                 // skills_used
            10,                   // user_prompt_count
            20,                   // api_call_count
            50,                   // tool_call_count
            "[]",                 // files_read
            r#"["src/main.rs"]"#, // files_edited
            15,                   // files_read_count
            5,                    // files_edited_count
            2,                    // reedited_files_count
            600,                  // duration_seconds
            3,                    // commit_count
            Some(now - 86400),    // first_message_at
            150000,               // total_input_tokens
            250000,               // total_output_tokens
            10000,                // cache_read_tokens
            5000,                 // cache_creation_tokens
            2,                    // thinking_block_count
            Some(500),            // turn_duration_avg_ms
            Some(2000),           // turn_duration_max_ms
            Some(4000),           // turn_duration_total_ms
            0,                    // api_error_count
            0,                    // api_retry_count
            0,                    // compaction_count
            0,                    // hook_blocked_count
            0,                    // agent_spawn_count
            0,                    // bash_progress_count
            0,                    // hook_progress_count
            0,                    // mcp_progress_count
            None,                 // summary_text
            1,                    // parse_version
            2048,                 // file_size
            now - 86400,          // file_mtime
            0,
            0,
            0, // lines_added, lines_removed, loc_source
            0,
            0,         // ai_lines_added, ai_lines_removed
            None,      // work_type
            None,      // git_branch
            None,      // primary_model
            None,      // last_message_at
            None,      // first_user_prompt
            0,         // total_task_time_seconds
            None,      // longest_task_seconds
            None,      // longest_task_preview
            Some(0.0), // total_cost_usd
        )
        .await
        .unwrap();

        // Update the primary_model column using the db pool directly
        db.set_session_primary_model("sess-ai-1", "claude-3-5-sonnet-20241022")
            .await
            .unwrap();

        // Ground-truth model rollups are sourced from turns.model_id.
        db.pool()
            .execute(sqlx::query(
                r#"
                    INSERT OR IGNORE INTO models (id, provider, family, first_seen, last_seen)
                    VALUES ('claude-3-5-sonnet-20241022', 'anthropic', 'sonnet', 0, 0)
                    "#,
            ))
            .await
            .unwrap();
        db.pool()
            .execute(
                sqlx::query(
                    r#"
                    INSERT INTO turns (
                        session_id, uuid, seq, model_id, input_tokens, output_tokens,
                        cache_read_tokens, cache_creation_tokens, timestamp
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind("sess-ai-1")
                .bind("turn-ai-1")
                .bind(1)
                .bind("claude-3-5-sonnet-20241022")
                .bind(150000)
                .bind(250000)
                .bind(10000)
                .bind(5000)
                .bind(now - 86400),
            )
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Lines are not tracked yet, should be 0
        assert_eq!(json["linesAdded"], 0);
        assert_eq!(json["linesRemoved"], 0);

        // Files created should match files_edited_count
        assert_eq!(json["filesCreated"], 5);

        // Token totals
        assert_eq!(json["totalInputTokens"], 150000);
        assert_eq!(json["totalOutputTokens"], 250000);

        // Token by model should have our model
        let models = json["tokensByModel"].as_array().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(models[0]["inputTokens"], 150000);
        assert_eq!(models[0]["outputTokens"], 250000);

        // Token by project should have our project
        let projects = json["tokensByProject"].as_array().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["project"], "Project AI");
        assert_eq!(projects[0]["inputTokens"], 150000);
        assert_eq!(projects[0]["outputTokens"], 250000);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_time_range() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session with a known first_message_at
        let session = SessionInfo {
            id: "sess-range".to_string(),
            project: "project-range".to_string(),
            project_path: "/home/user/project-range".to_string(),
            display_name: "project-range".to_string(),
            git_root: None,
            file_path: "/path/sess-range.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Range Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 10,
            files_edited_count: 3,
            reedited_files_count: 1,
            duration_seconds: 300,
            commit_count: 1,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-range", "Project Range")
            .await
            .unwrap();

        // Update with token data and first_message_at
        db.update_session_deep_fields(
            "sess-range",
            "Test msg",
            5,
            0,
            0,
            0,
            0,
            "[]",
            "[]",
            5,
            10,
            20,
            "[]",
            "[]",
            10,
            3,
            1,
            300,
            1,
            Some(now - 86400), // first_message_at: 1 day ago
            100000,
            200000,
            0,
            0,
            0,
            None,
            None,
            None,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            None,
            1,
            2048,
            now - 86400,
            0,
            0,
            0, // lines_added, lines_removed, loc_source
            0,
            0,         // ai_lines_added, ai_lines_removed
            None,      // work_type
            None,      // git_branch
            None,      // primary_model
            None,      // last_message_at
            None,      // first_user_prompt
            0,         // total_task_time_seconds
            None,      // longest_task_seconds
            None,      // longest_task_preview
            Some(0.0), // total_cost_usd
        )
        .await
        .unwrap();

        let app = build_app(db);

        // Query with time range that includes the session
        let seven_days_ago = now - (7 * 86400);
        let uri = format!(
            "/api/stats/ai-generation?from={}&to={}",
            seven_days_ago, now
        );
        let (status, body) = do_get(app.clone(), &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalInputTokens"], 100000);
        assert_eq!(json["totalOutputTokens"], 200000);

        // Query with time range that excludes the session (future)
        let uri = format!(
            "/api/stats/ai-generation?from={}&to={}",
            now + 86400,
            now + 172800
        );
        let (status, body) = do_get(app, &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalInputTokens"], 0);
        assert_eq!(json["totalOutputTokens"], 0);
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_project_filter() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session_a = SessionInfo {
            id: "sess-proj-a".to_string(),
            project: "project-alpha".to_string(),
            project_path: "/home/user/project-alpha".to_string(),
            display_name: "project-alpha".to_string(),
            git_root: None,
            file_path: "/path/sess-proj-a.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Alpha session".to_string(),
            last_message: "Test msg A".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: Some("main".to_string()),
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 15,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session_a, "project-alpha", "Project Alpha")
            .await
            .unwrap();

        let mut session_b = session_a.clone();
        session_b.id = "sess-proj-b".to_string();
        session_b.project = "project-beta".to_string();
        session_b.project_path = "/home/user/project-beta".to_string();
        session_b.file_path = "/path/sess-proj-b.jsonl".to_string();
        session_b.preview = "Beta session".to_string();
        session_b.git_branch = Some("develop".to_string());
        db.insert_session(&session_b, "project-beta", "Project Beta")
            .await
            .unwrap();

        let app = build_app(db);

        // Filter by project
        let (status, body) =
            do_get(app.clone(), "/api/stats/dashboard?project=project-alpha").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["totalSessions"], 1,
            "should only count project-alpha sessions"
        );
        assert_eq!(json["totalProjects"], 1);

        // Filter by project + branch
        let (status, body) = do_get(
            app.clone(),
            "/api/stats/dashboard?project=project-alpha&branch=main",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);

        // Filter by project + wrong branch = 0 sessions
        let (status, body) = do_get(
            app.clone(),
            "/api/stats/dashboard?project=project-alpha&branch=develop",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 0);

        // No filter — both sessions
        let (status, body) = do_get(app, "/api/stats/dashboard").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 2);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_project_filter() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session_a = SessionInfo {
            id: "sess-aigen-a".to_string(),
            project: "project-alpha".to_string(),
            project_path: "/home/user/project-alpha".to_string(),
            display_name: "project-alpha".to_string(),
            git_root: None,
            file_path: "/path/sess-aigen-a.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Alpha AI".to_string(),
            last_message: "msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: Some("main".to_string()),
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 5,
            files_edited_count: 3,
            reedited_files_count: 0,
            duration_seconds: 300,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session_a, "project-alpha", "Project Alpha")
            .await
            .unwrap();

        let app = build_app(db);

        // Filter by project
        let (status, body) = do_get(
            app.clone(),
            "/api/stats/ai-generation?project=project-alpha",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["filesCreated"], 3);

        // Filter by non-existent project = 0
        let (status, body) = do_get(app, "/api/stats/ai-generation?project=project-nope").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["filesCreated"], 0);
    }
}
