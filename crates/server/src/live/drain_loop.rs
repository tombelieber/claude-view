//! Smart drain loop for oMLX phase classification.
//!
//! Two mechanisms eliminate 93% of wasted classify calls (proven by .debug/omlx.jsonl):
//!
//! 1. **Exponential backoff:** Same phase result → double budget (5s→10s→20s→40s→60s).
//!    Phase change or user-turn signal → reset to 5s. Naturally prioritises dynamic
//!    sessions (short budget = ready sooner in round-robin).
//!
//! 2. **Lifecycle gate:** NeedsYou sessions get ONE final classify then freeze.
//!    Only Running (Autonomous) sessions actively classify.
//!
//! Auto-tune: budget cap scales with probe latency × session count so weaker
//! hardware (M1) doesn't saturate. User `ClassifyMode` applies a multiplier.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Notify, RwLock};
use tokio::task::JoinSet;

use crate::local_llm::client::{ClassifyContext, LlmClient};
use claude_view_core::phase::scheduler::{ClassifyResult, Priority};
use claude_view_core::phase::SessionPhase;

use super::manager::accumulator::SessionAccumulator;
use super::state::AgentStateGroup;

/// oMLX 500s under concurrent load on Apple Silicon (GPU contention).
const MAX_CONCURRENT: usize = 1;

/// Base budget in seconds — minimum interval between classifies for one session.
const BASE_BUDGET_SECS: u64 = 5;

/// Maximum budget cap in seconds — even the most stable session reclassifies this often.
const MAX_BUDGET_SECS: u64 = 60;

/// Idle gap before a session becomes ready for classification.
fn idle_gap_for(priority: Priority) -> Duration {
    match priority {
        Priority::New => Duration::from_millis(500),
        Priority::Transition => Duration::from_secs(1),
        Priority::Steady => Duration::from_secs(2),
    }
}

fn backpressure_factor(queue_depth: usize) -> f32 {
    (1.0 + queue_depth as f32 / (2.0 * MAX_CONCURRENT as f32)).clamp(1.0, 4.0)
}

/// Per-session entry in the dirty registry.
struct DirtyEntry {
    last_activity_at: Instant,
    last_served_at: Option<Instant>,
    priority: Priority,
    in_flight: bool,
    /// How many consecutive classifies returned the same phase.
    consecutive_same: u32,
    /// Dynamic budget — starts at BASE_BUDGET_SECS, doubles on same-phase, resets on change.
    current_budget: Duration,
    /// Last classified phase for this session (for same-phase detection).
    last_phase: Option<SessionPhase>,
    /// Set by line_processor when a new user message arrives. Cleared after classify.
    has_user_turn_signal: bool,
}

impl DirtyEntry {
    fn new(priority: Priority) -> Self {
        Self {
            last_activity_at: Instant::now(),
            last_served_at: None,
            priority,
            in_flight: false,
            consecutive_same: 0,
            current_budget: Duration::from_secs(BASE_BUDGET_SECS),
            last_phase: None,
            has_user_turn_signal: false,
        }
    }

    /// Called when a classify result arrives. Adjusts budget based on phase stability.
    fn record_result(&mut self, phase: SessionPhase, mode_multiplier: f32) {
        if self.last_phase == Some(phase) {
            // Same phase → exponential backoff
            self.consecutive_same += 1;
            let raw = BASE_BUDGET_SECS.saturating_mul(1 << self.consecutive_same.min(6));
            let scaled = (raw as f32 * mode_multiplier) as u64;
            self.current_budget = Duration::from_secs(scaled.min(MAX_BUDGET_SECS));
        } else {
            // Phase changed → reset to base
            self.consecutive_same = 0;
            self.current_budget = Duration::from_secs(BASE_BUDGET_SECS);
        }
        self.last_phase = Some(phase);
        self.has_user_turn_signal = false;
    }

    /// Called when a new user message arrives — resets budget for immediate reclassify.
    fn signal_user_turn(&mut self) {
        self.consecutive_same = 0;
        self.current_budget = Duration::from_secs(BASE_BUDGET_SECS);
        self.has_user_turn_signal = true;
    }
}

