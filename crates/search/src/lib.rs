//! Full-text search for Claude Code session conversations.
//!
//! Uses Tantivy (embedded Rust search engine) to index message content from
//! JSONL session files. Each message becomes a Tantivy document with metadata
//! fields (session_id, project, branch, model, role) for qualifier-based
//! filtering and a full-text `content` field for BM25-ranked search.
//!
//! # Architecture
//!
//! - **Schema**: 9 fields per document (see `build_schema`)
//! - **Write path**: `indexer::SearchDocument` -> `SearchIndex::index_session` -> `commit`
//! - **Read path**: `SearchIndex::search` -> qualifier parsing -> BooleanQuery -> snippets
//! - **Storage**: On-disk at `<cache_dir>/claude-view/search-index/` or in-RAM for tests

pub mod indexer;
pub mod query;
pub mod types;

use std::path::Path;
use std::sync::Mutex;

use tantivy::schema::{Field, Schema, FAST, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};

pub use indexer::SearchDocument;
pub use types::{MatchHit, SearchResponse, SessionHit};

/// Schema version for the Tantivy index. Bump when the schema changes
/// (field types, new fields, removed fields). A mismatch triggers auto-rebuild.
pub const SEARCH_SCHEMA_VERSION: u32 = 3;
// Version 1: Initial schema (project as STRING with encoded path)
// Version 2: model field changed to TEXT for partial matching
// Version 3: Force rebuild to ensure model TEXT schema is applied correctly

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

/// Build the Tantivy schema for indexing Claude Code conversation messages.
///
/// Fields:
/// - `session_id`: STRING | STORED — exact match, grouping, delete-by-session
/// - `project`: STRING | STORED — qualifier filter (`project:claude-view`)
/// - `branch`: STRING | STORED — qualifier filter (`branch:feature/auth`)
/// - `model`: TEXT | STORED — tokenized qualifier filter (`model:opus` matches `claude-opus-4-6`)
/// - `role`: STRING | STORED — qualifier filter (`role:user`)
/// - `content`: TEXT | STORED — full-text BM25 search + snippet generation
/// - `turn_number`: u64, FAST | STORED — display ("turn 3")
/// - `timestamp`: i64, FAST | STORED — range queries, sorting
/// - `skills`: STRING | STORED — multi-valued qualifier (`skill:commit`)
pub fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Untokenized string fields for exact-match qualifiers and grouping
    schema_builder.add_text_field("session_id", STRING | STORED);
    schema_builder.add_text_field("project", STRING | STORED);
    schema_builder.add_text_field("branch", STRING | STORED);
    schema_builder.add_text_field("model", TEXT | STORED);
    schema_builder.add_text_field("role", STRING | STORED);

    // Full-text field — tokenized, BM25-ranked, stored for snippet generation
    schema_builder.add_text_field("content", TEXT | STORED);

    // Numeric fast fields for range queries and display
    schema_builder.add_u64_field("turn_number", FAST | STORED);
    schema_builder.add_i64_field("timestamp", FAST | STORED);

    // Multi-valued string field — one doc can have multiple skills
    schema_builder.add_text_field("skills", STRING | STORED);

    schema_builder.build()
}

/// The main search index, holding a Tantivy index, reader, writer, and
/// pre-resolved field handles for all 9 schema fields.
pub struct SearchIndex {
    /// The underlying Tantivy index.
    pub index: Index,
    /// Reader for executing queries. Automatically reloads on commit.
    pub reader: IndexReader,
    /// Writer for indexing documents. Wrapped in Mutex because `IndexWriter`
    /// requires `&mut self` but may be used from different async contexts.
    pub writer: Mutex<IndexWriter>,
    /// The schema used by this index.
    pub schema: Schema,

    // Pre-resolved field handles (avoid repeated schema.get_field() lookups)
    pub(crate) session_id_field: Field,
    pub(crate) project_field: Field,
    pub(crate) branch_field: Field,
    pub(crate) model_field: Field,
    pub(crate) role_field: Field,
    pub(crate) content_field: Field,
    pub(crate) turn_number_field: Field,
    pub(crate) timestamp_field: Field,
    pub(crate) skills_field: Field,
}

