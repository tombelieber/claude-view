use std::time::{Duration, Instant};

use claude_view_core::phase::scheduler::Priority;
use claude_view_core::phase::SessionPhase;

use super::state::DrainState;
use super::types::{
    backpressure_factor, idle_gap_for, DirtyEntry, BASE_BUDGET_SECS, MAX_BUDGET_SECS,
};

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

    entry.record_result(SessionPhase::Building);
    assert_eq!(entry.consecutive_same, 1);
    assert_eq!(
        entry.current_budget,
        Duration::from_secs(BASE_BUDGET_SECS * 2)
    );

    entry.record_result(SessionPhase::Building);
    assert_eq!(entry.consecutive_same, 2);
    assert_eq!(
        entry.current_budget,
        Duration::from_secs(BASE_BUDGET_SECS * 4)
    );
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

    entry.record_result(SessionPhase::Testing);
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
        entry.record_result(SessionPhase::Building);
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

    // With multiplier 2.0 (efficient mode), budget = 10s, only 8s elapsed -> not ready
    assert!(!DrainState::is_idle_ready_with_mode(
        &entry,
        Instant::now(),
        1.0,
        2.0
    ));

    // With multiplier 0.5 (realtime mode), budget = 2.5s, 8s elapsed -> ready
    assert!(DrainState::is_idle_ready_with_mode(
        &entry,
        Instant::now(),
        1.0,
        0.5
    ));
}

#[test]
fn settled_needsyou_scenario() {
    // Simulates: session classified once, then goes NeedsYou
    // -> should not be re-classified (entry would be removed by lifecycle gate)
    let entry = DirtyEntry {
        last_activity_at: Instant::now() - Duration::from_secs(60),
        last_served_at: Some(Instant::now() - Duration::from_secs(50)),
        priority: Priority::Steady,
        in_flight: false,
        consecutive_same: 3,
        current_budget: Duration::from_secs(40),
        last_phase: Some(SessionPhase::Building),
        has_user_turn_signal: false,
    };
    // Entry IS idle_ready by timing alone...
    assert!(DrainState::is_idle_ready(&entry, Instant::now(), 1.0));
    // ...but lifecycle gate would remove it (tested via should_skip_for_lifecycle)
}

#[test]
fn user_turn_preempts_backoff() {
    let mut entry = DirtyEntry {
        last_activity_at: Instant::now(),
        last_served_at: Some(Instant::now()),
        priority: Priority::Steady,
        in_flight: false,
        consecutive_same: 5,
        current_budget: Duration::from_secs(60), // backed off to max
        last_phase: Some(SessionPhase::Building),
        has_user_turn_signal: false,
    };

    // Backed off -> not ready for a long time
    assert!(!DrainState::is_idle_ready(
        &entry,
        Instant::now() + Duration::from_secs(10),
        1.0
    ));

    // User sends new message -> budget resets
    entry.signal_user_turn();
    assert_eq!(entry.current_budget, Duration::from_secs(BASE_BUDGET_SECS));

    // Now ready much sooner
    let future = Instant::now() + Duration::from_secs(6);
    entry.last_activity_at = Instant::now();
    entry.last_served_at = Some(Instant::now());
    assert!(DrainState::is_idle_ready(&entry, future, 1.0));
}

#[test]
fn efficient_mode_doubles_effective_budget() {
    let entry = DirtyEntry {
        last_activity_at: Instant::now() - Duration::from_secs(60),
        last_served_at: Some(Instant::now() - Duration::from_secs(9)),
        priority: Priority::Steady,
        in_flight: false,
        consecutive_same: 0,
        current_budget: Duration::from_secs(BASE_BUDGET_SECS), // 5s
        last_phase: None,
        has_user_turn_signal: false,
    };

    // Balanced (1.0): budget=5s, 9s elapsed -> ready
    assert!(DrainState::is_idle_ready_with_mode(
        &entry,
        Instant::now(),
        1.0,
        1.0
    ));

    // Efficient (2.0): effective budget=10s, 9s elapsed -> not ready
    assert!(!DrainState::is_idle_ready_with_mode(
        &entry,
        Instant::now(),
        1.0,
        2.0
    ));
}
