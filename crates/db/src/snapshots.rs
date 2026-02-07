// crates/db/src/snapshots.rs
//! Contribution snapshot queries and aggregation.
//!
//! This module provides:
//! - Snapshot CRUD operations
//! - Time range aggregation queries
//! - Daily snapshot generation (for the nightly job)
//!
//! ## Snapshot Table Schema
//!
//! The `contribution_snapshots` table stores pre-aggregated daily metrics:
//! - `date` - YYYY-MM-DD format
//! - `project_id` - NULL for global aggregates
//! - `branch` - NULL for project-wide aggregates
//! - Metrics: sessions_count, ai_lines_added/removed, commits_count, etc.

use crate::{Database, DbResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Blended cost per token in cents.
///
/// Assumes ~50% Sonnet, ~40% Haiku, ~10% Opus usage with 2:1 input:output ratio.
/// Equates to ~$2.50 per million tokens = 0.00025 cents per token.
pub const BLENDED_COST_PER_TOKEN: f64 = 0.00025;

// ============================================================================
// Types
// ============================================================================

/// Time range for contribution queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    /// Today only (real-time query, not from snapshots)
    Today,
    /// Last 7 days (includes today)
    Week,
    /// Last 30 days (includes today)
    Month,
    /// Last 90 days (includes today)
    NinetyDays,
    /// All time
    All,
    /// Custom date range (from, to)
    Custom,
}

impl TimeRange {
    /// Parse from query string parameter.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "today" => Some(TimeRange::Today),
            "week" => Some(TimeRange::Week),
            "month" => Some(TimeRange::Month),
            "90days" => Some(TimeRange::NinetyDays),
            "all" => Some(TimeRange::All),
            "custom" => Some(TimeRange::Custom),
            _ => None,
        }
    }

    /// Get the number of days to look back (None for All or Custom).
    pub fn days_back(&self) -> Option<i64> {
        match self {
            TimeRange::Today => Some(0),
            TimeRange::Week => Some(7),
            TimeRange::Month => Some(30),
            TimeRange::NinetyDays => Some(90),
            TimeRange::All => None,
            TimeRange::Custom => None,
        }
    }

    /// Cache duration in seconds for this time range.
    pub fn cache_seconds(&self) -> u64 {
        match self {
            TimeRange::Today => 60,      // 1 minute for real-time data
            TimeRange::Week => 300,      // 5 minutes
            TimeRange::Month => 900,     // 15 minutes
            TimeRange::NinetyDays => 1800, // 30 minutes
            TimeRange::All => 1800,      // 30 minutes
            TimeRange::Custom => 900,    // 15 minutes
        }
    }
}

/// A single contribution snapshot row.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ContributionSnapshot {
    #[ts(type = "number")]
    pub id: i64,
    pub date: String,
    pub project_id: Option<String>,
    pub branch: Option<String>,
    #[ts(type = "number")]
    pub sessions_count: i64,
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    #[ts(type = "number")]
    pub commit_insertions: i64,
    #[ts(type = "number")]
    pub commit_deletions: i64,
    #[ts(type = "number")]
    pub tokens_used: i64,
    #[ts(type = "number")]
    pub cost_cents: i64,
}

/// Aggregated contribution metrics for a time period.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct AggregatedContributions {
    /// Total sessions in the period
    #[ts(type = "number")]
    pub sessions_count: i64,
    /// Total AI lines added
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    /// Total AI lines removed
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    /// Total commits linked
    #[ts(type = "number")]
    pub commits_count: i64,
    /// Total commit insertions
    #[ts(type = "number")]
    pub commit_insertions: i64,
    /// Total commit deletions
    #[ts(type = "number")]
    pub commit_deletions: i64,
    /// Total tokens used
    #[ts(type = "number")]
    pub tokens_used: i64,
    /// Total estimated cost in cents
    #[ts(type = "number")]
    pub cost_cents: i64,
    /// Total files edited across all sessions
    #[ts(type = "number")]
    pub files_edited_count: i64,
}

/// Daily trend data point for charts.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct DailyTrendPoint {
    pub date: String,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub commits: i64,
    #[ts(type = "number")]
    pub sessions: i64,
    #[ts(type = "number")]
    pub tokens_used: i64,
    #[ts(type = "number")]
    pub cost_cents: i64,
}

/// Model usage breakdown.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ModelBreakdown {
    pub model: String,
    #[ts(type = "number")]
    pub sessions: i64,
    #[ts(type = "number")]
    pub lines: i64,
    #[ts(type = "number")]
    pub tokens: i64,
    #[ts(type = "number")]
    pub cost_cents: i64,
    pub reedit_rate: Option<f64>,
}

/// Branch contribution breakdown.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BranchBreakdown {
    pub branch: String,
    #[ts(type = "number")]
    pub sessions_count: i64,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    pub ai_share: Option<f64>,
    #[ts(type = "number | null")]
    pub last_activity: Option<i64>,
}

/// Session contribution detail for the drill-down view.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionContribution {
    pub session_id: String,
    pub work_type: Option<String>,
    #[ts(type = "number")]
    pub duration_seconds: i64,
    #[ts(type = "number")]
    pub prompt_count: i64,
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    #[ts(type = "number")]
    pub files_edited_count: i64,
    #[ts(type = "number")]
    pub reedited_files_count: i64,
    #[ts(type = "number")]
    pub commit_count: i64,
}

/// Linked commit for session drill-down.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct LinkedCommit {
    pub hash: String,
    pub message: String,
    #[ts(type = "number | null")]
    pub insertions: Option<i64>,
    #[ts(type = "number | null")]
    pub deletions: Option<i64>,
    #[ts(type = "number")]
    pub tier: i64,
}

/// Model statistics for the byModel breakdown.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ModelStats {
    pub model: String,
    #[ts(type = "number")]
    pub lines: i64,
    pub reedit_rate: Option<f64>,
    pub cost_per_line: Option<f64>,
    pub insight: String,
}

/// Learning curve data point.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct LearningCurvePeriod {
    pub period: String,
    pub reedit_rate: f64,
}

/// Learning curve metrics.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct LearningCurve {
    pub periods: Vec<LearningCurvePeriod>,
    pub current_avg: f64,
    pub improvement: f64,
    pub insight: String,
}

/// Skill effectiveness statistics.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SkillStats {
    pub skill: String,
    #[ts(type = "number")]
    pub sessions: i64,
    #[ts(type = "number")]
    pub avg_loc: i64,
    pub commit_rate: f64,
    pub reedit_rate: f64,
}

/// Uncommitted work tracker entry.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct UncommittedWork {
    pub project_id: String,
    pub project_name: String,
    pub branch: Option<String>,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub files_count: i64,
    pub last_session_id: String,
    pub last_session_preview: String,
    #[ts(type = "number")]
    pub last_activity_at: i64,
    pub insight: String,
}

/// File impact for session detail view.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct FileImpact {
    pub path: String,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    pub action: String, // "created", "modified", "deleted"
}

/// Lightweight session summary for branch expansion.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BranchSession {
    pub session_id: String,
    pub work_type: Option<String>,
    #[ts(type = "number")]
    pub duration_seconds: i64,
    #[ts(type = "number")]
    pub ai_lines_added: i64,
    #[ts(type = "number")]
    pub ai_lines_removed: i64,
    #[ts(type = "number")]
    pub commit_count: i64,
    #[ts(type = "number")]
    pub last_message_at: i64,
}

/// Snapshot retention statistics for monitoring.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStats {
    /// Number of daily snapshots
    pub daily_count: i64,
    /// Number of weekly rollup snapshots
    pub weekly_count: i64,
    /// Oldest daily snapshot date (YYYY-MM-DD)
    pub oldest_daily: Option<String>,
    /// Oldest weekly snapshot date (YYYY-MM-DD, without W: prefix)
    pub oldest_weekly: Option<String>,
}

// ============================================================================
// Database Queries
// ============================================================================

impl Database {
    // ========================================================================
    // Snapshot CRUD
    // ========================================================================

