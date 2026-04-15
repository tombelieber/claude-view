//! Background search index rebuild orchestrator.
//!
//! Bridges `claude_view_search::migration::PendingMigration` (which knows
//! the disk layout) with `claude_view_db::indexer_parallel::scan_and_index_all`
//! (which knows how to read source data and produce search documents).
//!
//! Spawned from `main.rs` after `SearchIndex::open_versioned()` returns a
//! pending migration. Runs entirely in the background; the server is fully
//! interactive (with the old index serving search queries) the entire time.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use claude_view_core::Registry;
use claude_view_db::indexer_parallel::{scan_and_index_all, IndexHints};
use claude_view_db::Database;
use claude_view_search::migration::{cleanup_old_version, PendingMigration};
use claude_view_search::SearchIndex;

use crate::state::{RegistryHolder, SearchIndexHolder};

/// Maximum time the rebuild task will wait for the main indexer task to
/// populate the registry holder. The registry is normally ready within
/// a few seconds of startup; 60s is a generous safety margin.
const REGISTRY_WAIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Interval between polls of the registry holder while waiting for the
/// main indexer to populate it.
const REGISTRY_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Spawn a background task that builds a new search index at
/// `plan.target_path`, atomically swaps it into `holder`, and deletes the
/// old version.
///
/// On failure, logs a warning and leaves the old index in place — search
/// continues to work, just at the previous schema version. This is the
/// "trust over accuracy" property: we never ship a half-built index, we
/// either commit fully or roll back entirely.
pub fn spawn_background_rebuild(
    plan: PendingMigration,
    holder: SearchIndexHolder,
    claude_dir: PathBuf,
    db: Database,
    hints: HashMap<String, IndexHints>,
    registry_holder: RegistryHolder,
) {
    tokio::spawn(async move {
        tracing::info!(
            target_version = plan.target_version,
            target_path = %plan.target_path.display(),
            "starting background search index rebuild"
        );
        let started = std::time::Instant::now();

        // Wait for the main indexer task to populate the registry holder.
        // The rebuild can't run until the registry is ready because
        // scan_and_index_all needs it to seed invocables and enrich sessions.
        let registry = match wait_for_registry(&registry_holder).await {
            Some(r) => r,
            None => {
                tracing::warn!(
                    timeout_secs = REGISTRY_WAIT_TIMEOUT.as_secs(),
                    "background rebuild timed out waiting for registry — aborting, old index remains active"
                );
                return;
            }
        };

        // 1. Open an empty index at the target path.
        let new_index = match SearchIndex::open(&plan.target_path) {
            Ok(idx) => Arc::new(idx),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "background rebuild failed to open target index — old version remains active"
                );
                return;
            }
        };

        // 2. Run a full scan into the NEW index. Passing the new index
        //    directly (not via the production holder) keeps the production
        //    holder untouched during the rebuild — search queries continue
        //    to hit the old index.
        let scan_result = scan_and_index_all(
            &claude_dir,
            &db,
            &hints,
            Some(new_index.clone()),
            Some(registry),
            |_session_id: &str| {
                // Progress is not reported for the background rebuild —
                // the user experience is "search keeps working".
            },
            |_file_count: usize| {},
            || {},
        )
        .await;

        match scan_result {
            Ok((indexed, _skipped)) => {
                tracing::info!(
                    indexed,
                    target_version = plan.target_version,
                    duration_secs = started.elapsed().as_secs_f64(),
                    "background search rebuild: bulk pass complete"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "background rebuild scan failed — discarding new index, old remains active"
                );
                // Best-effort cleanup of half-built target.
                if let Err(cleanup_err) = std::fs::remove_dir_all(&plan.target_path) {
                    tracing::warn!(
                        error = %cleanup_err,
                        path = %plan.target_path.display(),
                        "failed to remove half-built target after scan failure"
                    );
                }
                return;
            }
        }

        // 3. Mark the new index as schema-synced so next startup recognises
        //    it as the valid v{target_version}/ directory.
        new_index.mark_schema_synced();

        // 4. Atomic swap: replace the inner Arc in the production holder.
        //    Existing in-flight searches still see the old Arc via their
        //    clone, so they complete safely. New searches after this point
        //    use the new index.
        {
            let mut guard = match holder.write() {
                Ok(g) => g,
                Err(poisoned) => {
                    tracing::warn!("search index holder lock poisoned during swap — recovering");
                    poisoned.into_inner()
                }
            };
            *guard = Some(new_index);
        }
        tracing::info!(
            target_version = plan.target_version,
            "background search rebuild: holder swapped to new version"
        );

        // 5. Delete the old version directory. Tantivy mmaps its segment
        //    files; on macOS/Linux the inode stays alive until the last
        //    mmap handle is dropped, so in-flight searches continue to
        //    read stale files even after unlink. This is safe.
        if let Some(old_path) = plan.old_version_path {
            if let Err(e) = cleanup_old_version(&old_path) {
                tracing::warn!(
                    error = %e,
                    path = %old_path.display(),
                    "failed to delete old search index version (non-fatal)"
                );
            }
        }

        tracing::info!(
            duration_secs = started.elapsed().as_secs_f64(),
            "background search rebuild complete"
        );
    });
}

/// Poll `registry_holder` until it becomes `Some(registry)`, returning a
/// cloned `Arc<Registry>`. Returns `None` if `REGISTRY_WAIT_TIMEOUT` elapses
/// first.
async fn wait_for_registry(registry_holder: &RegistryHolder) -> Option<Arc<Registry>> {
    let deadline = std::time::Instant::now() + REGISTRY_WAIT_TIMEOUT;
    loop {
        {
            let guard = match registry_holder.read() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(registry) = guard.as_ref() {
                return Some(Arc::new(registry.clone()));
            }
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(REGISTRY_POLL_INTERVAL).await;
    }
}
