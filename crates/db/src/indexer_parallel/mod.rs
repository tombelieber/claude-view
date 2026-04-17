// crates/db/src/indexer_parallel/mod.rs
// Fast JSONL parsing with memory-mapped I/O and SIMD-accelerated scanning.
// Unified single-pass indexing pipeline (`scan_and_index_all`) plus helpers
// for hint extraction and stale-session pruning.

mod backup;
pub(crate) mod cost;
pub(crate) mod handlers;
pub(crate) mod helpers;
mod orchestrator;
pub(crate) mod parser;
mod pipeline;
pub(crate) mod serde_types;
#[cfg(test)]
mod tests;
pub(crate) mod types;
pub(crate) mod writer;

// Re-export all public items to preserve the original module API.
pub use backup::ingest_backup_sessions;
pub use helpers::extract_commit_skill_invocations;
pub use orchestrator::scan_and_index_all;
pub use parser::parse_bytes;
pub use pipeline::{build_index_hints, prune_stale_sessions};
pub use types::{
    read_file_fast, CommitSkillInvocation, DeepIndexResult, ExtendedMetadata, FileData, IndexHints,
    ParseDiagnostics, ParseResult, ParsedSession, RawInvocation, COMMIT_SKILL_NAMES,
    CURRENT_PARSE_VERSION,
};