    /// Upsert a contribution snapshot.
    ///
    /// Uses INSERT OR REPLACE because SQLite's UNIQUE constraint doesn't work
    /// well with NULL values in ON CONFLICT.
    pub async fn upsert_snapshot(
        &self,
        date: &str,
        project_id: Option<&str>,
        branch: Option<&str>,
        sessions_count: i64,
        ai_lines_added: i64,
        ai_lines_removed: i64,
        commits_count: i64,
        commit_insertions: i64,
        commit_deletions: i64,
        tokens_used: i64,
        cost_cents: i64,
        files_edited_count: i64,
    ) -> DbResult<()> {
        // First try to find existing row with matching (date, project_id, branch)
        // We need special handling because NULL = NULL is false in SQL
        let existing_id: Option<(i64,)> = match (project_id, branch) {
            (None, None) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch IS NULL"
                )
                .bind(date)
                .fetch_optional(self.pool())
                .await?
            }
            (Some(pid), None) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id = ?2 AND branch IS NULL"
                )
                .bind(date)
                .bind(pid)
                .fetch_optional(self.pool())
                .await?
            }
            (None, Some(br)) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch = ?2"
                )
                .bind(date)
                .bind(br)
                .fetch_optional(self.pool())
                .await?
            }
            (Some(pid), Some(br)) => {
                sqlx::query_as(
                    "SELECT id FROM contribution_snapshots WHERE date = ?1 AND project_id = ?2 AND branch = ?3"
                )
                .bind(date)
                .bind(pid)
                .bind(br)
                .fetch_optional(self.pool())
                .await?
            }
        };

        if let Some((id,)) = existing_id {
            // Update existing row
            sqlx::query(
                r#"
                UPDATE contribution_snapshots SET
                    sessions_count = ?1,
                    ai_lines_added = ?2,
                    ai_lines_removed = ?3,
                    commits_count = ?4,
                    commit_insertions = ?5,
                    commit_deletions = ?6,
                    tokens_used = ?7,
                    cost_cents = ?8,
                    files_edited_count = ?9
                WHERE id = ?10
                "#,
            )
            .bind(sessions_count)
            .bind(ai_lines_added)
            .bind(ai_lines_removed)
            .bind(commits_count)
            .bind(commit_insertions)
            .bind(commit_deletions)
            .bind(tokens_used)
            .bind(cost_cents)
            .bind(files_edited_count)
            .bind(id)
            .execute(self.pool())
            .await?;
        } else {
            // Insert new row
            sqlx::query(
                r#"
                INSERT INTO contribution_snapshots (
                    date, project_id, branch,
                    sessions_count, ai_lines_added, ai_lines_removed,
                    commits_count, commit_insertions, commit_deletions,
                    tokens_used, cost_cents, files_edited_count
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
            )
            .bind(date)
            .bind(project_id)
            .bind(branch)
            .bind(sessions_count)
            .bind(ai_lines_added)
            .bind(ai_lines_removed)
            .bind(commits_count)
            .bind(commit_insertions)
            .bind(commit_deletions)
            .bind(tokens_used)
            .bind(cost_cents)
            .bind(files_edited_count)
            .execute(self.pool())
            .await?;
        }

        Ok(())
    }

    /// Get aggregated contributions for a time range.
    ///
    /// For `Today`, queries sessions directly (real-time).
    /// For other ranges, queries contribution_snapshots.
    pub async fn get_aggregated_contributions(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        match range {
            TimeRange::Today => self.get_today_contributions(project_id).await,
            TimeRange::All => self.get_all_contributions(project_id).await,
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01");
                let to_default = Utc::now().format("%Y-%m-%d").to_string();
                let to = to_date.unwrap_or(&to_default);
                self.get_contributions_in_range(from, to, project_id).await
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Utc::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Utc::now().format("%Y-%m-%d").to_string();
                self.get_contributions_in_range(&from, &to, project_id).await
            }
        }
    }

    /// Get today's contributions from sessions directly (real-time query).
    async fn get_today_contributions(
        &self,
        project_id: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let today_start = format!("{}T00:00:00Z", today);

        let row: (i64, i64, i64, i64, i64, i64) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                    COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(user_prompt_count), 0) as prompts,
                    COALESCE(SUM(files_edited_count), 0) as files_edited_count
                FROM sessions
                WHERE project_id = ?1
                  AND datetime(last_message_at, 'unixepoch') >= ?2
                "#,
            )
            .bind(pid)
            .bind(&today_start)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                    COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(user_prompt_count), 0) as prompts,
                    COALESCE(SUM(files_edited_count), 0) as files_edited_count
                FROM sessions
                WHERE datetime(last_message_at, 'unixepoch') >= ?1
                "#,
            )
            .bind(&today_start)
            .fetch_one(self.pool())
            .await?
        };

        // Get commit counts for today (from session_commits joined with commits)
        let (commits_count, commit_insertions, commit_deletions): (i64, i64, i64) =
            if let Some(pid) = project_id {
                sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN sessions s ON sc.session_id = s.id
                    WHERE s.project_id = ?1
                      AND datetime(s.last_message_at, 'unixepoch') >= ?2
                    "#,
                )
                .bind(pid)
                .bind(&today_start)
                .fetch_one(self.pool())
                .await?
            } else {
                sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(DISTINCT c.hash) as commits_count,
                        COALESCE(SUM(c.insertions), 0) as commit_insertions,
                        COALESCE(SUM(c.deletions), 0) as commit_deletions
                    FROM session_commits sc
                    JOIN commits c ON sc.commit_hash = c.hash
                    JOIN sessions s ON sc.session_id = s.id
                    WHERE datetime(s.last_message_at, 'unixepoch') >= ?1
                    "#,
                )
                .bind(&today_start)
                .fetch_one(self.pool())
                .await?
            };

        // Estimate cost (simplified - uses average pricing)
        let cost_cents = estimate_cost_cents(row.3);

        Ok(AggregatedContributions {
            sessions_count: row.0,
            ai_lines_added: row.1,
            ai_lines_removed: row.2,
            commits_count,
            commit_insertions,
            commit_deletions,
            tokens_used: row.3,
            cost_cents,
            files_edited_count: row.5,
        })
    }

    /// Get all-time contributions from snapshots + today's real-time data.
    async fn get_all_contributions(
        &self,
        project_id: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        // Get snapshot totals
        let snapshot: (i64, i64, i64, i64, i64, i64, i64, i64, i64) =
            if let Some(pid) = project_id {
                sqlx::query_as(
                    r#"
                SELECT
                    COALESCE(SUM(sessions_count), 0),
                    COALESCE(SUM(ai_lines_added), 0),
                    COALESCE(SUM(ai_lines_removed), 0),
                    COALESCE(SUM(commits_count), 0),
                    COALESCE(SUM(commit_insertions), 0),
                    COALESCE(SUM(commit_deletions), 0),
                    COALESCE(SUM(tokens_used), 0),
                    COALESCE(SUM(cost_cents), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM contribution_snapshots
                WHERE project_id = ?1
                "#,
                )
                .bind(pid)
                .fetch_one(self.pool())
                .await?
            } else {
                sqlx::query_as(
                    r#"
                SELECT
                    COALESCE(SUM(sessions_count), 0),
                    COALESCE(SUM(ai_lines_added), 0),
                    COALESCE(SUM(ai_lines_removed), 0),
                    COALESCE(SUM(commits_count), 0),
                    COALESCE(SUM(commit_insertions), 0),
                    COALESCE(SUM(commit_deletions), 0),
                    COALESCE(SUM(tokens_used), 0),
                    COALESCE(SUM(cost_cents), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM contribution_snapshots
                WHERE project_id IS NULL
                "#,
                )
                .fetch_one(self.pool())
                .await?
            };

        // Add today's real-time data
        let today = self.get_today_contributions(project_id).await?;

        Ok(AggregatedContributions {
            sessions_count: snapshot.0 + today.sessions_count,
            ai_lines_added: snapshot.1 + today.ai_lines_added,
            ai_lines_removed: snapshot.2 + today.ai_lines_removed,
            commits_count: snapshot.3 + today.commits_count,
            commit_insertions: snapshot.4 + today.commit_insertions,
            commit_deletions: snapshot.5 + today.commit_deletions,
            tokens_used: snapshot.6 + today.tokens_used,
            cost_cents: snapshot.7 + today.cost_cents,
            files_edited_count: snapshot.8 + today.files_edited_count,
        })
    }

    /// Get contributions in a date range from snapshots.
    async fn get_contributions_in_range(
        &self,
        from: &str,
        to: &str,
        project_id: Option<&str>,
    ) -> DbResult<AggregatedContributions> {
        let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(sessions_count), 0),
                    COALESCE(SUM(ai_lines_added), 0),
                    COALESCE(SUM(ai_lines_removed), 0),
                    COALESCE(SUM(commits_count), 0),
                    COALESCE(SUM(commit_insertions), 0),
                    COALESCE(SUM(commit_deletions), 0),
                    COALESCE(SUM(tokens_used), 0),
                    COALESCE(SUM(cost_cents), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM contribution_snapshots
                WHERE project_id = ?1 AND date >= ?2 AND date <= ?3
                "#,
            )
            .bind(pid)
            .bind(from)
            .bind(to)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(sessions_count), 0),
                    COALESCE(SUM(ai_lines_added), 0),
                    COALESCE(SUM(ai_lines_removed), 0),
                    COALESCE(SUM(commits_count), 0),
                    COALESCE(SUM(commit_insertions), 0),
                    COALESCE(SUM(commit_deletions), 0),
                    COALESCE(SUM(tokens_used), 0),
                    COALESCE(SUM(cost_cents), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM contribution_snapshots
                WHERE project_id IS NULL AND date >= ?1 AND date <= ?2
                "#,
            )
            .bind(from)
            .bind(to)
            .fetch_one(self.pool())
            .await?
        };

        Ok(AggregatedContributions {
            sessions_count: row.0,
            ai_lines_added: row.1,
            ai_lines_removed: row.2,
            commits_count: row.3,
            commit_insertions: row.4,
            commit_deletions: row.5,
            tokens_used: row.6,
            cost_cents: row.7,
            files_edited_count: row.8,
        })
    }

    // ========================================================================
    // Trend Data
    // ========================================================================

    /// Get daily trend data for charting.
    pub async fn get_contribution_trend(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<Vec<DailyTrendPoint>> {
        let (from, to) = match range {
            TimeRange::Today => {
                let today = Utc::now().format("%Y-%m-%d").to_string();
                (today.clone(), today)
            }
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => {
                ("1970-01-01".to_string(), Utc::now().format("%Y-%m-%d").to_string())
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Utc::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Utc::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        let rows: Vec<(String, i64, i64, i64, i64, i64, i64)> = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    date,
                    ai_lines_added,
                    ai_lines_removed,
                    commits_count,
                    sessions_count,
                    tokens_used,
                    cost_cents
                FROM contribution_snapshots
                WHERE project_id = ?1 AND date >= ?2 AND date <= ?3
                ORDER BY date ASC
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    date,
                    ai_lines_added,
                    ai_lines_removed,
                    commits_count,
                    sessions_count,
                    tokens_used,
                    cost_cents
                FROM contribution_snapshots
                WHERE project_id IS NULL AND date >= ?1 AND date <= ?2
                ORDER BY date ASC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|(date, lines_added, lines_removed, commits, sessions, tokens_used, cost_cents)| DailyTrendPoint {
                date,
                lines_added,
                lines_removed,
                commits,
                sessions,
                tokens_used,
                cost_cents,
            })
            .collect())
    }

    // ========================================================================
    // Branch Breakdown
    // ========================================================================

    /// Get contribution breakdown by branch.
    pub async fn get_branch_breakdown(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<Vec<BranchBreakdown>> {
        let (from, to) = match range {
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => {
                ("1970-01-01".to_string(), Utc::now().format("%Y-%m-%d").to_string())
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Utc::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Utc::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        // Query sessions grouped by branch for the time range
        let rows: Vec<(Option<String>, i64, i64, i64, i64, i64, Option<i64>)> = if let Some(pid) =
            project_id
        {
            sqlx::query_as(
                r#"
                SELECT
                    git_branch,
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as lines_removed,
                    COALESCE(SUM(commit_count), 0) as commits_count,
                    COALESCE(SUM(files_edited_count), 0) as files_edited,
                    MAX(last_message_at) as last_activity
                FROM sessions
                WHERE project_id = ?1
                  AND date(last_message_at, 'unixepoch') >= ?2
                  AND date(last_message_at, 'unixepoch') <= ?3
                GROUP BY git_branch
                ORDER BY sessions_count DESC
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    git_branch,
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as lines_removed,
                    COALESCE(SUM(commit_count), 0) as commits_count,
                    COALESCE(SUM(files_edited_count), 0) as files_edited,
                    MAX(last_message_at) as last_activity
                FROM sessions
                WHERE date(last_message_at, 'unixepoch') >= ?1
                  AND date(last_message_at, 'unixepoch') <= ?2
                GROUP BY git_branch
                ORDER BY sessions_count DESC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        };

        Ok(rows
            .into_iter()
            .map(
                |(branch, sessions_count, lines_added, lines_removed, commits_count, _files_edited, last_activity)| {
                    // Calculate AI share: ai_lines_added / total commit insertions
                    // For now, we just show the lines as-is; ai_share needs commit data
                    BranchBreakdown {
                        branch: branch.unwrap_or_else(|| "(no branch)".to_string()),
                        sessions_count,
                        lines_added,
                        lines_removed,
                        commits_count,
                        ai_share: None, // Would need to join with commits table
                        last_activity,
                    }
                },
            )
            .collect())
    }

    // ========================================================================
    // Branch Sessions
    // ========================================================================

    /// Get sessions for a specific branch.
    ///
    /// Returns lightweight session summaries for display when a branch is expanded.
    /// Sessions are ordered by last_message_at descending (most recent first).
    pub async fn get_branch_sessions(
        &self,
        branch: &str,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        limit: i64,
    ) -> DbResult<Vec<BranchSession>> {
        let (from, to) = self.date_range_from_time_range(range, from_date, to_date);

        // Handle "(no branch)" special case
        let branch_filter = if branch == "(no branch)" {
            None
        } else {
            Some(branch)
        };

        let rows: Vec<(String, Option<String>, i64, i64, i64, i64, i64)> = if let Some(pid) = project_id {
            if let Some(b) = branch_filter {
                sqlx::query_as(
                    r#"
                    SELECT
                        id,
                        work_type,
                        duration_seconds,
                        COALESCE(ai_lines_added, 0),
                        COALESCE(ai_lines_removed, 0),
                        COALESCE(commit_count, 0),
                        last_message_at
                    FROM sessions
                    WHERE project_id = ?1
                      AND git_branch = ?2
                      AND date(last_message_at, 'unixepoch') >= ?3
                      AND date(last_message_at, 'unixepoch') <= ?4
                    ORDER BY last_message_at DESC
                    LIMIT ?5
                    "#,
                )
                .bind(pid)
                .bind(b)
                .bind(&from)
                .bind(&to)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
            } else {
                sqlx::query_as(
                    r#"
                    SELECT
                        id,
                        work_type,
                        duration_seconds,
                        COALESCE(ai_lines_added, 0),
                        COALESCE(ai_lines_removed, 0),
                        COALESCE(commit_count, 0),
                        last_message_at
                    FROM sessions
                    WHERE project_id = ?1
                      AND git_branch IS NULL
                      AND date(last_message_at, 'unixepoch') >= ?2
                      AND date(last_message_at, 'unixepoch') <= ?3
                    ORDER BY last_message_at DESC
                    LIMIT ?4
                    "#,
                )
                .bind(pid)
                .bind(&from)
                .bind(&to)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
            }
        } else if let Some(b) = branch_filter {
            sqlx::query_as(
                r#"
                SELECT
                    id,
                    work_type,
                    duration_seconds,
                    COALESCE(ai_lines_added, 0),
                    COALESCE(ai_lines_removed, 0),
                    COALESCE(commit_count, 0),
                    last_message_at
                FROM sessions
                WHERE git_branch = ?1
                  AND date(last_message_at, 'unixepoch') >= ?2
                  AND date(last_message_at, 'unixepoch') <= ?3
                ORDER BY last_message_at DESC
                LIMIT ?4
                "#,
            )
            .bind(b)
            .bind(&from)
            .bind(&to)
            .bind(limit)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    id,
                    work_type,
                    duration_seconds,
                    COALESCE(ai_lines_added, 0),
                    COALESCE(ai_lines_removed, 0),
                    COALESCE(commit_count, 0),
                    last_message_at
                FROM sessions
                WHERE git_branch IS NULL
                  AND date(last_message_at, 'unixepoch') >= ?1
                  AND date(last_message_at, 'unixepoch') <= ?2
                ORDER BY last_message_at DESC
                LIMIT ?3
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(limit)
            .fetch_all(self.pool())
            .await?
        };

        Ok(rows
            .into_iter()
            .map(
                |(session_id, work_type, duration_seconds, ai_lines_added, ai_lines_removed, commit_count, last_message_at)| {
                    BranchSession {
                        session_id,
                        work_type,
                        duration_seconds,
                        ai_lines_added,
                        ai_lines_removed,
                        commit_count,
                        last_message_at,
                    }
                },
            )
            .collect())
    }

    // ========================================================================
    // Session Contribution Detail
    // ========================================================================

    /// Get contribution detail for a single session.
    pub async fn get_session_contribution(&self, session_id: &str) -> DbResult<Option<SessionContribution>> {
        let row: Option<(String, Option<String>, i64, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                id,
                work_type,
                duration_seconds,
                user_prompt_count,
                ai_lines_added,
                ai_lines_removed,
                files_edited_count,
                reedited_files_count,
                commit_count
            FROM sessions
            WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|(session_id, work_type, duration_seconds, prompt_count, ai_lines_added, ai_lines_removed, files_edited_count, reedited_files_count, commit_count)| {
            SessionContribution {
                session_id,
                work_type,
                duration_seconds,
                prompt_count,
                ai_lines_added,
                ai_lines_removed,
                files_edited_count,
                reedited_files_count,
                commit_count,
            }
        }))
    }

    /// Get commits linked to a session.
    pub async fn get_session_commits(&self, session_id: &str) -> DbResult<Vec<LinkedCommit>> {
        let rows: Vec<(String, String, Option<i64>, Option<i64>, i64)> = sqlx::query_as(
            r#"
            SELECT
                c.hash,
                c.message,
                c.insertions,
                c.deletions,
                sc.tier
            FROM session_commits sc
            JOIN commits c ON sc.commit_hash = c.hash
            WHERE sc.session_id = ?1
            ORDER BY c.timestamp ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(hash, message, insertions, deletions, tier)| LinkedCommit {
                hash,
                message,
                insertions,
                deletions,
                tier,
            })
            .collect())
    }

    // ========================================================================
    // Model Breakdown
    // ========================================================================

    /// Get model breakdown statistics for a time range.
    ///
    /// Aggregates by model from turn_metrics table, joining with sessions
    /// to get re-edit rates and line counts.
    pub async fn get_model_breakdown(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<Vec<ModelStats>> {
        let (from, to) = self.date_range_from_time_range(range, from_date, to_date);

        // Query aggregated model stats from turn_metrics joined with sessions
        let rows: Vec<(String, i64, i64, i64, i64, i64, i64)> = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(tm.model, 'unknown') as model,
                    COUNT(DISTINCT s.id) as sessions,
                    COALESCE(SUM(s.ai_lines_added + s.ai_lines_removed), 0) as lines,
                    COALESCE(SUM(tm.input_tokens + tm.output_tokens), 0) as tokens,
                    COALESCE(SUM(s.reedited_files_count), 0) as reedited,
                    COALESCE(SUM(s.files_edited_count), 0) as files_edited,
                    COUNT(DISTINCT tm.session_id) as turn_sessions
                FROM turn_metrics tm
                JOIN sessions s ON tm.session_id = s.id
                WHERE s.project_id = ?1
                  AND date(s.last_message_at, 'unixepoch') >= ?2
                  AND date(s.last_message_at, 'unixepoch') <= ?3
                GROUP BY tm.model
                ORDER BY lines DESC
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(tm.model, 'unknown') as model,
                    COUNT(DISTINCT s.id) as sessions,
                    COALESCE(SUM(s.ai_lines_added + s.ai_lines_removed), 0) as lines,
                    COALESCE(SUM(tm.input_tokens + tm.output_tokens), 0) as tokens,
                    COALESCE(SUM(s.reedited_files_count), 0) as reedited,
                    COALESCE(SUM(s.files_edited_count), 0) as files_edited,
                    COUNT(DISTINCT tm.session_id) as turn_sessions
                FROM turn_metrics tm
                JOIN sessions s ON tm.session_id = s.id
                WHERE date(s.last_message_at, 'unixepoch') >= ?1
                  AND date(s.last_message_at, 'unixepoch') <= ?2
                GROUP BY tm.model
                ORDER BY lines DESC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|(model, _sessions, lines, tokens, reedited, files_edited, _turn_sessions)| {
                let reedit_rate = if files_edited > 0 {
                    Some(reedited as f64 / files_edited as f64)
                } else {
                    None
                };

                let cost_cents = estimate_cost_cents(tokens);
                let cost_per_line = if lines > 0 {
                    Some(cost_cents as f64 / 100.0 / lines as f64)
                } else {
                    None
                };

                // Generate simple insight
                let insight = match reedit_rate {
                    Some(rr) if rr < 0.15 => format!("Low re-edit rate ({:.0}%)", rr * 100.0),
                    Some(rr) if rr > 0.35 => format!("High re-edit rate ({:.0}%)", rr * 100.0),
                    Some(rr) => format!("{:.0}% re-edit rate", rr * 100.0),
                    None => "No re-edit data".to_string(),
                };

                ModelStats {
                    model,
                    lines,
                    reedit_rate,
                    cost_per_line,
                    insight,
                }
            })
            .collect())
    }

    // ========================================================================
    // Learning Curve
    // ========================================================================

    /// Get learning curve data (re-edit rate over monthly periods).
    pub async fn get_learning_curve(
        &self,
        project_id: Option<&str>,
    ) -> DbResult<LearningCurve> {
        // Get monthly re-edit rates for the last 6 months
        let rows: Vec<(String, i64, i64)> = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    strftime('%Y-%m', datetime(last_message_at, 'unixepoch')) as period,
                    COALESCE(SUM(reedited_files_count), 0) as reedited,
                    COALESCE(SUM(files_edited_count), 0) as files_edited
                FROM sessions
                WHERE project_id = ?1
                  AND last_message_at >= strftime('%s', 'now', '-6 months')
                GROUP BY period
                ORDER BY period ASC
                "#,
            )
            .bind(pid)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    strftime('%Y-%m', datetime(last_message_at, 'unixepoch')) as period,
                    COALESCE(SUM(reedited_files_count), 0) as reedited,
                    COALESCE(SUM(files_edited_count), 0) as files_edited
                FROM sessions
                WHERE last_message_at >= strftime('%s', 'now', '-6 months')
                GROUP BY period
                ORDER BY period ASC
                "#,
            )
            .fetch_all(self.pool())
            .await?
        };

        let periods: Vec<LearningCurvePeriod> = rows
            .iter()
            .filter(|(_, _, files_edited)| *files_edited > 0)
            .map(|(period, reedited, files_edited)| {
                LearningCurvePeriod {
                    period: period.clone(),
                    reedit_rate: *reedited as f64 / *files_edited as f64,
                }
            })
            .collect();

        // Calculate current average and improvement
        let current_avg = periods.last().map(|p| p.reedit_rate).unwrap_or(0.0);
        let start_avg = periods.first().map(|p| p.reedit_rate).unwrap_or(0.0);

        let improvement = if start_avg > 0.0 {
            ((start_avg - current_avg) / start_avg) * 100.0
        } else {
            0.0
        };

        // Generate insight
        let insight = if periods.len() < 2 {
            "Not enough data for learning curve analysis".to_string()
        } else if improvement > 30.0 {
            format!(
                "Re-edit rate dropped {:.0}% - your prompting has improved significantly",
                improvement
            )
        } else if improvement > 10.0 {
            "Steady improvement in prompt accuracy".to_string()
        } else if improvement < -10.0 {
            "Re-edit rate increasing - consider reviewing prompt patterns".to_string()
        } else {
            "Consistent prompting quality".to_string()
        };

        Ok(LearningCurve {
            periods,
            current_avg,
            improvement,
            insight,
        })
    }

    // ========================================================================
    // Skill Breakdown
    // ========================================================================

    /// Get skill effectiveness breakdown.
    ///
    /// Parses skills_used JSON from sessions to aggregate by skill.
    pub async fn get_skill_breakdown(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<Vec<SkillStats>> {
        let (from, to) = self.date_range_from_time_range(range, from_date, to_date);

        // Get sessions with skills data
        let rows: Vec<(String, i64, i64, i64, i64, i64)> = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    skills_used,
                    ai_lines_added + ai_lines_removed as lines,
                    CASE WHEN commit_count > 0 THEN 1 ELSE 0 END as has_commit,
                    reedited_files_count,
                    files_edited_count,
                    1 as session_count
                FROM sessions
                WHERE project_id = ?1
                  AND date(last_message_at, 'unixepoch') >= ?2
                  AND date(last_message_at, 'unixepoch') <= ?3
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    skills_used,
                    ai_lines_added + ai_lines_removed as lines,
                    CASE WHEN commit_count > 0 THEN 1 ELSE 0 END as has_commit,
                    reedited_files_count,
                    files_edited_count,
                    1 as session_count
                FROM sessions
                WHERE date(last_message_at, 'unixepoch') >= ?1
                  AND date(last_message_at, 'unixepoch') <= ?2
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(self.pool())
            .await?
        };

        // Aggregate by skill
        let mut skill_map: std::collections::HashMap<String, (i64, i64, i64, i64, i64)> =
            std::collections::HashMap::new();

        for (skills_json, lines, has_commit, reedited, files_edited, _) in rows {
            // Parse skills JSON array
            let skills: Vec<String> = match serde_json::from_str(&skills_json) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to parse skills_used JSON: {e}");
                    Vec::new()
                }
            };

            if skills.is_empty() {
                // Track sessions without skills
                let entry = skill_map.entry("(no skill)".to_string()).or_default();
                entry.0 += 1; // sessions
                entry.1 += lines; // lines
                entry.2 += has_commit; // commits
                entry.3 += reedited; // reedited
                entry.4 += files_edited; // files_edited
            } else {
                for skill in skills {
                    let entry = skill_map.entry(skill).or_default();
                    entry.0 += 1;
                    entry.1 += lines;
                    entry.2 += has_commit;
                    entry.3 += reedited;
                    entry.4 += files_edited;
                }
            }
        }

        let mut results: Vec<SkillStats> = skill_map
            .into_iter()
            .map(|(skill, (sessions, lines, commits, reedited, files_edited))| {
                let avg_loc = if sessions > 0 { lines / sessions } else { 0 };
                let commit_rate = if sessions > 0 {
                    commits as f64 / sessions as f64
                } else {
                    0.0
                };
                let reedit_rate = if files_edited > 0 {
                    reedited as f64 / files_edited as f64
                } else {
                    0.0
                };

                SkillStats {
                    skill,
                    sessions,
                    avg_loc,
                    commit_rate,
                    reedit_rate,
                }
            })
            .collect();

        // Sort by sessions descending
        results.sort_by(|a, b| b.sessions.cmp(&a.sessions));

        Ok(results)
    }

    // ========================================================================
    // Uncommitted Work
    // ========================================================================

    /// Get uncommitted work across projects.
    ///
    /// Returns sessions that have AI lines but no linked commits.
    pub async fn get_uncommitted_work(&self) -> DbResult<Vec<UncommittedWork>> {
        // Find projects/branches with uncommitted AI work
        let rows: Vec<(String, String, Option<String>, i64, i64, String, String, i64)> =
            sqlx::query_as(
                r#"
                SELECT
                    s.project_id,
                    s.project_display_name,
                    s.git_branch,
                    COALESCE(SUM(s.ai_lines_added), 0) as lines_added,
                    COALESCE(SUM(s.files_edited_count), 0) as files_count,
                    (SELECT id FROM sessions s2
                     WHERE s2.project_id = s.project_id
                       AND (s2.git_branch = s.git_branch OR (s2.git_branch IS NULL AND s.git_branch IS NULL))
                       AND s2.commit_count = 0
                       AND s2.ai_lines_added > 0
                     ORDER BY s2.last_message_at DESC LIMIT 1
                    ) as last_session_id,
                    (SELECT preview FROM sessions s2
                     WHERE s2.project_id = s.project_id
                       AND (s2.git_branch = s.git_branch OR (s2.git_branch IS NULL AND s.git_branch IS NULL))
                       AND s2.commit_count = 0
                       AND s2.ai_lines_added > 0
                     ORDER BY s2.last_message_at DESC LIMIT 1
                    ) as last_session_preview,
                    MAX(s.last_message_at) as last_activity_at
                FROM sessions s
                WHERE s.commit_count = 0
                  AND s.ai_lines_added > 0
                  AND s.last_message_at >= strftime('%s', 'now', '-7 days')
                GROUP BY s.project_id, s.git_branch
                HAVING lines_added > 0
                ORDER BY last_activity_at DESC
                LIMIT 10
                "#,
            )
            .fetch_all(self.pool())
            .await?;

        let now = Utc::now().timestamp();

        Ok(rows
            .into_iter()
            .filter(|(_, _, _, _, _, last_id, _, _)| !last_id.is_empty())
            .map(
                |(
                    project_id,
                    project_name,
                    branch,
                    lines_added,
                    files_count,
                    last_session_id,
                    last_session_preview,
                    last_activity_at,
                )| {
                    let hours_since = (now - last_activity_at) as f64 / 3600.0;

                    let insight = if hours_since > 24.0 {
                        let days = (hours_since / 24.0).floor() as i64;
                        format!(
                            "{} lines uncommitted for {}+ days - consider committing",
                            lines_added, days
                        )
                    } else if hours_since > 2.0 {
                        format!(
                            "{:.0} hours old - consider committing or this work may be lost",
                            hours_since
                        )
                    } else {
                        "Recent work - commit when ready".to_string()
                    };

                    UncommittedWork {
                        project_id,
                        project_name,
                        branch,
                        lines_added,
                        files_count,
                        last_session_id,
                        last_session_preview,
                        last_activity_at,
                        insight,
                    }
                },
            )
            .collect())
    }

    // ========================================================================
    // File Impacts for Session Detail
    // ========================================================================

    /// Get file impacts for a session.
    ///
    /// Parses files_edited JSON from the session.
    pub async fn get_session_file_impacts(&self, session_id: &str) -> DbResult<Vec<FileImpact>> {
        // Get files_edited JSON from session
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT files_edited FROM sessions WHERE id = ?1",
        )
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;

        let Some((files_json,)) = row else {
            return Ok(Vec::new());
        };

        // Parse the files_edited JSON
        // Expected format: array of file paths or objects with path info
        let files: Vec<String> = match serde_json::from_str(&files_json) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("Failed to parse files_edited JSON for session {session_id}: {e}");
                Vec::new()
            }
        };

        // For now, we return basic file info without detailed line counts
        // (detailed line counts would require parsing the JSONL file)
        Ok(files
            .into_iter()
            .map(|path| FileImpact {
                path,
                lines_added: 0,  // Would need JSONL parsing for actual counts
                lines_removed: 0,
                action: "modified".to_string(),
            })
            .collect())
    }

    // ========================================================================
    // Helper: Date Range Calculation
    // ========================================================================

    /// Convert TimeRange to (from, to) date strings.
    fn date_range_from_time_range(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
    ) -> (String, String) {
        match range {
            TimeRange::Today => {
                let today = Utc::now().format("%Y-%m-%d").to_string();
                (today.clone(), today)
            }
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => {
                ("1970-01-01".to_string(), Utc::now().format("%Y-%m-%d").to_string())
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Utc::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Utc::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        }
    }

    // ========================================================================
    // Snapshot Generation (Daily Job)
    // ========================================================================

    /// Generate a daily snapshot for a specific date.
    ///
    /// This aggregates session data and commit data for the given date
    /// and upserts it into the contribution_snapshots table.
    pub async fn generate_daily_snapshot(&self, date: &str) -> DbResult<()> {
        // Get session aggregates for the date (global)
        let session_agg: (i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as sessions_count,
                COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                COALESCE(SUM(files_edited_count), 0) as files_edited_count
            FROM sessions
            WHERE date(last_message_at, 'unixepoch') = ?1
            "#,
        )
        .bind(date)
        .fetch_one(self.pool())
        .await?;

        // Get commit aggregates for the date (from commits linked to sessions on that date)
        let commit_agg: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(DISTINCT c.hash) as commits_count,
                COALESCE(SUM(c.insertions), 0) as commit_insertions,
                COALESCE(SUM(c.deletions), 0) as commit_deletions
            FROM session_commits sc
            JOIN commits c ON sc.commit_hash = c.hash
            JOIN sessions s ON sc.session_id = s.id
            WHERE date(s.last_message_at, 'unixepoch') = ?1
            "#,
        )
        .bind(date)
        .fetch_one(self.pool())
        .await?;

        let cost_cents = estimate_cost_cents(session_agg.3);

        // Upsert global snapshot (project_id = NULL, branch = NULL)
        self.upsert_snapshot(
            date,
            None, // global
            None, // all branches
            session_agg.0,
            session_agg.1,
            session_agg.2,
            commit_agg.0,
            commit_agg.1,
            commit_agg.2,
            session_agg.3,
            cost_cents,
            session_agg.4,
        )
        .await?;

        Ok(())
    }

    /// Generate daily snapshots for all dates missing in the range.
    ///
    /// Typically called by the nightly job to backfill any missing days.
    pub async fn generate_missing_snapshots(&self, days_back: i64) -> DbResult<u32> {
        let today = Utc::now().date_naive();

        // Collect all missing dates first
        let mut missing_dates = Vec::new();
        for i in 1..=days_back {
            let date = (today - chrono::Duration::days(i))
                .format("%Y-%m-%d")
                .to_string();

            let exists: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL",
            )
            .bind(&date)
            .fetch_one(self.pool())
            .await?;

            if exists.0 == 0 {
                missing_dates.push(date);
            }
        }

        if missing_dates.is_empty() {
            return Ok(0);
        }

        // Batch all snapshot generation in a single transaction
        let mut tx = self.pool().begin().await?;
        let count = missing_dates.len() as u32;

        for date in &missing_dates {
            // Inline the snapshot generation to use the transaction
            let session_agg: (i64, i64, i64, i64, i64) = sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as sessions_count,
                    COALESCE(SUM(ai_lines_added), 0) as ai_lines_added,
                    COALESCE(SUM(ai_lines_removed), 0) as ai_lines_removed,
                    COALESCE(SUM(total_input_tokens + total_output_tokens), 0) as tokens_used,
                    COALESCE(SUM(files_edited_count), 0) as files_edited_count
                FROM sessions
                WHERE date(last_message_at, 'unixepoch') = ?1
                "#,
            )
            .bind(date)
            .fetch_one(&mut *tx)
            .await?;

            let commit_agg: (i64, i64, i64) = sqlx::query_as(
                r#"
                SELECT
                    COUNT(DISTINCT c.hash) as commits_count,
                    COALESCE(SUM(c.insertions), 0) as commit_insertions,
                    COALESCE(SUM(c.deletions), 0) as commit_deletions
                FROM session_commits sc
                JOIN commits c ON sc.commit_hash = c.hash
                JOIN sessions s ON sc.session_id = s.id
                WHERE date(s.last_message_at, 'unixepoch') = ?1
                "#,
            )
            .bind(date)
            .fetch_one(&mut *tx)
            .await?;

            let cost_cents = estimate_cost_cents(session_agg.3);

            // First delete any existing row for this (date, NULL, NULL) combo,
            // since UNIQUE(date, project_id, branch) doesn't catch NULL duplicates.
            sqlx::query(
                "DELETE FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch IS NULL",
            )
            .bind(date)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO contribution_snapshots
                    (date, project_id, branch, sessions_count, ai_lines_added, ai_lines_removed,
                     commits_count, commit_insertions, commit_deletions, tokens_used, cost_cents, files_edited_count)
                VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
            )
            .bind(date)
            .bind(session_agg.0)
            .bind(session_agg.1)
            .bind(session_agg.2)
            .bind(commit_agg.0)
            .bind(commit_agg.1)
            .bind(commit_agg.2)
            .bind(session_agg.3)
            .bind(cost_cents)
            .bind(session_agg.4)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(count)
    }

    // ========================================================================
    // Weekly Rollup Job
    // ========================================================================

    /// Roll up daily snapshots into weekly aggregates.
    ///
    /// This function aggregates daily snapshots older than `retention_days` into
    /// weekly buckets, reducing storage while preserving historical data.
    ///
    /// Weekly snapshots use the Monday of each week as the date key.
    ///
    /// # Arguments
    /// * `retention_days` - Keep daily granularity for this many days (default: 30)
    ///
    /// # Returns
    /// * Number of weekly snapshots created
    pub async fn rollup_weekly_snapshots(&self, retention_days: i64) -> DbResult<u32> {
        let cutoff_date = (Utc::now() - chrono::Duration::days(retention_days))
            .format("%Y-%m-%d")
            .to_string();

        // Find weeks with daily snapshots that need rollup (global snapshots only)
        // Group by ISO week (Monday as start of week)
        let weeks: Vec<(String, i64, i64, i64, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                -- Get the Monday of the week (ISO week starts on Monday)
                date(date, 'weekday 0', '-6 days') as week_start,
                COALESCE(SUM(sessions_count), 0),
                COALESCE(SUM(ai_lines_added), 0),
                COALESCE(SUM(ai_lines_removed), 0),
                COALESCE(SUM(commits_count), 0),
                COALESCE(SUM(commit_insertions), 0),
                COALESCE(SUM(commit_deletions), 0),
                COALESCE(SUM(tokens_used), 0),
                COALESCE(SUM(cost_cents), 0),
                COALESCE(SUM(files_edited_count), 0)
            FROM contribution_snapshots
            WHERE project_id IS NULL
              AND branch IS NULL
              AND date < ?1
              AND length(date) = 10  -- Only daily snapshots (YYYY-MM-DD format)
            GROUP BY week_start
            HAVING COUNT(*) > 0
            ORDER BY week_start ASC
            "#,
        )
        .bind(&cutoff_date)
        .fetch_all(self.pool())
        .await?;

        if weeks.is_empty() {
            return Ok(0);
        }

        // Batch all inserts and deletes in a single transaction for performance
        let mut tx = self.pool().begin().await?;
        let mut count = 0u32;

        for (week_start, sessions, lines_added, lines_removed, commits, insertions, deletions, tokens, cost, files_edited) in weeks {
            // Create the week key format: "W:YYYY-MM-DD" to distinguish from daily
            let week_key = format!("W:{}", week_start);

            // Check if weekly rollup already exists
            let existing: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM contribution_snapshots WHERE date = ?1 AND project_id IS NULL AND branch IS NULL",
            )
            .bind(&week_key)
            .fetch_one(&mut *tx)
            .await?;

            if existing.0 == 0 {
                // Insert weekly rollup
                sqlx::query(
                    r#"
                    INSERT INTO contribution_snapshots (
                        date, project_id, branch,
                        sessions_count, ai_lines_added, ai_lines_removed,
                        commits_count, commit_insertions, commit_deletions,
                        tokens_used, cost_cents, files_edited_count
                    ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    "#,
                )
                .bind(&week_key)
                .bind(sessions)
                .bind(lines_added)
                .bind(lines_removed)
                .bind(commits)
                .bind(insertions)
                .bind(deletions)
                .bind(tokens)
                .bind(cost)
                .bind(files_edited)
                .execute(&mut *tx)
                .await?;

                count += 1;
            }

            // Delete the daily snapshots that were rolled up (keeping weekly)
            sqlx::query(
                r#"
                DELETE FROM contribution_snapshots
                WHERE project_id IS NULL
                  AND branch IS NULL
                  AND date >= ?1
                  AND date < date(?1, '+7 days')
                  AND length(date) = 10  -- Only delete daily snapshots
                "#,
            )
            .bind(&week_start)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Get snapshot retention statistics.
    ///
    /// Returns counts of daily vs weekly snapshots for monitoring.
    pub async fn get_snapshot_stats(&self) -> DbResult<SnapshotStats> {
        let (daily_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE length(date) = 10 AND project_id IS NULL",
        )
        .fetch_one(self.pool())
        .await?;

        let (weekly_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE date LIKE 'W:%' AND project_id IS NULL",
        )
        .fetch_one(self.pool())
        .await?;

        let oldest_daily: Option<(String,)> = sqlx::query_as(
            "SELECT MIN(date) FROM contribution_snapshots WHERE length(date) = 10 AND project_id IS NULL",
        )
        .fetch_optional(self.pool())
        .await?;

        let oldest_weekly: Option<(String,)> = sqlx::query_as(
            "SELECT MIN(date) FROM contribution_snapshots WHERE date LIKE 'W:%' AND project_id IS NULL",
        )
        .fetch_optional(self.pool())
        .await?;

        Ok(SnapshotStats {
            daily_count,
            weekly_count,
            oldest_daily: oldest_daily.and_then(|r| r.0.into()),
            oldest_weekly: oldest_weekly.map(|r| r.0.replace("W:", "")),
        })
    }

    // ========================================================================
    // Re-edit Rate Calculation
    // ========================================================================

    /// Calculate weighted average re-edit rate for a time range.
    ///
    /// Re-edit rate = total_reedited_files / total_files_edited
    pub async fn get_reedit_rate(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<Option<f64>> {
        let (from, to) = match range {
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => {
                ("1970-01-01".to_string(), Utc::now().format("%Y-%m-%d").to_string())
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Utc::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Utc::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        let row: (i64, i64) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(reedited_files_count), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM sessions
                WHERE project_id = ?1
                  AND date(last_message_at, 'unixepoch') >= ?2
                  AND date(last_message_at, 'unixepoch') <= ?3
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(reedited_files_count), 0),
                    COALESCE(SUM(files_edited_count), 0)
                FROM sessions
                WHERE date(last_message_at, 'unixepoch') >= ?1
                  AND date(last_message_at, 'unixepoch') <= ?2
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_one(self.pool())
            .await?
        };

        if row.1 == 0 {
            Ok(None)
        } else {
            Ok(Some(row.0 as f64 / row.1 as f64))
        }
    }

    // ========================================================================
    // Commit Rate Calculation
    // ========================================================================

    /// Calculate commit rate for a time range.
    ///
    /// Commit rate = sessions_with_commits / total_sessions
    pub async fn get_commit_rate(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<Option<f64>> {
        let (from, to) = match range {
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => {
                ("1970-01-01".to_string(), Utc::now().format("%Y-%m-%d").to_string())
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Utc::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Utc::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        let row: (i64, i64) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    SUM(CASE WHEN commit_count > 0 THEN 1 ELSE 0 END),
                    COUNT(*)
                FROM sessions
                WHERE project_id = ?1
                  AND date(last_message_at, 'unixepoch') >= ?2
                  AND date(last_message_at, 'unixepoch') <= ?3
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    SUM(CASE WHEN commit_count > 0 THEN 1 ELSE 0 END),
                    COUNT(*)
                FROM sessions
                WHERE date(last_message_at, 'unixepoch') >= ?1
                  AND date(last_message_at, 'unixepoch') <= ?2
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_one(self.pool())
            .await?
        };

        if row.1 == 0 {
            Ok(None)
        } else {
            Ok(Some(row.0 as f64 / row.1 as f64))
        }
    }

    /// Get total user prompt count for a time range.
    ///
    /// Queries sessions table directly since prompts are not stored in snapshots.
    pub async fn get_total_prompts(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
    ) -> DbResult<i64> {
        let (from, to) = match range {
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => {
                ("1970-01-01".to_string(), Utc::now().format("%Y-%m-%d").to_string())
            }
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Utc::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Utc::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        };

        let row: (i64,) = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT COALESCE(SUM(user_prompt_count), 0)
                FROM sessions
                WHERE project_id = ?1
                  AND date(last_message_at, 'unixepoch') >= ?2
                  AND date(last_message_at, 'unixepoch') <= ?3
                "#,
            )
            .bind(pid)
            .bind(&from)
            .bind(&to)
            .fetch_one(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT COALESCE(SUM(user_prompt_count), 0)
                FROM sessions
                WHERE date(last_message_at, 'unixepoch') >= ?1
                  AND date(last_message_at, 'unixepoch') <= ?2
                "#,
            )
            .bind(&from)
            .bind(&to)
            .fetch_one(self.pool())
            .await?
        };

        Ok(row.0)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Estimate cost in cents from token count.
///
/// Uses a blended rate assuming ~50% Sonnet, ~40% Haiku, ~10% Opus usage.
/// Input:output ratio assumed to be 2:1.
///
/// Pricing (per million tokens):
/// - Opus: $15 input, $75 output
/// - Sonnet: $3 input, $15 output
/// - Haiku: $0.25 input, $1.25 output
///
/// Blended rate: ~$2.5 per million tokens average
fn estimate_cost_cents(total_tokens: i64) -> i64 {
    (total_tokens as f64 * BLENDED_COST_PER_TOKEN).round() as i64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range_from_str() {
        assert_eq!(TimeRange::parse_str("today"), Some(TimeRange::Today));
        assert_eq!(TimeRange::parse_str("week"), Some(TimeRange::Week));
        assert_eq!(TimeRange::parse_str("month"), Some(TimeRange::Month));
        assert_eq!(TimeRange::parse_str("90days"), Some(TimeRange::NinetyDays));
        assert_eq!(TimeRange::parse_str("all"), Some(TimeRange::All));
        assert_eq!(TimeRange::parse_str("custom"), Some(TimeRange::Custom));
        assert_eq!(TimeRange::parse_str("invalid"), None);
    }

    #[test]
    fn test_time_range_days_back() {
        assert_eq!(TimeRange::Today.days_back(), Some(0));
        assert_eq!(TimeRange::Week.days_back(), Some(7));
        assert_eq!(TimeRange::Month.days_back(), Some(30));
        assert_eq!(TimeRange::NinetyDays.days_back(), Some(90));
        assert_eq!(TimeRange::All.days_back(), None);
        assert_eq!(TimeRange::Custom.days_back(), None);
    }

    #[test]
    fn test_time_range_cache_seconds() {
        assert_eq!(TimeRange::Today.cache_seconds(), 60);
        assert_eq!(TimeRange::Week.cache_seconds(), 300);
        assert_eq!(TimeRange::Month.cache_seconds(), 900);
        assert_eq!(TimeRange::NinetyDays.cache_seconds(), 1800);
        assert_eq!(TimeRange::All.cache_seconds(), 1800);
    }

    #[test]
    fn test_estimate_cost_cents() {
        // 1 million tokens = ~$2.50 = 250 cents
        assert_eq!(estimate_cost_cents(1_000_000), 250);

        // 0 tokens = 0 cost
        assert_eq!(estimate_cost_cents(0), 0);

        // 10k tokens = ~2.5 cents
        assert_eq!(estimate_cost_cents(10_000), 3); // rounded
    }

    #[tokio::test]
    async fn test_aggregated_contributions_default() {
        let agg = AggregatedContributions::default();
        assert_eq!(agg.sessions_count, 0);
        assert_eq!(agg.ai_lines_added, 0);
        assert_eq!(agg.commits_count, 0);
    }

    #[tokio::test]
    async fn test_get_aggregated_contributions_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let agg = db
            .get_aggregated_contributions(TimeRange::Week, None, None, None)
            .await
            .unwrap();

        assert_eq!(agg.sessions_count, 0);
        assert_eq!(agg.ai_lines_added, 0);
    }

    #[tokio::test]
    async fn test_upsert_snapshot() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a snapshot
        db.upsert_snapshot("2026-02-05", None, None, 10, 500, 100, 5, 450, 80, 100000, 25, 12)
            .await
            .unwrap();

        // Query it back
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT sessions_count, ai_lines_added, commits_count FROM contribution_snapshots WHERE date = '2026-02-05'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 10);
        assert_eq!(row.1, 500);
        assert_eq!(row.2, 5);

        // Upsert with different values
        db.upsert_snapshot("2026-02-05", None, None, 15, 600, 150, 7, 500, 100, 150000, 38, 18)
            .await
            .unwrap();

        // Should be updated, not duplicated
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE date = '2026-02-05'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(count.0, 1);

        let row: (i64, i64) = sqlx::query_as(
            "SELECT sessions_count, ai_lines_added FROM contribution_snapshots WHERE date = '2026-02-05'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.0, 15);
        assert_eq!(row.1, 600);
    }

    #[tokio::test]
    async fn test_get_contribution_trend_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let trend = db
            .get_contribution_trend(TimeRange::Week, None, None, None)
            .await
            .unwrap();

        assert!(trend.is_empty());
    }

    #[tokio::test]
    async fn test_get_contribution_trend_with_data() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert some snapshots
        db.upsert_snapshot("2026-02-03", None, None, 5, 200, 50, 2, 180, 40, 50000, 13, 5)
            .await
            .unwrap();
        db.upsert_snapshot("2026-02-04", None, None, 8, 350, 80, 4, 300, 60, 80000, 20, 10)
            .await
            .unwrap();
        db.upsert_snapshot("2026-02-05", None, None, 10, 500, 100, 5, 450, 80, 100000, 25, 15)
            .await
            .unwrap();

        let trend = db
            .get_contribution_trend(
                TimeRange::Custom,
                Some("2026-02-01"),
                Some("2026-02-10"),
                None,
            )
            .await
            .unwrap();

        assert_eq!(trend.len(), 3);
        assert_eq!(trend[0].date, "2026-02-03");
        assert_eq!(trend[0].lines_added, 200);
        assert_eq!(trend[2].date, "2026-02-05");
        assert_eq!(trend[2].sessions, 10);
    }

    #[tokio::test]
    async fn test_get_session_contribution_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.get_session_contribution("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_session_commits_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let commits = db.get_session_commits("nonexistent").await.unwrap();
        assert!(commits.is_empty());
    }

    // ========================================================================
    // New functionality tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_model_breakdown_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let breakdown = db
            .get_model_breakdown(TimeRange::Week, None, None, None)
            .await
            .unwrap();
        assert!(breakdown.is_empty());
    }

    #[tokio::test]
    async fn test_get_learning_curve_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let curve = db.get_learning_curve(None).await.unwrap();
        assert!(curve.periods.is_empty());
        assert_eq!(curve.current_avg, 0.0);
        assert_eq!(curve.improvement, 0.0);
    }

    #[tokio::test]
    async fn test_get_skill_breakdown_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let breakdown = db
            .get_skill_breakdown(TimeRange::Week, None, None, None)
            .await
            .unwrap();
        assert!(breakdown.is_empty());
    }

    #[tokio::test]
    async fn test_get_uncommitted_work_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let uncommitted = db.get_uncommitted_work().await.unwrap();
        assert!(uncommitted.is_empty());
    }

    #[tokio::test]
    async fn test_get_session_file_impacts_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let impacts = db.get_session_file_impacts("nonexistent").await.unwrap();
        assert!(impacts.is_empty());
    }

    #[tokio::test]
    async fn test_get_session_file_impacts_with_data() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a session with files_edited
        sqlx::query(
            r#"
            INSERT INTO sessions (id, project_id, file_path, preview, files_edited)
            VALUES ('test-sess', 'proj', '/tmp/t.jsonl', 'Preview', '["src/main.rs", "src/lib.rs"]')
            "#,
        )
        .execute(db.pool())
        .await
        .unwrap();

        let impacts = db.get_session_file_impacts("test-sess").await.unwrap();
        assert_eq!(impacts.len(), 2);
        assert_eq!(impacts[0].path, "src/main.rs");
        assert_eq!(impacts[1].path, "src/lib.rs");
        assert_eq!(impacts[0].action, "modified");
    }

    #[tokio::test]
    async fn test_get_skill_breakdown_with_data() {
        let db = Database::new_in_memory().await.unwrap();
        let now = Utc::now().timestamp();

        // Insert sessions with different skills
        sqlx::query(
            r#"
            INSERT INTO sessions (id, project_id, file_path, preview, skills_used, ai_lines_added, ai_lines_removed, commit_count, files_edited_count, reedited_files_count, last_message_at)
            VALUES
                ('sess1', 'proj', '/tmp/1.jsonl', 'Preview', '["tdd", "commit"]', 200, 50, 1, 5, 1, ?1),
                ('sess2', 'proj', '/tmp/2.jsonl', 'Preview', '["tdd"]', 150, 30, 1, 3, 0, ?1),
                ('sess3', 'proj', '/tmp/3.jsonl', 'Preview', '[]', 100, 20, 0, 4, 2, ?1)
            "#,
        )
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        let breakdown = db
            .get_skill_breakdown(TimeRange::All, None, None, None)
            .await
            .unwrap();

        // Should have 3 entries: tdd, commit, and (no skill)
        assert_eq!(breakdown.len(), 3);

        // Find the tdd skill
        let tdd = breakdown.iter().find(|s| s.skill == "tdd").unwrap();
        assert_eq!(tdd.sessions, 2);
        assert_eq!(tdd.commit_rate, 1.0); // Both sessions have commits

        // Find the no skill entry
        let no_skill = breakdown.iter().find(|s| s.skill == "(no skill)").unwrap();
        assert_eq!(no_skill.sessions, 1);
        assert_eq!(no_skill.commit_rate, 0.0); // No commit
    }

    #[tokio::test]
    async fn test_model_stats_serialization() {
        let stats = ModelStats {
            model: "claude-sonnet".to_string(),
            lines: 500,
            reedit_rate: Some(0.15),
            cost_per_line: Some(0.003),
            insight: "Low re-edit rate".to_string(),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"model\":\"claude-sonnet\""));
        assert!(json.contains("\"reeditRate\":0.15"));
        assert!(json.contains("\"costPerLine\":0.003"));
    }

    #[tokio::test]
    async fn test_learning_curve_serialization() {
        let curve = LearningCurve {
            periods: vec![
                LearningCurvePeriod {
                    period: "2026-01".to_string(),
                    reedit_rate: 0.3,
                },
                LearningCurvePeriod {
                    period: "2026-02".to_string(),
                    reedit_rate: 0.2,
                },
            ],
            current_avg: 0.2,
            improvement: 33.3,
            insight: "Improving".to_string(),
        };

        let json = serde_json::to_string(&curve).unwrap();
        assert!(json.contains("\"currentAvg\":0.2"));
        assert!(json.contains("\"improvement\":33.3"));
        assert!(json.contains("\"reeditRate\":0.3"));
    }

    #[tokio::test]
    async fn test_skill_stats_serialization() {
        let stats = SkillStats {
            skill: "tdd".to_string(),
            sessions: 10,
            avg_loc: 200,
            commit_rate: 0.9,
            reedit_rate: 0.12,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"skill\":\"tdd\""));
        assert!(json.contains("\"avgLoc\":200"));
        assert!(json.contains("\"commitRate\":0.9"));
        assert!(json.contains("\"reeditRate\":0.12"));
    }

    #[tokio::test]
    async fn test_uncommitted_work_serialization() {
        let work = UncommittedWork {
            project_id: "proj1".to_string(),
            project_name: "My Project".to_string(),
            branch: Some("feature/test".to_string()),
            lines_added: 500,
            files_count: 5,
            last_session_id: "sess123".to_string(),
            last_session_preview: "Add feature".to_string(),
            last_activity_at: 1700000000,
            insight: "Recent work".to_string(),
        };

        let json = serde_json::to_string(&work).unwrap();
        assert!(json.contains("\"projectId\":\"proj1\""));
        assert!(json.contains("\"projectName\":\"My Project\""));
        assert!(json.contains("\"linesAdded\":500"));
        assert!(json.contains("\"lastSessionId\":\"sess123\""));
    }

    #[tokio::test]
    async fn test_file_impact_serialization() {
        let impact = FileImpact {
            path: "src/main.rs".to_string(),
            lines_added: 50,
            lines_removed: 10,
            action: "modified".to_string(),
        };

        let json = serde_json::to_string(&impact).unwrap();
        assert!(json.contains("\"path\":\"src/main.rs\""));
        assert!(json.contains("\"linesAdded\":50"));
        assert!(json.contains("\"linesRemoved\":10"));
        assert!(json.contains("\"action\":\"modified\""));
    }

    // ========================================================================
    // Weekly Rollup Tests
    // ========================================================================

    #[tokio::test]
    async fn test_rollup_weekly_snapshots_empty_db() {
        let db = Database::new_in_memory().await.unwrap();

        // Should return 0 when no snapshots exist
        let count = db.rollup_weekly_snapshots(30).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_rollup_weekly_snapshots_creates_weekly() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert daily snapshots from 60 days ago (should be rolled up with retention=30)
        // Week of 2025-12-02 (Mon) to 2025-12-08 (Sun)
        db.upsert_snapshot("2025-12-02", None, None, 5, 100, 20, 2, 90, 15, 10000, 3, 4)
            .await
            .unwrap();
        db.upsert_snapshot("2025-12-03", None, None, 8, 200, 40, 3, 180, 30, 20000, 5, 7)
            .await
            .unwrap();
        db.upsert_snapshot("2025-12-04", None, None, 6, 150, 30, 2, 130, 25, 15000, 4, 5)
            .await
            .unwrap();

        // Perform rollup with 30 day retention
        let count = db.rollup_weekly_snapshots(30).await.unwrap();
        assert_eq!(count, 1); // One week rolled up

        // Check weekly snapshot was created
        let weekly: Option<(String, i64, i64)> = sqlx::query_as(
            "SELECT date, sessions_count, ai_lines_added FROM contribution_snapshots WHERE date LIKE 'W:%'",
        )
        .fetch_optional(db.pool())
        .await
        .unwrap();

        assert!(weekly.is_some());
        let (date, sessions, lines_added) = weekly.unwrap();
        assert!(date.starts_with("W:"));
        assert_eq!(sessions, 19); // 5 + 8 + 6
        assert_eq!(lines_added, 450); // 100 + 200 + 150

        // Daily snapshots should be deleted
        let daily_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE length(date) = 10 AND project_id IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(daily_count.0, 0);
    }

    #[tokio::test]
    async fn test_rollup_preserves_recent_daily() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert recent snapshot (should NOT be rolled up)
        let today = Utc::now().format("%Y-%m-%d").to_string();
        db.upsert_snapshot(&today, None, None, 10, 500, 100, 5, 450, 80, 50000, 13, 8)
            .await
            .unwrap();

        // Perform rollup
        let count = db.rollup_weekly_snapshots(30).await.unwrap();
        assert_eq!(count, 0); // Nothing rolled up

        // Recent snapshot should still exist
        let daily_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE date = ?1",
        )
        .bind(&today)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(daily_count.0, 1);
    }

    #[tokio::test]
    async fn test_get_snapshot_stats() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert some daily snapshots
        db.upsert_snapshot("2026-01-15", None, None, 5, 100, 20, 2, 90, 15, 10000, 3, 3)
            .await
            .unwrap();
        db.upsert_snapshot("2026-01-16", None, None, 8, 200, 40, 3, 180, 30, 20000, 5, 6)
            .await
            .unwrap();

        // Insert a weekly snapshot manually
        sqlx::query(
            "INSERT INTO contribution_snapshots (date, sessions_count, ai_lines_added, ai_lines_removed, commits_count, commit_insertions, commit_deletions, tokens_used, cost_cents) VALUES ('W:2025-12-02', 50, 1000, 200, 10, 900, 150, 100000, 25)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let stats = db.get_snapshot_stats().await.unwrap();

        assert_eq!(stats.daily_count, 2);
        assert_eq!(stats.weekly_count, 1);
        assert_eq!(stats.oldest_daily, Some("2026-01-15".to_string()));
        assert_eq!(stats.oldest_weekly, Some("2025-12-02".to_string()));
    }

    #[tokio::test]
    async fn test_snapshot_stats_serialization() {
        let stats = SnapshotStats {
            daily_count: 30,
            weekly_count: 12,
            oldest_daily: Some("2026-01-15".to_string()),
            oldest_weekly: Some("2025-10-07".to_string()),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"dailyCount\":30"));
        assert!(json.contains("\"weeklyCount\":12"));
        assert!(json.contains("\"oldestDaily\":\"2026-01-15\""));
        assert!(json.contains("\"oldestWeekly\":\"2025-10-07\""));
    }
}
