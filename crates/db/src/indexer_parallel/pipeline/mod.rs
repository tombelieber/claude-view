// crates/db/src/indexer_parallel/pipeline/mod.rs
// Pipeline utilities retained after the legacy pass_1/pass_2 removal:
// `build_index_hints` (reads sessions-index.json for hint map) and
// `prune_stale_sessions` (removes DB rows whose JSONL file is gone).

mod index_hints;
mod pruning;

pub use index_hints::*;
pub use pruning::*;
