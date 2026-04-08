// crates/db/src/indexer_parallel/parser/mod.rs
// Core JSONL parser: parse_bytes(), parse_file_bytes(), and subagent merge logic.

mod core;
mod file_io;
mod subagent;

pub use self::core::parse_bytes;
pub(crate) use file_io::parse_file_bytes;
pub(crate) use subagent::merge_subagent_workload;
