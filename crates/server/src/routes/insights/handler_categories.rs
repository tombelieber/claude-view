//! GET /api/insights/categories handler and category tree helpers.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;

use crate::error::{ApiError, ApiResult};
use crate::metrics::{record_time_range_resolution, record_time_range_resolution_error};
use crate::state::AppState;
use crate::time_range::{resolve_from_to_or_all_time, ResolveFromToInput};

use super::db::fetch_analytics_scope_meta_for_range;
use super::types::{
    CategoriesMeta, CategoriesQuery, CategoriesResponse, CategoryBreakdown, CategoryNode,
    CategorySummary, OverallAverages,
};

/// Row returned from category aggregation query.
struct CategoryCountRow {
    category_l1: String,
    category_l2: Option<String>,
    category_l3: Option<String>,
    count: u32,
    avg_reedit_rate: f64,
    avg_duration: u32,
    avg_prompts: f64,
    commit_rate: f64,
}

/// GET /api/insights/categories - Returns hierarchical category data.
#[utoipa::path(get, path = "/api/insights/categories", tag = "insights",
    params(CategoriesQuery),
    responses(
        (status = 200, description = "Hierarchical category breakdown for treemap", body = CategoriesResponse),
    )
)]
pub async fn get_categories(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CategoriesQuery>,
) -> ApiResult<Json<CategoriesResponse>> {
    let now = chrono::Utc::now().timestamp();
    let oldest_timestamp = match state.db.get_oldest_session_date(None, None).await {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(
                endpoint = "insights_categories",
                error = %e,
                "Failed to fetch oldest session date for default all-time range"
            );
            None
        }
    };
    let effective_range = match resolve_from_to_or_all_time(ResolveFromToInput {
        endpoint: "insights_categories",
        from: query.from,
        to: query.to,
        now,
        oldest_timestamp,
    }) {
        Ok(resolved) => {
            record_time_range_resolution("insights_categories", resolved.source);
            tracing::info!(
                endpoint = "insights_categories",
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
            record_time_range_resolution_error("insights_categories", err.reason.as_str());
            tracing::warn!(
                endpoint = "insights_categories",
                reason = err.reason.as_str(),
                requested_from = query.from,
                requested_to = query.to,
                "Rejected request time range"
            );
            return Err(ApiError::BadRequest(err.message));
        }
    };

    let pool = state.db.pool();

    // Get raw category counts grouped by L1/L2/L3
    let from = Some(effective_range.from);
    let to = Some(effective_range.to);
    let counts = fetch_category_counts(pool, from, to).await?;
    let uncategorized = fetch_uncategorized_count(pool, from, to).await?;
    let overall = fetch_overall_averages(pool, from, to).await?;

    // Calculate total
    let categorized_total: u32 = counts.iter().map(|c| c.count).sum();
    let total = categorized_total + uncategorized;

    // Build hierarchical tree
    let categories = build_category_tree(&counts, total);

    // Calculate L1 breakdown
    let breakdown = calculate_breakdown(&counts, uncategorized, total);
    let analytics_scope =
        fetch_analytics_scope_meta_for_range(&state, effective_range.from, effective_range.to)
            .await?;

    Ok(Json(CategoriesResponse {
        breakdown,
        categories,
        overall_averages: overall,
        meta: CategoriesMeta {
            effective_range,
            analytics_scope,
        },
    }))
}

