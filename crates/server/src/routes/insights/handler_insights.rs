//! GET /api/insights handler and overview computation helpers.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;

use claude_view_core::insights::generator::GeneratedInsight;
use claude_view_core::patterns::calculate_all_patterns;
use claude_view_core::types::SessionInfo;

use crate::error::{ApiError, ApiResult};
use crate::metrics::{record_time_range_resolution, record_time_range_resolution_error};
use crate::state::AppState;
use crate::time_range::{resolve_from_to_or_all_time, ResolveFromToInput};

use super::db::{fetch_analytics_scope_meta_for_range, LightSession};
use super::types::{
    BestTimeStats, ClassificationCoverage, EfficiencyStats, InsightsMeta, InsightsOverview,
    InsightsQuery, InsightsResponse, PatternGroups, WorkBreakdown,
};

/// GET /api/insights - Compute and return behavioral insights.
#[utoipa::path(get, path = "/api/insights", tag = "insights",
    params(InsightsQuery),
    responses(
        (status = 200, description = "Behavioral insights with patterns and classification status", body = InsightsResponse),
    )
)]
pub async fn get_insights(
    State(state): State<Arc<AppState>>,
    Query(query): Query<InsightsQuery>,
) -> ApiResult<Json<InsightsResponse>> {
    let now = chrono::Utc::now().timestamp();
    let oldest_timestamp = match state.db.get_oldest_session_date(None, None).await {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(
                endpoint = "insights",
                error = %e,
                "Failed to fetch oldest session date for default all-time range"
            );
            None
        }
    };
    let effective_range = match resolve_from_to_or_all_time(ResolveFromToInput {
        endpoint: "insights",
        from: query.from,
        to: query.to,
        now,
        oldest_timestamp,
    }) {
        Ok(resolved) => {
            record_time_range_resolution("insights", resolved.source);
            tracing::info!(
                endpoint = "insights",
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
            record_time_range_resolution_error("insights", err.reason.as_str());
            tracing::warn!(
                endpoint = "insights",
                reason = err.reason.as_str(),
                requested_from = query.from,
                requested_to = query.to,
                "Rejected request time range"
            );
            return Err(ApiError::BadRequest(err.message));
        }
    };
    let from_ts = effective_range.from;
    let to_ts = effective_range.to;
    let min_impact = query.min_impact.unwrap_or(0.3);
    let limit = query.limit.unwrap_or(50) as usize;

    let time_range_days = ((to_ts - from_ts) / 86400).max(1) as u32;
    let pool = state.db.pool();

    // 1. Fetch lightweight session data for pattern computation
    let rows: Vec<LightSession> = sqlx::query_as(
        r#"
        SELECT
            s.id, s.project_id, s.project_path, s.project_display_name,
            s.file_path, s.last_message_at, s.duration_seconds,
            s.files_edited_count, s.files_read_count, s.reedited_files_count,
            s.user_prompt_count, s.api_call_count, s.tool_call_count,
            s.commit_count, s.turn_count,
            s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
            s.total_input_tokens, s.total_output_tokens,
            s.git_branch, s.files_edited, s.files_read,
            s.category_l1, s.prompt_word_count,
            s.correction_count, s.same_file_edit_count, s.size_bytes,
            s.primary_model
        FROM valid_sessions s
        WHERE s.last_message_at >= ?1 AND s.last_message_at <= ?2
        "#,
    )
    .bind(from_ts)
    .bind(to_ts)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch sessions: {}", e)))?;

    let sessions: Vec<SessionInfo> = rows.into_iter().map(|r| r.into_session_info()).collect();

    // 2. Run pattern engine
    let mut all_insights = calculate_all_patterns(&sessions, time_range_days);

    // 3. Filter by minimum impact
    all_insights.retain(|i| i.impact_score >= min_impact);

    // 4. Filter by categories if specified
    if let Some(ref cats) = query.categories {
        let allowed: Vec<&str> = cats.split(',').map(|s| s.trim()).collect();
        all_insights.retain(|i| {
            allowed
                .iter()
                .any(|c| i.category.to_lowercase().contains(&c.to_lowercase()))
        });
    }

    let patterns_evaluated = all_insights.len() as u32;

    // 5. Sort by impact score descending
    claude_view_core::insights::generator::sort_by_impact(&mut all_insights);

    // 6. Group by tier
    let high: Vec<GeneratedInsight> = all_insights
        .iter()
        .filter(|i| i.impact_tier == "high")
        .take(limit / 3 + 1)
        .cloned()
        .collect();
    let medium: Vec<GeneratedInsight> = all_insights
        .iter()
        .filter(|i| i.impact_tier == "medium")
        .take(limit / 3 + 1)
        .cloned()
        .collect();
    let observations: Vec<GeneratedInsight> = all_insights
        .iter()
        .filter(|i| i.impact_tier == "observation")
        .take(limit / 3 + 1)
        .cloned()
        .collect();

    let top_insight = high.first().cloned().or_else(|| medium.first().cloned());
    let patterns_returned = (high.len() + medium.len() + observations.len()) as u32;

    // 7. Calculate overview stats
    let overview = calculate_overview(&sessions, from_ts, to_ts);

    // 8. Classification status
    let classification_status = get_classification_status(pool).await?;
    let analytics_scope = fetch_analytics_scope_meta_for_range(&state, from_ts, to_ts).await?;

    Ok(Json(InsightsResponse {
        top_insight,
        overview,
        patterns: PatternGroups {
            high,
            medium,
            observations,
        },
        classification_status,
        meta: InsightsMeta {
            computed_at: now,
            time_range_start: from_ts,
            time_range_end: to_ts,
            effective_range,
            patterns_evaluated,
            patterns_returned,
            analytics_scope,
        },
    }))
}

