//! Event-driven drain loop for oMLX phase classification.
//!
//! Replaces the cadence-based scheduler. Sessions are marked dirty on any
//! JSONL activity; the drain loop picks the best candidate whenever a slot
//! opens, keeping oMLX saturated while any session has new data.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Notify, RwLock};

use claude_view_core::phase::client::{ClassifyContext, OmlxClient};
use claude_view_core::phase::scheduler::{ClassifyResult, Priority};

use super::manager::accumulator::SessionAccumulator;

const MAX_CONCURRENT: usize = 2;

/// Debounce durations per priority tier.
fn debounce_for(priority: Priority) -> Duration {
    match priority {
        Priority::New => Duration::from_millis(250),
        Priority::Transition => Duration::from_millis(750),
        Priority::Steady => Duration::from_millis(2500),
    }
}

/// Backpressure multiplier: scales debounce when queue is deep.
fn backpressure_factor(queue_depth: usize) -> f32 {
    (1.0 + queue_depth as f32 / (2.0 * MAX_CONCURRENT as f32)).clamp(1.0, 4.0)
}

/// One entry per session in the dirty registry.
struct DirtyEntry {
    dirty_at: Instant,
    priority: Priority,
    in_flight: bool,
}

/// Run the drain loop as a long-lived tokio task.
///
/// - `dirty_rx`: receives `(session_id, priority)` from line processors
/// - `result_tx`: sends classify results back to the manager result handler
/// - `accumulators`: shared accumulator map (read at drain time for freshest data)
/// - `client`: oMLX HTTP client
/// - `omlx_ready`: health check flag
pub(crate) async fn run_drain_loop(
    mut dirty_rx: mpsc::Receiver<(String, Priority)>,
    result_tx: mpsc::Sender<ClassifyResult>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<OmlxClient>,
    omlx_ready: Arc<AtomicBool>,
    wake: Arc<Notify>,
) {
    let mut dirty: HashMap<String, DirtyEntry> = HashMap::new();
    let (done_tx, mut done_rx) = mpsc::channel::<String>(MAX_CONCURRENT);
    let mut in_flight_count: usize = 0;

    loop {
        tokio::select! {
            // New dirty notification from line processor
            msg = dirty_rx.recv() => {
                let Some((session_id, priority)) = msg else { break };
                let entry = dirty.entry(session_id).or_insert_with(|| DirtyEntry {
                    dirty_at: Instant::now(),
                    priority,
                    in_flight: false,
                });
                entry.dirty_at = Instant::now();
                // Promote priority (lower enum = higher priority)
                if priority < entry.priority {
                    entry.priority = priority;
                }
                try_drain(
                    &mut dirty, &mut in_flight_count, &accumulators,
                    &client, &result_tx, &done_tx, &omlx_ready,
                ).await;
            }

            // Slot freed — a classify call completed
            Some(session_id) = done_rx.recv() => {
                if let Some(entry) = dirty.get_mut(&session_id) {
                    entry.in_flight = false;
                }
                in_flight_count = in_flight_count.saturating_sub(1);
                try_drain(
                    &mut dirty, &mut in_flight_count, &accumulators,
                    &client, &result_tx, &done_tx, &omlx_ready,
                ).await;
            }

            // Periodic wake for debounce expiry
            _ = wake.notified() => {
                try_drain(
                    &mut dirty, &mut in_flight_count, &accumulators,
                    &client, &result_tx, &done_tx, &omlx_ready,
                ).await;
            }

            // Fallback periodic tick (in case no notifications arrive)
            _ = tokio::time::sleep(Duration::from_millis(200)) => {
                try_drain(
                    &mut dirty, &mut in_flight_count, &accumulators,
                    &client, &result_tx, &done_tx, &omlx_ready,
                ).await;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn try_drain(
    dirty: &mut HashMap<String, DirtyEntry>,
    in_flight_count: &mut usize,
    accumulators: &Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: &Arc<OmlxClient>,
    result_tx: &mpsc::Sender<ClassifyResult>,
    done_tx: &mpsc::Sender<String>,
    omlx_ready: &Arc<AtomicBool>,
) {
    if !omlx_ready.load(Ordering::Relaxed) {
        return;
    }

    let now = Instant::now();
    let queue_depth = dirty.values().filter(|e| !e.in_flight).count();

    while *in_flight_count < MAX_CONCURRENT {
        // Find best candidate: not in-flight, debounce elapsed, oldest ready_at first
        let bp = backpressure_factor(queue_depth);
        let candidate = dirty
            .iter()
            .filter(|(_, e)| {
                !e.in_flight && now.duration_since(e.dirty_at) >= debounce_for(e.priority).mul_f32(bp)
            })
            .min_by_key(|(_, e)| e.dirty_at)
            .map(|(id, _)| id.clone());

        let Some(session_id) = candidate else { break };

        // Mark in-flight
        let entry = dirty.get_mut(&session_id).unwrap();
        entry.in_flight = true;
        *in_flight_count += 1;

        // Snapshot context + generation from accumulator (authoritative counter)
        let (context, generation) = {
            let mut accs = accumulators.write().await;
            match accs.get_mut(&session_id) {
                Some(acc) => {
                    acc.classify_generation += 1;
                    (build_context_from_acc(acc), acc.classify_generation)
                }
                None => {
                    // Session disappeared — clean up
                    entry.in_flight = false;
                    *in_flight_count -= 1;
                    dirty.remove(&session_id);
                    continue;
                }
            }
        };

        // Spawn classify task
        let client = client.clone();
        let result_tx = result_tx.clone();
        let done_tx = done_tx.clone();
        let sid = session_id.clone();
        let temp = 0.15; // Fixed temperature (EMA stabilizer)

        tokio::spawn(async move {
            if let Some((phase, scope)) = client.classify(&context, temp, &sid, generation).await {
                let _ = result_tx
                    .send(ClassifyResult {
                        session_id: sid.clone(),
                        phase,
                        scope,
                        generation,
                    })
                    .await;
            }
            let _ = done_tx.send(sid).await;
        });
    }

    // Clean up entries that are clean (not dirty) and not in-flight
    // A session is "clean" if it was classified and no new dirty_at since
    // We don't remove dirty entries here — they get re-dirtied on next JSONL line
}

/// Build a ClassifyContext from the current accumulator state.
fn build_context_from_acc(acc: &SessionAccumulator) -> ClassifyContext {
    let tool_summary = format!(
        "Edit:{} Read:{} Bash:{} Write:{}",
        acc.tool_counts_edit, acc.tool_counts_read, acc.tool_counts_bash, acc.tool_counts_write,
    );
    ClassifyContext {
        turns: acc.message_buf.iter().cloned().collect(),
        first_user_message: acc.first_user_message.clone(),
        user_files: acc.at_files.iter().take(5).cloned().collect(),
        edited_files: acc.recent_edited_files.iter().cloned().collect(),
        bash_commands: acc.recent_bash_commands.iter().cloned().collect(),
        tool_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debounce_values() {
        assert_eq!(debounce_for(Priority::New), Duration::from_millis(250));
        assert_eq!(debounce_for(Priority::Transition), Duration::from_millis(750));
        assert_eq!(debounce_for(Priority::Steady), Duration::from_millis(2500));
    }

    #[test]
    fn backpressure_bounds() {
        assert_eq!(backpressure_factor(0), 1.0);
        assert!(backpressure_factor(4) > 1.0);
        assert_eq!(backpressure_factor(100), 4.0);
    }

    #[test]
    fn priority_promotion() {
        let mut entry = DirtyEntry {
            dirty_at: Instant::now(),
            priority: Priority::Steady,
            in_flight: false,
        };
        // Simulate promotion
        let new_priority = Priority::New;
        if new_priority < entry.priority {
            entry.priority = new_priority;
        }
        assert_eq!(entry.priority, Priority::New);
    }
}
