use claude_view_stats_rollup_derive::RollupTable;

#[derive(RollupTable)]
#[rollup(buckets = [daily, weekly, monthly])]
#[rollup(dimensions = [global])]
pub struct StatsCore {
    pub session_count: u64,
    pub total_tokens: u64,
}

fn main() {
    // Exercise the emitted marker so Phase 4's "was RollupTable derived?" check
    // has something concrete to test against.
    let _ = StatsCore::__ROLLUP_TABLE_STUB;
}
