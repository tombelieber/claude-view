//! GET /api/insights endpoint: Returns computed patterns and insights.
//!
//! This endpoint queries session data from the database, runs the pattern
//! detection engine, and returns scored, ranked insights grouped by tier.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use vibe_recall_core::insights::generator::GeneratedInsight;
use vibe_recall_core::patterns::calculate_all_patterns;
use vibe_recall_core::types::SessionInfo;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ============================================================================
// Categories response types (Phase 6)
// ============================================================================

/// Top-level category breakdown percentages.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoryBreakdown {
    pub code_work: CategorySummary,
    pub support_work: CategorySummary,
    pub thinking_work: CategorySummary,
    pub uncategorized: CategorySummary,
}

/// Count and percentage for a single L1 category.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    pub count: u32,
    pub percentage: f64,
}

/// Hierarchical category node for treemap.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoryNode {
    /// Hierarchical ID: "code_work", "code_work/feature", "code_work/feature/new-component"
    pub id: String,
    /// Category level: 1, 2, or 3
    pub level: u8,
    /// Display name
    pub name: String,
    /// Number of sessions
    pub count: u32,
    /// Percentage of total sessions
    pub percentage: f64,
    /// Average re-edit rate (files re-edited / files edited)
    pub avg_reedit_rate: f64,
    /// Average session duration in seconds
    pub avg_duration: u32,
    /// Average prompts per session
    pub avg_prompts: f64,
    /// Percentage of sessions with commits
    pub commit_rate: f64,
    /// AI-generated insight/recommendation (nullable)
    pub insight: Option<String>,
    /// Child categories (empty for L3)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CategoryNode>,
}

/// Full categories response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoriesResponse {
    /// High-level breakdown percentages
    pub breakdown: CategoryBreakdown,
    /// Hierarchical category tree
    pub categories: Vec<CategoryNode>,
    /// User's overall averages for comparison
    pub overall_averages: OverallAverages,
}

/// Overall averages across all sessions for comparison.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct OverallAverages {
    pub avg_reedit_rate: f64,
    pub avg_duration: u32,
    pub avg_prompts: f64,
    pub commit_rate: f64,
}

// ============================================================================
// Response types
// ============================================================================

/// Full insights API response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightsResponse {
    /// Hero insight (highest impact).
    pub top_insight: Option<GeneratedInsight>,
    /// Overview statistics.
    pub overview: InsightsOverview,
    /// Patterns grouped by impact tier.
    pub patterns: PatternGroups,
    /// Classification coverage statistics.
    pub classification_status: ClassificationCoverage,
    /// Response metadata.
    pub meta: InsightsMeta,
}

/// Overview statistics for the insights page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightsOverview {
    pub work_breakdown: WorkBreakdown,
    pub efficiency: EfficiencyStats,
    pub best_time: BestTimeStats,
}

/// Work type breakdown.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct WorkBreakdown {
    pub total_sessions: u32,
    pub with_commits: u32,
    pub exploration: u32,
    pub avg_session_minutes: f64,
}

/// Efficiency trend stats.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyStats {
    pub avg_reedit_rate: f64,
    pub avg_edit_velocity: f64,
    pub trend: String,
    pub trend_pct: f64,
}

/// Best time of day/week stats.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BestTimeStats {
    pub day_of_week: String,
    pub time_slot: String,
    pub improvement_pct: f64,
}

/// Patterns grouped by impact tier.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PatternGroups {
    pub high: Vec<GeneratedInsight>,
    pub medium: Vec<GeneratedInsight>,
    pub observations: Vec<GeneratedInsight>,
}

/// Classification coverage status.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassificationCoverage {
    pub classified: u32,
    pub total: u32,
    pub pending_classification: u32,
    pub classification_pct: f64,
}

/// Response metadata.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightsMeta {
    #[ts(type = "number")]
    pub computed_at: i64,
    #[ts(type = "number")]
    pub time_range_start: i64,
    #[ts(type = "number")]
    pub time_range_end: i64,
    pub patterns_evaluated: u32,
    pub patterns_returned: u32,
}

