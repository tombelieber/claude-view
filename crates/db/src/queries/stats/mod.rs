// crates/db/src/queries/stats/mod.rs
// Read-side accessors for the Phase 2 `session_stats` table.
//
// PR 2.1 only ships the staleness-header query; PR 2.2 adds the writer
// (`upsert_session_stats`) and PR 3.x adds the full-row read for the
// /api/sessions cutover. Keeping this module minimal until then.

use crate::{Database, DbResult};
use sqlx::Row;

/// Staleness header for a single `session_stats` row.
///
/// Used by the indexer_v2 orchestrator (Phase 2 PR 2.2) to decide whether
/// the source JSONL has changed since the last index — if the on-disk
/// `(content_hash, size, inode, mid_hash)` matches and `parser_version` /
/// `stats_version` are current, the parse is skipped.
#[derive(Debug, Clone)]
pub struct StatsHeader {
    pub session_id: String,
    pub source_content_hash: Vec<u8>,
    pub source_size: i64,
    pub source_inode: Option<i64>,
    pub source_mid_hash: Option<Vec<u8>>,
    pub parser_version: i64,
    pub stats_version: i64,
    pub indexed_at: i64,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for StatsHeader {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            session_id: row.try_get("session_id")?,
            source_content_hash: row.try_get("source_content_hash")?,
            source_size: row.try_get("source_size")?,
            source_inode: row.try_get("source_inode")?,
            source_mid_hash: row.try_get("source_mid_hash")?,
            parser_version: row.try_get("parser_version")?,
            stats_version: row.try_get("stats_version")?,
            indexed_at: row.try_get("indexed_at")?,
        })
    }
}

impl Database {
    /// Fetch the staleness header for `session_id`, or `None` if the
    /// session has never been indexed by the Phase 2 writer.
    ///
    /// `Option<StatsHeader>` — `None` is the explicit "never indexed"
    /// signal that the indexer_v2 orchestrator interprets as "always
    /// re-parse."
    pub async fn get_stats_header(&self, session_id: &str) -> DbResult<Option<StatsHeader>> {
        let header = sqlx::query_as::<_, StatsHeader>(
            r#"SELECT session_id, source_content_hash, source_size, source_inode,
                      source_mid_hash, parser_version, stats_version, indexed_at
                 FROM session_stats
                WHERE session_id = ?"#,
        )
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(header)
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[tokio::test]
    async fn get_stats_header_returns_none_for_unknown_session() {
        let db = Database::new_in_memory().await.unwrap();
        let header = db.get_stats_header("does-not-exist").await.unwrap();
        assert!(
            header.is_none(),
            "unknown session must yield None, got {:?}",
            header
        );
    }

    #[tokio::test]
    async fn get_stats_header_round_trips_inserted_row() {
        let db = Database::new_in_memory().await.unwrap();

        // Direct INSERT — the proper writer ships in PR 2.2; this test only
        // pins the read path against a hand-written row that exercises every
        // header column (incl. nullable inode + mid_hash).
        sqlx::query(
            r#"INSERT INTO session_stats (
                   session_id, source_content_hash, source_size,
                   source_inode, source_mid_hash,
                   parser_version, stats_version, indexed_at
               ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind("sess-rt")
        .bind(vec![0xDEu8, 0xAD, 0xBEu8, 0xEFu8])
        .bind(4096_i64)
        .bind(Some(1234567_i64))
        .bind(Some(vec![0xCAu8, 0xFEu8]))
        .bind(2_i64)
        .bind(3_i64)
        .bind(1_715_000_000_i64)
        .execute(db.pool())
        .await
        .unwrap();

        let header = db
            .get_stats_header("sess-rt")
            .await
            .unwrap()
            .expect("inserted row must be readable");
        assert_eq!(header.session_id, "sess-rt");
        assert_eq!(header.source_content_hash, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(header.source_size, 4096);
        assert_eq!(header.source_inode, Some(1234567));
        assert_eq!(header.source_mid_hash, Some(vec![0xCA, 0xFE]));
        assert_eq!(header.parser_version, 2);
        assert_eq!(header.stats_version, 3);
        assert_eq!(header.indexed_at, 1_715_000_000);
    }

    #[tokio::test]
    async fn get_stats_header_handles_nullable_columns() {
        let db = Database::new_in_memory().await.unwrap();

        sqlx::query(
            r#"INSERT INTO session_stats (
                   session_id, source_content_hash, source_size,
                   parser_version, stats_version, indexed_at
               ) VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind("sess-null")
        .bind(vec![0x01u8])
        .bind(1_i64)
        .bind(1_i64)
        .bind(1_i64)
        .bind(0_i64)
        .execute(db.pool())
        .await
        .unwrap();

        let header = db.get_stats_header("sess-null").await.unwrap().unwrap();
        assert_eq!(header.source_inode, None);
        assert_eq!(header.source_mid_hash, None);
    }
}
