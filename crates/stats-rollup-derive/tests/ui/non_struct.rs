use claude_view_stats_rollup_derive::RollupTable;

#[derive(RollupTable)]
#[rollup(buckets = [daily])]
#[rollup(dimensions = [global])]
pub enum Nope {
    A,
    B,
}

fn main() {}