// ============================================================================
// Query parameters
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct InsightsQuery {
    /// Period start (unix timestamp). Defaults to 30 days ago.
    pub from: Option<i64>,
    /// Period end (unix timestamp). Defaults to now.
    pub to: Option<i64>,
    /// Minimum impact score (0.0-1.0). Defaults to 0.3.
    pub min_impact: Option<f64>,
    /// Comma-separated pattern categories to include.
    pub categories: Option<String>,
    /// Max patterns to return. Defaults to 50.
    pub limit: Option<u32>,
}

// ============================================================================
// Lightweight DB row types (used for aggregate queries)
// ============================================================================

/// Lightweight session data for pattern computation (no full JSONL parse).
struct LightSession {
    id: String,
    project_id: String,
    project_path: String,
    #[allow(dead_code)]
    project_display_name: String,
    file_path: String,
    last_message_at: Option<i64>,
    duration_seconds: i32,
    files_edited_count: i32,
    files_read_count: i32,
    reedited_files_count: i32,
    user_prompt_count: i32,
    api_call_count: i32,
    tool_call_count: i32,
    commit_count: i32,
    turn_count: i32,
    tool_counts_edit: i32,
    tool_counts_read: i32,
    tool_counts_bash: i32,
    tool_counts_write: i32,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    primary_model: Option<String>,
    git_branch: Option<String>,
    files_edited: String,
    files_read: String,
    category_l1: Option<String>,
    prompt_word_count: Option<i32>,
    correction_count: i32,
    same_file_edit_count: i32,
    size_bytes: i64,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for LightSession {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            project_path: row.try_get("project_path")?,
            project_display_name: row.try_get("project_display_name")?,
            file_path: row.try_get("file_path")?,
            last_message_at: row.try_get("last_message_at")?,
            duration_seconds: row.try_get("duration_seconds")?,
            files_edited_count: row.try_get("files_edited_count")?,
            files_read_count: row.try_get("files_read_count")?,
            reedited_files_count: row.try_get("reedited_files_count")?,
            user_prompt_count: row.try_get("user_prompt_count")?,
            api_call_count: row.try_get("api_call_count")?,
            tool_call_count: row.try_get("tool_call_count")?,
            commit_count: row.try_get("commit_count")?,
            turn_count: row.try_get("turn_count")?,
            tool_counts_edit: row.try_get("tool_counts_edit")?,
            tool_counts_read: row.try_get("tool_counts_read")?,
            tool_counts_bash: row.try_get("tool_counts_bash")?,
            tool_counts_write: row.try_get("tool_counts_write")?,
            total_input_tokens: row.try_get("total_input_tokens").ok().flatten(),
            total_output_tokens: row.try_get("total_output_tokens").ok().flatten(),
            primary_model: row.try_get("primary_model").ok().flatten(),
            git_branch: row.try_get("git_branch").ok().flatten(),
            files_edited: row.try_get("files_edited")?,
            files_read: row.try_get("files_read")?,
            category_l1: row.try_get("category_l1").ok().flatten(),
            prompt_word_count: row.try_get("prompt_word_count").ok().flatten(),
            correction_count: row.try_get("correction_count").unwrap_or(0),
            same_file_edit_count: row.try_get("same_file_edit_count").unwrap_or(0),
            size_bytes: row.try_get("size_bytes")?,
        })
    }
}

