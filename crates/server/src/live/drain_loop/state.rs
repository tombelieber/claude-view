use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tokio::task::JoinSet;

use crate::live::manager::accumulator::SessionAccumulator;
use crate::live::state::AgentStateGroup;
use crate::local_llm::client::{ClassifyContext, LlmClient};
use claude_view_core::phase::scheduler::{ClassifyResult, Priority};
use claude_view_core::phase::SessionPhase;

use super::types::{backpressure_factor, idle_gap_for, DirtyEntry, MAX_CONCURRENT};

/// Drain loop state.
pub(super) struct DrainState {
    pub(super) dirty: HashMap<String, DirtyEntry>,
    pub(super) error_streak: u32,
    pub(super) last_error_at: Option<Instant>,
    pub(super) accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    pub(super) client: Arc<LlmClient>,
    pub(super) result_tx: tokio::sync::mpsc::Sender<ClassifyResult>,
    pub(super) llm_ready: Arc<AtomicBool>,
    pub(super) sessions: Arc<RwLock<HashMap<String, crate::live::state::LiveSession>>>,
    pub(super) tx: tokio::sync::broadcast::Sender<crate::live::state::SessionEvent>,
    /// User-configured classify mode multiplier (0.5 / 1.0 / 2.0).
    pub(super) mode_multiplier: f32,
    /// EMA of classify latency in ms, updated after each call.
    pub(super) avg_latency_ms: f32,
}

impl DrainState {
    pub(super) fn mark_dirty(&mut self, session_id: String, priority: Priority) {
        let entry = self
            .dirty
            .entry(session_id)
            .or_insert_with(|| DirtyEntry::new(priority));
        entry.last_activity_at = Instant::now();
        if priority < entry.priority {
            entry.priority = priority;
        }
    }

    pub(super) fn signal_user_turn(&mut self, session_id: &str) {
        if let Some(entry) = self.dirty.get_mut(session_id) {
            entry.signal_user_turn();
        }
    }

    pub(super) fn handle_completion(
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
                    entry.record_result(phase);
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

    #[expect(dead_code, reason = "reserved for non-mode-multiplied callers")]
    pub(super) fn is_idle_ready(entry: &DirtyEntry, now: Instant, bp: f32) -> bool {
        Self::is_idle_ready_with_mode(entry, now, bp, 1.0)
    }

    pub(super) fn is_idle_ready_with_mode(
        entry: &DirtyEntry,
        now: Instant,
        bp: f32,
        mode_mult: f32,
    ) -> bool {
        if entry.in_flight {
            return false;
        }
        let gap = idle_gap_for(entry.priority).mul_f32(bp);
        if now.duration_since(entry.last_activity_at) < gap {
            return false;
        }
        if let Some(last) = entry.last_served_at {
            let effective_budget =
                Duration::from_secs_f32(entry.current_budget.as_secs_f32() * mode_mult);
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
            return false; // Running -> always eligible
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

        // NeedsYou + already served + no new activity -> settled, skip
        true
    }

    pub(super) async fn try_drain(
        &mut self,
        tasks: &mut JoinSet<(String, bool, Option<SessionPhase>, u64)>,
    ) {
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
            if self
                .should_skip_for_lifecycle(&session_id, self.dirty.get(&session_id).unwrap())
                .await
            {
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
    /// Only in-flight sessions get Pending -- NOT all dirty sessions.
    pub(super) async fn broadcast_pending(&self) {
        use crate::live::state::SessionEvent;
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
    pub(super) async fn settle_idle_sessions(&self) {
        use crate::live::state::SessionEvent;
        use claude_view_core::phase::PhaseFreshness;

        let mut sessions = self.sessions.write().await;
        for (sid, session) in sessions.iter_mut() {
            if session.hook.agent_state.group == AgentStateGroup::NeedsYou
                && session.jsonl.phase.current.is_some()
                && session.jsonl.phase.freshness != PhaseFreshness::Settled
            {
                if self.dirty.get(sid).is_some_and(|e| e.in_flight) {
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
