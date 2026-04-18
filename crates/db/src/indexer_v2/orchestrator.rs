//! Phase 2 indexer_v2 orchestrator — fsnotify loop + per-session debouncer.
//!
//! Skeleton only. The fsnotify wiring + parse/extract loop ship in a
//! follow-up commit (PR 2.2.1) so this commit can land the writer in
//! isolation and be tested without server startup churn. Until then,
//! `spawn_shadow_indexer` is intentionally an `unimplemented!()` to
//! prevent accidental wiring before the orchestrator has been
//! reviewed.

use std::sync::Arc;

use crate::Database;

/// Spawn the shadow indexer task. **Not yet implemented.**
///
/// PR 2.2 (this commit): writer + skeleton only. Server startup must
/// not call this until the follow-up commit lands the fsnotify+parse
/// loop, otherwise the panic below fires and the server crash loops.
///
/// The follow-up commit will replace the body with the
/// `tokio::sync::broadcast::Receiver<FileEvent>` consumer + the per-
/// session debouncer + the `parse_jsonl` → `extract_stats` →
/// `upsert_session_stats` pipeline (Phase 1-7 design §3.1 PR 2.2).
pub fn spawn_shadow_indexer(_db: Arc<Database>) {
    unimplemented!(
        "indexer_v2 orchestrator not yet wired — see PR 2.2.1 follow-up. \
         The writer (upsert_session_stats) is the only piece live in PR 2.2."
    )
}