impl LightSession {
    /// Convert to SessionInfo for the pattern engine.
    fn into_session_info(self) -> SessionInfo {
        let files_edited: Vec<String> =
            serde_json::from_str(&self.files_edited).unwrap_or_default();
        let files_read: Vec<String> =
            serde_json::from_str(&self.files_read).unwrap_or_default();

        SessionInfo {
            id: self.id,
            project: self.project_id.clone(),
            project_path: self.project_path,
            file_path: self.file_path,
            modified_at: self.last_message_at.filter(|&ts| ts > 0).unwrap_or(0),
            size_bytes: self.size_bytes as u64,
            preview: String::new(),
            last_message: String::new(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: vibe_recall_core::ToolCounts {
                edit: self.tool_counts_edit as usize,
                read: self.tool_counts_read as usize,
                bash: self.tool_counts_bash as usize,
                write: self.tool_counts_write as usize,
            },
            message_count: 0,
            turn_count: self.turn_count as usize,
            summary: None,
            git_branch: self.git_branch,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: self.total_input_tokens.map(|v| v as u64),
            total_output_tokens: self.total_output_tokens.map(|v| v as u64),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: self.primary_model,
            user_prompt_count: self.user_prompt_count as u32,
            api_call_count: self.api_call_count as u32,
            tool_call_count: self.tool_call_count as u32,
            files_read,
            files_edited,
            files_read_count: self.files_read_count as u32,
            files_edited_count: self.files_edited_count as u32,
            reedited_files_count: self.reedited_files_count as u32,
            duration_seconds: self.duration_seconds as u32,
            commit_count: self.commit_count as u32,
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
            category_l1: self.category_l1,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: self.prompt_word_count.map(|v| v as u32),
            correction_count: self.correction_count as u32,
            same_file_edit_count: self.same_file_edit_count as u32,
        }
    }
}

// ============================================================================
// Handler
// ============================================================================

/// GET /api/insights - Compute and return behavioral insights.
pub async fn get_insights(
    State(state): State<Arc<AppState>>,
    Query(query): Query<InsightsQuery>,
) -> ApiResult<Json<InsightsResponse>> {
    let now = chrono::Utc::now().timestamp();
    let from_ts = query.from.unwrap_or(now - 30 * 86400);
    let to_ts = query.to.unwrap_or(now);
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
            COALESCE(
                (SELECT model_id FROM turns t
                 WHERE t.session_id = s.id
                 GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 1),
                NULL
            ) as primary_model
        FROM sessions s
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
        all_insights.retain(|i| allowed.iter().any(|c| i.category.to_lowercase().contains(&c.to_lowercase())));
    }

    let patterns_evaluated = all_insights.len() as u32;

    // 5. Sort by impact score descending
    vibe_recall_core::insights::generator::sort_by_impact(&mut all_insights);

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
            patterns_evaluated,
            patterns_returned,
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

