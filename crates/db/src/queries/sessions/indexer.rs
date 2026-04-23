// crates/db/src/queries/sessions/indexer.rs
// Indexer state management: check/update file indexing status, deep-index tracking.

use crate::{Database, DbResult};
use chrono::Utc;
use std::collections::HashMap;

use super::super::IndexerEntry;

impl Database {
    /// Check if a file needs re-indexing by retrieving its indexer state.
    pub async fn get_indexer_state(&self, file_path: &str) -> DbResult<Option<IndexerEntry>> {
        let row: Option<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT file_path, file_size, modified_at, indexed_at FROM indexer_state WHERE file_path = ?1",
        )
        .bind(file_path)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(
            |(file_path, file_size, modified_at, indexed_at)| IndexerEntry {
                file_path,
                file_size,
                modified_at,
                indexed_at,
            },
        ))
    }

    /// Batch-load all indexer states into a HashMap keyed by file_path.
    ///
    /// This avoids the N+1 query pattern when diffing many files against the DB.
    pub async fn get_all_indexer_states(&self) -> DbResult<HashMap<String, IndexerEntry>> {
        let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT file_path, file_size, modified_at, indexed_at FROM indexer_state",
        )
        .fetch_all(self.pool())
        .await?;

        let map = rows
            .into_iter()
            .map(|(file_path, file_size, modified_at, indexed_at)| {
                let entry = IndexerEntry {
                    file_path: file_path.clone(),
                    file_size,
                    modified_at,
                    indexed_at,
                };
                (file_path, entry)
            })
            .collect();

        Ok(map)
    }

    /// Mark a file as indexed with the given size and modification time.
    pub async fn update_indexer_state(
        &self,
        file_path: &str,
        size: i64,
        mtime: i64,
    ) -> DbResult<()> {
        let indexed_at = Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO indexer_state (file_path, file_size, modified_at, indexed_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(file_path)
        .bind(size)
        .bind(mtime)
        .bind(indexed_at)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Get all sessions with their file paths and stored file metadata.
    ///
    /// Returns all sessions so the caller can decide which ones need re-indexing
    /// based on: (1) never deep-indexed, (2) stale parse version, (3) file changed
    /// since last index (size or mtime differs).
    ///
    /// Tuple: `(id, file_path, file_size_at_index, file_mtime_at_index, deep_indexed_at, parse_version, project, archived_at)`
    ///
    /// The `project` value is the effective project identity: `COALESCE(NULLIF(git_root, ''), project_id)`.
    /// This matches what the sidebar sends as the project filter, so search scope filters align.
    ///
    /// CQRS Phase 5.5a — `archived_at` now joins from `session_flags`
    /// (unix-ms INTEGER) and is converted back to RFC3339 via
    /// `strftime` so the existing callers (which only look at
    /// `.is_some()`) keep working.
    pub async fn get_sessions_needing_deep_index(
        &self,
    ) -> DbResult<
        Vec<(
            String,
            String,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            i32,
            String,
            Option<String>,
        )>,
    > {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, i32, String, Option<String>)> =
            sqlx::query_as(
                r#"SELECT s.id, s.file_path, s.file_size_at_index, s.file_mtime_at_index,
                          s.deep_indexed_at, s.parse_version,
                          COALESCE(NULLIF(s.git_root, ''), s.project_id, ''),
                          CASE
                            WHEN sf.archived_at IS NULL THEN NULL
                            ELSE strftime('%Y-%m-%dT%H:%M:%fZ', sf.archived_at / 1000.0, 'unixepoch')
                          END AS archived_at
                   FROM sessions s
                   LEFT JOIN session_flags sf ON sf.session_id = s.id
                   WHERE s.file_path IS NOT NULL AND s.file_path != ''"#,
            )
            .fetch_all(self.pool())
            .await?;
        Ok(rows)
    }

    /// Mark all sessions for re-indexing by clearing their deep_indexed_at timestamps.
    ///
    /// This forces the next deep index pass to reprocess all sessions.
    /// Used by the "Rebuild Index" feature in the Settings UI.
    ///
    /// Returns the number of sessions marked for re-indexing.
    pub async fn mark_all_sessions_for_reindex(&self) -> DbResult<u64> {
        // CQRS Phase 7.h.3c: dual-reset deep_indexed_at / parse_version on both tables.
        let result = sqlx::query(
            "UPDATE sessions SET deep_indexed_at = NULL, parse_version = 0 WHERE file_path IS NOT NULL AND file_path != ''",
        )
        .execute(self.pool())
        .await?;
        sqlx::query(
            "UPDATE session_stats SET deep_indexed_at = NULL, parse_version = 0 WHERE file_path IS NOT NULL AND file_path != ''",
        )
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }
}
