//! Preset CRUD queries for Claude Code config profiles.

use crate::{Database, DbResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Summary view of a preset (list endpoint).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PresetSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_vanilla: bool,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// Full preset with all configuration fields.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_vanilla: bool,
    pub settings_json: String,
    pub settings_local_json: Option<String>,
    pub claude_md: Option<String>,
    pub keybindings_json: Option<String>,
    pub skills: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// Active preset state (single-row table).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PresetState {
    pub active_preset_id: Option<String>,
    pub previous_snapshot: Option<String>,
}

/// Response for the presets list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PresetsListResponse {
    pub presets: Vec<PresetSummary>,
    pub active_preset_id: Option<String>,
    pub rollback_available: bool,
}

impl Database {
    /// List all presets (summary view, ordered by name).
    pub async fn list_presets(&self) -> DbResult<Vec<PresetSummary>> {
        let rows: Vec<(String, String, Option<String>, bool, i64, i64)> = sqlx::query_as(
            "SELECT id, name, description, is_vanilla, created_at, updated_at FROM presets ORDER BY name",
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PresetSummary {
                id: r.0,
                name: r.1,
                description: r.2,
                is_vanilla: r.3,
                created_at: r.4,
                updated_at: r.5,
            })
            .collect())
    }

    /// Get a single preset by ID (full view).
    pub async fn get_preset(&self, id: &str) -> DbResult<Preset> {
        let row: (
            String,
            String,
            Option<String>,
            bool,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            i64,
        ) = sqlx::query_as(
            "SELECT id, name, description, is_vanilla, settings_json, settings_local_json, claude_md, keybindings_json, skills, created_at, updated_at FROM presets WHERE id = ?",
        )
        .bind(id)
        .fetch_one(self.pool())
        .await?;

        Ok(Preset {
            id: row.0,
            name: row.1,
            description: row.2,
            is_vanilla: row.3,
            settings_json: row.4,
            settings_local_json: row.5,
            claude_md: row.6,
            keybindings_json: row.7,
            skills: row.8,
            created_at: row.9,
            updated_at: row.10,
        })
    }

    /// Create a new preset.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_preset(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        settings_json: &str,
        settings_local_json: Option<&str>,
        claude_md: Option<&str>,
        keybindings_json: Option<&str>,
        skills: Option<&str>,
    ) -> DbResult<Preset> {
        sqlx::query(
            "INSERT INTO presets (id, name, description, settings_json, settings_local_json, claude_md, keybindings_json, skills) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(settings_json)
        .bind(settings_local_json)
        .bind(claude_md)
        .bind(keybindings_json)
        .bind(skills)
        .execute(self.pool())
        .await?;

        self.get_preset(id).await
    }

    /// Update preset name and/or description.
    pub async fn update_preset_metadata(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> DbResult<Preset> {
        if let Some(n) = name {
            sqlx::query(
                "UPDATE presets SET name = ?, updated_at = strftime('%s', 'now') WHERE id = ?",
            )
            .bind(n)
            .bind(id)
            .execute(self.pool())
            .await?;
        }
        if let Some(d) = description {
            sqlx::query("UPDATE presets SET description = ?, updated_at = strftime('%s', 'now') WHERE id = ?")
                .bind(d)
                .bind(id)
                .execute(self.pool())
                .await?;
        }
        self.get_preset(id).await
    }

    /// Delete a preset by ID.
    pub async fn delete_preset(&self, id: &str) -> DbResult<()> {
        sqlx::query("DELETE FROM presets WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Get the current preset state (active preset + previous snapshot).
    pub async fn get_preset_state(&self) -> DbResult<PresetState> {
        let row: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT active_preset_id, previous_snapshot FROM preset_state WHERE id = 1",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(PresetState {
            active_preset_id: row.0,
            previous_snapshot: row.1,
        })
    }

    /// Set the active preset and store a previous snapshot for rollback.
    pub async fn set_active_preset(
        &self,
        preset_id: &str,
        previous_snapshot: &str,
    ) -> DbResult<()> {
        sqlx::query(
            "UPDATE preset_state SET active_preset_id = ?, previous_snapshot = ? WHERE id = 1",
        )
        .bind(preset_id)
        .bind(previous_snapshot)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Clear preset state (deactivate preset and remove snapshot).
    pub async fn clear_preset_state(&self) -> DbResult<()> {
        sqlx::query("UPDATE preset_state SET active_preset_id = NULL, previous_snapshot = NULL WHERE id = 1")
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Set the active preset without storing a previous snapshot.
    pub async fn set_active_preset_only(&self, preset_id: &str) -> DbResult<()> {
        sqlx::query("UPDATE preset_state SET active_preset_id = ? WHERE id = 1")
            .bind(preset_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    const VANILLA_ID: &str = "00000000-0000-0000-0000-000000000000";

    #[tokio::test]
    async fn test_list_presets_returns_vanilla() {
        let db = Database::new_in_memory().await.unwrap();
        let presets = db.list_presets().await.unwrap();
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].id, VANILLA_ID);
        assert_eq!(presets[0].name, "Vanilla");
        assert!(presets[0].is_vanilla);
    }

    #[tokio::test]
    async fn test_create_and_get_preset() {
        let db = Database::new_in_memory().await.unwrap();
        let id = "11111111-1111-1111-1111-111111111111";
        let preset = db
            .create_preset(
                id,
                "My Preset",
                Some("A test preset"),
                r#"{"theme":"dark"}"#,
                Some(r#"{"local":true}"#),
                Some("# Claude MD"),
                Some(r#"{"key":"ctrl+s"}"#),
                Some("skill1,skill2"),
            )
            .await
            .unwrap();
        assert_eq!(preset.id, id);
        assert_eq!(preset.name, "My Preset");
        assert_eq!(preset.description.as_deref(), Some("A test preset"));
        assert!(!preset.is_vanilla);
        assert_eq!(preset.settings_json, r#"{"theme":"dark"}"#);
        assert_eq!(
            preset.settings_local_json.as_deref(),
            Some(r#"{"local":true}"#)
        );
        assert_eq!(preset.claude_md.as_deref(), Some("# Claude MD"));
        assert_eq!(
            preset.keybindings_json.as_deref(),
            Some(r#"{"key":"ctrl+s"}"#)
        );
        assert_eq!(preset.skills.as_deref(), Some("skill1,skill2"));

        // Verify it shows up in list
        let presets = db.list_presets().await.unwrap();
        assert_eq!(presets.len(), 2);

        // Verify get_preset returns the same data
        let fetched = db.get_preset(id).await.unwrap();
        assert_eq!(fetched.name, "My Preset");
    }

    #[tokio::test]
    async fn test_update_preset_metadata() {
        let db = Database::new_in_memory().await.unwrap();
        let id = "22222222-2222-2222-2222-222222222222";
        db.create_preset(id, "Original", None, "{}", None, None, None, None)
            .await
            .unwrap();

        let updated = db
            .update_preset_metadata(id, Some("Renamed"), Some("New desc"))
            .await
            .unwrap();
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.description.as_deref(), Some("New desc"));
    }

    #[tokio::test]
    async fn test_delete_preset() {
        let db = Database::new_in_memory().await.unwrap();
        let id = "33333333-3333-3333-3333-333333333333";
        db.create_preset(id, "ToDelete", None, "{}", None, None, None, None)
            .await
            .unwrap();

        let before = db.list_presets().await.unwrap();
        assert_eq!(before.len(), 2);

        db.delete_preset(id).await.unwrap();

        let after = db.list_presets().await.unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, "00000000-0000-0000-0000-000000000000");
    }

    #[tokio::test]
    async fn test_preset_state_defaults() {
        let db = Database::new_in_memory().await.unwrap();
        let state = db.get_preset_state().await.unwrap();
        assert!(state.active_preset_id.is_none());
        assert!(state.previous_snapshot.is_none());
    }

    #[tokio::test]
    async fn test_set_and_clear_preset_state() {
        let db = Database::new_in_memory().await.unwrap();

        // Set active preset with snapshot
        db.set_active_preset(
            "00000000-0000-0000-0000-000000000000",
            r#"{"old":"config"}"#,
        )
        .await
        .unwrap();

        let state = db.get_preset_state().await.unwrap();
        assert_eq!(
            state.active_preset_id.as_deref(),
            Some("00000000-0000-0000-0000-000000000000")
        );
        assert_eq!(
            state.previous_snapshot.as_deref(),
            Some(r#"{"old":"config"}"#)
        );

        // Test set_active_preset_only
        let id2 = "44444444-4444-4444-4444-444444444444";
        db.create_preset(id2, "Another", None, "{}", None, None, None, None)
            .await
            .unwrap();
        db.set_active_preset_only(id2).await.unwrap();
        let state2 = db.get_preset_state().await.unwrap();
        assert_eq!(state2.active_preset_id.as_deref(), Some(id2));
        // previous_snapshot should still be set from before
        assert_eq!(
            state2.previous_snapshot.as_deref(),
            Some(r#"{"old":"config"}"#)
        );

        // Clear
        db.clear_preset_state().await.unwrap();
        let state3 = db.get_preset_state().await.unwrap();
        assert!(state3.active_preset_id.is_none());
        assert!(state3.previous_snapshot.is_none());
    }
}