// ============================================================================
// Helper functions
// ============================================================================

/// Calculate overview statistics from session data (pure computation, no I/O).
fn calculate_overview(sessions: &[SessionInfo], from_ts: i64, to_ts: i64) -> InsightsOverview {
    let total_sessions = sessions.len() as u32;
    let with_commits = sessions.iter().filter(|s| s.commit_count > 0).count() as u32;
    let exploration = sessions
        .iter()
        .filter(|s| s.commit_count == 0 && s.files_edited_count == 0)
        .count() as u32;
    // Cap duration at 4 hours and exclude 0-duration sessions for a realistic average
    let valid_durations: Vec<f64> = sessions
        .iter()
        .filter(|s| s.duration_seconds > 0)
        .map(|s| s.duration_seconds.min(14400) as f64)
        .collect();
    let avg_session_minutes = if !valid_durations.is_empty() {
        valid_durations.iter().sum::<f64>() / valid_durations.len() as f64 / 60.0
    } else {
        0.0
    };

    // Efficiency stats
    let editing_sessions: Vec<&SessionInfo> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.duration_seconds > 0)
        .collect();

    let avg_reedit_rate = if !editing_sessions.is_empty() {
        editing_sessions
            .iter()
            .filter_map(|s| s.reedit_rate())
            .sum::<f64>()
            / editing_sessions.len() as f64
    } else {
        0.0
    };

    let avg_edit_velocity = if !editing_sessions.is_empty() {
        editing_sessions
            .iter()
            .filter_map(|s| s.edit_velocity())
            .sum::<f64>()
            / editing_sessions.len() as f64
    } else {
        0.0
    };

    // Trend: compare last 7 days vs earlier
    let week_ago = to_ts - 7 * 86400;
    let recent_rates: Vec<f64> = editing_sessions
        .iter()
        .filter(|s| s.modified_at >= week_ago)
        .filter_map(|s| s.reedit_rate())
        .collect();
    let earlier_rates: Vec<f64> = editing_sessions
        .iter()
        .filter(|s| s.modified_at < week_ago && s.modified_at >= from_ts)
        .filter_map(|s| s.reedit_rate())
        .collect();

    let (trend, trend_pct) = if !recent_rates.is_empty() && !earlier_rates.is_empty() {
        let recent_avg = recent_rates.iter().sum::<f64>() / recent_rates.len() as f64;
        let earlier_avg = earlier_rates.iter().sum::<f64>() / earlier_rates.len() as f64;
        if earlier_avg > 0.0 {
            let change = ((earlier_avg - recent_avg) / earlier_avg) * 100.0;
            if change > 5.0 {
                ("improving".to_string(), change.abs())
            } else if change < -5.0 {
                ("declining".to_string(), change.abs())
            } else {
                ("stable".to_string(), change.abs())
            }
        } else {
            ("stable".to_string(), 0.0)
        }
    } else {
        ("stable".to_string(), 0.0)
    };

    // Best time (by lowest reedit rate based on timestamp)
    let (best_day, best_slot, best_improvement) = compute_best_time(sessions);

    InsightsOverview {
        work_breakdown: WorkBreakdown {
            total_sessions,
            with_commits,
            exploration,
            avg_session_minutes,
        },
        efficiency: EfficiencyStats {
            avg_reedit_rate,
            avg_edit_velocity,
            trend,
            trend_pct,
        },
        best_time: BestTimeStats {
            day_of_week: best_day,
            time_slot: best_slot,
            improvement_pct: best_improvement,
        },
    }
}

