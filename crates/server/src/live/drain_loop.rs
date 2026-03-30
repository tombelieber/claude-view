//! Event-driven drain loop for oMLX phase classification.
//!
//! Uses idle-gap detection instead of fixed debounce: classify only when a
//! session's JSONL activity pauses (model is "thinking" or user is reading).
//! This naturally coalesces rapid tool bursts into a single classify call.
//! Fair round-robin via `last_served_at` prevents any session from starving.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Notify, RwLock};
use tokio::task::JoinSet;

use crate::local_llm::client::{ClassifyContext, LlmClient};
use claude_view_core::phase::scheduler::{ClassifyResult, Priority};

use super::manager::accumulator::SessionAccumulator;

/// oMLX 500s under concurrent load on Apple Silicon (GPU contention with
/// server's DB writes + JSONL parsing). Proven: 92% failure at 2 concurrent.
const MAX_CONCURRENT: usize = 1;

/// Idle gap before a session becomes ready for classification.
/// Classify when activity pauses — not during rapid tool-call bursts.
fn idle_gap_for(priority: Priority) -> Duration {
    match priority {
        Priority::New => Duration::from_millis(500),
        Priority::Transition => Duration::from_secs(1),
        Priority::Steady => Duration::from_secs(2),
    }
}

/// Per-session budget: minimum interval between successive classifications.
/// Even during constant activity, a session won't be classified more often
/// than this. Prevents one chatty session from dominating the oMLX slot.
const MIN_INTERVAL_SECS: u64 = 5;

fn backpressure_factor(queue_depth: usize) -> f32 {
    (1.0 + queue_depth as f32 / (2.0 * MAX_CONCURRENT as f32)).clamp(1.0, 4.0)
}

/// Per-session entry in the dirty registry.
struct DirtyEntry {
    /// Most recent JSONL activity. Reset on every dirty notification.
    /// Classify only when `now - last_activity_at >= idle_gap`.
    last_activity_at: Instant,
    /// Round-robin fairness: least-recently-served goes first.
    /// `None` = never served = highest priority.
    last_served_at: Option<Instant>,
    priority: Priority,
    in_flight: bool,
}

/// Drain loop state, separated from `JoinSet` to avoid borrow conflicts.
struct DrainState {
    dirty: HashMap<String, DirtyEntry>,
    error_streak: u32,
    last_error_at: Option<Instant>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<LlmClient>,
    result_tx: mpsc::Sender<ClassifyResult>,
    llm_ready: Arc<AtomicBool>,
    /// Shared sessions map for setting freshness=pending on dirty.
    sessions: Arc<RwLock<HashMap<String, super::state::LiveSession>>>,
    tx: tokio::sync::broadcast::Sender<super::state::SessionEvent>,
}

impl DrainState {
    fn mark_dirty(&mut self, session_id: String, priority: Priority) {
        let entry = self.dirty.entry(session_id).or_insert_with(|| DirtyEntry {
            last_activity_at: Instant::now(),
            last_served_at: None,
            priority,
            in_flight: false,
        });
        entry.last_activity_at = Instant::now();
        if priority < entry.priority {
            entry.priority = priority;
        }
    }

