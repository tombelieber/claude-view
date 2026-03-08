//! Tantivy search index for prompt history (separate from session search index).
//!
//! Indexes `~/.claude/history.jsonl` entries into a Tantivy full-text index
//! with per-prompt metadata for qualifier-based filtering.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, Occur, QueryParser, RangeQuery, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, Value, FAST, STORED, STRING, TEXT};
use tantivy::snippet::SnippetGenerator;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term};
use ts_rs::TS;

use std::hash::{Hash, Hasher};

use claude_view_core::prompt_templates::normalize_to_template;

use crate::SearchError;

/// Stable u64 hash of a byte slice using `DefaultHasher`.
fn fxhash(data: &[u8]) -> u64 {
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

fn build_prompt_schema() -> Schema {
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
    version_file_path: Option<PathBuf>,

    // Pre-resolved field handles
    prompt_id_field: Field,
    display_field: Field,
    paste_text_field: Field,
    project_field: Field,
    session_id_field: Field,
    branch_field: Field,
    model_field: Field,
    git_root_field: Field,
    intent_field: Field,
    complexity_field: Field,
    timestamp_field: Field,
    has_paste_field: Field,
    template_id_field: Field,
    is_template_field: Field,
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

impl PromptSearchIndex {
    /// Open or create a prompt index at the given path.
    /// Schema version mismatch triggers a full wipe and rebuild.
    pub fn open(path: &Path) -> Result<Self, SearchError> {
        std::fs::create_dir_all(path)?;

        let version_path = path.join("schema_version");
        let needs_rebuild = match std::fs::read_to_string(&version_path) {
            Ok(v) => v.trim().parse::<u32>().unwrap_or(0) != PROMPT_SCHEMA_VERSION,
            Err(_) => true,
        };

        if needs_rebuild {
            tracing::info!(
                path = %path.display(),
                "Prompt index schema version mismatch — rebuilding"
            );
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.file_name()
                        .map(|n| n != "schema_version")
                        .unwrap_or(false)
                    {
                        if p.is_dir() {
                            let _ = std::fs::remove_dir_all(&p);
                        } else {
                            let _ = std::fs::remove_file(&p);
                        }
                    }
                }
            }
        }

        let schema = build_prompt_schema();
        let index = match Index::open_in_dir(path) {
            Ok(idx) => {
                tracing::info!(path = %path.display(), "opened existing prompt index");
                idx
            }
            Err(_) => {
                tracing::info!(path = %path.display(), "creating new prompt index");
                Index::create_in_dir(path, schema.clone())?
            }
        };

        Self::from_index(index, schema, needs_rebuild, Some(version_path))
    }

    /// Create a prompt index in RAM (for tests).
    pub fn open_in_ram() -> Result<Self, SearchError> {
        let schema = build_prompt_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::from_index(index, schema, false, None)
    }

    /// Internal: set up reader, writer, and field handles from an Index + Schema.
    fn from_index(
        index: Index,
        schema: Schema,
        needs_full_reindex: bool,
        version_file_path: Option<PathBuf>,
    ) -> Result<Self, SearchError> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let writer = index.writer(50_000_000)?;

        let prompt_id_field = schema.get_field("prompt_id").expect("missing prompt_id");
        let display_field = schema.get_field("display").expect("missing display");
        let paste_text_field = schema.get_field("paste_text").expect("missing paste_text");
        let project_field = schema.get_field("project").expect("missing project");
        let session_id_field = schema.get_field("session_id").expect("missing session_id");
        let branch_field = schema.get_field("branch").expect("missing branch");
        let model_field = schema.get_field("model").expect("missing model");
        let git_root_field = schema.get_field("git_root").expect("missing git_root");
        let intent_field = schema.get_field("intent").expect("missing intent");
        let complexity_field = schema.get_field("complexity").expect("missing complexity");
        let timestamp_field = schema.get_field("timestamp").expect("missing timestamp");
        let has_paste_field = schema.get_field("has_paste").expect("missing has_paste");
        let template_id_field = schema
            .get_field("template_id")
            .expect("missing template_id");
        let is_template_field = schema
            .get_field("is_template")
            .expect("missing is_template");

        Ok(Self {
            index,
            reader,
            writer: Mutex::new(writer),
            schema,
            needs_full_reindex,
            version_file_path,
            prompt_id_field,
            display_field,
            paste_text_field,
            project_field,
            session_id_field,
            branch_field,
            model_field,
            git_root_field,
            intent_field,
            complexity_field,
            timestamp_field,
            has_paste_field,
            template_id_field,
            is_template_field,
        })
    }

    /// Bulk-index prompt documents. History is append-only, no deletes needed.
    pub fn index_prompts(&self, docs: &[PromptDocument]) -> Result<(), SearchError> {
        let writer = self.writer.lock().map_err(|e| {
            SearchError::Io(std::io::Error::other(format!("writer lock poisoned: {e}")))
        })?;
        for d in docs {
            let mut tantivy_doc = doc!(
                self.prompt_id_field => d.prompt_id.as_str(),
                self.display_field => d.display.as_str(),
                self.project_field => d.project.as_str(),
                self.session_id_field => d.session_id.as_deref().unwrap_or(""),
                self.branch_field => d.branch.as_str(),
                self.model_field => d.model.as_str(),
                self.git_root_field => d.git_root.as_str(),
                self.intent_field => d.intent.as_str(),
                self.complexity_field => d.complexity.as_str(),
                self.timestamp_field => d.timestamp,
                self.has_paste_field => if d.has_paste { "true" } else { "false" },
            );
            if let Some(ref paste) = d.paste_text {
                tantivy_doc.add_text(self.paste_text_field, paste);
            }
            // Compute template classification: normalize display text to detect slots.
            // template_id field stores the stable hash of the normalized pattern (empty = unique).
            // is_template field stores "true"/"false" for fast TermQuery filtering.
            let normalized = normalize_to_template(&d.display);
            let (template_id_val, is_template_val) = if normalized != d.display {
                (format!("{:x}", fxhash(normalized.as_bytes())), "true")
            } else {
                (String::new(), "false")
            };
            tantivy_doc.add_text(self.template_id_field, &template_id_val);
            tantivy_doc.add_text(self.is_template_field, is_template_val);
            writer.add_document(tantivy_doc)?;
        }
        Ok(())
    }

    /// Commit pending writes to disk.
    /// Call this after indexing a batch of prompts.
    pub fn commit(&self) -> Result<(), SearchError> {
        let mut writer = self.writer.lock().map_err(|e| {
            SearchError::Io(std::io::Error::other(format!("writer lock poisoned: {e}")))
        })?;
        writer.commit()?;
        self.reader.reload()?;
        tracing::info!("prompt index committed");
        Ok(())
    }

    /// Write schema version to disk after successful indexing.
    pub fn mark_schema_synced(&self) {
        if let Some(path) = &self.version_file_path {
            if let Err(e) = std::fs::write(path, format!("{PROMPT_SCHEMA_VERSION}")) {
                tracing::warn!(error = %e, "Failed to write prompt schema version file");
            } else {
                tracing::info!(
                    version = PROMPT_SCHEMA_VERSION,
                    "Prompt schema version synced"
                );
            }
        }
    }

    /// Search the prompt index with optional qualifier filtering.
    ///
    /// Supports qualifiers: `project:`, `intent:`, `branch:`, `complexity:`.
    /// Free-text searches both `display` and `paste_text` fields.
    pub fn search(
        &self,
        query: &str,
        scope: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<PromptSearchResponse, SearchError> {
        self.search_with(PromptSearchParams {
            query,
            scope,
            limit,
            offset,
            ..Default::default()
        })
    }

    /// Extended search with full filter support.
    pub fn search_with(
        &self,
        params: PromptSearchParams<'_>,
    ) -> Result<PromptSearchResponse, SearchError> {
        let query = params.query;
        let scope = params.scope;
        let limit = params.limit;
        let offset = params.offset;
        let start = std::time::Instant::now();
        let searcher = self.reader.searcher();

        // Parse qualifiers from query string
        let mut free_text_parts = Vec::new();
        let mut qualifier_clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

        for token in query.split_whitespace() {
            if let Some(val) = token.strip_prefix("project:") {
                qualifier_clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.project_field, val),
                        IndexRecordOption::Basic,
                    )),
                ));
            } else if let Some(val) = token.strip_prefix("intent:") {
                qualifier_clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.intent_field, val),
                        IndexRecordOption::Basic,
                    )),
                ));
            } else if let Some(val) = token.strip_prefix("branch:") {
                qualifier_clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.branch_field, val),
                        IndexRecordOption::Basic,
                    )),
                ));
            } else if let Some(val) = token.strip_prefix("complexity:") {
                qualifier_clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.complexity_field, val),
                        IndexRecordOption::Basic,
                    )),
                ));
            } else {
                free_text_parts.push(token);
            }
        }

        // Scope filter (polymorphic: check both project and git_root)
        if let Some(scope_val) = scope {
            let project_term = TermQuery::new(
                Term::from_field_text(self.project_field, scope_val),
                IndexRecordOption::Basic,
            );
            let git_root_term = TermQuery::new(
                Term::from_field_text(self.git_root_field, scope_val),
                IndexRecordOption::Basic,
            );
            let scope_query = BooleanQuery::new(vec![
                (Occur::Should, Box::new(project_term)),
                (Occur::Should, Box::new(git_root_term)),
            ]);
            qualifier_clauses.push((Occur::Must, Box::new(scope_query)));
        }

        // has_paste filter
        if let Some(hp) = params.has_paste {
            let val = if hp { "true" } else { "false" };
            qualifier_clauses.push((
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.has_paste_field, val),
                    IndexRecordOption::Basic,
                )),
            ));
        }

        // Time range filters (FAST i64 field — Tantivy RangeQuery)
        let time_lower = params.time_after.unwrap_or(i64::MIN);
        let time_upper = params.time_before.unwrap_or(i64::MAX);
        if params.time_after.is_some() || params.time_before.is_some() {
            qualifier_clauses.push((
                Occur::Must,
                Box::new(RangeQuery::new_i64_bounds(
                    "timestamp".to_string(),
                    std::ops::Bound::Included(time_lower),
                    std::ops::Bound::Included(time_upper),
                )),
            ));
        }

        // template_match filter:
        // "template" => only prompts with non-empty template_id (stored as any non-"" value)
        // "unique"   => only prompts with empty template_id (stored as "")
        // None / "any" => no filter
        // template_match filter uses the `is_template` field ("true"/"false")
        // which was designed for this exact TermQuery pattern.
        match params.template_match {
            Some("template") => {
                qualifier_clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.is_template_field, "true"),
                        IndexRecordOption::Basic,
                    )),
                ));
            }
            Some("unique") => {
                qualifier_clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.is_template_field, "false"),
                        IndexRecordOption::Basic,
                    )),
                ));
            }
            _ => {}
        }

        // Build final query
        let free_text = free_text_parts.join(" ");
        let final_query: Box<dyn tantivy::query::Query> =
            if free_text.is_empty() && qualifier_clauses.is_empty() {
                Box::new(tantivy::query::AllQuery)
            } else if free_text.is_empty() {
                Box::new(BooleanQuery::new(qualifier_clauses))
            } else {
                let parser = QueryParser::for_index(
                    &self.index,
                    vec![self.display_field, self.paste_text_field],
                );
                let text_query = parser.parse_query(&free_text)?;

                if qualifier_clauses.is_empty() {
                    text_query
                } else {
                    qualifier_clauses.push((Occur::Must, text_query));
                    Box::new(BooleanQuery::new(qualifier_clauses))
                }
            };

        // Build snippet generator when free-text query is present (TEXT field only).
        let snippet_gen = if !free_text.is_empty() {
            SnippetGenerator::create(&searcher, &*final_query, self.display_field).ok()
        } else {
            None
        };

        let total_matches = searcher.search(&*final_query, &Count)?;
        // Sort: newest (default) = descending timestamp, oldest = ascending timestamp.
        // When a free-text query is present we keep relevance score as primary sort
        // (matches user expectation for search results).
        let sort_oldest = params.sort.map(|s| s == "oldest").unwrap_or(false);
        let top_docs = if sort_oldest {
            searcher
                .search(
                    &*final_query,
                    &TopDocs::with_limit(limit + offset)
                        .order_by_fast_field::<i64>("timestamp", tantivy::Order::Asc),
                )?
                .into_iter()
                .map(|(_, addr)| (0.0f32, addr))
                .collect::<Vec<_>>()
        } else if free_text.is_empty() {
            // No text query — sort by newest first
            searcher
                .search(
                    &*final_query,
                    &TopDocs::with_limit(limit + offset)
                        .order_by_fast_field::<i64>("timestamp", tantivy::Order::Desc),
                )?
                .into_iter()
                .map(|(_, addr)| (0.0f32, addr))
                .collect::<Vec<_>>()
        } else {
            // Text query present — use relevance score (BM25), newest as tiebreaker
            searcher.search(&*final_query, &TopDocs::with_limit(limit + offset))?
        };

        let mut prompts = Vec::with_capacity(limit.min(top_docs.len()));
        for (_score, doc_addr) in top_docs.into_iter().skip(offset) {
            if prompts.len() >= limit {
                break;
            }
            let retrieved = searcher.doc::<tantivy::TantivyDocument>(doc_addr)?;

            let get_text = |field: Field| -> String {
                retrieved
                    .get_first(field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            let session_id_str = get_text(self.session_id_field);
            let timestamp = retrieved
                .get_first(self.timestamp_field)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let has_paste_str = get_text(self.has_paste_field);

            let snippet = snippet_gen.as_ref().and_then(|gen| {
                let s = gen.snippet_from_doc(&retrieved);
                let html = s.to_html();
                if html.is_empty() {
                    None
                } else {
                    Some(html)
                }
            });

            let raw_template_id = get_text(self.template_id_field);
            let template_id = if raw_template_id.is_empty() {
                None
            } else {
                Some(raw_template_id)
            };

            prompts.push(PromptHit {
                prompt_id: get_text(self.prompt_id_field),
                display: get_text(self.display_field),
                snippet,
                template_id,
                project: get_text(self.project_field),
                session_id: if session_id_str.is_empty() {
                    None
                } else {
                    Some(session_id_str)
                },
                branch: get_text(self.branch_field),
                model: get_text(self.model_field),
                git_root: get_text(self.git_root_field),
                intent: get_text(self.intent_field),
                complexity: get_text(self.complexity_field),
                timestamp,
                has_paste: has_paste_str == "true",
            });
        }

        Ok(PromptSearchResponse {
            prompts,
            total_matches,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_prompt_index_in_ram() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        assert_eq!(index.search("test", None, 10, 0).unwrap().total_matches, 0);
    }

    #[test]
    fn index_and_search_prompt() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let doc = PromptDocument {
            prompt_id: "p001".into(),
            display: "fix the authentication error".into(),
            paste_text: None,
            project: "claude-view".into(),
            session_id: Some("abc-123".into()),
            branch: "main".into(),
            model: "claude-opus-4-6".into(),
            git_root: "/Users/test/dev/claude-view".into(),
            intent: "fix".into(),
            complexity: "short".into(),
            timestamp: 1772924399,
            has_paste: false,
        };
        index.index_prompts(&[doc]).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let results = index.search("authentication", None, 10, 0).unwrap();
        assert_eq!(results.total_matches, 1);
        assert_eq!(results.prompts[0].display, "fix the authentication error");
    }

    #[test]
    fn search_paste_content() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let doc = PromptDocument {
            prompt_id: "p002".into(),
            display: "[Pasted text #1 +18 lines]".into(),
            paste_text: Some("NullPointerException in UserService.java".into()),
            project: "proj".into(),
            session_id: None,
            branch: "".into(),
            model: "".into(),
            git_root: "".into(),
            intent: "other".into(),
            complexity: "short".into(),
            timestamp: 1772924399,
            has_paste: true,
        };
        index.index_prompts(&[doc]).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let results = index.search("NullPointerException", None, 10, 0).unwrap();
        assert_eq!(results.total_matches, 1);
    }

    #[test]
    fn search_with_intent_qualifier() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let docs = vec![
            PromptDocument {
                prompt_id: "p003".into(),
                display: "fix the bug".into(),
                paste_text: None,
                project: "proj".into(),
                session_id: None,
                branch: "".into(),
                model: "".into(),
                git_root: "".into(),
                intent: "fix".into(),
                complexity: "micro".into(),
                timestamp: 100,
                has_paste: false,
            },
            PromptDocument {
                prompt_id: "p004".into(),
                display: "create a new module".into(),
                paste_text: None,
                project: "proj".into(),
                session_id: None,
                branch: "".into(),
                model: "".into(),
                git_root: "".into(),
                intent: "create".into(),
                complexity: "short".into(),
                timestamp: 200,
                has_paste: false,
            },
        ];
        index.index_prompts(&docs).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let results = index.search("intent:fix", None, 10, 0).unwrap();
        assert_eq!(results.total_matches, 1);
        assert_eq!(results.prompts[0].intent, "fix");
    }

    // ── template_match tests ────────────────────────────────────────────────

    fn make_doc(id: &str, display: &str, ts: i64) -> PromptDocument {
        PromptDocument {
            prompt_id: id.into(),
            display: display.into(),
            paste_text: None,
            project: "proj".into(),
            session_id: None,
            branch: "".into(),
            model: "".into(),
            git_root: "".into(),
            intent: "fix".into(),
            complexity: "short".into(),
            timestamp: ts,
            has_paste: false,
        }
    }

    #[test]
    fn template_prompts_share_same_template_id() {
        // Three prompts that normalize to the same pattern get a non-empty, identical template_id.
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let docs = vec![
            make_doc("t1", "review @docs/plan-a.md is this ready", 100),
            make_doc("t2", "review @docs/plan-b.md is this ready", 200),
            make_doc("t3", "review @docs/plan-c.md is this ready", 300),
        ];
        index.index_prompts(&docs).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let results = index.search("review", None, 10, 0).unwrap();
        assert_eq!(results.total_matches, 3);

        let ids: Vec<&str> = results
            .prompts
            .iter()
            .map(|h| h.template_id.as_deref().unwrap_or(""))
            .collect();
        // All three must have the same non-empty template_id
        assert!(
            !ids[0].is_empty(),
            "template prompts must have a non-empty template_id"
        );
        assert_eq!(ids[0], ids[1], "same pattern => same template_id");
        assert_eq!(ids[1], ids[2], "same pattern => same template_id");
    }

    #[test]
    fn unique_prompt_has_empty_template_id() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        // Each prompt is completely different — no pattern match
        let docs = vec![
            make_doc("u1", "what is the meaning of life", 100),
            make_doc("u2", "refactor the database connection pool", 200),
            make_doc("u3", "explain how oauth2 pkce works", 300),
        ];
        index.index_prompts(&docs).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let results = index.search("", None, 10, 0).unwrap();
        assert_eq!(results.total_matches, 3);

        for hit in &results.prompts {
            assert!(
                hit.template_id.as_deref().unwrap_or("").is_empty(),
                "unique prompt '{}' should have empty template_id",
                hit.display
            );
        }
    }

    #[test]
    fn template_match_filter_returns_only_template_prompts() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let docs = vec![
            make_doc("t1", "review @docs/plan-a.md is this ready", 100),
            make_doc("t2", "review @docs/plan-b.md is this ready", 200),
            make_doc("t3", "review @docs/plan-c.md is this ready", 300),
            make_doc("u1", "what is the meaning of life", 400),
        ];
        index.index_prompts(&docs).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let results = index
            .search_with(PromptSearchParams {
                query: "",
                scope: None,
                template_match: Some("template"),
                limit: 10,
                offset: 0,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            results.total_matches, 3,
            "only template prompts should be returned"
        );
        for hit in &results.prompts {
            assert!(
                !hit.template_id.as_deref().unwrap_or("").is_empty(),
                "all hits should have a template_id"
            );
        }
    }

    #[test]
    fn template_match_filter_returns_only_unique_prompts() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let docs = vec![
            make_doc("t1", "review @docs/plan-a.md is this ready", 100),
            make_doc("t2", "review @docs/plan-b.md is this ready", 200),
            make_doc("u1", "what is the meaning of life", 300),
            make_doc("u2", "explain oauth2 pkce flow", 400),
        ];
        index.index_prompts(&docs).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let results = index
            .search_with(PromptSearchParams {
                query: "",
                scope: None,
                template_match: Some("unique"),
                limit: 10,
                offset: 0,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            results.total_matches, 2,
            "only unique prompts should be returned"
        );
        for hit in &results.prompts {
            assert!(
                hit.template_id.as_deref().unwrap_or("").is_empty(),
                "all hits should have empty template_id"
            );
        }
    }

    #[test]
    fn template_match_any_returns_all_prompts() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let docs = vec![
            make_doc("t1", "review @docs/plan-a.md is this ready", 100),
            make_doc("t2", "review @docs/plan-b.md is this ready", 200),
            make_doc("u1", "what is the meaning of life", 300),
        ];
        index.index_prompts(&docs).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        // None = "any" — no template filter applied
        let results = index
            .search_with(PromptSearchParams {
                query: "",
                scope: None,
                template_match: None,
                limit: 10,
                offset: 0,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.total_matches, 3);
    }

    #[test]
    fn search_pagination() {
        let index = PromptSearchIndex::open_in_ram().unwrap();
        let docs: Vec<PromptDocument> = (0..5)
            .map(|i| PromptDocument {
                prompt_id: format!("p{i:03}"),
                display: format!("test prompt number {i}"),
                paste_text: None,
                project: "proj".into(),
                session_id: None,
                branch: "".into(),
                model: "".into(),
                git_root: "".into(),
                intent: "other".into(),
                complexity: "short".into(),
                timestamp: i as i64,
                has_paste: false,
            })
            .collect();
        index.index_prompts(&docs).unwrap();
        index.commit().unwrap();
        index.reader.reload().unwrap();

        let page1 = index.search("test prompt", None, 2, 0).unwrap();
        assert_eq!(page1.prompts.len(), 2);
        assert_eq!(page1.total_matches, 5);

        let page2 = index.search("test prompt", None, 2, 2).unwrap();
        assert_eq!(page2.prompts.len(), 2);
    }
}
