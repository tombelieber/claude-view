use crate::{pricing::ModelContext, Database, DbResult};

impl Database {
    /// Upsert model context data into the models table.
    /// Uses COALESCE to preserve existing values from other sources (indexer).
    pub async fn upsert_model_context(&self, models: &[ModelContext]) -> DbResult<usize> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut count = 0;
        for m in models {
            sqlx::query(
                r#"INSERT INTO models (id, provider, family, max_input_tokens, max_output_tokens, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?)
                   ON CONFLICT(id) DO UPDATE SET
                       provider = COALESCE(excluded.provider, models.provider),
                       family = COALESCE(excluded.family, models.family),
                       max_input_tokens = COALESCE(excluded.max_input_tokens, models.max_input_tokens),
                       max_output_tokens = COALESCE(excluded.max_output_tokens, models.max_output_tokens),
                       updated_at = excluded.updated_at"#,
            )
            .bind(&m.model_id)
            .bind(&m.provider)
            .bind(&m.family)
            .bind(m.max_input_tokens)
            .bind(m.max_output_tokens)
            .bind(now)
            .execute(self.pool())
            .await?;
            count += 1;
        }

        Ok(count)
    }

    // NOTE: upsert_sdk_models() was removed — model selection now fetches
    // directly from sidecar's SDK model cache. The sdk_supported column
    // remains in the schema but is no longer written to.
}
