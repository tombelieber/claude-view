// crates/db/src/queries/sessions/archive.rs
// Session archive/unarchive and stale-session cleanup operations.
//
// CQRS Phase 5 PR 5.6a (D.2): the action log is now the SOLE writer for
// archive/unarchive. The Phase 5.3 fold worker consumes the log and
// upserts `session_flags.archived_at`, which Phase 5.5 made the
// authoritative source for every reader. The legacy
// `sessions.archived_at` column is dropped by migration 85.
//
// Idempotence is preserved by reading `session_flags.archived_at`
// before emitting each action:
//   - archive is a no-op if `sf.archived_at IS NOT NULL` already.
//   - unarchive is a no-op if no row in session_flags or
//     `sf.archived_at IS NULL`.
//
// File-path plumbing now lives on `session_stats.file_path`; migration 91
// drops the legacy `sessions` table.

use crate::queries::action_log::insert_action_log_tx;
use crate::{Database, DbResult};
use chrono::Utc;

impl Database {
    /// Archive a session: emit an "archive" action-log entry if the
    /// session exists and is not already archived in the shadow. Returns
    /// the file_path so the caller can move the file after commit.
    pub async fn archive_session(&self, session_id: &str) -> DbResult<Option<String>> {
        let now_ms = Utc::now().timestamp_millis();

        let mut tx = self.pool().begin().await?;

        // Resolve the file_path + current shadow archived_at inside the
        // transaction. `LEFT JOIN session_flags` — a session with no
        // shadow row has sf.archived_at = NULL → archive applies.
        let row: Option<(String, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT s.file_path, sf.archived_at
            FROM session_stats s
            LEFT JOIN session_flags sf ON sf.session_id = s.session_id
            WHERE s.session_id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await?;

        let file_path = match row {
            Some((path, archived_at_ms)) if archived_at_ms.is_none() => {
                insert_action_log_tx(&mut *tx, session_id, "archive", "{}", "user", now_ms).await?;
                Some(path)
            }
            // Session missing or already archived → idempotent no-op.
            _ => None,
        };

        tx.commit().await?;
        Ok(file_path)
    }

    /// Unarchive a session: update `file_path`, then emit an "unarchive" action-log entry iff the
    /// session was previously archived in the shadow.
    pub async fn unarchive_session(&self, session_id: &str, new_file_path: &str) -> DbResult<bool> {
        let now_ms = Utc::now().timestamp_millis();

        let mut tx = self.pool().begin().await?;

        // Check shadow state for current archive status.
        let archived_at_ms: Option<Option<i64>> =
            sqlx::query_scalar("SELECT archived_at FROM session_flags WHERE session_id = ?1")
                .bind(session_id)
                .fetch_optional(&mut *tx)
                .await?;
        let is_currently_archived = matches!(archived_at_ms, Some(Some(_)));

        if !is_currently_archived {
            tx.commit().await?;
            return Ok(false);
        }

        sqlx::query("UPDATE session_stats SET file_path = ?1 WHERE session_id = ?2")
            .bind(new_file_path)
            .bind(session_id)
            .execute(&mut *tx)
            .await?;

        insert_action_log_tx(&mut *tx, session_id, "unarchive", "{}", "user", now_ms).await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Archive multiple sessions in a single transaction.
    ///
    /// Emits an "archive" action-log row for each session that exists
    /// and is not already archived. Returns vec of (session_id,
    /// file_path) for file moves.
    pub async fn archive_sessions_bulk(
        &self,
        session_ids: &[String],
    ) -> DbResult<Vec<(String, String)>> {
        let now_ms = Utc::now().timestamp_millis();
        let mut tx = self.pool().begin().await?;
        let mut results = Vec::new();
        for id in session_ids {
            let row: Option<(String, Option<i64>)> = sqlx::query_as(
                r#"
                SELECT s.file_path, sf.archived_at
                FROM session_stats s
                LEFT JOIN session_flags sf ON sf.session_id = s.session_id
                WHERE s.session_id = ?1
                "#,
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some((path, archived_at_ms)) = row {
                if archived_at_ms.is_none() {
                    insert_action_log_tx(&mut *tx, id, "archive", "{}", "user", now_ms).await?;
                    results.push((id.clone(), path));
                }
            }
        }
        tx.commit().await?;
        Ok(results)
    }

    /// Bulk unarchive: update `file_path` and emit an "unarchive"
    /// action-log entry for each previously-archived session.
    pub async fn unarchive_sessions_bulk(
        &self,
        file_paths: &[(String, String)],
    ) -> DbResult<usize> {
        let now_ms = Utc::now().timestamp_millis();
        let mut tx = self.pool().begin().await?;
        let mut count = 0usize;
        for (id, new_path) in file_paths {
            let archived_at_ms: Option<Option<i64>> =
                sqlx::query_scalar("SELECT archived_at FROM session_flags WHERE session_id = ?1")
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if !matches!(archived_at_ms, Some(Some(_))) {
                continue;
            }

            sqlx::query("UPDATE session_stats SET file_path = ?1 WHERE session_id = ?2")
                .bind(new_path)
                .bind(id)
                .execute(&mut *tx)
                .await?;

            insert_action_log_tx(&mut *tx, id, "unarchive", "{}", "user", now_ms).await?;
            count += 1;
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
            let result = sqlx::query("DELETE FROM session_stats")
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

        let delete_session_stats_sql = format!(
            "DELETE FROM session_stats WHERE file_path NOT IN ({})",
            in_clause
        );
        let delete_indexer_sql = format!(
            "DELETE FROM indexer_state WHERE file_path NOT IN ({})",
            in_clause
        );

        let mut query = sqlx::query(&delete_session_stats_sql);
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
