//! Tantivy search index for prompt history (separate from session search index).
//!
//! Indexes `~/.claude/history.jsonl` entries into a Tantivy full-text index
//! with per-prompt metadata for qualifier-based filtering.

mod indexing;
mod search;
mod types;

#[cfg(test)]
mod tests;

pub use types::{
    PromptDocument, PromptHit, PromptSearchIndex, PromptSearchParams, PromptSearchResponse,
    PROMPT_SCHEMA_VERSION,
};
