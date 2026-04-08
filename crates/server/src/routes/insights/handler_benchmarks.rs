//! GET /api/insights/benchmarks handler and benchmark helper functions.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;

use claude_view_core::{
    BenchmarksResponse, CategoryPerformance, CategoryVerdict, ImprovementMetrics,
    LearningCurvePoint, PeriodMetrics, ProgressComparison, ReportSummary, SkillAdoption,
};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::db::fetch_analytics_scope_meta_for_range;
use super::types::{BenchmarksQuery, BenchmarksResponseWithMeta};

/// GET /api/insights/benchmarks - Compute personal progress benchmarks.
#[utoipa::path(get, path = "/api/insights/benchmarks", tag = "insights",
    params(BenchmarksQuery),
    responses(
        (status = 200, description = "Personal progress benchmarks vs past periods", body = serde_json::Value),
    )
)]
pub async fn get_benchmarks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BenchmarksQuery>,
) -> ApiResult<Json<BenchmarksResponseWithMeta>> {
    let range = query.range.as_deref().unwrap_or("all");
    let now = chrono::Utc::now().timestamp();
    let thirty_days: i64 = 30 * 86400;

    // Validate range
    let data_start = match range {
        "all" => 0i64,
        "1y" => now - 365 * 86400,
        "90d" => now - 90 * 86400,
        "30d" => now - 30 * 86400,
        _ => {
            return Err(ApiError::BadRequest(format!(
                "Invalid range parameter: {}. Must be one of: all, 30d, 90d, 1y",
                range
            )));
        }
    };

    let pool = state.db.pool();

    // ----------------------------------------------------------------
    // 1. Compute period metrics for "first month" and "last month"
    // ----------------------------------------------------------------
    let compute_period = |start: i64, end: i64| {
        let pool = pool.clone();
        async move {
            let row: (i64, f64, f64, f64, f64) = sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as cnt,
                    COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0),
                    COALESCE(AVG(CAST(tool_counts_edit AS REAL) / NULLIF(files_edited_count, 0)), 0.0),
                    COALESCE(AVG(CAST(user_prompt_count AS REAL)), 0.0),
                    COALESCE(AVG(CASE WHEN commit_count > 0 THEN 1.0 ELSE 0.0 END), 0.0)
                FROM valid_sessions
                WHERE last_message_at >= ?1 AND last_message_at < ?2
                "#,
            )
            .bind(start)
            .bind(end)
            .fetch_one(&pool)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to compute period metrics: {}", e)))?;

            if row.0 == 0 {
                return Ok::<_, ApiError>(None);
            }

            Ok(Some(PeriodMetrics {
                reedit_rate: row.1,
                edits_per_file: row.2,
                prompts_per_task: row.3,
                commit_rate: row.4,
            }))
        }
    };

    // Find the earliest session in the data window
    let earliest_row: (Option<i64>,) = sqlx::query_as(
        "SELECT MIN(last_message_at) FROM valid_sessions WHERE last_message_at >= ?1",
    )
    .bind(data_start)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to find earliest session: {}", e)))?;

    let earliest = earliest_row.0.unwrap_or(now);

    // First month: first 30 days from the earliest session in the data window
    let first_month = if range != "30d" {
        compute_period(earliest, earliest + thirty_days).await?
    } else {
        None
    };

    // Last month: most recent 30 days
    let last_month = compute_period(now - thirty_days, now)
        .await?
        .unwrap_or(PeriodMetrics {
            reedit_rate: 0.0,
            edits_per_file: 0.0,
            prompts_per_task: 0.0,
            commit_rate: 0.0,
        });

    // Calculate improvement
    let improvement = first_month.as_ref().map(|first| {
        let pct_change = |old: f64, new: f64| -> f64 {
            if old.abs() < 1e-10 {
                0.0
            } else {
                ((new - old) / old * 100.0).round()
            }
        };
        ImprovementMetrics {
            reedit_rate: pct_change(first.reedit_rate, last_month.reedit_rate),
            edits_per_file: pct_change(first.edits_per_file, last_month.edits_per_file),
            prompts_per_task: pct_change(first.prompts_per_task, last_month.prompts_per_task),
            commit_rate: pct_change(first.commit_rate, last_month.commit_rate),
        }
    });

    let insight = bench_progress_insight(&first_month, &last_month, &improvement);

    let progress = ProgressComparison {
        first_month,
        last_month,
        improvement,
        insight,
    };

    // ----------------------------------------------------------------
    // 2. Category performance breakdown
    // ----------------------------------------------------------------
    let overall_reedit: (f64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0)
        FROM valid_sessions
        WHERE last_message_at >= ?1 AND last_message_at <= ?2
        "#,
    )
    .bind(data_start)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to compute overall reedit rate: {}", e)))?;

    let user_average_reedit_rate = overall_reedit.0;

    let cat_rows: Vec<(String, f64)> = sqlx::query_as(
        r#"
        SELECT
            category_l1,
            COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0)
        FROM valid_sessions
        WHERE last_message_at >= ?1 AND last_message_at <= ?2
          AND category_l1 IS NOT NULL
        GROUP BY category_l1
        ORDER BY 2 ASC
        "#,
    )
    .bind(data_start)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to compute category metrics: {}", e)))?;

    let by_category: Vec<CategoryPerformance> = cat_rows
        .into_iter()
        .map(|(category, reedit_rate)| {
            let vs_average = reedit_rate - user_average_reedit_rate;
            let verdict = match vs_average {
                v if v <= -0.20 => CategoryVerdict::Excellent,
                v if v <= -0.05 => CategoryVerdict::Good,
                v if v <= 0.10 => CategoryVerdict::Average,
                _ => CategoryVerdict::NeedsWork,
            };
            let insight = bench_category_insight(&category, verdict);
            CategoryPerformance {
                category,
                reedit_rate,
                vs_average,
                verdict,
                insight,
            }
        })
        .collect();

    // ----------------------------------------------------------------
    // 3. Skill adoption impact
    // ----------------------------------------------------------------
    let skill_rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT j.value as skill
        FROM valid_sessions, json_each(valid_sessions.skills_used) AS j
        WHERE valid_sessions.last_message_at >= ?1
          AND j.value != ''
        "#,
    )
    .bind(data_start)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch skills: {}", e)))?;

    let mut skill_adoption: Vec<SkillAdoption> = Vec::new();

    for (skill_name,) in &skill_rows {
        let adoption_info: (Option<i64>, i64) = sqlx::query_as(
            r#"
            SELECT
                MIN(s.last_message_at),
                COUNT(*)
            FROM valid_sessions s, json_each(s.skills_used) AS j
            WHERE j.value = ?1
              AND s.last_message_at >= ?2
            "#,
        )
        .bind(skill_name)
        .bind(data_start)
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch skill info: {}", e)))?;

        let adopted_at = match adoption_info.0.filter(|&ts| ts > 0) {
            Some(ts) => ts,
            None => continue, // skip skills with no valid adoption timestamp
        };
        let session_count = adoption_info.1 as u32;

        if session_count < 3 {
            continue;
        }

        let before_rate: (f64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0)
            FROM valid_sessions
            WHERE last_message_at < ?1 AND last_message_at >= ?2
            "#,
        )
        .bind(adopted_at)
        .bind(data_start)
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to compute before rate: {}", e)))?;

        let after_rate: (f64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(AVG(CAST(s.reedited_files_count AS REAL) / NULLIF(s.files_edited_count, 0)), 0.0)
            FROM valid_sessions s, json_each(s.skills_used) AS j
            WHERE j.value = ?1 AND s.last_message_at >= ?2
            "#,
        )
        .bind(skill_name)
        .bind(data_start)
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to compute after rate: {}", e)))?;

        let impact_on_reedit = if before_rate.0.abs() > 1e-10 {
            ((after_rate.0 - before_rate.0) / before_rate.0 * 100.0).round()
        } else {
            0.0
        };

        let curve_rows: Vec<(f64,)> = sqlx::query_as(
            r#"
            SELECT COALESCE(CAST(s.reedited_files_count AS REAL) / NULLIF(s.files_edited_count, 0), 0.0)
            FROM valid_sessions s, json_each(s.skills_used) AS j
            WHERE j.value = ?1 AND s.last_message_at >= ?2
            ORDER BY s.last_message_at ASC
            LIMIT 10
            "#,
        )
        .bind(skill_name)
        .bind(data_start)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to build learning curve: {}", e)))?;

        let learning_curve: Vec<LearningCurvePoint> = curve_rows
            .into_iter()
            .enumerate()
            .map(|(i, (rate,))| LearningCurvePoint {
                session: (i + 1) as u32,
                reedit_rate: rate,
            })
            .collect();

        let adopted_at_str = chrono::DateTime::from_timestamp(adopted_at, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();

        skill_adoption.push(SkillAdoption {
            skill: skill_name.clone(),
            adopted_at: adopted_at_str,
            session_count,
            impact_on_reedit,
            learning_curve,
        });
    }

    skill_adoption.sort_by(|a, b| a.impact_on_reedit.total_cmp(&b.impact_on_reedit));
    skill_adoption.truncate(10);

    // ----------------------------------------------------------------
    // 4. Report summary (current month)
    // ----------------------------------------------------------------
    use chrono::Datelike;

    let now_dt = chrono::DateTime::from_timestamp(now, 0).unwrap_or_default();
    let current_month_start = chrono::NaiveDate::from_ymd_opt(now_dt.year(), now_dt.month(), 1)
        .unwrap_or_default()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .timestamp();

    let report_row: (i64, i64, i64, Option<f64>, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*),
               (SELECT COUNT(DISTINCT sc.commit_hash) FROM session_commits sc
                INNER JOIN valid_sessions s2 ON sc.session_id = s2.id
                WHERE s2.last_message_at >= ?1),
               COALESCE(SUM(total_input_tokens + total_output_tokens), 0),
               SUM(total_cost_usd),
               SUM(CASE WHEN total_cost_usd IS NULL THEN 1 ELSE 0 END)
        FROM valid_sessions WHERE last_message_at >= ?1
        "#,
    )
    .bind(current_month_start)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to compute report summary: {}", e)))?;

    let report_session_count = report_row.0 as u32;
    let report_commit_count = report_row.1 as u32;
    let _total_tokens = report_row.2;
    let total_cost_usd = report_row.3;
    let has_unpriced_usage = report_row.4 > 0;

    let month_name = format!(
        "{} {}",
        match now_dt.month() {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        },
        now_dt.year()
    );

    let top_wins = bench_top_wins(&progress, &by_category, &skill_adoption);
    let focus_areas = bench_focus_areas(&by_category);

    let base = BenchmarksResponse {
        progress,
        by_category,
        user_average_reedit_rate,
        skill_adoption,
        report_summary: ReportSummary {
            month: month_name,
            session_count: report_session_count,
            lines_added: 0,
            lines_removed: 0,
            commit_count: report_commit_count,
            total_cost_usd,
            has_unpriced_usage,
            top_wins,
            focus_areas,
        },
    };
    let meta = fetch_analytics_scope_meta_for_range(&state, data_start, now).await?;

    Ok(Json(BenchmarksResponseWithMeta { base, meta }))
}

