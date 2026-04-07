//! Types and schema for the prompt search index.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tantivy::schema::{Field, Schema, FAST, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter};
use ts_rs::TS;

/// Stable u64 hash of a byte slice using `DefaultHasher`.
pub(crate) fn fxhash(data: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

/// Schema version for the prompt index. Bump when the schema changes
/// (field types, new fields, removed fields). A mismatch triggers auto-rebuild.
// Version 1: Initial schema — 12 fields for prompt history
// Version 2: Added `template_id` field (STRING | STORED) + snippet via SnippetGenerator
pub const PROMPT_SCHEMA_VERSION: u32 = 2;

/// A document to be indexed into the prompt search index.
pub struct PromptDocument {
    pub prompt_id: String,
    pub display: String,
    pub paste_text: Option<String>,
    pub project: String,
    pub session_id: Option<String>,
    pub branch: String,
    pub model: String,
    pub git_root: String,
    pub intent: String,
    pub complexity: String,
    pub timestamp: i64,
    pub has_paste: bool,
}

/// A single search result hit from the prompt index.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PromptHit {
    pub prompt_id: String,
    pub display: String,
    /// HTML snippet with `<b>` tags around matched terms. Present when a
    /// free-text query was used; `None` otherwise (browse/filter-only mode).
    pub snippet: Option<String>,
    /// Stable hash of the normalized prompt pattern. Non-empty means the prompt
    /// matches a recurring template; empty means it is unique.
    pub template_id: Option<String>,
    pub project: String,
    pub session_id: Option<String>,
    pub branch: String,
    pub model: String,
    pub git_root: String,
    pub intent: String,
    pub complexity: String,
    pub timestamp: i64,
    pub has_paste: bool,
}

/// Response from a prompt search query.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PromptSearchResponse {
    pub prompts: Vec<PromptHit>,
    pub total_matches: usize,
    pub elapsed_ms: u64,
}

pub(crate) fn build_prompt_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("prompt_id", STRING | STORED);
    builder.add_text_field("display", TEXT | STORED);
    builder.add_text_field("paste_text", TEXT);
    builder.add_text_field("project", STRING | STORED);
    builder.add_text_field("session_id", STRING | STORED);
    builder.add_text_field("branch", STRING | STORED);
    builder.add_text_field("model", TEXT | STORED);
    builder.add_text_field("git_root", STRING | STORED);
    builder.add_text_field("intent", STRING | STORED);
    builder.add_text_field("complexity", STRING | STORED);
    builder.add_i64_field("timestamp", FAST | STORED);
    builder.add_text_field("has_paste", STRING | STORED);
    // template_id: stable hash of the normalized pattern — empty string = unique prompt.
    builder.add_text_field("template_id", STRING | STORED);
    // is_template: "true" if pattern detected, "false" if unique — used for TermQuery filtering.
    builder.add_text_field("is_template", STRING | STORED);
    builder.build()
}

/// Tantivy search index for prompt history entries.
pub struct PromptSearchIndex {
    /// The underlying Tantivy index.
    pub index: Index,
    /// Reader for executing queries. Automatically reloads on commit.
    pub reader: IndexReader,
    /// Writer for indexing documents. Wrapped in Mutex because `IndexWriter`
    /// requires `&mut self` but may be used from different async contexts.
    pub writer: Mutex<IndexWriter>,
    /// The schema used by this index.
    pub schema: Schema,
    pub needs_full_reindex: bool,
    pub(crate) version_file_path: Option<PathBuf>,

    // Pre-resolved field handles
    pub(crate) prompt_id_field: Field,
    pub(crate) display_field: Field,
    pub(crate) paste_text_field: Field,
    pub(crate) project_field: Field,
    pub(crate) session_id_field: Field,
    pub(crate) branch_field: Field,
    pub(crate) model_field: Field,
    pub(crate) git_root_field: Field,
    pub(crate) intent_field: Field,
    pub(crate) complexity_field: Field,
    pub(crate) timestamp_field: Field,
    pub(crate) has_paste_field: Field,
    pub(crate) template_id_field: Field,
    pub(crate) is_template_field: Field,
}

/// Search parameters for the prompt index.
#[derive(Debug, Default, Clone)]
pub struct PromptSearchParams<'a> {
    /// Free-text query (supports `qualifier:value` tokens).
    pub query: &'a str,
    /// Project scope filter (matches `project` or `git_root`).
    pub scope: Option<&'a str>,
    /// Filter: only prompts with `has_paste == true/false`.
    pub has_paste: Option<bool>,
    /// Filter: only prompts on or after this Unix timestamp (seconds).
    pub time_after: Option<i64>,
    /// Filter: only prompts on or before this Unix timestamp (seconds).
    pub time_before: Option<i64>,
    /// Sort order: "newest" (default) or "oldest".
    pub sort: Option<&'a str>,
    /// Filter by template match: "template" = has template_id, "unique" = no template_id.
    pub template_match: Option<&'a str>,
    pub limit: usize,
    pub offset: usize,
}
