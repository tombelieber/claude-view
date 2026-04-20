//! Phase 3 PR 3.a — `SessionCatalogAdapter`.
//!
//! Thin wrapper around the `session_stats` read-side that serves the
//! same shape as `claude_view_core::session_catalog::SessionCatalog`. It
//! is the read-side surface every API endpoint gets cut to during Phase
//! 3 (PRs 3.1 → 3.7).
//!
//! The adapter carries a handle to the legacy in-memory
//! `SessionCatalog` so it can fall back gracefully on rows with NULL
//! project_id / file_path. Rows indexed before migration 66 stay NULL
//! until the next re-index; in the meantime the adapter looks them up
//! from the in-memory map and fills in the gap. A one-shot
//! `full_rebuild` at startup (Phase 2 exit gate, shipped) typically
//! clears this in minutes.
//!
//! CQRS Phase 7.d — the `CLAUDE_VIEW_USE_LEGACY_SESSIONS_READ` env-var
//! escape hatch has been retired now that session_stats reads are soak-
//! tested. The adapter is the sole read surface.
//!
//! The adapter is `Clone`-cheap (`Arc<Database>` + `SessionCatalog` are
//! both `Clone` + `Send + Sync`). Pass it through `AppState` by value.

use claude_view_core::session_catalog::{
    CatalogRow, Filter as CatFilter, ProjectId, SessionCatalog, SessionId, Sort as CatSort,
};
use claude_view_db::{CatalogFilter, CatalogSort, Database, FullSessionStatsRow, StatsCatalogRow};
use std::collections::HashMap;

/// Read-side catalog adapter used by every Phase 3 endpoint cutover.
///
/// Path selection:
///
/// | Condition                                                | Path used     |
/// |----------------------------------------------------------|---------------|
/// | DB returns row with `project_id IS NOT NULL`             | DB row        |
/// | DB returns row with `project_id IS NULL` (pre-migration) | Legacy lookup |
/// | DB returns nothing                                       | Legacy lookup |
///
/// The fallback path is a strict superset — no read can fail just
/// because migration 66 hasn't indexed a row yet.
#[derive(Clone)]
pub struct SessionCatalogAdapter {
    db: Database,
    legacy: SessionCatalog,
}

impl SessionCatalogAdapter {
    pub fn new(db: Database, legacy: SessionCatalog) -> Self {
        Self { db, legacy }
    }

    /// Access the underlying in-memory catalog. Needed during Phase 3
    /// transition so route modules can still rebuild / replace_all
    /// during startup without going through the adapter. Removed in
    /// PR 3.z once the legacy catalog is retired.
    pub fn legacy(&self) -> &SessionCatalog {
        &self.legacy
    }

    /// Single session lookup by id. Mirrors `SessionCatalog::get`.
    pub async fn get(&self, session_id: &str) -> Option<CatalogRow> {
        match self.db.get_session_catalog_entry(session_id).await {
            Ok(Some(row)) => match stats_row_to_catalog_row(&row) {
                Some(cr) => Some(cr),
                None => self.legacy.get(session_id),
            },
            Ok(None) => self.legacy.get(session_id),
            Err(err) => {
                // DB errors are rare (disk full, corruption). Degrade to
                // the legacy path rather than surfacing a 500 — the read
                // path must not take the whole API down because a single
                // SELECT hiccupped.
                tracing::warn!(
                    error = %err,
                    session_id,
                    "SessionCatalogAdapter::get — session_stats query failed, falling back to in-memory catalog"
                );
                self.legacy.get(session_id)
            }
        }
    }

    /// List sessions matching `filter`, sorted + limited. Mirrors
    /// `SessionCatalog::list`.
    ///
    /// If the DB returns no rows at all (fresh install, migration not
    /// yet run, empty table) the legacy in-memory catalog handles the
    /// whole request. This keeps dev + test environments that don't
    /// invoke the indexer working by default.
    pub async fn list(&self, filter: &CatFilter, sort: CatSort, limit: usize) -> Vec<CatalogRow> {
        let db_filter = CatalogFilter {
            project_id: filter.project_id.clone(),
            min_last_ts: filter.min_last_ts,
            max_last_ts: filter.max_last_ts,
        };
        let db_sort = match sort {
            CatSort::LastTsDesc => CatalogSort::LastTsDesc,
            CatSort::LastTsAsc => CatalogSort::LastTsAsc,
        };
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);

