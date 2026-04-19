use claude_view_stats_rollup_derive::RollupTable;

#[derive(RollupTable)]
#[rollup(buckets = [daily])]
pub struct StatsCore {
    pub session_count: u64,
}

fn main() {}
