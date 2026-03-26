use crate::{Database, DbResult};

impl Database {
    /// Seeds the `models` table from `load_pricing()` keys if the table is empty.
    /// Called once after migrations on first-ever launch. After that, the table
    /// always has data (from indexer or SDK upserts).
    pub async fn seed_models_if_empty(&self) -> DbResult<()> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM models")
            .fetch_one(self.pool())
            .await?;

        if count > 0 {
            return Ok(());
        }

        let pricing = claude_view_core::pricing::load_pricing();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        for model_id in pricing.keys() {
            let (provider, family) = claude_view_core::parse_model_id(model_id);
            sqlx::query(
                r#"INSERT OR IGNORE INTO models (id, provider, family, updated_at)
                   VALUES (?, ?, ?, ?)"#,
            )
            .bind(model_id)
            .bind(provider)
            .bind(family)
            .bind(now)
            .execute(self.pool())
            .await?;
        }

        tracing::info!(
            models = pricing.len(),
            "Seeded models table from pricing JSON keys"
        );
        Ok(())
    }
}