        let rows = match self
            .db
            .list_session_catalog_entries(&db_filter, db_sort, limit_i64)
            .await
        {
            Ok(rows) => rows,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "SessionCatalogAdapter::list — session_stats query failed, falling back to in-memory catalog"
                );
                return self.legacy.list(filter, sort, limit);
            }
        };

        if rows.is_empty() {
            // Empty DB or freshly-migrated table — legacy path has it.
            return self.legacy.list(filter, sort, limit);
        }

        let mut out: Vec<CatalogRow> = Vec::with_capacity(rows.len());
        for row in &rows {
            if let Some(cr) = stats_row_to_catalog_row(row) {
                out.push(cr);
            } else if let Some(cr) = self.legacy.get(&row.session_id) {
                out.push(cr);
            }
            // else: drop the row — it's NULL in both sources, so it
            // effectively doesn't exist yet from the reader's point of
            // view. Fsnotify + full_rebuild converge within minutes.
        }
        out
    }

    /// Projects-with-counts map. Mirrors `SessionCatalog::projects`.
    pub async fn projects(&self) -> HashMap<ProjectId, usize> {
        let from_db = match self.db.list_projects_with_counts().await {
            Ok(m) => m,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "SessionCatalogAdapter::projects — session_stats query failed, falling back to in-memory catalog"
                );
                return self.legacy.projects();
            }
        };

        // Union with the legacy map so projects whose sessions are all
        // NULL in session_stats (pre-migration or not-yet-indexed) still
        // show up in the UI. DB counts win on key collision.
        let mut merged = self.legacy.projects();
        for (pid, cnt) in from_db {
            merged.insert(pid, cnt);
        }
        merged
    }

    /// Phase 3 PR 3.2 — full-row list. Returns every column needed to
    /// render a `/api/sessions` list entry (catalog metadata + stats +
    /// per-model token breakdown) without a second DB call and without
    /// parsing the JSONL. Filters, sorts, limits match
    /// [`Self::list`] semantics.
    ///
    /// DB errors fall through as `Err(())` — the caller decides whether
    /// to fail the request or serve stale data.
    pub async fn list_full(
        &self,
        filter: &CatFilter,
        sort: CatSort,
        limit: usize,
    ) -> Result<Vec<FullSessionStatsRow>, ()> {
        let db_filter = CatalogFilter {
            project_id: filter.project_id.clone(),
            min_last_ts: filter.min_last_ts,
            max_last_ts: filter.max_last_ts,
        };
        let db_sort = match sort {
            CatSort::LastTsDesc => CatalogSort::LastTsDesc,
            CatSort::LastTsAsc => CatalogSort::LastTsAsc,
        };
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);

        self.db
            .list_full_session_stats(&db_filter, db_sort, limit_i64)
            .await
            .map_err(|err| {
                tracing::warn!(
                    error = %err,
                    "SessionCatalogAdapter::list_full — session_stats query failed"
                );
            })
    }

    /// Phase 3 PR 3.3 — single-session full-row load.
    pub async fn get_full(&self, session_id: &str) -> Result<Option<FullSessionStatsRow>, ()> {
        self.db
            .get_full_session_stats(session_id)
            .await
            .map_err(|err| {
                tracing::warn!(
                    error = %err,
                    session_id,
                    "SessionCatalogAdapter::get_full — session_stats query failed"
                );
            })
    }

    /// Last-activity-per-project helper. Delegates straight to the DB in
    /// the normal path — `/api/projects` needs both the count and the
    /// last activity and assembling them from two calls would require
    /// client-side joining. This one method is DB-only; callers that
    /// need legacy semantics should use `projects()` + per-project
    /// `list(filter, LastTsDesc, 1)`.
    pub async fn projects_with_last_activity(&self) -> HashMap<ProjectId, Option<i64>> {
        // CQRS Phase 7.d removed the legacy-only passthrough for this
        // method; the DB is now always the source of truth.
        match self.db.list_projects_with_last_activity().await {
            Ok(m) => m,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "SessionCatalogAdapter::projects_with_last_activity — session_stats query failed, falling back"
                );
                self.legacy
                    .projects()
                    .into_keys()
                    .map(|pid| {
                        let newest = self
                            .legacy
                            .list(&CatFilter::by_project(&pid), CatSort::LastTsDesc, 1)
                            .first()
                            .map(|row| row.mtime);
                        (pid, newest)
                    })
                    .collect()
            }
        }
    }
}

/// Convert a `StatsCatalogRow` into the legacy `CatalogRow` shape, or
/// `None` if the row is missing the filesystem-mirror fields (i.e. was
/// indexed before migration 66 landed). Callers that get `None` should
/// fall back to the in-memory `SessionCatalog`.
fn stats_row_to_catalog_row(row: &StatsCatalogRow) -> Option<CatalogRow> {
    let file_path = row.file_path.clone()?;
    let project_id = row.project_id.clone()?;
    Some(CatalogRow {
        id: row.session_id.clone() as SessionId,
        file_path,
        is_compressed: row.is_compressed,
        bytes: u64::try_from(row.source_size.max(0)).unwrap_or(0),
        mtime: row.source_mtime.unwrap_or(0),
        project_id,
        first_ts: row.first_message_at,
        last_ts: row.last_message_at,
    })
}

