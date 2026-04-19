//! `session_action_log` → `session_flags` fold — CQRS Phase 5 PR 5.3.
//!
//! This module owns the background task that consumes the append-only
//! log produced by PR 5.2 and materialises the current flag state onto
//! `session_flags`. PR 5.5's reader cutover will switch every
//! `sessions.archived_at` / `sessions.category_*` reader onto
//! `session_flags.*`; until then the fold runs as a shadow writer and
//! parity is observed by PR 5.4's drift monitor.
//!
//! ## Invariants (enforced by tests + §7.2 kill-9 property)
//!
//! 1. **Single TX per batch.** `fold_state.applied_seq` advances in the
//!    same transaction as every `session_flags` UPSERT in that batch.
//!    A crash between two events in the same batch rolls back both the
//!    events and the watermark — after restart the fold replays the
//!    entire batch.
//! 2. **Monotone watermark.** `applied_seq` only grows. Batches are
//!    selected with `WHERE seq > applied_seq ORDER BY seq ASC`; the
//!    AUTOINCREMENT on `session_action_log.seq` means a reused row is
//!    impossible (migration 82).
//! 3. **LWW on classify.** Classify events are applied iff
//!    `event.at >= session_flags.classified_at`. This keeps the fold
//!    deterministic under out-of-order delivery (e.g. a slow
//!    re-classifier lands after a fast one).
//! 4. **Unknown actions skip, never stall.** A typo in `action` column
//!    (e.g. a hypothetical future `rebucket` typoed as `rebuket`)
//!    advances `applied_seq` past it rather than stalling the fold.
//!    `rows_skipped_unknown` counts these for drift monitoring.
//! 5. **Kill-9 resume is byte-identical to one-shot.** Proven by
//!    `crates/db/tests/fold_kill9_test.rs` — randomly-generated event
//!    streams folded incrementally (with a simulated crash between
//!    every batch) produce the same `session_flags` as the same stream
//!    folded in a single pass.
//!
//! ## Lifecycle
//!
//! Server startup spawns exactly one fold task per `Database` handle
//! (wired in `crates/server/src/app_factory.rs` next to the Stage C
//! startup rebuild). The task polls every `POLL_EMPTY_SLEEP_MS` ms on
//! empty batches; on error it backs off to `POLL_ERROR_SLEEP_MS` to
//! avoid a spin loop against a broken DB handle.

mod apply;
pub mod parity;
mod types;

pub use parity::{
    compare_flags_session, run_parity_sweep, FlagFieldDiff, FlagParityReport, ParitySweepSummary,
    FLAG_FIELDS,
};
pub use types::{ActionEvent, ClassifyPayload, FoldBatchSummary};

use std::sync::Arc;
use std::time::Duration;

use crate::{Database, DbResult};

const POLL_EMPTY_SLEEP_MS: u64 = 200;
const POLL_ERROR_SLEEP_MS: u64 = 1_000;
const BATCH_SIZE: i64 = 100;

/// Apply one batch of events. Returns the summary for observability
/// + testing. Used both by `spawn_flags_fold` and directly by tests.
pub async fn run_fold_batch(db: Arc<Database>) -> DbResult<FoldBatchSummary> {
    let applied_seq = db.fold_get_applied_seq().await?;
    let events = db
        .action_log_select_after_seq(applied_seq, BATCH_SIZE)
        .await?;

    let mut summary = FoldBatchSummary {
        rows_observed: events.len() as u64,
        ..Default::default()
    };

    if events.is_empty() {
        return Ok(summary);
    }

    let mut tx = db.pool().begin().await?;
    for event in &events {
        let (applied, lww_skipped) = apply::fold_event_tx(&mut tx, event).await?;
        if applied {
            summary.rows_applied += 1;
        } else if lww_skipped {
            summary.rows_skipped_lww += 1;
        } else {
            summary.rows_skipped_unknown += 1;
        }
    }

    summary.max_seq = events.last().unwrap().seq;
    sqlx::query("UPDATE fold_state SET applied_seq = ?1 WHERE id = 0")
        .bind(summary.max_seq)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(summary)
}