/// Compute best time of day/week by reedit rate.
fn compute_best_time(sessions: &[SessionInfo]) -> (String, String, f64) {
    use chrono::{DateTime, Datelike, Timelike};

    let editing: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.modified_at > 0)
        .collect();

    if editing.len() < 10 {
        return (String::new(), String::new(), 0.0);
    }

    // Group by (day_of_week, time_slot)
    let mut groups: HashMap<(u32, &str), Vec<f64>> = HashMap::new();
    for s in &editing {
        if let Some(dt) = DateTime::from_timestamp(s.modified_at, 0) {
            let day = dt.weekday().num_days_from_monday();
            let slot = match dt.hour() {
                6..=11 => "morning",
                12..=17 => "afternoon",
                18..=22 => "evening",
                _ => "night",
            };
            let rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
            groups.entry((day, slot)).or_default().push(rate);
        }
    }

    if groups.is_empty() {
        return (String::new(), String::new(), 0.0);
    }

    let averages: Vec<((u32, &str), f64)> = groups
        .into_iter()
        .filter(|(_, vals)| vals.len() >= 3)
        .map(|(key, vals)| {
            let avg = vals.iter().sum::<f64>() / vals.len() as f64;
            (key, avg)
        })
        .collect();

    if averages.is_empty() {
        return (String::new(), String::new(), 0.0);
    }

    let best = averages.iter().min_by(|a, b| a.1.total_cmp(&b.1));
    let worst = averages.iter().max_by(|a, b| a.1.total_cmp(&b.1));

    if let (Some(best), Some(worst)) = (best, worst) {
        let day_name = match best.0 .0 {
            0 => "Monday",
            1 => "Tuesday",
            2 => "Wednesday",
            3 => "Thursday",
            4 => "Friday",
            5 => "Saturday",
            _ => "Sunday",
        };
        let improvement = if worst.1 > 0.0 {
            ((worst.1 - best.1) / worst.1 * 100.0).max(0.0)
        } else {
            0.0
        };
        (day_name.to_string(), best.0 .1.to_string(), improvement)
    } else {
        (String::new(), String::new(), 0.0)
    }
}

/// Get classification status from the database.
async fn get_classification_status(pool: &sqlx::SqlitePool) -> ApiResult<ClassificationCoverage> {
    let row: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(CASE WHEN category_l1 IS NOT NULL THEN 1 END) as classified
        FROM valid_sessions
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to get classification status: {}", e)))?;

    let total = row.0 as u32;
    let classified = row.1 as u32;
    let pending = total.saturating_sub(classified);
    let pct = if total > 0 {
        (classified as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(ClassificationCoverage {
        classified,
        total,
        pending_classification: pending,
        classification_pct: pct,
    })
}