/// Fetch category counts grouped by L1/L2/L3 from the database.
async fn fetch_category_counts(
    pool: &sqlx::SqlitePool,
    from: Option<i64>,
    to: Option<i64>,
) -> ApiResult<Vec<CategoryCountRow>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, Option<String>, Option<String>, i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>)> =
        sqlx::query_as(
            r#"
            SELECT
                category_l1,
                category_l2,
                category_l3,
                COUNT(*) as count,
                AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
                AVG(duration_seconds) as avg_duration,
                AVG(user_prompt_count) as avg_prompts,
                SUM(CASE WHEN commit_count > 0 THEN 1.0 ELSE 0.0 END) * 100.0 / COUNT(*) as commit_rate
            FROM valid_sessions
            WHERE category_l1 IS NOT NULL
              AND (?1 IS NULL OR last_message_at >= ?1)
              AND (?2 IS NULL OR last_message_at <= ?2)
            GROUP BY category_l1, category_l2, category_l3
            ORDER BY count DESC
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch category counts: {}", e)))?;

    Ok(rows
        .into_iter()
        .map(
            |(l1, l2, l3, count, reedit, dur, prompts, commit)| CategoryCountRow {
                category_l1: l1,
                category_l2: l2,
                category_l3: l3,
                count: count as u32,
                avg_reedit_rate: reedit.unwrap_or(0.0),
                avg_duration: dur.unwrap_or(0.0) as u32,
                avg_prompts: prompts.unwrap_or(0.0),
                commit_rate: commit.unwrap_or(0.0),
            },
        )
        .collect())
}

/// Fetch count of sessions without categories.
async fn fetch_uncategorized_count(
    pool: &sqlx::SqlitePool,
    from: Option<i64>,
    to: Option<i64>,
) -> ApiResult<u32> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM valid_sessions
        WHERE category_l1 IS NULL
          AND (?1 IS NULL OR last_message_at >= ?1)
          AND (?2 IS NULL OR last_message_at <= ?2)
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch uncategorized count: {}", e)))?;

    Ok(row.0 as u32)
}

