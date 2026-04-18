// crates/db/src/queries/stats/mod.rs
// Read-side accessors for the Phase 2 `session_stats` table.
//
// PR 2.1 shipped `get_stats_header` (staleness header). PR 2.2 added the
// writer. Phase 3 PR 3.a adds the catalog-shape read functions consumed
// by `SessionCatalogAdapter`:
//   - `list_session_catalog_entries` — per-project or all, sort + limit
//   - `get_session_catalog_entry`   — single session lookup by id
//   - `list_projects_with_counts`    — distinct projects + counts for
//                                      `/api/projects`
//
// Every row returned here is shaped like the legacy
// `claude_view_core::session_catalog::CatalogRow` so swapping the
// read-side source is a type-alias flip.

use std::collections::HashMap;
use std::path::PathBuf;

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

// ---------------------------------------------------------------------------
// Phase 3 PR 3.a — catalog-shape read functions
// ---------------------------------------------------------------------------

/// Sort direction for `list_session_catalog_entries`. Mirrors
/// `claude_view_core::session_catalog::Sort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSort {
    /// Most-recent first, using COALESCE(last_message_at, source_mtime, 0).
    LastTsDesc,
    /// Oldest first, same fallback chain.
    LastTsAsc,
}

/// Filter applied by `list_session_catalog_entries`. Field subset matches
/// `claude_view_core::session_catalog::Filter`.
#[derive(Debug, Default, Clone)]
pub struct CatalogFilter {
    pub project_id: Option<String>,
    pub min_last_ts: Option<i64>,
    pub max_last_ts: Option<i64>,
}

/// One row of the catalog view over `session_stats`. Shape is a strict
/// superset of `claude_view_core::session_catalog::CatalogRow` so the
/// adapter can construct the legacy struct directly from a row.
///
/// `None` values in `project_id` / `file_path` / `source_mtime` can only
/// appear for rows indexed before migration 66 (Phase 3 PR 3.a). The
/// adapter treats such rows as filesystem-opaque and falls back to the
/// in-memory `SessionCatalog` for that session until the next reindex
/// fills them in. Phase 7 drift detector alerts if any row stays NULL
/// past the soak window.
#[derive(Debug, Clone)]
pub struct StatsCatalogRow {
    pub session_id: String,
    pub project_id: Option<String>,
    pub file_path: Option<PathBuf>,
    pub is_compressed: bool,
    pub source_size: i64,
    pub source_mtime: Option<i64>,
    pub first_message_at: Option<i64>,
    pub last_message_at: Option<i64>,
}

impl StatsCatalogRow {
    /// Sort key, mirroring `CatalogRow::sort_ts`: prefer `last_message_at`
    /// (the parsed timestamp of the last message), fall back to
    /// `source_mtime` (filesystem mtime), then 0.
    pub fn sort_ts(&self) -> i64 {
        self.last_message_at.or(self.source_mtime).unwrap_or(0)
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for StatsCatalogRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        let file_path_str: Option<String> = row.try_get("file_path")?;
        let is_compressed_int: i64 = row.try_get("is_compressed")?;
        Ok(Self {
            session_id: row.try_get("session_id")?,
            project_id: row.try_get("project_id")?,
            file_path: file_path_str.map(PathBuf::from),
            is_compressed: is_compressed_int != 0,
            source_size: row.try_get("source_size")?,
            source_mtime: row.try_get("source_mtime")?,
            first_message_at: row.try_get("first_message_at")?,
            last_message_at: row.try_get("last_message_at")?,
        })
    }
}