// ============================================================================
// Benchmark helper functions
// ============================================================================

fn bench_progress_insight(
    first: &Option<PeriodMetrics>,
    last: &PeriodMetrics,
    improvement: &Option<ImprovementMetrics>,
) -> String {
    let Some(_first) = first else {
        if last.reedit_rate < 0.3 {
            return "Your re-edit rate is already good -- keep up the great work!".to_string();
        }
        return "Not enough historical data for comparison yet. Keep using Claude Code to track your progress!".to_string();
    };

    let Some(imp) = improvement else {
        return "Keep going! More data will reveal your improvement trends.".to_string();
    };

    let mut parts = Vec::new();
    if imp.reedit_rate < -30.0 {
        parts.push(format!(
            "You've cut re-edits by {:.0}%",
            imp.reedit_rate.abs()
        ));
    } else if imp.reedit_rate < -10.0 {
        parts.push(format!(
            "Re-edit rate improved by {:.0}%",
            imp.reedit_rate.abs()
        ));
    }
    if imp.commit_rate > 20.0 {
        parts.push(format!("commit rate up {:.0}%", imp.commit_rate));
    }
    if imp.prompts_per_task < -20.0 {
        parts.push("you're getting more done with fewer prompts".to_string());
    }

    if parts.is_empty() {
        "Your metrics are stable -- consistency is a strength!".to_string()
    } else {
        format!(
            "{} -- your prompts are significantly more effective than when you started",
            parts.join(" and ")
        )
    }
}