/// Fetch overall averages across all sessions for comparison.
async fn fetch_overall_averages(
    pool: &sqlx::SqlitePool,
    from: Option<i64>,
    to: Option<i64>,
) -> ApiResult<OverallAverages> {
    let row: (Option<f64>, Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
        r#"
        SELECT
            AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
            AVG(duration_seconds) as avg_duration,
            AVG(user_prompt_count) as avg_prompts,
            SUM(CASE WHEN commit_count > 0 THEN 1.0 ELSE 0.0 END) * 100.0 / NULLIF(COUNT(*), 0) as commit_rate
        FROM valid_sessions
        WHERE (?1 IS NULL OR last_message_at >= ?1)
          AND (?2 IS NULL OR last_message_at <= ?2)
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch overall averages: {}", e)))?;

    Ok(OverallAverages {
        avg_reedit_rate: row.0.unwrap_or(0.0),
        avg_duration: row.1.unwrap_or(0.0) as u32,
        avg_prompts: row.2.unwrap_or(0.0),
        commit_rate: row.3.unwrap_or(0.0),
    })
}

/// Build hierarchical category tree from flat count rows.
fn build_category_tree(counts: &[CategoryCountRow], total: u32) -> Vec<CategoryNode> {
    if total == 0 {
        return vec![];
    }

    // Group by L1
    let mut l1_map: HashMap<String, Vec<&CategoryCountRow>> = HashMap::new();
    for count in counts {
        l1_map
            .entry(count.category_l1.clone())
            .or_default()
            .push(count);
    }

    let mut result = Vec::new();

    for (l1_name, l1_counts) in &l1_map {
        let l1_total: u32 = l1_counts.iter().map(|c| c.count).sum();

        // Group by L2 within L1
        let mut l2_map: HashMap<String, Vec<&CategoryCountRow>> = HashMap::new();
        for count in l1_counts {
            if let Some(l2) = &count.category_l2 {
                l2_map.entry(l2.clone()).or_default().push(*count);
            }
        }

        let mut l2_children = Vec::new();
        for (l2_name, l2_counts) in &l2_map {
            let l2_total: u32 = l2_counts.iter().map(|c| c.count).sum();

            // Build L3 children
            let mut l3_children: Vec<CategoryNode> = l2_counts
                .iter()
                .filter_map(|c| {
                    c.category_l3.as_ref().map(|l3| CategoryNode {
                        id: format!("{}/{}/{}", l1_name, l2_name, l3),
                        level: 3,
                        name: format_category_name(l3),
                        count: c.count,
                        percentage: (c.count as f64 / total as f64) * 100.0,
                        avg_reedit_rate: c.avg_reedit_rate,
                        avg_duration: c.avg_duration,
                        avg_prompts: c.avg_prompts,
                        commit_rate: c.commit_rate,
                        insight: None,
                        children: vec![],
                    })
                })
                .collect();
            l3_children.sort_by(|a, b| b.count.cmp(&a.count));

            // Calculate L2 aggregates
            let (avg_reedit, avg_dur, avg_prompts, commit_rate) =
                aggregate_category_metrics(l2_counts);

            l2_children.push(CategoryNode {
                id: format!("{}/{}", l1_name, l2_name),
                level: 2,
                name: format_category_name(l2_name),
                count: l2_total,
                percentage: (l2_total as f64 / total as f64) * 100.0,
                avg_reedit_rate: avg_reedit,
                avg_duration: avg_dur,
                avg_prompts,
                commit_rate,
                insight: None,
                children: l3_children,
            });
        }

        // Sort L2 by count descending
        l2_children.sort_by(|a, b| b.count.cmp(&a.count));

        // Calculate L1 aggregates
        let (avg_reedit, avg_dur, avg_prompts, commit_rate) = aggregate_category_metrics(l1_counts);

        result.push(CategoryNode {
            id: l1_name.clone(),
            level: 1,
            name: format_category_name(l1_name),
            count: l1_total,
            percentage: (l1_total as f64 / total as f64) * 100.0,
            avg_reedit_rate: avg_reedit,
            avg_duration: avg_dur,
            avg_prompts,
            commit_rate,
            insight: None,
            children: l2_children,
        });
    }

    // Sort L1 by count descending
    result.sort_by(|a, b| b.count.cmp(&a.count));
    result
}

/// Calculate L1 breakdown from counts.
fn calculate_breakdown(
    counts: &[CategoryCountRow],
    uncategorized: u32,
    total: u32,
) -> CategoryBreakdown {
    let pct = |n: u32| -> f64 {
        if total > 0 {
            (n as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    };

    let mut l1_totals: HashMap<&str, u32> = HashMap::new();
    for c in counts {
        *l1_totals.entry(&c.category_l1).or_insert(0) += c.count;
    }

    let code_count = l1_totals.get("code_work").copied().unwrap_or(0);
    let support_count = l1_totals.get("support_work").copied().unwrap_or(0);
    let thinking_count = l1_totals.get("thinking_work").copied().unwrap_or(0);

    CategoryBreakdown {
        code_work: CategorySummary {
            count: code_count,
            percentage: pct(code_count),
        },
        support_work: CategorySummary {
            count: support_count,
            percentage: pct(support_count),
        },
        thinking_work: CategorySummary {
            count: thinking_count,
            percentage: pct(thinking_count),
        },
        uncategorized: CategorySummary {
            count: uncategorized,
            percentage: pct(uncategorized),
        },
    }
}

/// Format a snake_case/kebab-case slug into title case.
fn format_category_name(slug: &str) -> String {
    slug.split(['_', '-'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Weighted average metrics across category count rows.
fn aggregate_category_metrics(counts: &[&CategoryCountRow]) -> (f64, u32, f64, f64) {
    let total: u32 = counts.iter().map(|c| c.count).sum();
    if total == 0 {
        return (0.0, 0, 0.0, 0.0);
    }

    let total_f = total as f64;

    let weighted_reedit: f64 = counts
        .iter()
        .map(|c| c.avg_reedit_rate * c.count as f64)
        .sum::<f64>()
        / total_f;

    let weighted_dur: f64 = counts
        .iter()
        .map(|c| c.avg_duration as f64 * c.count as f64)
        .sum::<f64>()
        / total_f;

    let weighted_prompts: f64 = counts
        .iter()
        .map(|c| c.avg_prompts * c.count as f64)
        .sum::<f64>()
        / total_f;

    let weighted_commit: f64 = counts
        .iter()
        .map(|c| c.commit_rate * c.count as f64)
        .sum::<f64>()
        / total_f;

    (
        weighted_reedit,
        weighted_dur as u32,
        weighted_prompts,
        weighted_commit,
    )
}
