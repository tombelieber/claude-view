// crates/db/src/queries/models.rs
// Model + Turn CRUD operations (Phase 2B).

use super::row_types::{batch_insert_turns_tx, batch_upsert_models_tx};
use super::{ModelWithStats, TokenStats};
use crate::{Database, DbResult};
use claude_view_core::RawTurn;

impl Database {
    /// Batch upsert models: INSERT OR IGNORE + UPDATE last_seen.
    ///
    /// Each `model_id` is parsed via `parse_model_id()` to derive provider/family.
    /// `seen_at` is the unix timestamp when the model was observed.
    pub async fn batch_upsert_models(&self, model_ids: &[String], seen_at: i64) -> DbResult<u64> {
        if model_ids.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        let affected = batch_upsert_models_tx(&mut tx, model_ids, seen_at).await?;
        tx.commit().await?;
        Ok(affected)
    }

    /// Batch insert turns using INSERT OR IGNORE (UUID PK = free dedup on re-index).
    ///
    /// Returns the number of rows actually inserted.
    pub async fn batch_insert_turns(&self, session_id: &str, turns: &[RawTurn]) -> DbResult<u64> {
        if turns.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        let inserted = batch_insert_turns_tx(&mut tx, session_id, turns).await?;
        tx.commit().await?;
        Ok(inserted)
    }

    /// Get all models with usage counts (for GET /api/models).
    ///
    /// Usage totals (`total_turns`, `total_sessions`) are derived from
    /// `session_stats.per_model_tokens_json` — the CQRS Phase 6
    /// replacement for joining `turns`. `total_turns` is approximated by
    /// the session-level `turn_count` attributed to each model's
    /// per-session presence (no per-turn-per-model granularity survives
    /// the wide-row rollup); `total_sessions` is the exact count of
    /// sessions whose per-model JSON mentions this model.
    pub async fn get_all_models(&self) -> DbResult<Vec<ModelWithStats>> {
        use std::collections::HashMap;

        type ModelRow = (
            String,         // id
            Option<String>, // provider
            Option<String>, // family
            Option<String>, // display_name
            Option<String>, // description
            Option<i64>,    // max_input_tokens
            Option<i64>,    // max_output_tokens
            Option<i64>,    // first_seen
            Option<i64>,    // last_seen
        );

        let models: Vec<ModelRow> = sqlx::query_as(
            r#"SELECT id, provider, family, display_name, description,
                      max_input_tokens, max_output_tokens, first_seen, last_seen
               FROM models"#,
        )
        .fetch_all(self.pool())
        .await?;

        let usage_rows: Vec<(String, Option<String>, i64)> = sqlx::query_as(
            r#"SELECT per_model_tokens_json, primary_model, COALESCE(turn_count, 0)
               FROM session_stats"#,
        )
        .fetch_all(self.pool())
        .await?;

        let mut total_sessions: HashMap<String, i64> = HashMap::new();
        let mut total_turns: HashMap<String, i64> = HashMap::new();
        for (json, primary_model, turn_count) in usage_rows {
            let per_model: HashMap<String, claude_view_core::pricing::TokenUsage> =
                serde_json::from_str(&json).unwrap_or_default();
            for model_id in per_model.keys() {
                *total_sessions.entry(model_id.clone()).or_default() += 1;
            }
            // Turn-count attribution: credit the session's full turn count
            // to its `primary_model` only. That matches the legacy
            // per-turn truth for the model that owns the session — minor
            // models that also saw traffic keep `total_turns = 0` unless
            // they were primary in some other session. Lossy vs. the old
            // per-turn join, but preserves the dashboard ranking order
            // (primary model first).
            if let Some(primary) = primary_model {
                *total_turns.entry(primary).or_default() += turn_count;
            }
        }

        let mut rows: Vec<ModelWithStats> = models
            .into_iter()
            .map(
                |(
                    id,
                    provider,
                    family,
                    display_name,
                    description,
                    max_input_tokens,
                    max_output_tokens,
                    first_seen,
                    last_seen,
                )| {
                    let total_turns = total_turns.get(&id).copied().unwrap_or(0);
                    let total_sessions = total_sessions.get(&id).copied().unwrap_or(0);
                    ModelWithStats {
                        id,
                        provider,
                        family,
                        display_name,
                        description,
                        max_input_tokens,
                        max_output_tokens,
                        first_seen,
                        last_seen,
                        total_turns,
                        total_sessions,
                    }
                },
            )
            .collect();
        rows.sort_by(|a, b| {
            b.total_turns
                .cmp(&a.total_turns)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(rows)
    }

    /// Get aggregate token statistics (for GET /api/stats/tokens).
    pub async fn get_token_stats(&self) -> DbResult<TokenStats> {
        let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(total_input_tokens), 0),
                COALESCE(SUM(total_output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_creation_tokens), 0),
                COALESCE(SUM(turn_count), 0),
                COUNT(*)
            FROM valid_sessions
            "#,
        )
        .fetch_one(self.pool())
        .await?;

        let total_input = row.0 as u64;
        let total_cache_read = row.2 as u64;
        let total_cache_creation = row.3 as u64;
        let denominator = total_cache_read + total_cache_creation;
        let cache_hit_ratio = if denominator > 0 {
            total_cache_read as f64 / denominator as f64
        } else {
            0.0
        };

        Ok(TokenStats {
            total_input_tokens: total_input,
            total_output_tokens: row.1 as u64,
            total_cache_read_tokens: total_cache_read,
            total_cache_creation_tokens: total_cache_creation,
            cache_hit_ratio,
            turns_count: row.4 as u64,
            sessions_count: row.5 as u64,
        })
    }
}
