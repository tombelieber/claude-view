//! Index metadata CRUD operations.

use super::types::IndexMetadata;
use crate::{Database, DbResult};
use chrono::Utc;

impl Database {
    /// Update index metadata after a successful index operation.
    ///
    /// Only call this when indexing completes successfully.
    /// Do NOT call on failure — preserve the last successful timestamp.
    pub async fn update_index_metadata_on_success(
        &self,
        duration_ms: i64,
        sessions_indexed: i64,
        projects_indexed: i64,
    ) -> DbResult<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE index_metadata SET
                last_indexed_at = ?1,
                last_index_duration_ms = ?2,
                sessions_indexed = ?3,
                projects_indexed = ?4,
                updated_at = ?5
            WHERE id = 1
            "#,
        )
        .bind(now)
        .bind(duration_ms)
        .bind(sessions_indexed)
        .bind(projects_indexed)
        .bind(now)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Update git sync metadata after a successful git sync operation.
    ///
    /// Only call this when git sync completes successfully.
    /// Do NOT call on failure — preserve the last successful timestamp.
    pub async fn update_git_sync_metadata_on_success(
        &self,
        commits_found: i64,
        links_created: i64,
    ) -> DbResult<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE index_metadata SET
                last_git_sync_at = ?1,
                commits_found = ?2,
                links_created = ?3,
                updated_at = ?4
            WHERE id = 1
            "#,
        )
        .bind(now)
        .bind(commits_found)
        .bind(links_created)
        .bind(now)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Get the current index metadata.
    pub async fn get_index_metadata(&self) -> DbResult<IndexMetadata> {
        #[allow(clippy::type_complexity)]
        let row: (
            Option<i64>,
            Option<i64>,
            i64,
            i64,
            Option<i64>,
            i64,
            i64,
            i64,
            i64,
        ) = sqlx::query_as(
            r#"
            SELECT
                last_indexed_at,
                last_index_duration_ms,
                sessions_indexed,
                projects_indexed,
                last_git_sync_at,
                commits_found,
                links_created,
                updated_at,
                git_sync_interval_secs
            FROM index_metadata
            WHERE id = 1
            "#,
        )
        .fetch_one(self.pool())
        .await?;

        Ok(IndexMetadata {
            last_indexed_at: row.0,
            last_index_duration_ms: row.1,
            sessions_indexed: row.2,
            projects_indexed: row.3,
            last_git_sync_at: row.4,
            commits_found: row.5,
            links_created: row.6,
            updated_at: row.7,
            git_sync_interval_secs: row.8,
        })
    }

    /// Get the git sync interval in seconds.
    pub async fn get_git_sync_interval(&self) -> DbResult<u64> {
        let (interval,): (i64,) =
            sqlx::query_as("SELECT git_sync_interval_secs FROM index_metadata WHERE id = 1")
                .fetch_one(self.pool())
                .await?;
        Ok(interval as u64)
    }

    /// Set the git sync interval in seconds.
    pub async fn set_git_sync_interval(&self, seconds: u64) -> DbResult<()> {
        sqlx::query("UPDATE index_metadata SET git_sync_interval_secs = ?1 WHERE id = 1")
            .bind(seconds as i64)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Get the stored registry fingerprint (if any).
    pub async fn get_registry_hash(&self) -> DbResult<Option<String>> {
        let (hash,): (Option<String>,) =
            sqlx::query_as("SELECT registry_hash FROM index_metadata WHERE id = 1")
                .fetch_one(self.pool())
                .await?;
        Ok(hash)
    }

    /// Store the registry fingerprint after successful indexing.
    pub async fn set_registry_hash(&self, hash: &str) -> DbResult<()> {
        sqlx::query("UPDATE index_metadata SET registry_hash = ?1 WHERE id = 1")
            .bind(hash)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
