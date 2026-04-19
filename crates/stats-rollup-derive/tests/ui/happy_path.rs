//! Phase 4 happy path — full 5-dimension × 3-bucket macro input.
//!
//! Asserts the macro expands without error for the canonical `StatsCore`
//! shape from the design doc §6.2. The generated artifacts themselves
//! are validated structurally by `stats-rollup/tests/generated_shape.rs`.

use claude_view_stats_rollup_derive::RollupTable;

#[derive(RollupTable)]
#[rollup(buckets = [daily, weekly, monthly])]
#[rollup(dimensions = [
    global,
    project(project_id: TEXT),
    branch(project_id: TEXT, branch: TEXT),
    model(model_id: TEXT),
    category(category_l1: TEXT),
])]
pub struct StatsCore {
    pub session_count: u64,
    pub total_tokens: u64,
    pub total_cost_cents: u64,
    pub prompt_count: u64,
    pub file_count: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub commit_count: u64,
    pub commit_insertions: u64,
    pub commit_deletions: u64,
    pub duration_sum_ms: u64,
    pub duration_count: u64,
    pub reedit_rate_sum: f64,
    pub reedit_rate_count: u64,
}

fn main() {
    // Sanity touchpoints — if these compile, the macro produced the
    // expected shapes and name-mangles are stable.
    let _ = StatsCore::__ROLLUP_TABLE_STUB;
    assert_eq!(TABLE_COUNT, 15);

    let _: DailyGlobalStats = DailyGlobalStats {
        period_start: 0,
        session_count: 0,
        total_tokens: 0,
        total_cost_cents: 0,
        prompt_count: 0,
        file_count: 0,
        lines_added: 0,
        lines_removed: 0,
        commit_count: 0,
        commit_insertions: 0,
        commit_deletions: 0,
        duration_sum_ms: 0,
        duration_count: 0,
        reedit_rate_sum: 0.0,
        reedit_rate_count: 0,
    };
}
