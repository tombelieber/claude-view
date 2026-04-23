//! Facet ingest (initial + periodic) and git-root backfill task spawners.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Intervals and logging
//! shape are unchanged.

use std::sync::Arc;
use std::time::Duration;

use claude_view_db::Database;

use crate::FacetIngestState;

/// Spawn the initial facet-ingest task (delayed 3 s to let indexing finish
/// first) and the periodic re-ingest task (every 12 hours).
///
/// Uses a dedicated `FacetIngestState` — the `AppState` keeps its own
/// `FacetIngestState` for user-triggered ingest via the API/SSE endpoint.
/// Background ingest is fire-and-forget with tracing output only.
pub fn spawn_facet_ingest(db: &Database) {
    let ingest_state = Arc::new(FacetIngestState::new());

    // Initial ingest (delayed 3s to let indexing finish first)
    let db_init = db.clone();
    let state_init = ingest_state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if state_init.is_running() {
            return;
        }
        match crate::facet_ingest::run_facet_ingest(&db_init, &state_init, None).await {
            Ok(n) => {
                if n > 0 {
                    tracing::info!("Facet ingest: imported {n} new facets from /insights cache");
                }
            }
            Err(e) => tracing::warn!("Facet ingest skipped: {e}"),
        }
    });

    // Periodic re-ingest (every 12 hours)
    let db_periodic = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(12 * 3600));
        interval.tick().await; // skip immediate tick (startup already handled above)
        loop {
            interval.tick().await;
            if ingest_state.is_running() {
                tracing::debug!("Periodic facet ingest skipped: already running");
                continue;
            }
            tracing::info!("Periodic facet re-ingest starting");
            match crate::facet_ingest::run_facet_ingest(&db_periodic, &ingest_state, None).await {
                Ok(n) => {
                    if n > 0 {
                        tracing::info!("Periodic facet ingest: imported {n} new facets");
                    }
                }
                Err(e) => tracing::warn!("Periodic facet ingest failed: {e}"),
            }
        }
    });
}

/// Spawn the git-root backfill task for sessions indexed before the
/// `git_root` column existed. Runs once, in the background.
pub fn spawn_git_root_backfill(db: &Database) {
    let db = Arc::new(db.clone());
    tokio::spawn(async move {
        crate::backfill::backfill_git_roots(db).await;
    });
}
