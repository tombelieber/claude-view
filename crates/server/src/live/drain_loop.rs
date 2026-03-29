//! Event-driven drain loop for oMLX phase classification.
//!
//! Uses `tokio::JoinSet` as the task executor — spawned classify calls are
//! tracked, awaited, and abortable without manual channel/counter plumbing.
//! Fair round-robin scheduling via `last_served_at` ensures no session
//! starves when the queue is deep: 20 sessions = each gets 1/20 of bandwidth.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Notify, RwLock};
use tokio::task::JoinSet;

use claude_view_core::phase::client::{ClassifyContext, OmlxClient};
use claude_view_core::phase::scheduler::{ClassifyResult, Priority};

use super::manager::accumulator::SessionAccumulator;

/// oMLX 500s under concurrent load on Apple Silicon (GPU contention with
/// server's DB writes + JSONL parsing). Proven: 92% failure at 2 concurrent.
const MAX_CONCURRENT: usize = 1;

fn debounce_for(priority: Priority) -> Duration {
    match priority {
        Priority::New => Duration::from_millis(250),
        Priority::Transition => Duration::from_millis(750),
        Priority::Steady => Duration::from_millis(2500),
    }
}

fn backpressure_factor(queue_depth: usize) -> f32 {
    (1.0 + queue_depth as f32 / (2.0 * MAX_CONCURRENT as f32)).clamp(1.0, 4.0)
}

/// Per-session entry in the dirty registry.
struct DirtyEntry {
    dirty_at: Instant,
    /// Round-robin fairness: least-recently-served goes first.
    /// `None` = never served = highest priority.
    last_served_at: Option<Instant>,
    priority: Priority,
    in_flight: bool,
}

/// Drain loop state, separated from `JoinSet` to avoid borrow conflicts
/// in `tokio::select!` (JoinSet is polled in a branch while state is
/// mutated in the handler).
struct DrainState {
    dirty: HashMap<String, DirtyEntry>,
    error_streak: u32,
    last_error_at: Option<Instant>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<OmlxClient>,
    result_tx: mpsc::Sender<ClassifyResult>,
    omlx_ready: Arc<AtomicBool>,
}

impl DrainState {
    fn mark_dirty(&mut self, session_id: String, priority: Priority) {
        let entry = self.dirty.entry(session_id).or_insert_with(|| DirtyEntry {
            dirty_at: Instant::now(),
            last_served_at: None,
            priority,
            in_flight: false,
        });
        entry.dirty_at = Instant::now();
        if priority < entry.priority {
            entry.priority = priority;
        }
    }

    fn handle_completion(&mut self, result: Result<(String, bool), tokio::task::JoinError>) {
        let (session_id, success) = match result {
            Ok(v) => v,
            Err(_) => return, // Aborted or panicked — JoinSet already freed the slot
        };
        if let Some(entry) = self.dirty.get_mut(&session_id) {
            entry.in_flight = false;
            entry.last_served_at = Some(Instant::now());
            if !success {
                entry.dirty_at = Instant::now();
            }
        }
        if success {
            self.error_streak = 0;
            self.last_error_at = None;
        } else {
            self.error_streak = self.error_streak.saturating_add(1);
            self.last_error_at = Some(Instant::now());
        }
    }

    async fn try_drain(&mut self, tasks: &mut JoinSet<(String, bool)>) {
        if !self.omlx_ready.load(Ordering::Relaxed) {
            return;
        }

        // Exponential cooldown: 500ms × 2^streak, capped at 30s.
        if let Some(last_err) = self.last_error_at {
            let cooldown_ms = (500u64 << self.error_streak.min(6)).min(30_000);
            if last_err.elapsed() < Duration::from_millis(cooldown_ms) {
                return;
            }
        }

        let now = Instant::now();
        let queue_depth = self.dirty.values().filter(|e| !e.in_flight).count();

        while tasks.len() < MAX_CONCURRENT {
            let bp = backpressure_factor(queue_depth);

            // Fair round-robin: among debounce-ready, pick least-recently-served.
            // None < Some(_), so never-served sessions always go first.
            let candidate = self
                .dirty
                .iter()
                .filter(|(_, e)| {
                    !e.in_flight
                        && now.duration_since(e.dirty_at) >= debounce_for(e.priority).mul_f32(bp)
                })
                .min_by_key(|(_, e)| e.last_served_at)
                .map(|(id, _)| id.clone());

            let Some(session_id) = candidate else { break };

            let entry = self.dirty.get_mut(&session_id).unwrap();
            entry.in_flight = true;

            let (context, generation) = {
                let mut accs = self.accumulators.write().await;
                match accs.get_mut(&session_id) {
                    Some(acc) => {
                        acc.classify_generation += 1;
                        (build_context_from_acc(acc), acc.classify_generation)
                    }
                    None => {
                        entry.in_flight = false;
                        self.dirty.remove(&session_id);
                        continue;
                    }
                }
            };

            let client = self.client.clone();
            let result_tx = self.result_tx.clone();
            let sid = session_id;

            tasks.spawn(async move {
                let result = client.classify(&context, 0.15, &sid, generation).await;
                let success = result.is_some();
                if let Some((phase, scope)) = result {
                    let _ = result_tx
                        .send(ClassifyResult {
                            session_id: sid.clone(),
                            phase,
                            scope,
                            generation,
                        })
                        .await;
                }
                (sid, success)
            });
        }
    }
}