    let best = averages
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let worst = averages
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

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
async fn get_classification_status(
    pool: &sqlx::SqlitePool,
) -> ApiResult<ClassificationCoverage> {
    let row: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(CASE WHEN category_l1 IS NOT NULL THEN 1 END) as classified
        FROM sessions
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

// ============================================================================
// Categories handler (Phase 6)
// ============================================================================

/// Query parameters for GET /api/insights/categories.
#[derive(Debug, Deserialize)]
pub struct CategoriesQuery {
    /// Period start (unix timestamp).
    pub from: Option<i64>,
    /// Period end (unix timestamp).
    pub to: Option<i64>,
}

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
pub async fn get_categories(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CategoriesQuery>,
) -> ApiResult<Json<CategoriesResponse>> {
    // Validate time range
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if from > to {
            return Err(ApiError::BadRequest("'from' must be <= 'to'".to_string()));
        }
    }

    let pool = state.db.pool();

    // Get raw category counts grouped by L1/L2/L3
    let counts = fetch_category_counts(pool, query.from, query.to).await?;
    let uncategorized = fetch_uncategorized_count(pool, query.from, query.to).await?;
    let overall = fetch_overall_averages(pool, query.from, query.to).await?;

    // Calculate total
    let categorized_total: u32 = counts.iter().map(|c| c.count).sum();
    let total = categorized_total + uncategorized;

    // Build hierarchical tree
    let categories = build_category_tree(&counts, total);

    // Calculate L1 breakdown
    let breakdown = calculate_breakdown(&counts, uncategorized, total);

    Ok(Json(CategoriesResponse {
        breakdown,
        categories,
        overall_averages: overall,
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
            FROM sessions
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
        .map(|(l1, l2, l3, count, reedit, dur, prompts, commit)| CategoryCountRow {
            category_l1: l1,
            category_l2: l2,
            category_l3: l3,
            count: count as u32,
            avg_reedit_rate: reedit.unwrap_or(0.0),
            avg_duration: dur.unwrap_or(0.0) as u32,
            avg_prompts: prompts.unwrap_or(0.0),
            commit_rate: commit.unwrap_or(0.0),
        })
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
        SELECT COUNT(*) FROM sessions
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
        FROM sessions
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
        let (avg_reedit, avg_dur, avg_prompts, commit_rate) =
            aggregate_category_metrics(l1_counts);

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

    (weighted_reedit, weighted_dur as u32, weighted_prompts, weighted_commit)
}

// ============================================================================
// Trends handler (Phase 7)
// ============================================================================

/// Query parameters for GET /api/insights/trends.
#[derive(Debug, Deserialize)]
pub struct TrendsQuery {
    #[serde(default = "default_metric")]
    pub metric: String,
    #[serde(default = "default_range")]
    pub range: String,
    #[serde(default = "default_granularity")]
    pub granularity: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

fn default_metric() -> String {
    "reedit_rate".to_string()
}
fn default_range() -> String {
    "6mo".to_string()
}
fn default_granularity() -> String {
    "week".to_string()
}

const VALID_METRICS: &[&str] = &["reedit_rate", "sessions", "lines", "cost_per_line", "prompts"];
const VALID_TREND_RANGES: &[&str] = &["3mo", "6mo", "1yr", "all"];
const VALID_GRANULARITIES: &[&str] = &["day", "week", "month"];

/// GET /api/insights/trends - Get time-series trend data for charts.
pub async fn get_insights_trends(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrendsQuery>,
) -> ApiResult<Json<vibe_recall_db::insights_trends::InsightsTrendsResponse>> {
    use vibe_recall_db::insights_trends::*;

    // Validate inputs
    if !VALID_METRICS.contains(&query.metric.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid metric. Must be one of: {}",
            VALID_METRICS.join(", ")
        )));
    }
    if !VALID_TREND_RANGES.contains(&query.range.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid range. Must be one of: {}",
            VALID_TREND_RANGES.join(", ")
        )));
    }
    if !VALID_GRANULARITIES.contains(&query.granularity.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid granularity. Must be one of: {}",
            VALID_GRANULARITIES.join(", ")
        )));
    }

    // Calculate time bounds
    let now = chrono::Utc::now().timestamp();
    let (from, to) = match (query.from, query.to) {
        (Some(f), Some(t)) if f > t => {
            return Err(ApiError::BadRequest(
                "'from' timestamp must be less than 'to'".to_string(),
            ));
        }
        (Some(f), Some(t)) => (f, t),
        _ => {
            let seconds = match query.range.as_str() {
                "3mo" => 90 * 86400_i64,
                "6mo" => 180 * 86400_i64,
                "1yr" => 365 * 86400_i64,
                "all" => 365 * 10 * 86400_i64,
                _ => 180 * 86400_i64,
            };
            (now - seconds, now)
        }
    };

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
    let insight = generate_metric_insight(&query.metric, trend, &query.range);
    let category_insight = category_evolution
        .as_ref()
        .map(|data| generate_category_insight(data));
    let heatmap_insight = generate_heatmap_insight(&activity_heatmap);

    let classification_required = category_evolution.is_none();

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
    }))
}

// ============================================================================
// Benchmarks endpoint (Phase 8)
// ============================================================================

/// Query parameters for the benchmarks endpoint.
#[derive(Debug, Deserialize)]
pub struct BenchmarksQuery {
    /// Time range: all, 30d, 90d, 1y. Defaults to all.
    pub range: Option<String>,
}