/// Background fold task. Runs `run_fold_batch` forever, sleeping on
/// empty batches. Returns only when the Tokio runtime shuts down
/// (there is no graceful signal channel — shutdown is process-level).
pub async fn run_forever(db: Arc<Database>) {
    loop {
        match run_fold_batch(db.clone()).await {
            Ok(summary) if summary.rows_observed == 0 => {
                tokio::time::sleep(Duration::from_millis(POLL_EMPTY_SLEEP_MS)).await;
            }
            Ok(summary) => {
                tracing::debug!(
                    rows_observed = summary.rows_observed,
                    rows_applied = summary.rows_applied,
                    rows_skipped_lww = summary.rows_skipped_lww,
                    rows_skipped_unknown = summary.rows_skipped_unknown,
                    max_seq = summary.max_seq,
                    "session_flags fold batch complete"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "session_flags fold batch failed — backing off"
                );
                tokio::time::sleep(Duration::from_millis(POLL_ERROR_SLEEP_MS)).await;
            }
        }
    }
}

/// Convenience wrapper: spawn the fold task on the current tokio
/// runtime. Callers that need a `JoinHandle` back should use
/// `tokio::spawn(run_forever(db))` directly.
pub fn spawn_flags_fold(db: Arc<Database>) {
    tokio::spawn(run_forever(db));
}

// ──────────────────────────────────────────────────────────────────
// PR 5.4 — shadow parity monitor
// ──────────────────────────────────────────────────────────────────

/// How many recent sessions the parity monitor samples per sweep.
/// Matches the §7.1 exit gate's "≥ 10,000 fold events processed" ask
/// — a 10k-session prod DB samples the entire table per cycle; a
/// larger corpus gets the most-recent slice.
const PARITY_SWEEP_LIMIT: i64 = 10_000;

/// How often the parity monitor runs. 15 min balances "useful
/// feedback loop during the 48 h Phase 5 soak" with "negligible DB
/// load (~10 k rows × 8 fields × 2 reads per cycle)".
const PARITY_SWEEP_INTERVAL_SECS: u64 = 900;

/// Background loop that periodically runs `run_parity_sweep` and logs
/// the aggregated counts at INFO level. Phase 7's /metrics exporter
/// will replace the tracing hop with direct Prometheus counter
/// increments; until then the logs are the observability channel.
pub async fn run_parity_forever(db: Arc<Database>) {
    loop {
        tokio::time::sleep(Duration::from_secs(PARITY_SWEEP_INTERVAL_SECS)).await;
        match parity::run_parity_sweep(&db, PARITY_SWEEP_LIMIT).await {
            Ok(summary) => {
                let any_drift = summary.total_diverged > 0;
                // Fold per-field counts into a comma-joined display string
                // so the log entry is one line even when most fields are 0.
                let per_field: String = summary
                    .per_field_counts
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(",");
                if any_drift {
                    tracing::warn!(
                        sampled = summary.total_sampled,
                        diverged = summary.total_diverged,
                        missing_shadow = summary.total_missing_shadow,
                        fields = %per_field,
                        "shadow_flags_diff_total non-zero — §6.4 48h soak gate NOT clean"
                    );
                } else {
                    tracing::info!(
                        sampled = summary.total_sampled,
                        fields = %per_field,
                        "shadow_flags_diff_total all zero — §6.4 soak-clean cycle"
                    );
                }
            }
            Err(e) => tracing::warn!(
                error = %e,
                "parity sweep failed — retrying at next interval"
            ),
        }
    }
}

/// Spawn the parity monitor on the current tokio runtime.
pub fn spawn_parity_monitor(db: Arc<Database>) {
    tokio::spawn(run_parity_forever(db));
}

impl Database {
    /// Read the current `applied_seq` watermark. 0 on a fresh DB.
    pub async fn fold_get_applied_seq(&self) -> DbResult<i64> {
        let seq: i64 = sqlx::query_scalar("SELECT applied_seq FROM fold_state WHERE id = 0")
            .fetch_one(self.pool())
            .await?;
        Ok(seq)
    }
}
