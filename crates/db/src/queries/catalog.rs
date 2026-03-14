use crate::{pricing::LiteLlmModelContext, Database, DbResult};

impl Database {
    /// Upsert model context data from LiteLLM into the models table.
    /// Uses COALESCE to preserve existing values from other sources (SDK, indexer).
    pub async fn upsert_litellm_context(&self, models: &[LiteLlmModelContext]) -> DbResult<usize> {
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

    /// Upsert model display metadata from SDK into the models table.
    /// Uses COALESCE to preserve existing values from other sources (LiteLLM, indexer).
    pub async fn upsert_sdk_models(
        &self,
        models: &[(String, String, String, Option<String>, Option<String>)],
    ) -> DbResult<usize> {
        // Tuple: (id, provider, family, display_name, description)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut count = 0;
        for (id, provider, family, display_name, description) in models {
            sqlx::query(
                r#"INSERT INTO models (id, provider, family, display_name, description, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?)
                   ON CONFLICT(id) DO UPDATE SET
                       provider = COALESCE(excluded.provider, models.provider),
                       family = COALESCE(excluded.family, models.family),
                       display_name = COALESCE(excluded.display_name, models.display_name),
                       description = COALESCE(excluded.description, models.description),
                       updated_at = excluded.updated_at"#,
            )
            .bind(id)
            .bind(provider)
            .bind(family)
            .bind(display_name.as_deref())
            .bind(description.as_deref())
            .bind(now)
            .execute(self.pool())
            .await?;
            count += 1;
        }

        Ok(count)
    }
}