impl Database {
    /// Fetch one catalog row by session id, or `None` if the session has
    /// never been indexed (first fsnotify event hasn't landed yet).
    pub async fn get_session_catalog_entry(
        &self,
        session_id: &str,
    ) -> DbResult<Option<StatsCatalogRow>> {
        let row = sqlx::query_as::<_, StatsCatalogRow>(
            r#"SELECT session_id, project_id, file_path, is_compressed,
                      source_size, source_mtime, first_message_at, last_message_at
                 FROM session_stats
                WHERE session_id = ?"#,
        )
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row)
    }

    /// List catalog rows matching `filter`, sorted, limited.
    ///
    /// Sort key is `COALESCE(last_message_at, source_mtime, 0)` because
    /// short sessions with no parsed messages still have a valid fs mtime
    /// and the legacy catalog used mtime as a fallback. Keeping the
    /// fallback matches the `SessionCatalog::list` behavior.
    ///
    /// Filter semantics when `project_id` is `None` → all rows. When
    /// `min_last_ts` / `max_last_ts` are set they're compared against the
    /// same COALESCE expression used for sorting, so `min_last_ts` filter
    /// + `last_message_at` NULL doesn't accidentally drop a row whose
    ///   mtime would satisfy the filter.
    pub async fn list_session_catalog_entries(
        &self,
        filter: &CatalogFilter,
        sort: CatalogSort,
        limit: i64,
    ) -> DbResult<Vec<StatsCatalogRow>> {
        let direction = match sort {
            CatalogSort::LastTsDesc => "DESC",
            CatalogSort::LastTsAsc => "ASC",
        };

        // Dynamic SQL is unavoidable for optional filters + variable
        // ORDER BY direction. Everything substituted is a fixed string
        // from our own enum/struct — no user-supplied content.
        let sql = format!(
            r#"SELECT session_id, project_id, file_path, is_compressed,
                      source_size, source_mtime, first_message_at, last_message_at
                 FROM session_stats
                WHERE (?1 IS NULL OR project_id = ?1)
                  AND (?2 IS NULL OR COALESCE(last_message_at, source_mtime, 0) >= ?2)
                  AND (?3 IS NULL OR COALESCE(last_message_at, source_mtime, 0) <= ?3)
                ORDER BY COALESCE(last_message_at, source_mtime, 0) {direction}
                LIMIT ?4"#
        );

        let rows = sqlx::query_as::<_, StatsCatalogRow>(&sql)
            .bind(filter.project_id.as_deref())
            .bind(filter.min_last_ts)
            .bind(filter.max_last_ts)
            .bind(limit)
            .fetch_all(self.pool())
            .await?;
        Ok(rows)
    }

    /// Return distinct `project_id` values with a session count for each.
    ///
    /// Rows with `project_id IS NULL` (never-reindexed pre-migration-66
    /// rows) are excluded — the adapter falls back to the in-memory
    /// catalog for the full projects list when it sees gaps.
    pub async fn list_projects_with_counts(&self) -> DbResult<HashMap<String, usize>> {
        let rows = sqlx::query(
            r#"SELECT project_id, COUNT(*) as session_count
                 FROM session_stats
                WHERE project_id IS NOT NULL
                GROUP BY project_id"#,
        )
        .fetch_all(self.pool())
        .await?;

        let mut out: HashMap<String, usize> = HashMap::with_capacity(rows.len());
        for row in rows {
            let pid: String = row.try_get("project_id")?;
            let count: i64 = row.try_get("session_count")?;
            out.insert(pid, count as usize);
        }
        Ok(out)
    }

    /// Return distinct project_id values with a "last activity" timestamp
    /// (max of last_message_at, falling back to source_mtime).
    ///
    /// Powers `/api/projects` ordering. Excludes NULL project_id rows
    /// (same rationale as `list_projects_with_counts`).
    pub async fn list_projects_with_last_activity(&self) -> DbResult<HashMap<String, Option<i64>>> {
        let rows = sqlx::query(
            r#"SELECT project_id,
                      MAX(COALESCE(last_message_at, source_mtime, 0)) as last_activity
                 FROM session_stats
                WHERE project_id IS NOT NULL
                GROUP BY project_id"#,
        )
        .fetch_all(self.pool())
        .await?;

        let mut out: HashMap<String, Option<i64>> = HashMap::with_capacity(rows.len());
        for row in rows {
            let pid: String = row.try_get("project_id")?;
            let last: i64 = row.try_get("last_activity")?;
            out.insert(pid, if last > 0 { Some(last) } else { None });
        }
        Ok(out)
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

    // -----------------------------------------------------------------
    // Phase 3 PR 3.a — catalog-shape read tests
    // -----------------------------------------------------------------

    use super::{CatalogFilter, CatalogSort};

    /// Helper: seed one session_stats row with filesystem-mirror columns
    /// populated. Mirrors what the indexer_v2 writer produces so the read
    /// functions see realistic data without pulling the full writer
    /// machinery into this test module.
    async fn seed_catalog_row(
        db: &Database,
        session_id: &str,
        project_id: Option<&str>,
        file_path: Option<&str>,
        is_compressed: bool,
        last_message_at: Option<i64>,
        source_mtime: Option<i64>,
    ) {
        sqlx::query(
            r#"INSERT INTO session_stats (
                   session_id, source_content_hash, source_size,
                   parser_version, stats_version, indexed_at,
                   last_message_at,
                   project_id, file_path, is_compressed, source_mtime
               ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(session_id)
        .bind(vec![0x01u8])
        .bind(1_i64)
        .bind(1_i64)
        .bind(1_i64)
        .bind(1_i64)
        .bind(last_message_at)
        .bind(project_id)
        .bind(file_path)
        .bind(if is_compressed { 1_i64 } else { 0_i64 })
        .bind(source_mtime)
        .execute(db.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn get_session_catalog_entry_returns_none_for_unknown() {
        let db = Database::new_in_memory().await.unwrap();
        let row = db.get_session_catalog_entry("nope").await.unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn get_session_catalog_entry_round_trips() {
        let db = Database::new_in_memory().await.unwrap();
        seed_catalog_row(
            &db,
            "sess-rt",
            Some("proj-a"),
            Some("/home/user/.claude/projects/proj-a/sess-rt.jsonl"),
            false,
            Some(1_800_000_000),
            Some(1_799_999_000),
        )
        .await;

        let row = db
            .get_session_catalog_entry("sess-rt")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.session_id, "sess-rt");
        assert_eq!(row.project_id.as_deref(), Some("proj-a"));
        assert_eq!(
            row.file_path.as_deref().and_then(|p| p.to_str()),
            Some("/home/user/.claude/projects/proj-a/sess-rt.jsonl"),
        );
        assert!(!row.is_compressed);
        assert_eq!(row.last_message_at, Some(1_800_000_000));
        assert_eq!(row.source_mtime, Some(1_799_999_000));
        assert_eq!(row.sort_ts(), 1_800_000_000, "prefer last_message_at");
    }

    #[tokio::test]
    async fn catalog_row_sort_ts_falls_back_to_mtime() {
        let db = Database::new_in_memory().await.unwrap();
        seed_catalog_row(
            &db,
            "sess-mtime",
            Some("proj-a"),
            Some("/tmp/sess-mtime.jsonl"),
            false,
            None, // last_message_at NULL
            Some(42),
        )
        .await;
        let row = db
            .get_session_catalog_entry("sess-mtime")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.sort_ts(), 42, "fallback to source_mtime");
    }

    #[tokio::test]
    async fn list_session_catalog_entries_filters_and_sorts_desc() {
        let db = Database::new_in_memory().await.unwrap();
        seed_catalog_row(&db, "s1", Some("p1"), Some("/t/1"), false, Some(100), None).await;
        seed_catalog_row(&db, "s2", Some("p1"), Some("/t/2"), false, Some(300), None).await;
        seed_catalog_row(&db, "s3", Some("p2"), Some("/t/3"), false, Some(200), None).await;

        let rows = db
            .list_session_catalog_entries(
                &CatalogFilter {
                    project_id: Some("p1".into()),
                    ..Default::default()
                },
                CatalogSort::LastTsDesc,
                10,
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session_id, "s2", "most-recent first");
        assert_eq!(rows[1].session_id, "s1");
    }

    #[tokio::test]
    async fn list_session_catalog_entries_honors_limit() {
        let db = Database::new_in_memory().await.unwrap();
        for i in 0..10 {
            seed_catalog_row(
                &db,
                &format!("s{i}"),
                Some("p"),
                Some(&format!("/t/{i}")),
                false,
                Some(1000 + i),
                None,
            )
            .await;
        }

        let rows = db
            .list_session_catalog_entries(&CatalogFilter::default(), CatalogSort::LastTsDesc, 3)
            .await
            .unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[tokio::test]
    async fn list_projects_with_counts_skips_null_project_id() {
        let db = Database::new_in_memory().await.unwrap();
        seed_catalog_row(&db, "s1", Some("p1"), Some("/1"), false, None, None).await;
        seed_catalog_row(&db, "s2", Some("p1"), Some("/2"), false, None, None).await;
        seed_catalog_row(&db, "s3", Some("p2"), Some("/3"), false, None, None).await;
        seed_catalog_row(&db, "s-orphan", None, None, false, None, None).await;

        let counts = db.list_projects_with_counts().await.unwrap();
        assert_eq!(counts.get("p1").copied(), Some(2));
        assert_eq!(counts.get("p2").copied(), Some(1));
        assert_eq!(counts.len(), 2, "NULL project_id row must be skipped");
    }

    #[tokio::test]
    async fn list_projects_with_last_activity_uses_coalesce_ts() {
        let db = Database::new_in_memory().await.unwrap();
        // Project p1 has a session with last_message_at and one with only mtime
        seed_catalog_row(&db, "s1", Some("p1"), Some("/1"), false, Some(100), None).await;
        seed_catalog_row(&db, "s2", Some("p1"), Some("/2"), false, None, Some(500)).await;

        let last = db.list_projects_with_last_activity().await.unwrap();
        assert_eq!(
            last.get("p1").copied().flatten(),
            Some(500),
            "MAX over COALESCE(last_message_at, mtime) picks the mtime",
        );
    }
}
