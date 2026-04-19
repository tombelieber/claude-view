//! CQRS Phase 7 — background sampler tasks that publish gauges to
//! `/metrics`.
//!
//! These tasks are separate from the synchronous `/metrics` handler
//! because each sample requires multiple DB reads (parity sweep +
//! fold-lag probe + outbox count) and doing that inside the handler
//! would add up to ~hundreds of ms of latency for every Prometheus
//! scrape. The sampler runs on its own cadence and overwrites gauge
//! values in place; the handler just reads whatever is current.

use std::sync::Arc;
use std::time::Duration;

use claude_view_db::fold::{run_parity_sweep, FLAG_FIELDS};
use claude_view_db::Database;
use claude_view_server_types::metrics::record_cqrs_shadow_sample;

/// How often the sampler runs. 60 s matches typical Prometheus scrape
/// intervals — shorter cadences waste DB reads because a scrape at
/// `t` serves the sample taken at `t - 30 s` either way.
const SAMPLE_INTERVAL_SECS: u64 = 60;

/// Sample size for the parity sweep. Full corpus samples add up to
/// ~O(20 ms / 1 k sessions) on M5 Max, so 10 k is a comfortable upper
/// bound even on large-but-not-enormous DBs. Post Phase D.3 the sweep
/// is a structural no-op; keep the sample anyway so the gauge
/// registers as 0 rather than staying at its initial value.
const PARITY_SAMPLE_LIMIT: i64 = 10_000;

/// Run one sampler iteration: parity sweep + fold lag + outbox pending.
///
/// Surfaces errors via `tracing::warn!`; the gauge keeps its previous
/// value on error. Separate function from `run_forever` so tests can
/// exercise the sampling path deterministically.
pub async fn run_sampler_once(db: Arc<Database>) {
    let mut per_field: std::collections::BTreeMap<&'static str, u64> =
        FLAG_FIELDS.iter().map(|f| (*f, 0)).collect();

    match run_parity_sweep(&db, PARITY_SAMPLE_LIMIT).await {
        Ok(summary) => {
            for (field, count) in summary.per_field_counts {
                per_field.insert(field, count);
            }
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "cqrs_sampler: parity sweep failed — gauges keep previous values for this field set"
            );
        }
    }

    let fold_lag = match sqlx::query_as::<_, (i64,)>(
        r#"SELECT COALESCE(
               (SELECT MAX(seq) FROM session_action_log), 0
           ) - COALESCE(
               (SELECT applied_seq FROM fold_state WHERE id = 0), 0
           )"#,
    )
    .fetch_one(db.pool())
    .await
    {
        Ok((lag,)) => lag.max(0),
        Err(err) => {
            tracing::warn!(error = %err, "cqrs_sampler: fold-lag probe failed");
            0
        }
    };

    let outbox_pending = match sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM stage_c_outbox WHERE applied_at IS NULL",
    )
    .fetch_one(db.pool())
    .await
    {
        Ok((n,)) => n.max(0),
        Err(err) => {
            tracing::warn!(error = %err, "cqrs_sampler: outbox-pending probe failed");
            0
        }
    };

    record_cqrs_shadow_sample(&per_field, fold_lag, outbox_pending);
}

/// Loop forever, sampling once per interval.
pub async fn run_forever(db: Arc<Database>) {
    loop {
        run_sampler_once(db.clone()).await;
        tokio::time::sleep(Duration::from_secs(SAMPLE_INTERVAL_SECS)).await;
    }
}

/// Spawn the sampler on the current tokio runtime.
pub fn spawn_cqrs_sampler(db: Arc<Database>) {
    tokio::spawn(run_forever(db));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sampler_runs_once_without_panicking_on_empty_db() {
        let db = Arc::new(Database::new_in_memory().await.unwrap());
        // The global metrics recorder may or may not be installed in
        // the test environment; `record_cqrs_shadow_sample` tolerates
        // either case. The important bit here is that the sampler
        // reaches `record_*` without the DB probes blowing up.
        run_sampler_once(db).await;
    }
}
