use std::time::{Duration, Instant};

use claude_view_core::phase::scheduler::Priority;
use claude_view_core::phase::SessionPhase;

/// oMLX 500s under concurrent load on Apple Silicon (GPU contention).
pub(super) const MAX_CONCURRENT: usize = 1;

/// Base budget in seconds -- minimum interval between classifies for one session.
pub(super) const BASE_BUDGET_SECS: u64 = 5;

/// Maximum budget cap in seconds -- even the most stable session reclassifies this often.
pub(super) const MAX_BUDGET_SECS: u64 = 60;

/// Idle gap before a session becomes ready for classification.
pub(super) fn idle_gap_for(priority: Priority) -> Duration {
    match priority {
        Priority::New => Duration::from_millis(500),
        Priority::Transition => Duration::from_secs(1),
        Priority::Steady => Duration::from_secs(2),
    }
}

pub(super) fn backpressure_factor(queue_depth: usize) -> f32 {
    (1.0 + queue_depth as f32 / (2.0 * MAX_CONCURRENT as f32)).clamp(1.0, 4.0)
}

/// Per-session entry in the dirty registry.
pub(super) struct DirtyEntry {
    pub(super) last_activity_at: Instant,
    pub(super) last_served_at: Option<Instant>,
    pub(super) priority: Priority,
    pub(super) in_flight: bool,
    /// How many consecutive classifies returned the same phase.
    pub(super) consecutive_same: u32,
    /// Dynamic budget -- starts at BASE_BUDGET_SECS, doubles on same-phase, resets on change.
    pub(super) current_budget: Duration,
    /// Last classified phase for this session (for same-phase detection).
    pub(super) last_phase: Option<SessionPhase>,
    /// Set by line_processor when a new user message arrives. Cleared after classify.
    pub(super) has_user_turn_signal: bool,
}

impl DirtyEntry {
    pub(super) fn new(priority: Priority) -> Self {
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
    /// Stores RAW budget -- mode_multiplier is applied at check time in is_idle_ready_with_mode.
    pub(super) fn record_result(&mut self, phase: SessionPhase) {
        if self.last_phase == Some(phase) {
            // Same phase -> exponential backoff
            self.consecutive_same += 1;
            let raw = BASE_BUDGET_SECS.saturating_mul(1 << self.consecutive_same.min(6));
            self.current_budget = Duration::from_secs(raw.min(MAX_BUDGET_SECS));
        } else {
            // Phase changed -> reset to base
            self.consecutive_same = 0;
            self.current_budget = Duration::from_secs(BASE_BUDGET_SECS);
        }
        self.last_phase = Some(phase);
        self.has_user_turn_signal = false;
    }

    /// Called when a new user message arrives -- resets budget for immediate reclassify.
    pub(super) fn signal_user_turn(&mut self) {
        self.consecutive_same = 0;
        self.current_budget = Duration::from_secs(BASE_BUDGET_SECS);
        self.has_user_turn_signal = true;
    }
}

/// Dirty signal from the line processor. `UserTurn` resets the session's budget.
pub(crate) enum DirtySignal {
    /// Normal activity line (tool call, assistant response, etc.)
    Activity(String, Priority),
    /// A new user message was detected -- reset budget for this session.
    UserTurn(String, Priority),
}
