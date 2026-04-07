// crates/db/src/snapshots/breakdowns.rs
//! Model breakdown, learning curve, skill breakdown, and uncommitted work queries.

use super::types::{
    LearningCurve, LearningCurvePeriod, ModelStats, SkillStats, TimeRange, UncommittedWork,
};
use crate::{Database, DbResult};
use chrono::Local;

impl Database {
    /// Get model breakdown statistics for a time range.
    ///
    /// Token usage is aggregated from `turns.model_id` (ground-truth per API
    /// call). Session-level editing metrics remain attributed by `primary_model`
    /// to preserve existing line/re-edit summaries.
    pub async fn get_model_breakdown(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Vec<ModelStats>> {
        let (from, to) = self.date_range_from_time_range(range, from_date, to_date);

        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, i64, i64, i64, i64, i64, i64, i64, i64)> = if let Some(pid) =
            project_id
        {
            sqlx::query_as(
                    r#"
                WITH token_agg AS (
                    SELECT
                        t.model_id AS model,
                        COUNT(DISTINCT s.id) AS sessions_with_model,
                        COALESCE(SUM(t.input_tokens), 0) AS input_tokens,
                        COALESCE(SUM(t.output_tokens), 0) AS output_tokens,
                        COALESCE(SUM(t.cache_read_tokens), 0) AS cache_read_tokens,
                        COALESCE(SUM(t.cache_creation_tokens), 0) AS cache_creation_tokens
                    FROM valid_sessions s
                    JOIN turns t ON t.session_id = s.id
                    WHERE t.model_id IS NOT NULL
                      AND (s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1))
                      AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?2
                      AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?3
                      AND (?4 IS NULL OR s.git_branch = ?4)
                    GROUP BY t.model_id
                ),
                session_agg AS (
                    SELECT
                        s.primary_model AS model,
                        COUNT(*) AS sessions_primary,
                        COALESCE(SUM(s.ai_lines_added + s.ai_lines_removed), 0) AS lines,
                        COALESCE(SUM(s.reedited_files_count), 0) AS reedited,
                        COALESCE(SUM(s.files_edited_count), 0) AS files_edited
                    FROM valid_sessions s
                    WHERE s.primary_model IS NOT NULL
                      AND (s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1))
                      AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?2
                      AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?3
                      AND (?4 IS NULL OR s.git_branch = ?4)
                    GROUP BY s.primary_model
                )
                SELECT
                    t.model as model,
                    COALESCE(sa.sessions_primary, t.sessions_with_model) as sessions,
                    COALESCE(sa.lines, 0) as lines,
                    t.input_tokens as input_tokens,
                    t.output_tokens as output_tokens,
                    t.cache_read_tokens as cache_read_tokens,
                    t.cache_creation_tokens as cache_creation_tokens,
                    COALESCE(sa.reedited, 0) as reedited,
                    COALESCE(sa.files_edited, 0) as files_edited
                FROM token_agg t
                LEFT JOIN session_agg sa ON sa.model = t.model
                ORDER BY t.input_tokens + t.output_tokens DESC
                "#,
                )
                .bind(pid)
                .bind(&from)
                .bind(&to)
                .bind(branch)
                .fetch_all(self.pool())
                .await?
        } else {
            sqlx::query_as(
                r#"
                WITH token_agg AS (
                    SELECT
                        t.model_id AS model,
                        COUNT(DISTINCT s.id) AS sessions_with_model,
                        COALESCE(SUM(t.input_tokens), 0) AS input_tokens,
                        COALESCE(SUM(t.output_tokens), 0) AS output_tokens,
                        COALESCE(SUM(t.cache_read_tokens), 0) AS cache_read_tokens,
                        COALESCE(SUM(t.cache_creation_tokens), 0) AS cache_creation_tokens
                    FROM valid_sessions s
                    JOIN turns t ON t.session_id = s.id
                    WHERE t.model_id IS NOT NULL
                      AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                      AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                      AND (?3 IS NULL OR s.git_branch = ?3)
                    GROUP BY t.model_id
                ),
                session_agg AS (
                    SELECT
                        s.primary_model AS model,
                        COUNT(*) AS sessions_primary,
                        COALESCE(SUM(s.ai_lines_added + s.ai_lines_removed), 0) AS lines,
                        COALESCE(SUM(s.reedited_files_count), 0) AS reedited,
                        COALESCE(SUM(s.files_edited_count), 0) AS files_edited
                    FROM valid_sessions s
                    WHERE s.primary_model IS NOT NULL
                      AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                      AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                      AND (?3 IS NULL OR s.git_branch = ?3)
                    GROUP BY s.primary_model
                )
                SELECT
                    t.model as model,
                    COALESCE(sa.sessions_primary, t.sessions_with_model) as sessions,
                    COALESCE(sa.lines, 0) as lines,
                    t.input_tokens as input_tokens,
                    t.output_tokens as output_tokens,
                    t.cache_read_tokens as cache_read_tokens,
                    t.cache_creation_tokens as cache_creation_tokens,
                    COALESCE(sa.reedited, 0) as reedited,
                    COALESCE(sa.files_edited, 0) as files_edited
                FROM token_agg t
                LEFT JOIN session_agg sa ON sa.model = t.model
                ORDER BY t.input_tokens + t.output_tokens DESC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_all(self.pool())
            .await?
        };

        Ok(rows
            .into_iter()
            .map(
                |(
                    model,
                    sessions,
                    lines,
                    input_tokens,
                    output_tokens,
                    cache_read_tokens,
                    cache_creation_tokens,
                    reedited,
                    files_edited,
                )| {
                    let reedit_rate = if files_edited > 0 {
                        Some(reedited as f64 / files_edited as f64)
                    } else {
                        None
                    };

                    // cost_per_line is computed by the handler using per-model pricing
                    // (set to None here, filled in by contributions.rs)

                    // Generate simple insight
                    let insight = match reedit_rate {
                        Some(rr) if rr < 0.15 => format!("Low re-edit rate ({:.0}%)", rr * 100.0),
                        Some(rr) if rr > 0.35 => format!("High re-edit rate ({:.0}%)", rr * 100.0),
                        Some(rr) => format!("{:.0}% re-edit rate", rr * 100.0),
                        None => "No re-edit data".to_string(),
                    };

                    ModelStats {
                        model,
                        sessions,
                        lines,
                        input_tokens,
                        output_tokens,
                        cache_read_tokens,
                        cache_creation_tokens,
                        reedit_rate,
                        cost_per_line: None,
                        insight,
                    }
                },
            )
            .collect())
    }

    /// Get learning curve data (re-edit rate over monthly periods).
    pub async fn get_learning_curve(
        &self,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<LearningCurve> {
        // Get monthly re-edit rates for the last 6 months
        let rows: Vec<(String, i64, i64)> = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    strftime('%Y-%m', datetime(last_message_at, 'unixepoch', 'localtime')) as period,
                    COALESCE(SUM(reedited_files_count), 0) as reedited,
                    COALESCE(SUM(files_edited_count), 0) as files_edited
                FROM valid_sessions
                WHERE (project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1))
                  AND last_message_at >= strftime('%s', 'now', '-6 months')
                  AND (?2 IS NULL OR git_branch = ?2)
                GROUP BY period
                ORDER BY period ASC
                "#,
            )
            .bind(pid)
            .bind(branch)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    strftime('%Y-%m', datetime(last_message_at, 'unixepoch', 'localtime')) as period,
                    COALESCE(SUM(reedited_files_count), 0) as reedited,
                    COALESCE(SUM(files_edited_count), 0) as files_edited
                FROM valid_sessions
                WHERE last_message_at >= strftime('%s', 'now', '-6 months')
                  AND (?1 IS NULL OR git_branch = ?1)
                GROUP BY period
                ORDER BY period ASC
                "#,
            )
            .bind(branch)
            .fetch_all(self.pool())
            .await?
        };

        let periods: Vec<LearningCurvePeriod> = rows
            .iter()
            .filter(|(_, _, files_edited)| *files_edited > 0)
            .map(|(period, reedited, files_edited)| LearningCurvePeriod {
                period: period.clone(),
                reedit_rate: *reedited as f64 / *files_edited as f64,
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

    /// Get skill effectiveness breakdown.
    ///
    /// Queries the `invocations` + `invocables` tables (where `kind = 'skill'`)
    /// instead of the unreliable `sessions.skills_used` JSON column. Uses a CTE
    /// to deduplicate to one row per (skill, session) pair so session-level
    /// metrics aren't double-counted.
    pub async fn get_skill_breakdown(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Vec<SkillStats>> {
        let (from, to) = self.date_range_from_time_range(range, from_date, to_date);

        // Step 1: Skills with session metrics (deduplicated via CTE)
        let skill_rows: Vec<(String, i64, f64, f64, f64)> = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                WITH skill_sessions AS (
                    SELECT DISTINCT
                        inv.name as skill_name,
                        i.session_id
                    FROM invocations i
                    JOIN invocables inv ON i.invocable_id = inv.id
                    WHERE inv.kind = 'skill'
                )
                SELECT
                    ss.skill_name,
                    COUNT(*) as session_count,
                    COALESCE(AVG(s.ai_lines_added + s.ai_lines_removed), 0.0) as avg_loc,
                    COALESCE(
                        SUM(CASE WHEN s.commit_count > 0 THEN 1.0 ELSE 0.0 END) / COUNT(*),
                        0.0
                    ) as commit_rate,
                    COALESCE(
                        CAST(SUM(s.reedited_files_count) AS REAL) /
                        NULLIF(SUM(s.files_edited_count), 0),
                        0.0
                    ) as reedit_rate
                FROM skill_sessions ss
                JOIN valid_sessions s ON ss.session_id = s.id
                WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (s.project_id = ?3 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
                  AND (?4 IS NULL OR s.git_branch = ?4)
                GROUP BY ss.skill_name
                ORDER BY session_count DESC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(pid)
            .bind(branch)
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                WITH skill_sessions AS (
                    SELECT DISTINCT
                        inv.name as skill_name,
                        i.session_id
                    FROM invocations i
                    JOIN invocables inv ON i.invocable_id = inv.id
                    WHERE inv.kind = 'skill'
                )
                SELECT
                    ss.skill_name,
                    COUNT(*) as session_count,
                    COALESCE(AVG(s.ai_lines_added + s.ai_lines_removed), 0.0) as avg_loc,
                    COALESCE(
                        SUM(CASE WHEN s.commit_count > 0 THEN 1.0 ELSE 0.0 END) / COUNT(*),
                        0.0
                    ) as commit_rate,
                    COALESCE(
                        CAST(SUM(s.reedited_files_count) AS REAL) /
                        NULLIF(SUM(s.files_edited_count), 0),
                        0.0
                    ) as reedit_rate
                FROM skill_sessions ss
                JOIN valid_sessions s ON ss.session_id = s.id
                WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (?3 IS NULL OR s.git_branch = ?3)
                GROUP BY ss.skill_name
                ORDER BY session_count DESC
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_all(self.pool())
            .await?
        };

        // Step 2: "(no skill)" baseline -- sessions with zero skill invocations
        let no_skill_row: Option<(i64, f64, f64, f64)> = if let Some(pid) = project_id {
            sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as session_count,
                    COALESCE(AVG(s.ai_lines_added + s.ai_lines_removed), 0.0) as avg_loc,
                    COALESCE(
                        SUM(CASE WHEN s.commit_count > 0 THEN 1.0 ELSE 0.0 END) /
                        NULLIF(COUNT(*), 0),
                        0.0
                    ) as commit_rate,
                    COALESCE(
                        CAST(SUM(s.reedited_files_count) AS REAL) /
                        NULLIF(SUM(s.files_edited_count), 0),
                        0.0
                    ) as reedit_rate
                FROM valid_sessions s
                LEFT JOIN (
                    SELECT DISTINCT i2.session_id
                    FROM invocations i2
                    JOIN invocables inv2 ON i2.invocable_id = inv2.id
                    WHERE inv2.kind = 'skill'
                ) skill_sessions ON s.id = skill_sessions.session_id
                WHERE skill_sessions.session_id IS NULL
                  AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (s.project_id = ?3 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
                  AND (?4 IS NULL OR s.git_branch = ?4)
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(pid)
            .bind(branch)
            .fetch_optional(self.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    COUNT(*) as session_count,
                    COALESCE(AVG(s.ai_lines_added + s.ai_lines_removed), 0.0) as avg_loc,
                    COALESCE(
                        SUM(CASE WHEN s.commit_count > 0 THEN 1.0 ELSE 0.0 END) /
                        NULLIF(COUNT(*), 0),
                        0.0
                    ) as commit_rate,
                    COALESCE(
                        CAST(SUM(s.reedited_files_count) AS REAL) /
                        NULLIF(SUM(s.files_edited_count), 0),
                        0.0
                    ) as reedit_rate
                FROM valid_sessions s
                LEFT JOIN (
                    SELECT DISTINCT i2.session_id
                    FROM invocations i2
                    JOIN invocables inv2 ON i2.invocable_id = inv2.id
                    WHERE inv2.kind = 'skill'
                ) skill_sessions ON s.id = skill_sessions.session_id
                WHERE skill_sessions.session_id IS NULL
                  AND date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (?3 IS NULL OR s.git_branch = ?3)
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(branch)
            .fetch_optional(self.pool())
            .await?
        };

        // Build results
        let mut results: Vec<SkillStats> = skill_rows
            .into_iter()
            .map(
                |(skill_name, sessions, avg_loc, commit_rate, reedit_rate)| SkillStats {
                    skill: skill_name,
                    sessions,
                    avg_loc: avg_loc.round() as i64,
                    commit_rate,
                    reedit_rate,
                },
            )
            .collect();

        // Add "(no skill)" baseline if there are sessions without skills
        if let Some((count, avg_loc, commit_rate, reedit_rate)) = no_skill_row {
            if count > 0 {
                results.push(SkillStats {
                    skill: "(no skill)".to_string(),
                    sessions: count,
                    avg_loc: avg_loc.round() as i64,
                    commit_rate,
                    reedit_rate,
                });
            }
        }

        // Sort by sessions descending
        results.sort_by(|a, b| b.sessions.cmp(&a.sessions));

        Ok(results)
    }

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
                    (SELECT id FROM valid_sessions s2
                     WHERE s2.project_id = s.project_id
                       AND (s2.git_branch = s.git_branch OR (s2.git_branch IS NULL AND s.git_branch IS NULL))
                       AND s2.commit_count = 0
                       AND s2.ai_lines_added > 0
                     ORDER BY s2.last_message_at DESC LIMIT 1
                    ) as last_session_id,
                    (SELECT preview FROM valid_sessions s2
                     WHERE s2.project_id = s.project_id
                       AND (s2.git_branch = s.git_branch OR (s2.git_branch IS NULL AND s.git_branch IS NULL))
                       AND s2.commit_count = 0
                       AND s2.ai_lines_added > 0
                     ORDER BY s2.last_message_at DESC LIMIT 1
                    ) as last_session_preview,
                    MAX(s.last_message_at) as last_activity_at
                FROM valid_sessions s
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

        let now = Local::now().timestamp();

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
}