impl SearchIndex {
    /// Open or create a Tantivy index at the given directory path.
    ///
    /// If the directory does not exist, it will be created. If an index already
    /// exists at the path, it will be opened. If the path exists but contains
    /// no valid index, a new one is created.
    ///
    /// Schema versioning: if a `schema_version` file exists in the index
    /// directory and its value does not match `SEARCH_SCHEMA_VERSION`, the
    /// index is wiped and rebuilt from scratch.
    pub fn open(path: &Path) -> Result<Self, SearchError> {
        std::fs::create_dir_all(path)?;

        let version_path = path.join("schema_version");
        let needs_rebuild = match std::fs::read_to_string(&version_path) {
            Ok(v) => v.trim().parse::<u32>().unwrap_or(0) != SEARCH_SCHEMA_VERSION,
            Err(_) => false, // no version file = first creation, not a rebuild
        };

        if needs_rebuild {
            tracing::info!(
                path = %path.display(),
                "Search schema version mismatch — rebuilding index"
            );
            // Remove all files in the directory except schema_version
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.file_name().map(|n| n != "schema_version").unwrap_or(false) {
                        if p.is_dir() {
                            let _ = std::fs::remove_dir_all(&p);
                        } else {
                            let _ = std::fs::remove_file(&p);
                        }
                    }
                }
            }
        }

        let schema = build_schema();

        let index = match Index::open_in_dir(path) {
            Ok(idx) => {
                tracing::info!(path = %path.display(), "opened existing search index");
                idx
            }
            Err(_) => {
                tracing::info!(path = %path.display(), "creating new search index");
                Index::create_in_dir(path, schema.clone())?
            }
        };

        // Write current schema version
        let _ = std::fs::write(&version_path, format!("{}", SEARCH_SCHEMA_VERSION));

        Self::from_index(index, schema)
    }

    /// Create a Tantivy index entirely in RAM. Useful for tests.
    pub fn open_in_ram() -> Result<Self, SearchError> {
        let schema = build_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::from_index(index, schema)
    }

    /// Internal helper: given an `Index` and `Schema`, set up the reader, writer,
    /// and field handles.
    fn from_index(index: Index, schema: Schema) -> Result<Self, SearchError> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        // 50MB writer heap — reasonable for batch indexing
        let writer = index.writer(50_000_000)?;

        // Pre-resolve all field handles
        let session_id_field = schema
            .get_field("session_id")
            .expect("schema missing session_id field");
        let project_field = schema
            .get_field("project")
            .expect("schema missing project field");
        let branch_field = schema
            .get_field("branch")
            .expect("schema missing branch field");
        let model_field = schema
            .get_field("model")
            .expect("schema missing model field");
        let role_field = schema
            .get_field("role")
            .expect("schema missing role field");
        let content_field = schema
            .get_field("content")
            .expect("schema missing content field");
        let turn_number_field = schema
            .get_field("turn_number")
            .expect("schema missing turn_number field");
        let timestamp_field = schema
            .get_field("timestamp")
            .expect("schema missing timestamp field");
        let skills_field = schema
            .get_field("skills")
            .expect("schema missing skills field");

        Ok(Self {
            index,
            reader,
            writer: Mutex::new(writer),
            schema,
            session_id_field,
            project_field,
            branch_field,
            model_field,
            role_field,
            content_field,
            turn_number_field,
            timestamp_field,
            skills_field,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_schema_has_all_fields() {
        let schema = build_schema();
        assert!(schema.get_field("session_id").is_ok());
        assert!(schema.get_field("project").is_ok());
        assert!(schema.get_field("branch").is_ok());
        assert!(schema.get_field("model").is_ok());
        assert!(schema.get_field("role").is_ok());
        assert!(schema.get_field("content").is_ok());
        assert!(schema.get_field("turn_number").is_ok());
        assert!(schema.get_field("timestamp").is_ok());
        assert!(schema.get_field("skills").is_ok());
        // Verify exactly 9 fields
        assert_eq!(schema.fields().count(), 9);
    }

    #[test]
    fn test_open_in_ram() {
        let idx = SearchIndex::open_in_ram().expect("should create in-ram index");
        assert_eq!(idx.schema.fields().count(), 9);
    }

    #[test]
    fn test_open_on_disk() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let idx = SearchIndex::open(dir.path()).expect("should create on-disk index");
        assert_eq!(idx.schema.fields().count(), 9);

        // Drop and re-open to verify persistence
        drop(idx);
        let idx2 = SearchIndex::open(dir.path()).expect("should re-open existing index");
        assert_eq!(idx2.schema.fields().count(), 9);
    }

    #[test]
    fn test_index_and_search_roundtrip() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        // Index some documents
        let docs = vec![
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "claude-view".to_string(),
                branch: "main".to_string(),
                model: "opus".to_string(),
                role: "user".to_string(),
                content: "Add JWT authentication to the login endpoint".to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec![],
            },
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "claude-view".to_string(),
                branch: "main".to_string(),
                model: "opus".to_string(),
                role: "assistant".to_string(),
                content: "I'll implement JWT authentication using the jsonwebtoken crate"
                    .to_string(),
                turn_number: 2,
                timestamp: 1739598060,
                skills: vec!["Edit".to_string()],
            },
            SearchDocument {
                session_id: "sess-002".to_string(),
                project: "other-project".to_string(),
                branch: "feature/auth".to_string(),
                model: "sonnet".to_string(),
                role: "user".to_string(),
                content: "Fix the database connection pooling issue".to_string(),
                turn_number: 1,
                timestamp: 1739600000,
                skills: vec![],
            },
        ];

        idx.index_session("sess-001", &docs[0..2])
            .expect("index session 1");
        idx.index_session("sess-002", &docs[2..3])
            .expect("index session 2");
        idx.commit().expect("commit");

        // Wait for reader to pick up changes
        idx.reader.reload().expect("reload reader");

        // Search for "JWT authentication"
        let result = idx
            .search("JWT authentication", None, 10, 0)
            .expect("search");
        assert_eq!(result.total_sessions, 1, "only session 1 matches JWT");
        assert_eq!(result.sessions[0].session_id, "sess-001");
        assert_eq!(result.sessions[0].match_count, 2);
        assert!(result.elapsed_ms >= 0.0);

        // Verify snippets contain mark tags
        let top = &result.sessions[0].top_match;
        assert!(
            top.snippet.contains("<b>") || top.snippet.contains("JWT"),
            "snippet should highlight or contain search terms: {}",
            top.snippet
        );
    }

    #[test]
    fn test_search_with_project_qualifier() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        let docs = vec![
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "claude-view".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "Fix the login bug in the auth module".to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec![],
            },
            SearchDocument {
                session_id: "sess-002".to_string(),
                project: "other-project".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "Fix the login bug in the payment module".to_string(),
                turn_number: 1,
                timestamp: 1739600000,
                skills: vec![],
            },
        ];

        idx.index_session("sess-001", &docs[0..1])
            .expect("index");
        idx.index_session("sess-002", &docs[1..2])
            .expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Search with project qualifier
        let result = idx
            .search("project:claude-view login bug", None, 10, 0)
            .expect("search");
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "sess-001");
    }

    #[test]
    fn test_search_with_scope() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        let docs = vec![
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "project-a".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "implement the feature".to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec![],
            },
            SearchDocument {
                session_id: "sess-002".to_string(),
                project: "project-b".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "implement the feature".to_string(),
                turn_number: 1,
                timestamp: 1739600000,
                skills: vec![],
            },
        ];

        idx.index_session("sess-001", &docs[0..1])
            .expect("index");
        idx.index_session("sess-002", &docs[1..2])
            .expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Search with scope filter
        let result = idx
            .search("feature", Some("project:project-a"), 10, 0)
            .expect("search");
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "sess-001");
    }

    #[test]
    fn test_delete_session() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        let docs = vec![SearchDocument {
            session_id: "sess-to-delete".to_string(),
            project: "test".to_string(),
            branch: "".to_string(),
            model: "".to_string(),
            role: "user".to_string(),
            content: "this content should be deleted".to_string(),
            turn_number: 1,
            timestamp: 1739598000,
            skills: vec![],
        }];

        idx.index_session("sess-to-delete", &docs)
            .expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Verify it's there
        let result = idx.search("deleted", None, 10, 0).expect("search");
        assert_eq!(result.total_sessions, 1);

        // Delete and commit
        idx.delete_session("sess-to-delete").expect("delete");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Verify it's gone
        let result = idx.search("deleted", None, 10, 0).expect("search");
        assert_eq!(result.total_sessions, 0);
    }

    #[test]
    fn test_reindex_session_replaces_old_docs() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        // Index version 1
        let docs_v1 = vec![SearchDocument {
            session_id: "sess-reindex".to_string(),
            project: "test".to_string(),
            branch: "".to_string(),
            model: "".to_string(),
            role: "user".to_string(),
            content: "original content about databases".to_string(),
            turn_number: 1,
            timestamp: 1739598000,
            skills: vec![],
        }];

        idx.index_session("sess-reindex", &docs_v1)
            .expect("index v1");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Verify v1 is searchable
        let result = idx.search("databases", None, 10, 0).expect("search v1");
        assert_eq!(result.total_sessions, 1);

        // Re-index with version 2 (different content)
        let docs_v2 = vec![SearchDocument {
            session_id: "sess-reindex".to_string(),
            project: "test".to_string(),
            branch: "".to_string(),
            model: "".to_string(),
            role: "user".to_string(),
            content: "updated content about networking".to_string(),
            turn_number: 1,
            timestamp: 1739599000,
            skills: vec![],
        }];

        idx.index_session("sess-reindex", &docs_v2)
            .expect("index v2");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Old content should not be found
        let result = idx
            .search("databases", None, 10, 0)
            .expect("search old");
        assert_eq!(result.total_sessions, 0);

        // New content should be found
        let result = idx
            .search("networking", None, 10, 0)
            .expect("search new");
        assert_eq!(result.total_sessions, 1);
    }

    #[test]
    fn test_search_empty_query_returns_empty() {
        let idx = SearchIndex::open_in_ram().expect("create index");
        let result = idx.search("", None, 10, 0).expect("search empty");
        assert_eq!(result.total_sessions, 0);
        assert_eq!(result.total_matches, 0);
    }

    #[test]
    fn test_search_with_role_qualifier() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        let docs = vec![
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "please fix the authentication system".to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec![],
            },
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "assistant".to_string(),
                content: "I will fix the authentication system now".to_string(),
                turn_number: 2,
                timestamp: 1739598060,
                skills: vec![],
            },
        ];

        idx.index_session("sess-001", &docs).expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Search only user messages
        let result = idx
            .search("role:user authentication", None, 10, 0)
            .expect("search");
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.sessions[0].top_match.role, "user");

        // Search only assistant messages
        let result = idx
            .search("role:assistant authentication", None, 10, 0)
            .expect("search");
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.sessions[0].top_match.role, "assistant");
    }

    #[test]
    fn test_search_with_skill_qualifier() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        let docs = vec![
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "assistant".to_string(),
                content: "I will edit the file to fix the bug".to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec!["Edit".to_string()],
            },
            SearchDocument {
                session_id: "sess-002".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "assistant".to_string(),
                content: "I will search for the bug in the codebase".to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec!["Grep".to_string()],
            },
        ];

        idx.index_session("sess-001", &docs[0..1])
            .expect("index");
        idx.index_session("sess-002", &docs[1..2])
            .expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Search with skill qualifier
        let result = idx
            .search("skill:Edit bug", None, 10, 0)
            .expect("search");
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "sess-001");
    }

    #[test]
    fn test_search_pagination() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        // Create 5 sessions each with content containing "rust"
        for i in 0..5 {
            let docs = vec![SearchDocument {
                session_id: format!("sess-{:03}", i),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: format!("building a rust application part {}", i),
                turn_number: 1,
                timestamp: 1739598000 + i as i64,
                skills: vec![],
            }];
            idx.index_session(&format!("sess-{:03}", i), &docs)
                .expect("index");
        }
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Get first 2
        let result = idx.search("rust", None, 2, 0).expect("search page 1");
        assert_eq!(result.total_sessions, 5);
        assert_eq!(result.sessions.len(), 2);

        // Get next 2
        let result = idx.search("rust", None, 2, 2).expect("search page 2");
        assert_eq!(result.total_sessions, 5);
        assert_eq!(result.sessions.len(), 2);

        // Get last 1
        let result = idx.search("rust", None, 2, 4).expect("search page 3");
        assert_eq!(result.total_sessions, 5);
        assert_eq!(result.sessions.len(), 1);
    }

    #[test]
    fn test_session_hit_branch_none_when_empty() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        let docs = vec![SearchDocument {
            session_id: "sess-no-branch".to_string(),
            project: "test".to_string(),
            branch: "".to_string(), // empty = no branch
            model: "".to_string(),
            role: "user".to_string(),
            content: "some query content here".to_string(),
            turn_number: 1,
            timestamp: 1739598000,
            skills: vec![],
        }];

        idx.index_session("sess-no-branch", &docs).expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        let result = idx.search("query content", None, 10, 0).expect("search");
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].branch, None);
    }

    #[test]
    fn test_search_quoted_phrase() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        let docs = vec![
            SearchDocument {
                session_id: "sess-001".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "fix the login authentication system".to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec![],
            },
            SearchDocument {
                session_id: "sess-002".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "the authentication for the login page".to_string(),
                turn_number: 1,
                timestamp: 1739598060,
                skills: vec![],
            },
        ];

        idx.index_session("sess-001", &docs[0..1])
            .expect("index");
        idx.index_session("sess-002", &docs[1..2])
            .expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        // Exact phrase "login authentication" should only match sess-001
        let result = idx
            .search("\"login authentication\"", None, 10, 0)
            .expect("phrase search");
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "sess-001");
    }

    #[test]
    fn test_multiple_sessions_sorted_by_best_score() {
        let idx = SearchIndex::open_in_ram().expect("create index");

        // Session 1: weak match (one mention)
        let docs1 = vec![SearchDocument {
            session_id: "sess-weak".to_string(),
            project: "test".to_string(),
            branch: "".to_string(),
            model: "".to_string(),
            role: "user".to_string(),
            content: "something about authentication".to_string(),
            turn_number: 1,
            timestamp: 1739598000,
            skills: vec![],
        }];

        // Session 2: strong match (multiple mentions)
        let docs2 = vec![
            SearchDocument {
                session_id: "sess-strong".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "user".to_string(),
                content: "authentication authentication authentication is critical for authentication"
                    .to_string(),
                turn_number: 1,
                timestamp: 1739598000,
                skills: vec![],
            },
            SearchDocument {
                session_id: "sess-strong".to_string(),
                project: "test".to_string(),
                branch: "".to_string(),
                model: "".to_string(),
                role: "assistant".to_string(),
                content: "I will implement robust authentication for the authentication system"
                    .to_string(),
                turn_number: 2,
                timestamp: 1739598060,
                skills: vec![],
            },
        ];

        idx.index_session("sess-weak", &docs1).expect("index");
        idx.index_session("sess-strong", &docs2).expect("index");
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        let result = idx
            .search("authentication", None, 10, 0)
            .expect("search");
        assert_eq!(result.total_sessions, 2);
        // The session with the strongest match should come first
        assert_eq!(result.sessions[0].session_id, "sess-strong");
        assert!(result.sessions[0].best_score > result.sessions[1].best_score);
    }

    #[test]
    fn test_schema_version_mismatch_triggers_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let idx_path = dir.path().join("search");

        // Create an index at version 1
        std::fs::create_dir_all(&idx_path).unwrap();
        std::fs::write(idx_path.join("schema_version"), "1").unwrap();
        let _idx = SearchIndex::open(&idx_path).unwrap();

        // Now "upgrade" to version 999 and re-open
        let version_path = idx_path.join("schema_version");
        std::fs::write(&version_path, "1").unwrap(); // simulate old version on disk

        let current = format!("{}", SEARCH_SCHEMA_VERSION);
        // After open(), the version file should always match SEARCH_SCHEMA_VERSION
        let _idx2 = SearchIndex::open(&idx_path).unwrap();
        let after = std::fs::read_to_string(&version_path).unwrap();
        assert_eq!(after.trim(), current, "schema_version file should be updated to current version");
    }

    #[test]
    fn test_search_model_partial_match() {
        let idx = SearchIndex::open_in_ram().unwrap();

        let docs = vec![
            SearchDocument {
                session_id: "s1".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: "claude-opus-4-6".to_string(),
                role: "user".to_string(),
                content: "hello world".to_string(),
                turn_number: 1,
                timestamp: 1000,
                skills: vec![],
            },
            SearchDocument {
                session_id: "s2".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: "claude-sonnet-4-5".to_string(),
                role: "user".to_string(),
                content: "hello world".to_string(),
                turn_number: 1,
                timestamp: 1000,
                skills: vec![],
            },
        ];

        idx.index_session("s1", &docs[..1]).unwrap();
        idx.index_session("s2", &docs[1..]).unwrap();
        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        // Partial model name should match
        let result = idx.search("model:opus hello", None, 10, 0).unwrap();
        assert_eq!(result.total_sessions, 1, "model:opus should match claude-opus-4-6");
        assert_eq!(result.sessions[0].session_id, "s1");

        // Full model name should also still match
        let result2 = idx.search("model:claude-opus-4-6 hello", None, 10, 0).unwrap();
        assert_eq!(result2.total_sessions, 1, "full model name should still match");
    }

    #[test]
    fn test_search_project_qualifier_with_display_name() {
        let idx = SearchIndex::open_in_ram().unwrap();

        let docs_a = vec![SearchDocument {
            session_id: "s1".to_string(),
            project: "claude-view".to_string(),
            branch: "main".to_string(),
            model: "claude-opus-4-6".to_string(),
            role: "user".to_string(),
            content: "fix the login bug".to_string(),
            turn_number: 1,
            timestamp: 1000,
            skills: vec![],
        }];

        let docs_b = vec![SearchDocument {
            session_id: "s2".to_string(),
            project: "test-app".to_string(),
            branch: "main".to_string(),
            model: "claude-sonnet-4-5".to_string(),
            role: "user".to_string(),
            content: "setup the project".to_string(),
            turn_number: 1,
            timestamp: 2000,
            skills: vec![],
        }];

        idx.index_session("s1", &docs_a).unwrap();
        idx.index_session("s2", &docs_b).unwrap();
        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        let result = idx.search("project:test-app", None, 10, 0).unwrap();
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "s2");

        let result2 = idx.search("project:claude-view fix", None, 10, 0).unwrap();
        assert_eq!(result2.total_sessions, 1);
        assert_eq!(result2.sessions[0].session_id, "s1");
    }

    #[test]
    fn test_search_qualifier_only_no_text() {
        let idx = SearchIndex::open_in_ram().unwrap();

        let docs = vec![SearchDocument {
            session_id: "s1".to_string(),
            project: "my-project".to_string(),
            branch: "main".to_string(),
            model: "claude-opus-4-6".to_string(),
            role: "user".to_string(),
            content: "implement authentication".to_string(),
            turn_number: 1,
            timestamp: 1000,
            skills: vec!["commit".to_string()],
        }];

        idx.index_session("s1", &docs).unwrap();
        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        let r1 = idx.search("project:my-project", None, 10, 0).unwrap();
        assert_eq!(r1.total_sessions, 1, "project-only qualifier should work");

        let r2 = idx.search("branch:main", None, 10, 0).unwrap();
        assert_eq!(r2.total_sessions, 1, "branch-only qualifier should work");

        let r3 = idx.search("role:user", None, 10, 0).unwrap();
        assert_eq!(r3.total_sessions, 1, "role-only qualifier should work");

        let r4 = idx.search("skill:commit", None, 10, 0).unwrap();
        assert_eq!(r4.total_sessions, 1, "skill-only qualifier should work");

        let r5 = idx.search("model:opus", None, 10, 0).unwrap();
        assert_eq!(r5.total_sessions, 1, "model-only qualifier (partial) should work");
    }
}
