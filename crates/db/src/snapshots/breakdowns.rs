// crates/db/src/snapshots/breakdowns.rs
//! Model breakdown, learning curve, skill breakdown, and uncommitted work queries.

use std::collections::HashMap;

use super::types::{
    LearningCurve, LearningCurvePeriod, ModelStats, SkillStats, TimeRange, UncommittedWork,
};
use crate::queries::invocation_agg::classify_key;
use crate::{Database, DbResult};
use chrono::Local;

impl Database {
    /// Get model breakdown statistics for a time range.
    ///
    /// Token usage is aggregated from `session_stats.per_model_tokens_json`
    /// (the CQRS Phase 6 replacement for joining `turns`). Session-level
    /// editing metrics (lines, reedits, files edited) are attributed by
    /// `primary_model` to preserve existing line/re-edit summaries.
    pub async fn get_model_breakdown(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
        project_id: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Vec<ModelStats>> {
        let (from, to) = self.date_range_from_time_range(range, from_date, to_date);

        // Single fetch: all sessions in range with their per-model tokens
        // JSON and the session-level editing columns we need. Aggregation
        // happens in Rust — simpler than the former two-CTE SQL path.
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, Option<String>, String, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
              s.id,
              s.primary_model,
              ss.per_model_tokens_json,
              COALESCE(s.ai_lines_added + s.ai_lines_removed, 0) AS lines,
              COALESCE(s.reedited_files_count, 0) AS reedited,
              COALESCE(s.files_edited_count, 0) AS files_edited
            FROM valid_sessions s
            JOIN session_stats ss ON ss.session_id = s.id
            WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
              AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
              AND (?3 IS NULL OR s.project_id = ?3 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
              AND (?4 IS NULL OR s.git_branch = ?4)
            "#,
        )
        .bind(&from)
        .bind(&to)
        .bind(project_id)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        #[derive(Default)]
        struct ModelAgg {
            sessions_with_model: i64,
            input_tokens: i64,
            output_tokens: i64,
            cache_read_tokens: i64,
            cache_creation_tokens: i64,
            sessions_primary: i64,
            lines: i64,
            reedited: i64,
            files_edited: i64,
        }

        let mut by_model: HashMap<String, ModelAgg> = HashMap::new();
        for (_id, primary_model, per_model_json, lines, reedited, files_edited) in rows {
            let per_model: HashMap<String, claude_view_core::pricing::TokenUsage> =
                serde_json::from_str(&per_model_json).unwrap_or_default();
            for (model_id, usage) in &per_model {
                let agg = by_model.entry(model_id.clone()).or_default();
                agg.sessions_with_model += 1;
                agg.input_tokens += usage.input_tokens as i64;
                agg.output_tokens += usage.output_tokens as i64;
                agg.cache_read_tokens += usage.cache_read_tokens as i64;
                agg.cache_creation_tokens += usage.cache_creation_tokens as i64;
            }
            if let Some(model) = primary_model {
                let agg = by_model.entry(model).or_default();
                agg.sessions_primary += 1;
                agg.lines += lines;
                agg.reedited += reedited;
                agg.files_edited += files_edited;
            }
        }

        let mut stats: Vec<ModelStats> = by_model
            .into_iter()
            .map(|(model, agg)| {
                let reedit_rate = if agg.files_edited > 0 {
                    Some(agg.reedited as f64 / agg.files_edited as f64)
                } else {
                    None
                };

                let insight = match reedit_rate {
                    Some(rr) if rr < 0.15 => format!("Low re-edit rate ({:.0}%)", rr * 100.0),
                    Some(rr) if rr > 0.35 => format!("High re-edit rate ({:.0}%)", rr * 100.0),
                    Some(rr) => format!("{:.0}% re-edit rate", rr * 100.0),
                    None => "No re-edit data".to_string(),
                };

                // Match the old COALESCE(sessions_primary, sessions_with_model)
                // precedence: prefer primary-attribution count, fall back to
                // per-model-presence count for models only seen via tokens.
                let sessions = if agg.sessions_primary > 0 {
                    agg.sessions_primary
                } else {
                    agg.sessions_with_model
                };

                ModelStats {
                    model,
                    sessions,
                    lines: agg.lines,
                    input_tokens: agg.input_tokens,
                    output_tokens: agg.output_tokens,
                    cache_read_tokens: agg.cache_read_tokens,
                    cache_creation_tokens: agg.cache_creation_tokens,
                    reedit_rate,
                    cost_per_line: None,
                    insight,
                }
            })
            .collect();
        stats.sort_by(|a, b| {
            (b.input_tokens + b.output_tokens)
                .cmp(&(a.input_tokens + a.output_tokens))
                .then_with(|| a.model.cmp(&b.model))
        });
        // Filter out models that have zero activity in this scope —
        // matches the old `FROM token_agg` behaviour, which only emitted
        // rows that had any turn data.
        stats.retain(|m| {
            m.input_tokens != 0
                || m.output_tokens != 0
                || m.cache_read_tokens != 0
                || m.cache_creation_tokens != 0
                || m.sessions > 0
        });
        Ok(stats)
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

        // Fetch all sessions in scope with their invocation_counts JSON and
        // the session-level editing / commit columns we aggregate over.
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
              ss.invocation_counts,
              COALESCE(s.ai_lines_added + s.ai_lines_removed, 0) AS loc,
              CASE WHEN COALESCE(s.commit_count, 0) > 0 THEN 1 ELSE 0 END AS has_commit,
              COALESCE(s.reedited_files_count, 0) AS reedited,
              COALESCE(s.files_edited_count, 0) AS files_edited
            FROM valid_sessions s
            JOIN session_stats ss ON ss.session_id = s.id
            WHERE date(s.last_message_at, 'unixepoch', 'localtime') >= ?1
              AND date(s.last_message_at, 'unixepoch', 'localtime') <= ?2
              AND (?3 IS NULL OR s.project_id = ?3 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
              AND (?4 IS NULL OR s.git_branch = ?4)
            "#,
        )
        .bind(&from)
        .bind(&to)
        .bind(project_id)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        #[derive(Default)]
        struct Agg {
            sessions: i64,
            loc_sum: i64,
            commit_hits: i64,
            reedited: i64,
            files_edited: i64,
        }
        impl Agg {
            fn push(&mut self, loc: i64, has_commit: i64, reedited: i64, files_edited: i64) {
                self.sessions += 1;
                self.loc_sum += loc;
                self.commit_hits += has_commit;
                self.reedited += reedited;
                self.files_edited += files_edited;
            }
            fn into_stats(self, skill: String) -> SkillStats {
                let avg_loc = if self.sessions > 0 {
                    (self.loc_sum as f64 / self.sessions as f64).round() as i64
                } else {
                    0
                };
                let commit_rate = if self.sessions > 0 {
                    self.commit_hits as f64 / self.sessions as f64
                } else {
                    0.0
                };
                let reedit_rate = if self.files_edited > 0 {
                    self.reedited as f64 / self.files_edited as f64
                } else {
                    0.0
                };
                SkillStats {
                    skill,
                    sessions: self.sessions,
                    avg_loc,
                    commit_rate,
                    reedit_rate,
                }
            }
        }

        let mut by_skill: HashMap<String, Agg> = HashMap::new();
        let mut no_skill = Agg::default();

        for (json, loc, has_commit, reedited, files_edited) in rows {
            let counts: HashMap<String, u64> = serde_json::from_str(&json).unwrap_or_default();
            let mut seen_skills: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for key in counts.keys() {
                if matches!(
                    classify_key(key),
                    crate::queries::invocation_agg::ToolKind::Skill
                ) {
                    if let Some((_, name)) = key.split_once(':') {
                        seen_skills.insert(name.to_string());
                    }
                }
            }
            if seen_skills.is_empty() {
                no_skill.push(loc, has_commit, reedited, files_edited);
                continue;
            }
            for skill in seen_skills {
                by_skill
                    .entry(skill)
                    .or_default()
                    .push(loc, has_commit, reedited, files_edited);
            }
        }

        let mut results: Vec<SkillStats> = by_skill
            .into_iter()
            .map(|(skill, agg)| agg.into_stats(skill))
            .collect();
        if no_skill.sessions > 0 {
            results.push(no_skill.into_stats("(no skill)".to_string()));
        }
        results.sort_by(|a, b| {
            b.sessions
                .cmp(&a.sessions)
                .then_with(|| a.skill.cmp(&b.skill))
        });
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