fn bench_category_insight(category: &str, verdict: CategoryVerdict) -> String {
    use CategoryVerdict as CV;
    let display = category.replace('_', " ");
    match verdict {
        CV::Excellent => format!("{} is your strongest area -- excellent re-edit rate", display),
        CV::Good => format!("{} performs well -- above your average", display),
        CV::Average => format!("{} is at your average -- room for improvement", display),
        CV::NeedsWork => format!("{} has a high re-edit rate -- try being more specific about desired patterns and constraints", display),
    }
}

fn bench_top_wins(
    progress: &ProgressComparison,
    categories: &[CategoryPerformance],
    skills: &[SkillAdoption],
) -> Vec<String> {
    use CategoryVerdict as CV;
    let mut wins = Vec::new();

    if let Some(ref imp) = progress.improvement {
        if imp.reedit_rate < -20.0 {
            wins.push(format!(
                "Re-edit rate improved by {:.0}%",
                imp.reedit_rate.abs()
            ));
        }
        if imp.commit_rate > 20.0 {
            wins.push(format!("Commit rate up {:.0}%", imp.commit_rate));
        }
        if imp.prompts_per_task < -15.0 {
            wins.push(format!(
                "Prompts per task reduced by {:.0}%",
                imp.prompts_per_task.abs()
            ));
        }
    }

    let excellent_count = categories
        .iter()
        .filter(|c| c.verdict == CV::Excellent)
        .count();
    if excellent_count > 0 {
        wins.push(format!(
            "{} categor{} rated excellent",
            excellent_count,
            if excellent_count == 1 { "y" } else { "ies" }
        ));
    }

    if let Some(best) = skills.first() {
        if best.impact_on_reedit < -20.0 {
            wins.push(format!(
                "{} skill reduced re-edits by {:.0}%",
                best.skill,
                best.impact_on_reedit.abs()
            ));
        }
    }

    wins.truncate(3);
    if wins.is_empty() {
        wins.push("Keep using Claude Code to build your track record".to_string());
    }
    wins
}

fn bench_focus_areas(categories: &[CategoryPerformance]) -> Vec<String> {
    use CategoryVerdict as CV;
    let mut areas: Vec<String> = categories
        .iter()
        .filter(|c| c.verdict == CV::NeedsWork)
        .map(|c| {
            format!(
                "{} sessions have high re-edit rate ({:.2})",
                c.category.replace('_', " "),
                c.reedit_rate
            )
        })
        .collect();
    areas.truncate(3);
    if areas.is_empty() {
        areas.push("All categories performing well -- maintain your current approach".to_string());
    }
    areas
}