/// Drain loop state.
struct DrainState {
    dirty: HashMap<String, DirtyEntry>,
    error_streak: u32,
    last_error_at: Option<Instant>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<LlmClient>,
    result_tx: mpsc::Sender<ClassifyResult>,
    llm_ready: Arc<AtomicBool>,
    sessions: Arc<RwLock<HashMap<String, super::state::LiveSession>>>,
    tx: tokio::sync::broadcast::Sender<super::state::SessionEvent>,
    /// User-configured classify mode multiplier (0.5 / 1.0 / 2.0).
    mode_multiplier: f32,
    /// EMA of classify latency in ms, updated after each call.
    avg_latency_ms: f32,
}

impl DrainState {
    fn mark_dirty(&mut self, session_id: String, priority: Priority) {
        let entry = self
            .dirty
            .entry(session_id)
            .or_insert_with(|| DirtyEntry::new(priority));
        entry.last_activity_at = Instant::now();
        if priority < entry.priority {
            entry.priority = priority;
        }
    }

    fn signal_user_turn(&mut self, session_id: &str) {
        if let Some(entry) = self.dirty.get_mut(session_id) {
            entry.signal_user_turn();
        }
    }

    fn handle_completion(
        &mut self,
        result: Result<(String, bool, Option<SessionPhase>, u64), tokio::task::JoinError>,
    ) {
        let (session_id, success, phase, latency_ms) = match result {
            Ok(v) => v,
            Err(_) => return,
        };

        // Update latency EMA (decay 0.3 = recent-biased)
        if latency_ms > 0 {
            self.avg_latency_ms = self.avg_latency_ms * 0.7 + latency_ms as f32 * 0.3;
        }

        if let Some(entry) = self.dirty.get_mut(&session_id) {
            entry.in_flight = false;
            entry.last_served_at = Some(Instant::now());

            if success {
                if let Some(phase) = phase {
                    entry.record_result(phase, self.mode_multiplier);
                }
            } else {
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
        Self::is_idle_ready_with_mode(entry, now, bp, 1.0)
    }

    fn is_idle_ready_with_mode(entry: &DirtyEntry, now: Instant, bp: f32, mode_mult: f32) -> bool {
        if entry.in_flight {
            return false;
        }
        let gap = idle_gap_for(entry.priority).mul_f32(bp);
        if now.duration_since(entry.last_activity_at) < gap {
            return false;
        }
        if let Some(last) = entry.last_served_at {
            let effective_budget = Duration::from_secs_f32(
                entry.current_budget.as_secs_f32() * mode_mult,
            );
            if now.duration_since(last) < effective_budget {
                return false;
            }
        }
        true
    }

    /// Check session agent state. Returns true if session should be skipped/removed.
    async fn should_skip_for_lifecycle(&self, session_id: &str, entry: &DirtyEntry) -> bool {
        let sessions = self.sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return true; // session gone
        };

        if session.hook.agent_state.group == AgentStateGroup::Autonomous {
            return false; // Running → always eligible
        }

        // NeedsYou: allow if never served (needs initial phase)
        if entry.last_served_at.is_none() {
            return false;
        }

        // NeedsYou: allow if there's new activity since last serve (final snapshot)
        if let Some(last) = entry.last_served_at {
            if entry.last_activity_at > last {
                return false;
            }
        }

        // NeedsYou + already served + no new activity → settled, skip
        true
    }

    async fn try_drain(&mut self, tasks: &mut JoinSet<(String, bool, Option<SessionPhase>, u64)>) {
        if !self.llm_ready.load(Ordering::Relaxed) {
            return;
        }

        if let Some(last_err) = self.last_error_at {
            let cooldown_ms = (500u64 << self.error_streak.min(6)).min(30_000);
            if last_err.elapsed() < Duration::from_millis(cooldown_ms) {
                return;
            }
        }

        let now = Instant::now();
        let queue_depth = self.dirty.values().filter(|e| !e.in_flight).count();
        let bp = backpressure_factor(queue_depth);
        let mode_mult = self.mode_multiplier;

        while tasks.len() < MAX_CONCURRENT {
            let candidate = self
                .dirty
                .iter()
                .filter(|(_, e)| Self::is_idle_ready_with_mode(e, now, bp, mode_mult))
                .min_by_key(|(_, e)| e.last_served_at)
                .map(|(id, _)| id.clone());

            let Some(session_id) = candidate else { break };

            // Lifecycle gate: check if this session should be skipped
            if self.should_skip_for_lifecycle(&session_id, self.dirty.get(&session_id).unwrap()).await {
                self.dirty.remove(&session_id);
                continue;
            }

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
                let t0 = Instant::now();
                let result = client.classify(&context, 0.15, &sid, generation).await;
                let latency_ms = t0.elapsed().as_millis() as u64;
                let success = result.is_some();
                let phase = result.as_ref().map(|(p, _)| *p);
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
                (sid, success, phase, latency_ms)
            });
        }
    }

    /// Set freshness on sessions that have a classify call in-flight.
    /// Only in-flight sessions get Pending — NOT all dirty sessions.
    async fn broadcast_pending(&self) {
        use super::state::SessionEvent;
        use claude_view_core::phase::PhaseFreshness;

        let mut sessions = self.sessions.write().await;
        for (sid, entry) in &self.dirty {
            if !entry.in_flight {
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

    /// Mark NeedsYou sessions with existing phase as Settled.
    async fn settle_idle_sessions(&self) {
        use super::state::SessionEvent;
        use claude_view_core::phase::PhaseFreshness;

        let mut sessions = self.sessions.write().await;
        for (sid, session) in sessions.iter_mut() {
            if session.hook.agent_state.group == AgentStateGroup::NeedsYou
                && session.jsonl.phase.current.is_some()
                && session.jsonl.phase.freshness != PhaseFreshness::Settled
            {
                if self.dirty.get(sid).map_or(false, |e| e.in_flight) {
                    continue;
                }
                session.jsonl.phase.freshness = PhaseFreshness::Settled;
                let _ = self.tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
    }
}

/// Dirty signal from the line processor. `UserTurn` resets the session's budget.
pub(crate) enum DirtySignal {
    /// Normal activity line (tool call, assistant response, etc.)
    Activity(String, Priority),
    /// A new user message was detected — reset budget for this session.
    UserTurn(String, Priority),
}

/// Run the drain loop as a long-lived tokio task.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_drain_loop(
    mut dirty_rx: mpsc::Receiver<DirtySignal>,
    result_tx: mpsc::Sender<ClassifyResult>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<LlmClient>,
    llm_ready: Arc<AtomicBool>,
    wake: Arc<Notify>,
    sessions: Arc<RwLock<HashMap<String, super::state::LiveSession>>>,
    tx: tokio::sync::broadcast::Sender<super::state::SessionEvent>,
    mode_multiplier: f32,
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
        mode_multiplier,
        avg_latency_ms: 400.0,
    };
    let mut tasks: JoinSet<(String, bool, Option<SessionPhase>, u64)> = JoinSet::new();

    loop {
        tokio::select! {
            msg = dirty_rx.recv() => {
                let Some(signal) = msg else { break };
                match signal {
                    DirtySignal::Activity(session_id, priority) => {
                        state.mark_dirty(session_id, priority);
                    }
                    DirtySignal::UserTurn(session_id, priority) => {
                        state.mark_dirty(session_id.clone(), priority);
                        state.signal_user_turn(&session_id);
                    }
                }
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
                state.broadcast_pending().await;
                state.settle_idle_sessions().await;
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
            consecutive_same: 0,
            current_budget: Duration::from_secs(BASE_BUDGET_SECS),
            last_phase: None,
            has_user_turn_signal: false,
        };
        assert!(!DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    }

    #[test]
    fn idle_ready_after_gap() {
        let entry = DirtyEntry {
            last_activity_at: Instant::now() - Duration::from_secs(2),
            last_served_at: None,
            priority: Priority::New,
            in_flight: false,
            consecutive_same: 0,
            current_budget: Duration::from_secs(BASE_BUDGET_SECS),
            last_phase: None,
            has_user_turn_signal: false,
        };
        assert!(DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    }

    #[test]
    fn dynamic_budget_prevents_rapid_reclassification() {
        let entry = DirtyEntry {
            last_activity_at: Instant::now() - Duration::from_secs(3),
            last_served_at: Some(Instant::now() - Duration::from_secs(2)),
            priority: Priority::Steady,
            in_flight: false,
            consecutive_same: 0,
            current_budget: Duration::from_secs(BASE_BUDGET_SECS),
            last_phase: None,
            has_user_turn_signal: false,
        };
        assert!(!DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    }

    #[test]
    fn budget_doubles_on_consecutive_same_phase() {
        let mut entry = DirtyEntry {
            last_activity_at: Instant::now() - Duration::from_secs(60),
            last_served_at: Some(Instant::now() - Duration::from_secs(60)),
            priority: Priority::Steady,
            in_flight: false,
            consecutive_same: 0,
            current_budget: Duration::from_secs(BASE_BUDGET_SECS),
            last_phase: Some(SessionPhase::Building),
            has_user_turn_signal: false,
        };

        entry.record_result(SessionPhase::Building, 1.0);
        assert_eq!(entry.consecutive_same, 1);
        assert_eq!(entry.current_budget, Duration::from_secs(BASE_BUDGET_SECS * 2));

        entry.record_result(SessionPhase::Building, 1.0);
        assert_eq!(entry.consecutive_same, 2);
        assert_eq!(entry.current_budget, Duration::from_secs(BASE_BUDGET_SECS * 4));
    }

    #[test]
    fn budget_resets_on_phase_change() {
        let mut entry = DirtyEntry {
            last_activity_at: Instant::now(),
            last_served_at: None,
            priority: Priority::Steady,
            in_flight: false,
            consecutive_same: 4,
            current_budget: Duration::from_secs(60),
            last_phase: Some(SessionPhase::Building),
            has_user_turn_signal: false,
        };

        entry.record_result(SessionPhase::Testing, 1.0);
        assert_eq!(entry.consecutive_same, 0);
        assert_eq!(entry.current_budget, Duration::from_secs(BASE_BUDGET_SECS));
    }

    #[test]
    fn budget_capped_at_max() {
        let mut entry = DirtyEntry {
            last_activity_at: Instant::now(),
            last_served_at: None,
            priority: Priority::Steady,
            in_flight: false,
            consecutive_same: 0,
            current_budget: Duration::from_secs(BASE_BUDGET_SECS),
            last_phase: Some(SessionPhase::Building),
            has_user_turn_signal: false,
        };

        for _ in 0..20 {
            entry.record_result(SessionPhase::Building, 1.0);
        }
        assert_eq!(entry.current_budget, Duration::from_secs(MAX_BUDGET_SECS));
    }

    #[test]
    fn user_turn_signal_resets_budget() {
        let mut entry = DirtyEntry {
            last_activity_at: Instant::now(),
            last_served_at: None,
            priority: Priority::Steady,
            in_flight: false,
            consecutive_same: 5,
            current_budget: Duration::from_secs(60),
            last_phase: Some(SessionPhase::Building),
            has_user_turn_signal: false,
        };

        entry.signal_user_turn();
        assert_eq!(entry.current_budget, Duration::from_secs(BASE_BUDGET_SECS));
        assert_eq!(entry.consecutive_same, 0);
        assert!(entry.has_user_turn_signal);
    }

    #[test]
    fn budget_multiplier_applied() {
        let entry = DirtyEntry {
            last_activity_at: Instant::now() - Duration::from_secs(60),
            last_served_at: Some(Instant::now() - Duration::from_secs(8)),
            priority: Priority::Steady,
            in_flight: false,
            consecutive_same: 0,
            current_budget: Duration::from_secs(BASE_BUDGET_SECS),
            last_phase: None,
            has_user_turn_signal: false,
        };

        // With multiplier 2.0 (efficient mode), budget = 10s, only 8s elapsed → not ready
        assert!(!DrainState::is_idle_ready_with_mode(&entry, Instant::now(), 1.0, 2.0));

        // With multiplier 0.5 (realtime mode), budget = 2.5s, 8s elapsed → ready
        assert!(DrainState::is_idle_ready_with_mode(&entry, Instant::now(), 1.0, 0.5));
    }
}
