// crates/db/src/queries/ai_generation.rs
// AI generation statistics queries (token usage by model/project).

use super::{AggregateCostBreakdown, AIGenerationStats, TokensByModel, TokensByProject};
use crate::{Database, DbResult};

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

        let (files_created, total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens): (i64, i64, i64, i64, i64) =
            sqlx::query_as(
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
                  AND (?3 IS NULL OR project_id = ?3)
                  AND (?4 IS NULL OR git_branch = ?4)
                "#,
            )
            .bind(from)
            .bind(to)
            .bind(project)
            .bind(branch)
            .fetch_one(self.pool())
            .await?;

        let model_rows: Vec<(Option<String>, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                primary_model,
                COALESCE(SUM(total_input_tokens), 0) as input_tokens,
                COALESCE(SUM(total_output_tokens), 0) as output_tokens
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3)
              AND (?4 IS NULL OR git_branch = ?4)
              AND primary_model IS NOT NULL
            GROUP BY primary_model
            ORDER BY (COALESCE(SUM(total_input_tokens), 0) + COALESCE(SUM(total_output_tokens), 0)) DESC
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let tokens_by_model: Vec<TokensByModel> = model_rows
            .into_iter()
            .filter_map(|(model, input_tokens, output_tokens)| {
                model.map(|m| TokensByModel {
                    model: m,
                    input_tokens,
                    output_tokens,
                })
            })
            .collect();

        let project_rows: Vec<(String, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                COALESCE(project_display_name, project_id) as project,
                COALESCE(SUM(total_input_tokens), 0) as input_tokens,
                COALESCE(SUM(total_output_tokens), 0) as output_tokens
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3)
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

        let rows: Vec<(Option<String>, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                primary_model,
                COALESCE(SUM(total_input_tokens), 0),
                COALESCE(SUM(total_output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_creation_tokens), 0)
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3)
              AND (?4 IS NULL OR git_branch = ?4)
              AND primary_model IS NOT NULL
            GROUP BY primary_model
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(model, input, output, cache_read, cache_create)| {
                model.map(|m| (m, input, output, cache_read, cache_create))
            })
            .collect())
    }
}