    fn handle_completion(&mut self, result: Result<(String, bool), tokio::task::JoinError>) {
        let (session_id, success) = match result {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(entry) = self.dirty.get_mut(&session_id) {
            entry.in_flight = false;
            entry.last_served_at = Some(Instant::now());
            if !success {
                // Reset activity timestamp so idle gap restarts.
                entry.last_activity_at = Instant::now();
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

    fn is_idle_ready(entry: &DirtyEntry, now: Instant, bp: f32) -> bool {
        if entry.in_flight {
            return false;
        }
        // Idle gap check: activity must have paused long enough.
        let gap = idle_gap_for(entry.priority).mul_f32(bp);
        if now.duration_since(entry.last_activity_at) < gap {
            return false;
        }
        // Budget check: don't reclassify too soon after last classification.
        if let Some(last) = entry.last_served_at {
            if now.duration_since(last) < Duration::from_secs(MIN_INTERVAL_SECS) {
                return false;
            }
        }
        true
    }

    async fn try_drain(&mut self, tasks: &mut JoinSet<(String, bool)>) {
        if !self.llm_ready.load(Ordering::Relaxed) {
            return;
        }

        // Exponential cooldown after consecutive errors.
        if let Some(last_err) = self.last_error_at {
            let cooldown_ms = (500u64 << self.error_streak.min(6)).min(30_000);
            if last_err.elapsed() < Duration::from_millis(cooldown_ms) {
                return;
            }
        }

        let now = Instant::now();
        let queue_depth = self.dirty.values().filter(|e| !e.in_flight).count();
        let bp = backpressure_factor(queue_depth);

        while tasks.len() < MAX_CONCURRENT {
            // Fair round-robin among idle-ready sessions.
            let candidate = self
                .dirty
                .iter()
                .filter(|(_, e)| Self::is_idle_ready(e, now, bp))
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

    /// Set freshness=pending on sessions that are dirty but not yet classified,
    /// then broadcast the update so the frontend can show the breathing animation.
    async fn broadcast_pending(&self) {
        use super::state::SessionEvent;
        use claude_view_core::phase::PhaseFreshness;

        let mut sessions = self.sessions.write().await;
        for (sid, entry) in &self.dirty {
            // Only mark pending if the session has at least one prior classification
            // (otherwise there's no badge to animate).
            if entry.in_flight {
                continue;
            }
            if let Some(session) = sessions.get_mut(sid) {
                if session.jsonl.phase.current.is_some()
                    && session.jsonl.phase.freshness != PhaseFreshness::Pending
                {
                    session.jsonl.phase.freshness = PhaseFreshness::Pending;
                    let _ = self.tx.send(SessionEvent::SessionUpdated {
                        session: session.clone(),
                    });
                }
            }
        }
    }
}

/// Run the drain loop as a long-lived tokio task.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_drain_loop(
    mut dirty_rx: mpsc::Receiver<(String, Priority)>,
    result_tx: mpsc::Sender<ClassifyResult>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<LlmClient>,
    llm_ready: Arc<AtomicBool>,
    wake: Arc<Notify>,
    sessions: Arc<RwLock<HashMap<String, super::state::LiveSession>>>,
    tx: tokio::sync::broadcast::Sender<super::state::SessionEvent>,
) {
    let mut state = DrainState {
        dirty: HashMap::new(),
        error_streak: 0,
        last_error_at: None,
        accumulators,
        client,
        result_tx,
        llm_ready,
        sessions,
        tx,
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

            // Tick: check for idle-gap expiry + broadcast pending freshness.
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                state.broadcast_pending().await;
                state.try_drain(&mut tasks).await;
            }
        }
    }

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
    fn idle_gap_values() {
        assert_eq!(idle_gap_for(Priority::New), Duration::from_millis(500));
        assert_eq!(idle_gap_for(Priority::Transition), Duration::from_secs(1));
        assert_eq!(idle_gap_for(Priority::Steady), Duration::from_secs(2));
    }

    #[test]
    fn backpressure_bounds() {
        assert_eq!(backpressure_factor(0), 1.0);
        assert!(backpressure_factor(4) > 1.0);
        assert_eq!(backpressure_factor(100), 4.0);
    }

    #[test]
    fn round_robin_fairness() {
        let never: Option<Instant> = None;
        let served = Some(Instant::now());
        assert!(
            never < served,
            "None < Some ensures never-served goes first"
        );
    }

    #[test]
    fn idle_ready_respects_gap() {
        let entry = DirtyEntry {
            last_activity_at: Instant::now(),
            last_served_at: None,
            priority: Priority::New,
            in_flight: false,
        };
        // Just dirtied — not ready yet (gap not elapsed).
        assert!(!DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    }

    #[test]
    fn idle_ready_after_gap() {
        let entry = DirtyEntry {
            last_activity_at: Instant::now() - Duration::from_secs(2),
            last_served_at: None,
            priority: Priority::New,
            in_flight: false,
        };
        // 2s since last activity, gap is 500ms → ready.
        assert!(DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    }

    #[test]
    fn budget_prevents_rapid_reclassification() {
        let entry = DirtyEntry {
            last_activity_at: Instant::now() - Duration::from_secs(3),
            last_served_at: Some(Instant::now() - Duration::from_secs(2)),
            priority: Priority::Steady,
            in_flight: false,
        };
        // 3s since activity (gap OK), but only 2s since last serve (budget=5s).
        assert!(!DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    }

    #[test]
    fn budget_allows_after_interval() {
        let entry = DirtyEntry {
            last_activity_at: Instant::now() - Duration::from_secs(10),
            last_served_at: Some(Instant::now() - Duration::from_secs(6)),
            priority: Priority::Steady,
            in_flight: false,
        };
        // 10s since activity, 6s since last serve (>5s budget) → ready.
        assert!(DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    }
}
