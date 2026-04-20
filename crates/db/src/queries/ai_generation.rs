// crates/db/src/queries/ai_generation.rs
// AI generation statistics queries (token usage by model/project).
//
// Per-model token breakdowns are derived from `session_stats.per_model_tokens_json`
// (written by indexer_v2). This replaces the pre-CQRS `JOIN turns` aggregation.

use std::collections::HashMap;

use claude_view_core::pricing::TokenUsage;

use super::{AIGenerationStats, AggregateCostBreakdown, TokensByModel, TokensByProject};
use crate::{Database, DbResult};

/// Aggregated per-model token buckets: (input, output, cache_read, cache_creation).
type PerModelAgg = (i64, i64, i64, i64);

/// Aggregate `session_stats.per_model_tokens_json` blobs across a set of sessions.
///
/// Empty model-id keys and parse failures are skipped silently (same forgiving
/// semantics as the previous SQL path's `AND t.model_id IS NOT NULL`).
fn aggregate_per_model(rows: Vec<(String,)>) -> HashMap<String, PerModelAgg> {
    let mut agg: HashMap<String, PerModelAgg> = HashMap::new();
    for (json,) in rows {
        let per_model: HashMap<String, TokenUsage> =
            serde_json::from_str(&json).unwrap_or_default();
        for (model_id, usage) in per_model {
            if model_id.is_empty() {
                continue;
            }
            let entry = agg.entry(model_id).or_default();
            entry.0 += usage.input_tokens as i64;
            entry.1 += usage.output_tokens as i64;
            entry.2 += usage.cache_read_tokens as i64;
            entry.3 += usage.cache_creation_tokens as i64;
        }
    }
    agg
}

impl Database {
    // ========================================================================
    // AI Generation Statistics (for dashboard AI generation breakdown)
    // ========================================================================

    /// Get AI generation statistics with optional time range filter.
    pub async fn get_ai_generation_stats(
        &self,
        from: Option<i64>,
        to: Option<i64>,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<AIGenerationStats> {
        let from = from.unwrap_or(1);
        let to = to.unwrap_or(i64::MAX);

        let (
            files_created,
            total_input_tokens,
            total_output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
        ): (i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
                SELECT
                    COALESCE(SUM(files_edited_count), 0),
                    COALESCE(SUM(total_input_tokens), 0),
                    COALESCE(SUM(total_output_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0),
                    COALESCE(SUM(cache_creation_tokens), 0)
                FROM valid_sessions
                WHERE last_message_at >= ?1
                  AND last_message_at <= ?2
                  AND (?3 IS NULL OR project_id = ?3
                       OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3)
                       OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3))
                  AND (?4 IS NULL OR git_branch = ?4)
                "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;

        // Per-model token breakdown from session_stats.per_model_tokens_json.
        let model_json_rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT ss.per_model_tokens_json
            FROM valid_sessions s
            JOIN session_stats ss ON ss.session_id = s.id
            WHERE s.last_message_at >= ?1
              AND s.last_message_at <= ?2
              AND (?3 IS NULL OR s.project_id = ?3
                   OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3)
                   OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
              AND (?4 IS NULL OR s.git_branch = ?4)
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let agg = aggregate_per_model(model_json_rows);
        let mut tokens_by_model: Vec<TokensByModel> = agg
            .into_iter()
            .map(|(model, (input, output, _cr, _cc))| TokensByModel {
                model,
                input_tokens: input,
                output_tokens: output,
            })
            .collect();
        tokens_by_model.sort_by(|a, b| {
            (b.input_tokens + b.output_tokens).cmp(&(a.input_tokens + a.output_tokens))
        });

        let project_rows: Vec<(String, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                COALESCE(project_display_name, project_id) as project,
                COALESCE(SUM(total_input_tokens), 0) as input_tokens,
                COALESCE(SUM(total_output_tokens), 0) as output_tokens
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3
                   OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3)
                   OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3))
              AND (?4 IS NULL OR git_branch = ?4)
            GROUP BY project_id
            ORDER BY (COALESCE(SUM(total_input_tokens), 0) + COALESCE(SUM(total_output_tokens), 0)) DESC
            LIMIT 6
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let tokens_by_project: Vec<TokensByProject> = if project_rows.len() > 5 {
            let mut result: Vec<TokensByProject> = project_rows
                .iter()
                .take(5)
                .map(|(project, input_tokens, output_tokens)| TokensByProject {
                    project: project.clone(),
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                })
                .collect();

            let top5_input: i64 = result.iter().map(|p| p.input_tokens).sum();
            let top5_output: i64 = result.iter().map(|p| p.output_tokens).sum();
            let others_input = (total_input_tokens - top5_input).max(0);
            let others_output = (total_output_tokens - top5_output).max(0);

            if others_input > 0 || others_output > 0 {
                result.push(TokensByProject {
                    project: "Others".to_string(),
                    input_tokens: others_input,
                    output_tokens: others_output,
                });
            }
            result
        } else {
            project_rows
                .into_iter()
                .map(|(project, input_tokens, output_tokens)| TokensByProject {
                    project,
                    input_tokens,
                    output_tokens,
                })
                .collect()
        };

        Ok(AIGenerationStats {
            lines_added: 0,
            lines_removed: 0,
            files_created,
            total_input_tokens,
            total_output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            tokens_by_model,
            tokens_by_project,
            cost: AggregateCostBreakdown::default(),
        })
    }

    /// Get per-model token breakdown with ALL 4 token types for cost computation.
    pub async fn get_per_model_token_breakdown(
        &self,
        from: Option<i64>,
        to: Option<i64>,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<Vec<(String, i64, i64, i64, i64)>> {
        let from = from.unwrap_or(1);
        let to = to.unwrap_or(i64::MAX);

        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT ss.per_model_tokens_json
            FROM valid_sessions s
            JOIN session_stats ss ON ss.session_id = s.id
            WHERE s.last_message_at >= ?1
              AND s.last_message_at <= ?2
              AND (?3 IS NULL OR s.project_id = ?3
                   OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3)
                   OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
              AND (?4 IS NULL OR s.git_branch = ?4)
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        Ok(aggregate_per_model(rows)
            .into_iter()
            .map(|(model, (i, o, cr, cc))| (model, i, o, cr, cc))
            .collect())
    }
}
