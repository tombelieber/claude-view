//! App settings CRUD queries.

use crate::{Database, DbResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Application settings (single-row table).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub llm_model: String,
    #[ts(type = "number")]
    pub llm_timeout_secs: i64,
}

impl Database {
    /// Read current app settings.
    pub async fn get_app_settings(&self) -> DbResult<AppSettings> {
        let row: (String, i64) = sqlx::query_as(
            "SELECT llm_model, llm_timeout_secs FROM app_settings WHERE id = 1"
        )
        .fetch_one(self.pool())
        .await?;
        Ok(AppSettings {
            llm_model: row.0,
            llm_timeout_secs: row.1,
        })
    }

    /// Update app settings (partial — only provided fields are changed).
    pub async fn update_app_settings(
        &self,
        model: Option<&str>,
        timeout_secs: Option<i64>,
    ) -> DbResult<AppSettings> {
        if let Some(m) = model {
            sqlx::query("UPDATE app_settings SET llm_model = ? WHERE id = 1")
                .bind(m)
                .execute(self.pool())
                .await?;
        }
        if let Some(t) = timeout_secs {
            sqlx::query("UPDATE app_settings SET llm_timeout_secs = ? WHERE id = 1")
                .bind(t)
                .execute(self.pool())
                .await?;
        }
        self.get_app_settings().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[tokio::test]
    async fn test_get_default_settings() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.get_app_settings().await.unwrap();
        assert_eq!(settings.llm_model, "haiku");
        assert_eq!(settings.llm_timeout_secs, 120);
    }

    #[tokio::test]
    async fn test_update_model() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.update_app_settings(Some("sonnet"), None).await.unwrap();
        assert_eq!(settings.llm_model, "sonnet");
        assert_eq!(settings.llm_timeout_secs, 120);

        let read_back = db.get_app_settings().await.unwrap();
        assert_eq!(read_back.llm_model, "sonnet");
    }

    #[tokio::test]
    async fn test_update_timeout() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.update_app_settings(None, Some(90)).await.unwrap();
        assert_eq!(settings.llm_model, "haiku");
        assert_eq!(settings.llm_timeout_secs, 90);
    }

    #[tokio::test]
    async fn test_update_both() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.update_app_settings(Some("opus"), Some(180)).await.unwrap();
        assert_eq!(settings.llm_model, "opus");
        assert_eq!(settings.llm_timeout_secs, 180);
    }
}
