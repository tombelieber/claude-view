//! `session_action_log` append-only event log — CQRS Phase 5.
//!
//! Every archive / unarchive / classify / dismiss / reclassify mutation
//! lands one row here. Writers include the insert inside the SAME
//! transaction as the legacy `sessions.*` column write so both sides
//! commit atomically; readers (the PR 5.3 fold worker) select by
//! `seq > applied_seq` to advance the fold watermark.
//!
//! Schema lives in `crates/db/src/migrations/events.rs` (migration 82).
//! The column contracts the insert relies on:
//!   - `seq INTEGER PRIMARY KEY AUTOINCREMENT` — strictly increasing
//!   - `at INTEGER NOT NULL` — caller-supplied unix ms; the caller owns
//!     the clock so dual-write timestamps stay bit-identical between
//!     the legacy column write and the log entry
//!   - `action`, `actor`, `payload` — free-form TEXT; Rust enum serde
//!     is the authoritative validator per §7.1

use sqlx::{Executor, Sqlite};

use crate::{Database, DbResult};

/// Insert a row into `session_action_log` on an arbitrary executor.
///
/// Accepts either `&Pool<Sqlite>` (standalone use) or `&mut Transaction`
/// (shared with a legacy column UPDATE in the same TX — the dual-write
/// pattern PR 5.2 uses). Returns the generated `seq` so callers can
/// correlate the row for tracing / tests.
pub(crate) async fn insert_action_log_tx<'e, E>(
    executor: E,
    session_id: &str,
    action: &str,
    payload: &str,
    actor: &str,
    at_ms: i64,
) -> DbResult<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let seq = sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO session_action_log (session_id, action, payload, actor, at)
           VALUES (?1, ?2, ?3, ?4, ?5)
           RETURNING seq"#,
    )
    .bind(session_id)
    .bind(action)
    .bind(payload)
    .bind(actor)
    .bind(at_ms)
    .fetch_one(executor)
    .await?;
    Ok(seq)
}

impl Database {
    /// Standalone `session_action_log` insert (opens its own pool conn).
    ///
    /// Used by the dismiss handler, which today has no accompanying
    /// column write to dual-write with (dismissal is in-memory ring only
    /// pre-Phase-5). When Phase 5.3's fold writer populates
    /// `session_flags.dismissed_at`, the dismiss handler's ring update
    /// becomes the visible state; the action log is the authoritative
    /// audit trail.
    pub async fn insert_action_log(
        &self,
        session_id: &str,
        action: &str,
        payload: &str,
        actor: &str,
        at_ms: i64,
    ) -> DbResult<i64> {
        insert_action_log_tx(self.pool(), session_id, action, payload, actor, at_ms).await
    }
}