/// Convert a full-stats row into the legacy `CatalogRow` used by
/// `build_session_info`. Returns `None` if the row still has NULL
/// filesystem-mirror columns (pre-migration-66 data).
pub fn full_row_to_catalog_row(row: &FullSessionStatsRow) -> Option<CatalogRow> {
    let file_path = row.file_path.clone()?;
    let project_id = row.project_id.clone()?;
    Some(CatalogRow {
        id: row.session_id.clone() as SessionId,
        file_path,
        is_compressed: row.is_compressed,
        bytes: u64::try_from(row.source_size.max(0)).unwrap_or(0),
        mtime: row.source_mtime.unwrap_or(0),
        project_id,
        first_ts: row.first_message_at,
        last_ts: row.last_message_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn fresh_db() -> Database {
        Database::new_in_memory().await.unwrap()
    }

    async fn seed_stats(
        db: &Database,
        session_id: &str,
        project_id: Option<&str>,
        file_path: Option<&str>,
        last_message_at: Option<i64>,
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
        .bind(0_i64)
        .bind(last_message_at.map(|ts| ts - 1))
        .execute(db.pool())
        .await
        .unwrap();
    }

    fn seed_legacy(legacy: &SessionCatalog, session_id: &str, project_id: &str, mtime: i64) {
        legacy.replace_all(vec![CatalogRow {
            id: session_id.to_string(),
            file_path: PathBuf::from(format!("/legacy/{project_id}/{session_id}.jsonl")),
            is_compressed: false,
            bytes: 100,
            mtime,
            project_id: project_id.to_string(),
            first_ts: None,
            last_ts: Some(mtime),
        }]);
    }

    #[tokio::test]
    async fn get_prefers_db_row_when_project_id_populated() {
        let db = fresh_db().await;
        seed_stats(
            &db,
            "sess-1",
            Some("proj-db"),
            Some("/db/proj-db/sess-1.jsonl"),
            Some(1_800_000_000),
        )
        .await;
        let legacy = SessionCatalog::new();
        let adapter = SessionCatalogAdapter::new(db, legacy);

        let row = adapter.get("sess-1").await.unwrap();
        assert_eq!(row.project_id, "proj-db");
        assert_eq!(
            row.file_path,
            PathBuf::from("/db/proj-db/sess-1.jsonl"),
            "DB row wins when project_id is populated"
        );
    }

    #[tokio::test]
    async fn get_falls_back_to_legacy_when_db_row_has_null_project_id() {
        let db = fresh_db().await;
        seed_stats(&db, "sess-1", None, None, Some(42)).await;
        let legacy = SessionCatalog::new();
        seed_legacy(&legacy, "sess-1", "proj-legacy", 100);
        let adapter = SessionCatalogAdapter::new(db, legacy);

        let row = adapter.get("sess-1").await.unwrap();
        assert_eq!(
            row.project_id, "proj-legacy",
            "NULL project_id must fall back to in-memory catalog"
        );
    }

    #[tokio::test]
    async fn get_falls_back_to_legacy_when_db_has_no_row() {
        let db = fresh_db().await;
        let legacy = SessionCatalog::new();
        seed_legacy(&legacy, "sess-1", "proj-legacy", 100);
        let adapter = SessionCatalogAdapter::new(db, legacy);

        let row = adapter.get("sess-1").await.unwrap();
        assert_eq!(row.project_id, "proj-legacy");
    }

    // CQRS Phase 7.d — retired `legacy_env_var_forces_in_memory_path`:
    // the `CLAUDE_VIEW_USE_LEGACY_SESSIONS_READ` escape hatch was removed
    // after soak proved the session_stats read path is stable.

    #[tokio::test]
    async fn list_returns_db_rows_sorted() {
        let db = fresh_db().await;
        seed_stats(&db, "s1", Some("p"), Some("/p/s1"), Some(100)).await;
        seed_stats(&db, "s2", Some("p"), Some("/p/s2"), Some(300)).await;
        seed_stats(&db, "s3", Some("p"), Some("/p/s3"), Some(200)).await;
        let legacy = SessionCatalog::new();
        let adapter = SessionCatalogAdapter::new(db, legacy);

        let rows = adapter
            .list(&CatFilter::default(), CatSort::LastTsDesc, 10)
            .await;
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, "s2");
        assert_eq!(rows[1].id, "s3");
        assert_eq!(rows[2].id, "s1");
    }

    #[tokio::test]
    async fn list_falls_back_to_legacy_on_empty_db() {
        let db = fresh_db().await;
        let legacy = SessionCatalog::new();
        seed_legacy(&legacy, "legacy-only", "p", 100);
        let adapter = SessionCatalogAdapter::new(db, legacy);

        let rows = adapter
            .list(&CatFilter::default(), CatSort::LastTsDesc, 10)
            .await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "legacy-only");
    }

    #[tokio::test]
    async fn projects_unions_legacy_and_db() {
        let db = fresh_db().await;
        seed_stats(&db, "s-db", Some("proj-db"), Some("/p/s-db"), Some(100)).await;
        let legacy = SessionCatalog::new();
        legacy.replace_all(vec![CatalogRow {
            id: "s-legacy".into(),
            file_path: PathBuf::from("/legacy/proj-legacy/s-legacy.jsonl"),
            is_compressed: false,
            bytes: 100,
            mtime: 50,
            project_id: "proj-legacy".into(),
            first_ts: None,
            last_ts: None,
        }]);
        let adapter = SessionCatalogAdapter::new(db, legacy);

        let map = adapter.projects().await;
        assert_eq!(map.get("proj-db").copied(), Some(1));
        assert_eq!(
            map.get("proj-legacy").copied(),
            Some(1),
            "legacy-only project must still appear"
        );
    }
}
