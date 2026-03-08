//! Tantivy search index for prompt history (separate from session search index).
//!
//! Indexes `~/.claude/history.jsonl` entries into a Tantivy full-text index
//! with per-prompt metadata for qualifier-based filtering.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, Value, FAST, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term};
use ts_rs::TS;

use crate::SearchError;

/// Schema version for the prompt index. Bump when the schema changes
/// (field types, new fields, removed fields). A mismatch triggers auto-rebuild.
// Version 1: Initial schema — 12 fields for prompt history
pub const PROMPT_SCHEMA_VERSION: u32 = 1;

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

        let total_matches = searcher.search(&*final_query, &Count)?;
        let top_docs = searcher.search(&*final_query, &TopDocs::with_limit(limit + offset))?;

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

            prompts.push(PromptHit {
                prompt_id: get_text(self.prompt_id_field),
                display: get_text(self.display_field),
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
