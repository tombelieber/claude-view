// crates/db/src/queries/sessions/archive.rs
// Session archive/unarchive and stale-session cleanup operations.

use crate::{Database, DbResult};
use chrono::Utc;

impl Database {
    /// Archive a session: set archived_at timestamp.
    /// Returns the file_path so the caller can move the file.
    pub async fn archive_session(&self, session_id: &str) -> DbResult<Option<String>> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query_scalar::<_, String>(
            "UPDATE sessions SET archived_at = ?1 WHERE id = ?2 AND archived_at IS NULL RETURNING file_path",
        )
        .bind(&now)
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(result)
    }

    /// Unarchive a session: clear archived_at, update file_path to new location.
    pub async fn unarchive_session(&self, session_id: &str, new_file_path: &str) -> DbResult<bool> {
        let rows = sqlx::query(
            "UPDATE sessions SET archived_at = NULL, file_path = ?1 WHERE id = ?2 AND archived_at IS NOT NULL",
        )
        .bind(new_file_path)
        .bind(session_id)
        .execute(self.pool())
        .await?
        .rows_affected();
        Ok(rows > 0)
    }

    /// Archive multiple sessions in a single transaction.
    /// Returns vec of (session_id, file_path) for file moves.
    pub async fn archive_sessions_bulk(
        &self,
        session_ids: &[String],
    ) -> DbResult<Vec<(String, String)>> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool().begin().await?;
        let mut results = Vec::new();
        for id in session_ids {
            let result = sqlx::query_scalar::<_, String>(
                "UPDATE sessions SET archived_at = ?1 WHERE id = ?2 AND archived_at IS NULL RETURNING file_path",
            )
            .bind(&now)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(path) = result {
                results.push((id.clone(), path));
            }
        }
        tx.commit().await?;
        Ok(results)
    }

    /// Bulk unarchive: clear archived_at for multiple sessions.
    pub async fn unarchive_sessions_bulk(
        &self,
        file_paths: &[(String, String)],
    ) -> DbResult<usize> {
        let mut tx = self.pool().begin().await?;
        let mut count = 0usize;
        for (id, new_path) in file_paths {
            let rows = sqlx::query(
                "UPDATE sessions SET archived_at = NULL, file_path = ?1 WHERE id = ?2 AND archived_at IS NOT NULL",
            )
            .bind(new_path)
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            count += rows as usize;
        }
        tx.commit().await?;
        Ok(count)
    }

    /// Remove sessions whose file_path is NOT in the given list of valid paths.
    /// Also cleans up corresponding indexer_state entries.
    /// Both deletes run in a transaction for consistency.
    pub async fn remove_stale_sessions(&self, valid_paths: &[String]) -> DbResult<u64> {
        let mut tx = self.pool().begin().await?;

        if valid_paths.is_empty() {
            let result = sqlx::query("DELETE FROM sessions")
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM indexer_state")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(result.rows_affected());
        }

        // Build placeholders for the IN clause
        let placeholders: Vec<String> =
            (1..=valid_paths.len()).map(|i| format!("?{}", i)).collect();
        let in_clause = placeholders.join(", ");

        let delete_sessions_sql = format!(
            "DELETE FROM sessions WHERE file_path NOT IN ({})",
            in_clause
        );
        let delete_indexer_sql = format!(
            "DELETE FROM indexer_state WHERE file_path NOT IN ({})",
            in_clause
        );

        let mut query = sqlx::query(&delete_sessions_sql);
        for path in valid_paths {
            query = query.bind(path);
        }
        let result = query.execute(&mut *tx).await?;

        let mut query = sqlx::query(&delete_indexer_sql);
        for path in valid_paths {
            query = query.bind(path);
        }
        query.execute(&mut *tx).await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }
}