/// GET /api/insights/benchmarks - Compute personal progress benchmarks.
pub async fn get_benchmarks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BenchmarksQuery>,
) -> ApiResult<Json<vibe_recall_core::BenchmarksResponse>> {
    use vibe_recall_core::{
        BenchmarksResponse, CategoryPerformance, CategoryVerdict, ImprovementMetrics,
        LearningCurvePoint, PeriodMetrics, ProgressComparison, ReportSummary, SkillAdoption,
    };

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
                FROM sessions
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
        "SELECT MIN(last_message_at) FROM sessions WHERE last_message_at > 0 AND last_message_at >= ?1",
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
        FROM sessions
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
        FROM sessions
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
        FROM sessions, json_each(sessions.skills_used) AS j
        WHERE sessions.last_message_at >= ?1
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
            FROM sessions s, json_each(s.skills_used) AS j
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
            FROM sessions
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
            FROM sessions s, json_each(s.skills_used) AS j
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
            FROM sessions s, json_each(s.skills_used) AS j
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

    skill_adoption.sort_by(|a, b| {
        a.impact_on_reedit
            .partial_cmp(&b.impact_on_reedit)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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

    let report_row: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COALESCE(SUM(commit_count), 0),
               COALESCE(SUM(total_input_tokens + total_output_tokens), 0)
        FROM sessions WHERE last_message_at >= ?1
        "#,
    )
    .bind(current_month_start)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to compute report summary: {}", e)))?;

    let report_session_count = report_row.0 as u32;
    let report_commit_count = report_row.1 as u32;
    let total_tokens = report_row.2;
    let estimated_cost = total_tokens as f64 / 1000.0 * 0.005;

    let month_name = format!(
        "{} {}",
        match now_dt.month() {
            1 => "January", 2 => "February", 3 => "March", 4 => "April",
            5 => "May", 6 => "June", 7 => "July", 8 => "August",
            9 => "September", 10 => "October", 11 => "November", 12 => "December",
            _ => "Unknown",
        },
        now_dt.year()
    );

    let top_wins = bench_top_wins(&progress, &by_category, &skill_adoption);
    let focus_areas = bench_focus_areas(&by_category);

    Ok(Json(BenchmarksResponse {
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
            estimated_cost,
            top_wins,
            focus_areas,
        },
    }))
}

// ============================================================================
// Benchmark helper functions
// ============================================================================

fn bench_progress_insight(
    first: &Option<vibe_recall_core::PeriodMetrics>,
    last: &vibe_recall_core::PeriodMetrics,
    improvement: &Option<vibe_recall_core::ImprovementMetrics>,
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
        parts.push(format!("You've cut re-edits by {:.0}%", imp.reedit_rate.abs()));
    } else if imp.reedit_rate < -10.0 {
        parts.push(format!("Re-edit rate improved by {:.0}%", imp.reedit_rate.abs()));
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
        format!("{} -- your prompts are significantly more effective than when you started", parts.join(" and "))
    }
}

fn bench_category_insight(category: &str, verdict: vibe_recall_core::CategoryVerdict) -> String {
    use vibe_recall_core::CategoryVerdict as CV;
    let display = category.replace('_', " ");
    match verdict {
        CV::Excellent => format!("{} is your strongest area -- excellent re-edit rate", display),
        CV::Good => format!("{} performs well -- above your average", display),
        CV::Average => format!("{} is at your average -- room for improvement", display),
        CV::NeedsWork => format!("{} has a high re-edit rate -- try being more specific about desired patterns and constraints", display),
    }
}

fn bench_top_wins(
    progress: &vibe_recall_core::ProgressComparison,
    categories: &[vibe_recall_core::CategoryPerformance],
    skills: &[vibe_recall_core::SkillAdoption],
) -> Vec<String> {
    use vibe_recall_core::CategoryVerdict as CV;
    let mut wins = Vec::new();

    if let Some(ref imp) = progress.improvement {
        if imp.reedit_rate < -20.0 { wins.push(format!("Re-edit rate improved by {:.0}%", imp.reedit_rate.abs())); }
        if imp.commit_rate > 20.0 { wins.push(format!("Commit rate up {:.0}%", imp.commit_rate)); }
        if imp.prompts_per_task < -15.0 { wins.push(format!("Prompts per task reduced by {:.0}%", imp.prompts_per_task.abs())); }
    }

    let excellent_count = categories.iter().filter(|c| c.verdict == CV::Excellent).count();
    if excellent_count > 0 {
        wins.push(format!("{} categor{} rated excellent", excellent_count, if excellent_count == 1 { "y" } else { "ies" }));
    }

    if let Some(best) = skills.first() {
        if best.impact_on_reedit < -20.0 {
            wins.push(format!("{} skill reduced re-edits by {:.0}%", best.skill, best.impact_on_reedit.abs()));
        }
    }

    wins.truncate(3);
    if wins.is_empty() { wins.push("Keep using Claude Code to build your track record".to_string()); }
    wins
}

