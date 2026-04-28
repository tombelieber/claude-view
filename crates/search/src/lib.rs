//! Search support for Claude Code sessions and prompt history.
//!
//! Runtime session search scans raw JSONL files with ripgrep-core. Prompt
//! history keeps its separate Tantivy-backed index under `prompt_index`.
//!
//! # Architecture
//!
//! - **Session runtime**: raw JSONL files -> ripgrep-core -> grouped snippets
//! - **Prompt history**: `prompt_index::PromptSearchIndex`

pub mod grep;
pub mod grep_types;
pub mod prompt_index;
pub mod types;
pub mod unified;

pub use grep::JsonlFile;
pub use types::{MatchHit, SearchResponse, SessionHit};
pub use unified::{
    unified_search, SearchEngine, UnifiedSearchError, UnifiedSearchOptions, UnifiedSearchResult,
};

/// Writer heap size for bulk prompt-history indexing.
pub const BULK_WRITER_HEAP: usize = 50_000_000;
/// Writer heap size for incremental prompt-history indexing.
pub const INCREMENTAL_WRITER_HEAP: usize = 15_000_000;

/// Errors that can occur during search operations.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("Query parse error: {0}")]
    QueryParse(#[from] tantivy::query::QueryParserError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Index not ready")]
    NotReady,
}
