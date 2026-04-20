//! stage_c_outbox drainer task.
//!
//! Polls the outbox every `POLL_EMPTY_SLEEP_MS` ms, drains pending
//! FlagDelta rows in batches, applies each under its own TX, and
//! marks applied_at. On error: backoff 1 s, retry forever.
//!
//! The drainer does NOT coordinate with the fold task — each writes
//! to its own table and reads the outbox with `WHERE applied_at IS NULL`
//! / AUTOINCREMENT seq. Backpressure happens naturally: if the
//! drainer falls behind, pending rows accumulate but the fold task
//! keeps writing.

use std::sync::Arc;
use std::time::Duration;

use super::flag_delta::{apply_flag_delta_tx, FlagDelta};
use super::outbox::{mark_applied, select_pending};
use crate::{Database, DbResult};

const POLL_EMPTY_SLEEP_MS: u64 = 200;
const POLL_ERROR_SLEEP_MS: u64 = 1_000;
const BATCH_SIZE: i64 = 100;

#[derive(Debug, Clone, Default)]
pub struct DrainBatchSummary {
    pub rows_observed: u64,
    pub rows_applied: u64,
    pub rows_failed: u64,
}

/// Drain one batch of pending FlagDeltas. Each row applies under its
/// own TX so a single bad payload doesn't abort the whole batch.
/// Successful rows are bulk-marked with a single UPDATE at the end.
pub async fn run_drain_batch(db: Arc<Database>) -> DbResult<DrainBatchSummary> {
    let pending = select_pending(db.pool(), BATCH_SIZE).await?;
    let mut summary = DrainBatchSummary {
        rows_observed: pending.len() as u64,
        ..Default::default()
    };
    if pending.is_empty() {
        return Ok(summary);
    }

    let mut applied_seqs = Vec::with_capacity(pending.len());
    for row in &pending {
        let delta: FlagDelta = match serde_json::from_str(&row.payload) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(
                    seq = row.seq,
                    error = %e,
                    "stage_c_outbox row has unparseable payload — skipping"
                );
                summary.rows_failed += 1;
                continue;
            }
        };
        let mut tx = db.pool().begin().await?;
        match apply_flag_delta_tx(&mut tx, &delta).await {
            Ok(()) => {
                tx.commit().await?;
                applied_seqs.push(row.seq);
                summary.rows_applied += 1;
            }
            Err(e) => {
                // Roll back, leave applied_at NULL — drainer will retry on next batch.
                let _ = tx.rollback().await;
                tracing::warn!(
                    seq = row.seq,
                    error = %e,
                    "stage_c_outbox apply failed — will retry"
                );
                summary.rows_failed += 1;
            }
        }
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    mark_applied(db.pool(), &applied_seqs, now_ms).await?;
    Ok(summary)
}

pub async fn run_forever(db: Arc<Database>) {
    loop {
        match run_drain_batch(db.clone()).await {
            Ok(s) if s.rows_observed == 0 => {
                tokio::time::sleep(Duration::from_millis(POLL_EMPTY_SLEEP_MS)).await;
            }
            Ok(s) => {
                tracing::debug!(
                    observed = s.rows_observed,
                    applied = s.rows_applied,
                    failed = s.rows_failed,
                    "stage_c_outbox drain batch complete"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "drainer batch failed — backing off");
                tokio::time::sleep(Duration::from_millis(POLL_ERROR_SLEEP_MS)).await;
            }
        }
    }
}

/// Spawn the drainer on the current tokio runtime.
pub fn spawn_outbox_drainer(db: Arc<Database>) {
    tokio::spawn(run_forever(db));
}