fn bench_focus_areas(categories: &[vibe_recall_core::CategoryPerformance]) -> Vec<String> {
    use vibe_recall_core::CategoryVerdict as CV;
    let mut areas: Vec<String> = categories
        .iter()
        .filter(|c| c.verdict == CV::NeedsWork)
        .map(|c| format!("{} sessions have high re-edit rate ({:.2})", c.category.replace('_', " "), c.reedit_rate))
        .collect();
    areas.truncate(3);
    if areas.is_empty() { areas.push("All categories performing well -- maintain your current approach".to_string()); }
    areas
}

// ============================================================================
// Router
// ============================================================================

/// Create the insights routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/insights", get(get_insights))
        .route("/insights/categories", get(get_categories))
        .route("/insights/trends", get(get_insights_trends))
        .route("/insights/benchmarks", get(get_benchmarks))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_db::Database;

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

    #[tokio::test]
    async fn test_insights_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have all expected top-level fields
        assert!(json.get("topInsight").is_some());
        assert!(json.get("overview").is_some());
        assert!(json.get("patterns").is_some());
        assert!(json.get("classificationStatus").is_some());
        assert!(json.get("meta").is_some());

        // top_insight should be null with no data
        assert!(json["topInsight"].is_null());

        // overview should have zero values
        assert_eq!(json["overview"]["workBreakdown"]["totalSessions"], 0);

        // patterns should be empty arrays
        assert_eq!(json["patterns"]["high"].as_array().unwrap().len(), 0);
        assert_eq!(json["patterns"]["medium"].as_array().unwrap().len(), 0);
        assert_eq!(json["patterns"]["observations"].as_array().unwrap().len(), 0);

        // classification status
        assert_eq!(json["classificationStatus"]["total"], 0);
        assert_eq!(json["classificationStatus"]["classified"], 0);

        // meta
        assert!(json["meta"]["computedAt"].is_number());
        assert!(json["meta"]["patternsEvaluated"].is_number());
    }

    #[tokio::test]
    async fn test_insights_with_custom_params() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(
            app,
            "/api/insights?from=1700000000&to=1700100000&min_impact=0.5&limit=10",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["timeRangeStart"], 1700000000);
        assert_eq!(json["meta"]["timeRangeEnd"], 1700100000);
    }

    #[tokio::test]
    async fn test_insights_with_seeded_data() {
        let db = test_db().await;

        // Insert test sessions
        let now = chrono::Utc::now().timestamp();
        for i in 0..100 {
            let id = format!("test-{}", i);
            let duration = match i % 4 {
                0 => 600,
                1 => 1800,
                2 => 3600,
                _ => 7200,
            };
            let files_edited = if duration == 1800 { 10 } else { 3 };
            let reedited = if duration == 1800 { 1 } else { 2 };

            sqlx::query(
                r#"
                INSERT INTO sessions (
                    id, project_id, file_path, preview, project_path,
                    duration_seconds, files_edited_count, reedited_files_count,
                    files_read_count, user_prompt_count, api_call_count,
                    tool_call_count, commit_count, turn_count,
                    last_message_at, size_bytes, last_message,
                    files_touched, skills_used, files_read, files_edited
                )
                VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                    ?2, ?3, ?4, 5, 5, 10, 20, ?5, 10,
                    ?6, 1024, '', '[]', '[]', '[]', '[]')
                "#,
            )
            .bind(&id)
            .bind(duration)
            .bind(files_edited)
            .bind(reedited)
            .bind(if i % 3 == 0 { 1 } else { 0 })
            .bind(now - (i as i64 * 3600))
            .execute(db.pool())
            .await
            .unwrap();
        }

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have some sessions in overview
        assert!(
            json["overview"]["workBreakdown"]["totalSessions"].as_u64().unwrap() > 0,
            "Should have sessions: {}",
            body
        );

        // Meta should report patterns evaluated
        assert!(json["meta"]["patternsEvaluated"].is_number());
    }

    #[tokio::test]
    async fn test_insights_response_structure() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Verify the full response structure matches the API spec
        assert!(json["overview"]["workBreakdown"]["totalSessions"].is_number());
        assert!(json["overview"]["workBreakdown"]["withCommits"].is_number());
        assert!(json["overview"]["workBreakdown"]["exploration"].is_number());
        assert!(json["overview"]["workBreakdown"]["avgSessionMinutes"].is_number());

        assert!(json["overview"]["efficiency"]["avgReeditRate"].is_number());
        assert!(json["overview"]["efficiency"]["avgEditVelocity"].is_number());
        assert!(json["overview"]["efficiency"]["trend"].is_string());
        assert!(json["overview"]["efficiency"]["trendPct"].is_number());

        assert!(json["overview"]["bestTime"]["dayOfWeek"].is_string());
        assert!(json["overview"]["bestTime"]["timeSlot"].is_string());
        assert!(json["overview"]["bestTime"]["improvementPct"].is_number());

        assert!(json["classificationStatus"]["classified"].is_number());
        assert!(json["classificationStatus"]["total"].is_number());
        assert!(json["classificationStatus"]["pendingClassification"].is_number());
        assert!(json["classificationStatus"]["classificationPct"].is_number());
    }

    // ========================================================================
    // GET /api/insights/categories tests (Phase 6)
    // ========================================================================

    #[tokio::test]
    async fn test_categories_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have all top-level fields
        assert!(json.get("breakdown").is_some());
        assert!(json.get("categories").is_some());
        assert!(json.get("overallAverages").is_some());

        // Breakdown should be zero
        assert_eq!(json["breakdown"]["codeWork"]["count"], 0);
        assert_eq!(json["breakdown"]["supportWork"]["count"], 0);
        assert_eq!(json["breakdown"]["thinkingWork"]["count"], 0);
        assert_eq!(json["breakdown"]["uncategorized"]["count"], 0);

        // Categories should be empty
        assert!(json["categories"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_categories_with_data() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        // Insert sessions with categories
        for i in 0..20 {
            let id = format!("cat-{}", i);
            let (l1, l2, l3) = match i % 5 {
                0 => ("code_work", "feature", "new-component"),
                1 => ("code_work", "feature", "add-functionality"),
                2 => ("code_work", "bug_fix", "error-fix"),
                3 => ("support_work", "docs", "readme-guides"),
                _ => ("thinking_work", "planning", "brainstorming"),
            };

            sqlx::query(
                r#"
                INSERT INTO sessions (
                    id, project_id, file_path, preview, project_path,
                    duration_seconds, files_edited_count, reedited_files_count,
                    files_read_count, user_prompt_count, api_call_count,
                    tool_call_count, commit_count, turn_count,
                    last_message_at, size_bytes, last_message,
                    files_touched, skills_used, files_read, files_edited,
                    category_l1, category_l2, category_l3
                )
                VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                    1800, 5, 1, 5, 10, 10, 20, ?2, 10,
                    ?3, 1024, '', '[]', '[]', '[]', '[]',
                    ?4, ?5, ?6)
                "#,
            )
            .bind(&id)
            .bind(if i % 2 == 0 { 1 } else { 0 })
            .bind(now - (i as i64 * 3600))
            .bind(l1)
            .bind(l2)
            .bind(l3)
            .execute(db.pool())
            .await
            .unwrap();
        }

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have categories
        let categories = json["categories"].as_array().unwrap();
        assert!(!categories.is_empty(), "Should have category nodes");

        // Code work should have the most sessions (12 out of 20)
        assert_eq!(json["breakdown"]["codeWork"]["count"], 12);
        assert_eq!(json["breakdown"]["supportWork"]["count"], 4);
        assert_eq!(json["breakdown"]["thinkingWork"]["count"], 4);
        assert_eq!(json["breakdown"]["uncategorized"]["count"], 0);

        // Overall averages should be present
        assert!(json["overallAverages"]["avgReeditRate"].is_number());
        assert!(json["overallAverages"]["avgDuration"].is_number());
        assert!(json["overallAverages"]["avgPrompts"].is_number());
        assert!(json["overallAverages"]["commitRate"].is_number());
    }

    #[tokio::test]
    async fn test_categories_time_filter() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        // Insert sessions: some recent, some old
        for i in 0..10 {
            let id = format!("tf-{}", i);
            let ts = if i < 5 {
                now - 3600 // 1 hour ago (recent)
            } else {
                now - 30 * 86400 // 30 days ago (old)
            };

            sqlx::query(
                r#"
                INSERT INTO sessions (
                    id, project_id, file_path, preview, project_path,
                    duration_seconds, files_edited_count, reedited_files_count,
                    files_read_count, user_prompt_count, api_call_count,
                    tool_call_count, commit_count, turn_count,
                    last_message_at, size_bytes, last_message,
                    files_touched, skills_used, files_read, files_edited,
                    category_l1, category_l2, category_l3
                )
                VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                    1800, 5, 1, 5, 10, 10, 20, 1, 10,
                    ?2, 1024, '', '[]', '[]', '[]', '[]',
                    'code_work', 'feature', 'new-component')
                "#,
            )
            .bind(&id)
            .bind(ts)
            .execute(db.pool())
            .await
            .unwrap();
        }

        let app = build_app(db);
        // Filter to last 7 days
        let from = now - 7 * 86400;
        let (status, body) = do_get(
            app,
            &format!("/api/insights/categories?from={}&to={}", from, now),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        // Only recent sessions should be counted
        assert_eq!(json["breakdown"]["codeWork"]["count"], 5);
    }

    #[tokio::test]
    async fn test_categories_invalid_range() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(
            app,
            "/api/insights/categories?from=1700100000&to=1700000000",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"]
            .as_str()
            .unwrap()
            .contains("'from' must be <= 'to'"));
    }

    #[tokio::test]
    async fn test_categories_response_structure() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Verify full response structure
        assert!(json["breakdown"]["codeWork"]["count"].is_number());
        assert!(json["breakdown"]["codeWork"]["percentage"].is_number());
        assert!(json["breakdown"]["supportWork"]["count"].is_number());
        assert!(json["breakdown"]["thinkingWork"]["count"].is_number());
        assert!(json["breakdown"]["uncategorized"]["count"].is_number());

        assert!(json["overallAverages"]["avgReeditRate"].is_number());
        assert!(json["overallAverages"]["avgDuration"].is_number());
        assert!(json["overallAverages"]["avgPrompts"].is_number());
        assert!(json["overallAverages"]["commitRate"].is_number());
    }

    // ========================================================================
    // GET /api/insights/trends tests (Phase 7)
    // ========================================================================

    #[tokio::test]
    async fn test_trends_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/trends").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(json["metric"], "reedit_rate");
        assert!(json["dataPoints"].is_array());
        assert!(json["activityHeatmap"].is_array());
        assert!(json["average"].is_number());
        assert!(json["trend"].is_number());
        assert!(json["trendDirection"].is_string());
        assert!(json["insight"].is_string());
        assert!(json["heatmapInsight"].is_string());
        assert!(json["periodStart"].is_string());
        assert!(json["periodEnd"].is_string());
        assert!(json["totalSessions"].is_number());
        assert_eq!(json["classificationRequired"], true);
        assert!(json["categoryEvolution"].is_null());
    }

    #[tokio::test]
    async fn test_trends_invalid_metric() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) = do_get(app, "/api/insights/trends?metric=invalid").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_invalid_range() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) = do_get(app, "/api/insights/trends?range=2yr").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_invalid_granularity() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) =
            do_get(app, "/api/insights/trends?granularity=quarter").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_from_greater_than_to() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) =
            do_get(app, "/api/insights/trends?from=1700100000&to=1700000000").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_custom_range() {
        let db = test_db().await;
        let app = build_app(db);
        let now = chrono::Utc::now().timestamp();
        let from = now - 86400 * 30;

        let (status, body) = do_get(
            app,
            &format!("/api/insights/trends?from={}&to={}", from, now),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["periodStart"].is_string());
        assert!(json["periodEnd"].is_string());
    }

    #[tokio::test]
    async fn test_trends_sessions_metric() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) =
            do_get(app, "/api/insights/trends?metric=sessions").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["metric"], "sessions");
    }

    #[tokio::test]
    async fn test_trends_day_granularity() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(
            app,
            "/api/insights/trends?granularity=day&range=3mo",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["dataPoints"].is_array());
    }
}
