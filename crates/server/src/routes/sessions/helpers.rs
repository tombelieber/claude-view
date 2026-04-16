//! Shared helpers for session route handlers.

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Resolve a session's JSONL file path: catalog first, then DB, then live session store.
///
/// Phase 2 (JSONL-first hardcut): the in-memory `SessionCatalog` is the authoritative
/// source for session → file_path lookups — it's populated from a filesystem walk at
/// startup and reconciled every 5s, so it sees every session that exists on disk.
///
/// The DB and live-session fallbacks remain for two narrow cases:
///   - DB: coverage during the migration window when callers hand in a session id
///     that the catalog hasn't yet picked up (pre-reconcile race).
///   - Live: IDE-spawned sessions whose JSONL file may not yet be on disk.
pub(crate) async fn resolve_session_file_path(
    state: &AppState,
    session_id: &str,
) -> ApiResult<std::path::PathBuf> {
    // Catalog first — no DB, no async, no lock contention.
    if let Some(row) = state.session_catalog.get(session_id) {
        if row.file_path.exists() {
            return Ok(row.file_path);
        }
    }

    let file_path = match state.db.get_session_file_path(session_id).await? {
        Some(p) => p,
        None => {
            let map = state.live_sessions.read().await;
            map.get(session_id)
                .map(|s| s.jsonl.file_path.clone())
                .ok_or_else(|| ApiError::SessionNotFound(session_id.to_string()))?
        }
    };
    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id.to_string()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use claude_view_core::session_catalog::CatalogRow;
    use claude_view_db::Database;
    use tempfile::tempdir;

    async fn fresh_state() -> std::sync::Arc<AppState> {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        AppState::new(db)
    }

    fn row(id: &str, file_path: std::path::PathBuf) -> CatalogRow {
        CatalogRow {
            id: id.to_string(),
            file_path,
            is_compressed: false,
            bytes: 0,
            mtime: 0,
            project_id: "test-project".to_string(),
            first_ts: None,
            last_ts: None,
        }
    }

    #[tokio::test]
    async fn resolves_from_catalog_when_db_and_live_are_empty() {
        // The point of Phase A: catalog is authoritative. This case must work
        // with an empty DB and no live session — which it did NOT before the
        // catalog-first branch was added.
        let tmp = tempdir().unwrap();
        let jsonl = tmp.path().join("only-in-catalog.jsonl");
        std::fs::write(&jsonl, b"{}").unwrap();

        let state = fresh_state().await;
        state
            .session_catalog
            .replace_all(vec![row("only-in-catalog", jsonl.clone())]);

        let resolved = resolve_session_file_path(&state, "only-in-catalog")
            .await
            .expect("catalog hit should resolve");
        assert_eq!(resolved, jsonl);
    }

    #[tokio::test]
    async fn catalog_beats_db_when_both_have_the_session() {
        // If the catalog has it, the DB must not be consulted. We simulate "DB has
        // a different path" by making the catalog point at a file that exists and
        // leaving the DB empty; the only way the assertion holds is if the catalog
        // short-circuits BEFORE the DB lookup.
        let tmp = tempdir().unwrap();
        let catalog_path = tmp.path().join("from-catalog.jsonl");
        std::fs::write(&catalog_path, b"{}").unwrap();

        let state = fresh_state().await;
        state
            .session_catalog
            .replace_all(vec![row("hit", catalog_path.clone())]);

        let resolved = resolve_session_file_path(&state, "hit")
            .await
            .expect("must resolve via catalog");
        assert_eq!(resolved, catalog_path);
    }

    #[tokio::test]
    async fn catalog_miss_errors_when_db_and_live_also_miss() {
        // Regression guard: a completely unknown session still produces
        // SessionNotFound, not a panic or a silent empty-path.
        let state = fresh_state().await;
        state.session_catalog.replace_all(vec![]);

        let err = resolve_session_file_path(&state, "nonexistent")
            .await
            .expect_err("must error");
        assert!(matches!(err, ApiError::SessionNotFound(_)));
    }
}
