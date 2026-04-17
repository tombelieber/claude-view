//! Stage C rollup smoke tests — public-API exercise.
//!
//! These tests pin down the observable contract for PeriodStats + rollup:
//!   - empty-zero identity element
//!   - empty-items rollup returns EMPTY
//!   - token summation matches SessionStats fields (all 4 token families)
//!   - session_count and duration_count match items.len()
//!
//! Associativity / commutativity / merge-identity laws are proptest'd in
//! `tests/associativity.rs`. Keep smoke tests here deterministic.

use claude_view_core::session_stats::SessionStats;
use claude_view_stats_rollup::{rollup, Bucket, PeriodStats, ROLLUP_VERSION};

#[test]
fn empty_is_zero() {
    assert_eq!(PeriodStats::EMPTY.session_count, 0);
    assert_eq!(PeriodStats::EMPTY.total_tokens, 0);
    assert_eq!(PeriodStats::EMPTY.total_cost_cents, 0);
    assert_eq!(PeriodStats::EMPTY.prompt_count, 0);
    assert_eq!(PeriodStats::EMPTY.file_count, 0);
    assert_eq!(PeriodStats::EMPTY.lines_added, 0);
    assert_eq!(PeriodStats::EMPTY.lines_removed, 0);
    assert_eq!(PeriodStats::EMPTY.commit_count, 0);
    assert_eq!(PeriodStats::EMPTY.duration_sum_ms, 0);
    assert_eq!(PeriodStats::EMPTY.duration_count, 0);
    assert_eq!(PeriodStats::EMPTY.reedit_rate_sum, 0.0);
    assert_eq!(PeriodStats::EMPTY.reedit_rate_count, 0);
}

#[test]
fn rollup_empty_items_returns_empty() {
    let result = rollup(&[], ROLLUP_VERSION, Bucket::Daily);
    assert_eq!(result, PeriodStats::EMPTY);
}

#[test]
fn rollup_sums_tokens() {
    let s1 = SessionStats {
        total_input_tokens: 100,
        total_output_tokens: 50,
        ..Default::default()
    };
    let s2 = SessionStats {
        total_input_tokens: 200,
        total_output_tokens: 25,
        cache_read_tokens: 10,
        cache_creation_tokens: 5,
        ..Default::default()
    };
    let stats = vec![&s1, &s2];
    let result = rollup(&stats, ROLLUP_VERSION, Bucket::Weekly);
    assert_eq!(result.session_count, 2);
    // 100 + 50 + 200 + 25 + 10 + 5 = 390
    assert_eq!(result.total_tokens, 390);
}

#[test]
fn rollup_counts_sessions() {
    let s = SessionStats::default();
    let stats = vec![&s, &s, &s];
    let result = rollup(&stats, ROLLUP_VERSION, Bucket::Monthly);
    assert_eq!(result.session_count, 3);
    assert_eq!(result.duration_count, 3);
}

#[test]
fn rollup_aggregates_prompt_and_file_counts() {
    let s1 = SessionStats {
        user_prompt_count: 3,
        files_edited_count: 2,
        ..Default::default()
    };
    let s2 = SessionStats {
        user_prompt_count: 7,
        files_edited_count: 4,
        ..Default::default()
    };
    let stats = vec![&s1, &s2];
    let result = rollup(&stats, ROLLUP_VERSION, Bucket::Daily);
    assert_eq!(result.prompt_count, 10);
    assert_eq!(result.file_count, 6);
}

#[test]
fn rollup_duration_seconds_to_ms() {
    // duration_seconds: u32 → duration_sum_ms: u64 (× 1000)
    let s1 = SessionStats {
        duration_seconds: 42,
        ..Default::default()
    };
    let s2 = SessionStats {
        duration_seconds: 58,
        ..Default::default()
    };
    let stats = vec![&s1, &s2];
    let result = rollup(&stats, ROLLUP_VERSION, Bucket::Daily);
    // 42_000 + 58_000 = 100_000
    assert_eq!(result.duration_sum_ms, 100_000);
    assert_eq!(result.duration_count, 2);
}

#[test]
fn rollup_phase_4_fields_stay_zero() {
    // PR 1.3 (Phase 1) derives only from SessionStats. Phase 4 fills
    // cost, lines, commits, reedit via RollupInput { stats, flags }.
    // Here we assert the Phase 1 contract: those fields stay at 0.
    let s = SessionStats {
        total_input_tokens: 1_000_000,
        user_prompt_count: 999,
        files_edited_count: 999,
        duration_seconds: 99_999,
        ..Default::default()
    };
    let stats = vec![&s];
    let result = rollup(&stats, ROLLUP_VERSION, Bucket::Daily);
    assert_eq!(result.total_cost_cents, 0, "Phase 4 fills cost");
    assert_eq!(result.lines_added, 0, "Phase 4 fills lines_added");
    assert_eq!(result.lines_removed, 0, "Phase 4 fills lines_removed");
    assert_eq!(result.commit_count, 0, "Phase 4 fills commit_count");
    assert_eq!(result.reedit_rate_sum, 0.0, "Phase 4 fills reedit_rate_sum");
    assert_eq!(
        result.reedit_rate_count, 0,
        "Phase 4 fills reedit_rate_count"
    );
}