/// Run the drain loop as a long-lived tokio task.
pub(crate) async fn run_drain_loop(
    mut dirty_rx: mpsc::Receiver<(String, Priority)>,
    result_tx: mpsc::Sender<ClassifyResult>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<OmlxClient>,
    omlx_ready: Arc<AtomicBool>,
    wake: Arc<Notify>,
) {
    let mut state = DrainState {
        dirty: HashMap::new(),
        error_streak: 0,
        last_error_at: None,
        accumulators,
        client,
        result_tx,
        omlx_ready,
    };
    let mut tasks: JoinSet<(String, bool)> = JoinSet::new();

    loop {
        tokio::select! {
            msg = dirty_rx.recv() => {
                let Some((session_id, priority)) = msg else { break };
                state.mark_dirty(session_id, priority);
                state.try_drain(&mut tasks).await;
            }

            Some(result) = tasks.join_next() => {
                state.handle_completion(result);
                state.try_drain(&mut tasks).await;
            }

            _ = wake.notified() => {
                state.try_drain(&mut tasks).await;
            }

            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                state.try_drain(&mut tasks).await;
            }
        }
    }

    // Graceful shutdown: abort in-flight tasks and wait for cleanup.
    tasks.shutdown().await;
}

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
        assert_eq!(
            debounce_for(Priority::Transition),
            Duration::from_millis(750)
        );
        assert_eq!(debounce_for(Priority::Steady), Duration::from_millis(2500));
    }

    #[test]
    fn backpressure_bounds() {
        assert_eq!(backpressure_factor(0), 1.0);
        assert!(backpressure_factor(4) > 1.0);
        assert_eq!(backpressure_factor(100), 4.0);
    }

    #[test]
    fn round_robin_fairness() {
        // Never-served (None) should sort before any served session.
        let never: Option<Instant> = None;
        let served = Some(Instant::now());
        assert!(
            never < served,
            "None < Some ensures never-served goes first"
        );
    }

    #[test]
    fn priority_promotion() {
        let mut state = DrainState {
            dirty: HashMap::new(),
            error_streak: 0,
            last_error_at: None,
            accumulators: Arc::new(RwLock::new(HashMap::new())),
            client: Arc::new(OmlxClient::new("http://test".into(), "test".into())),
            result_tx: mpsc::channel(1).0,
            omlx_ready: Arc::new(AtomicBool::new(true)),
        };
        state.mark_dirty("s1".into(), Priority::Steady);
        state.mark_dirty("s1".into(), Priority::New);
        assert_eq!(state.dirty["s1"].priority, Priority::New);
    }

    #[test]
    fn handle_completion_resets_streak_on_success() {
        let mut state = DrainState {
            dirty: HashMap::new(),
            error_streak: 5,
            last_error_at: Some(Instant::now()),
            accumulators: Arc::new(RwLock::new(HashMap::new())),
            client: Arc::new(OmlxClient::new("http://test".into(), "test".into())),
            result_tx: mpsc::channel(1).0,
            omlx_ready: Arc::new(AtomicBool::new(true)),
        };
        state.handle_completion(Ok(("s1".into(), true)));
        assert_eq!(state.error_streak, 0);
        assert!(state.last_error_at.is_none());
    }

    #[test]
    fn handle_completion_increments_streak_on_failure() {
        let mut state = DrainState {
            dirty: HashMap::new(),
            error_streak: 2,
            last_error_at: None,
            accumulators: Arc::new(RwLock::new(HashMap::new())),
            client: Arc::new(OmlxClient::new("http://test".into(), "test".into())),
            result_tx: mpsc::channel(1).0,
            omlx_ready: Arc::new(AtomicBool::new(true)),
        };
        state.dirty.insert(
            "s1".into(),
            DirtyEntry {
                dirty_at: Instant::now() - Duration::from_secs(10),
                last_served_at: None,
                priority: Priority::Steady,
                in_flight: true,
            },
        );
        state.handle_completion(Ok(("s1".into(), false)));
        assert_eq!(state.error_streak, 3);
        assert!(state.last_error_at.is_some());
        // dirty_at should be reset (debounce restart)
        assert!(state.dirty["s1"].dirty_at.elapsed() < Duration::from_millis(50));
    }
}
