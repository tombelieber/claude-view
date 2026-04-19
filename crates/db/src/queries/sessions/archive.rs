// crates/db/src/queries/sessions/archive.rs
// Session archive/unarchive and stale-session cleanup operations.
//
// CQRS Phase 5 PR 5.2: every archive / unarchive / bulk variant writes
// a row to `session_action_log` inside the SAME transaction as the
// legacy `sessions.archived_at` UPDATE. Stage C's fold worker (PR 5.3)
// consumes the log into `session_flags` under LWW semantics; until then
// the log is an inert audit trail that costs ~1 row × ~60 bytes per
// mutation.

use crate::queries::action_log::insert_action_log_tx;
use crate::{Database, DbResult};
use chrono::Utc;

impl Database {
    /// Archive a session: set archived_at timestamp and log the action.
    ///
    /// The `sessions.archived_at` UPDATE and the `session_action_log`
    /// INSERT happen in the same transaction so a crash between them
    /// cannot leave the log out of sync with the flag column. Returns
    /// the file_path so the caller can move the file after commit.
    pub async fn archive_session(&self, session_id: &str) -> DbResult<Option<String>> {
        let now_rfc = Utc::now().to_rfc3339();
        let now_ms = Utc::now().timestamp_millis();

        let mut tx = self.pool().begin().await?;
        let file_path = sqlx::query_scalar::<_, String>(
            "UPDATE sessions SET archived_at = ?1 WHERE id = ?2 AND archived_at IS NULL RETURNING file_path",
        )
        .bind(&now_rfc)
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await?;

        if file_path.is_some() {
            insert_action_log_tx(&mut *tx, session_id, "archive", "{}", "user", now_ms).await?;
        }

        tx.commit().await?;
        Ok(file_path)
    }

    /// Unarchive a session: clear archived_at, update file_path, log the action.
    pub async fn unarchive_session(&self, session_id: &str, new_file_path: &str) -> DbResult<bool> {
        let now_ms = Utc::now().timestamp_millis();

        let mut tx = self.pool().begin().await?;
        let rows = sqlx::query(
            "UPDATE sessions SET archived_at = NULL, file_path = ?1 WHERE id = ?2 AND archived_at IS NOT NULL",
        )
        .bind(new_file_path)
        .bind(session_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        let changed = rows > 0;
        if changed {
            insert_action_log_tx(&mut *tx, session_id, "unarchive", "{}", "user", now_ms).await?;
        }

        tx.commit().await?;
        Ok(changed)
    }

    /// Archive multiple sessions in a single transaction.
    ///
    /// Every successful UPDATE is paired with a matching
    /// `session_action_log` row inside the same transaction. Returns
    /// vec of (session_id, file_path) for file moves.
    pub async fn archive_sessions_bulk(
        &self,
        session_ids: &[String],
    ) -> DbResult<Vec<(String, String)>> {
        let now_rfc = Utc::now().to_rfc3339();
        let now_ms = Utc::now().timestamp_millis();
        let mut tx = self.pool().begin().await?;
        let mut results = Vec::new();
        for id in session_ids {
            let result = sqlx::query_scalar::<_, String>(
                "UPDATE sessions SET archived_at = ?1 WHERE id = ?2 AND archived_at IS NULL RETURNING file_path",
            )
            .bind(&now_rfc)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(path) = result {
                insert_action_log_tx(&mut *tx, id, "archive", "{}", "user", now_ms).await?;
                results.push((id.clone(), path));
            }
        }
        tx.commit().await?;
        Ok(results)
    }

    /// Bulk unarchive: clear archived_at for multiple sessions, log each.
    pub async fn unarchive_sessions_bulk(
        &self,
        file_paths: &[(String, String)],
    ) -> DbResult<usize> {
        let now_ms = Utc::now().timestamp_millis();
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
            if rows > 0 {
                insert_action_log_tx(&mut *tx, id, "unarchive", "{}", "user", now_ms).await?;
                count += rows as usize;
            }
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
