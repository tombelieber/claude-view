// crates/search/src/unified.rs
//! Unified search: Tantivy first, grep fallback if 0 results.
//!
//! Implements the "One Endpoint Per Capability" design principle:
//! the frontend sends one request, the backend tries all strategies
//! internally and returns a unified `SearchResponse`.

use crate::grep::{grep_files, GrepOptions, JsonlFile};
use crate::types::{MatchHit, SearchResponse, SessionHit};
use crate::SearchIndex;

/// Which engine produced the search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEngine {
    /// Results came from Tantivy full-text index.
    Tantivy,
    /// Results came from grep fallback (Tantivy returned 0).
    Grep,
}

/// Options for unified search.
pub struct UnifiedSearchOptions {
    /// The raw query string.
    pub query: String,
    /// Optional scope filter (e.g. `"project:claude-view"`).
    pub scope: Option<String>,
    /// Maximum session groups to return.
    pub limit: usize,
    /// Session groups to skip (pagination). NOTE: only applies to Tantivy.
    /// Grep fallback ignores offset (no session-level pagination in grep).
    pub offset: usize,
}

/// Extended search response with engine metadata.
pub struct UnifiedSearchResult {
    pub response: SearchResponse,
    pub engine: SearchEngine,
}

#[derive(Debug, thiserror::Error)]
pub enum UnifiedSearchError {
    #[error("Search error: {0}")]
    Search(#[from] crate::SearchError),
    #[error("Grep error: {0}")]
    Grep(#[from] crate::grep::GrepError),
}

/// Run unified search: Tantivy first, grep fallback if 0 results.
///
/// - `search_index`: If `None`, skips Tantivy and goes straight to grep.
///   (The route handler should return 503 when index is building — this
///   `None` path is for tests and future direct-grep contexts only.)
/// - `jsonl_files`: Pre-collected files to grep if fallback is needed.
/// - `opts`: Query, scope, pagination.
///
/// **Qualifier limitation:** Qualifier-based project filters (e.g. `project:foo`
/// embedded in `opts.query`) are Tantivy-only. Grep fallback scoping requires
/// the explicit `scope` field in `opts`, which the route handler populates from
/// the `scope` query parameter. This is an inherent grep limitation.
pub fn unified_search(
    search_index: Option<&SearchIndex>,
    jsonl_files: &[JsonlFile],
    opts: &UnifiedSearchOptions,
) -> Result<UnifiedSearchResult, UnifiedSearchError> {
    // Phase 1: Tantivy
    if let Some(idx) = search_index {
        let tantivy_result =
            idx.search(&opts.query, opts.scope.as_deref(), opts.limit, opts.offset)?;

        if tantivy_result.total_sessions > 0 {
            return Ok(UnifiedSearchResult {
                response: tantivy_result,
                engine: SearchEngine::Tantivy,
            });
        }
    }

    // Phase 2: Grep fallback
    if jsonl_files.is_empty() {
        // No files to grep — return empty results with no engine indicator.
        // Don't set search_engine: "grep" here because grep didn't actually run.
        return Ok(UnifiedSearchResult {
            response: SearchResponse {
                query: opts.query.clone(),
                total_sessions: 0,
                total_matches: 0,
                elapsed_ms: 0.0,
                sessions: vec![],
                search_engine: None,
            },
            engine: SearchEngine::Grep,
        });
    }

    let grep_opts = GrepOptions {
        pattern: regex_escape_for_literal(&opts.query),
        case_sensitive: false,
        whole_word: false,
        limit: opts.limit * 10, // over-fetch lines, we group by session
    };

    let grep_result = grep_files(jsonl_files, &grep_opts)?;

    // Normalize grep results into SearchResponse shape
    let sessions: Vec<SessionHit> = grep_result
        .results
        .into_iter()
        .take(opts.limit)
        .map(|hit| {
            let match_count = hit.matches.len();
            let top_match = hit
                .matches
                .first()
                .map(|m| MatchHit {
                    role: "unknown".to_string(),
                    turn_number: 0,
                    snippet: truncate_and_highlight(&m.content, m.match_start, m.match_end),
                    timestamp: hit.modified_at,
                })
                .unwrap_or_else(|| MatchHit {
                    role: "unknown".to_string(),
                    turn_number: 0,
                    snippet: String::new(),
                    timestamp: 0,
                });

            let matches: Vec<MatchHit> = hit
                .matches
                .iter()
                .map(|m| MatchHit {
                    role: "unknown".to_string(),
                    turn_number: 0,
                    snippet: truncate_and_highlight(&m.content, m.match_start, m.match_end),
                    timestamp: hit.modified_at,
                })
                .collect();

            SessionHit {
                session_id: hit.session_id,
                project: hit.project,
                branch: None,
                modified_at: hit.modified_at,
                match_count,
                best_score: 1.0, // grep has no scoring — uniform
                top_match,
                matches,
            }
        })
        .collect();

    let total_sessions = sessions.len();
    let total_matches: usize = sessions.iter().map(|s| s.match_count).sum();

    Ok(UnifiedSearchResult {
        response: SearchResponse {
            query: opts.query.clone(),
            total_sessions,
            total_matches,
            elapsed_ms: grep_result.elapsed_ms,
            sessions,
            search_engine: Some("grep".to_string()),
        },
        engine: SearchEngine::Grep,
    })
}

/// Escape regex metacharacters for literal grep search.
/// When the user types plain text, we want grep to find it literally.
fn regex_escape_for_literal(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        if "\\.*+?()[]{}|^$".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Truncate raw JSONL line content and wrap match region with <mark> tags.
fn truncate_and_highlight(content: &str, match_start: usize, match_end: usize) -> String {
    let chars: Vec<char> = content.chars().collect();
    let total = chars.len();

    let context_before = 80;
    let context_after = 150;
    let start = match_start.saturating_sub(context_before);
    let end = (match_end + context_after).min(total);

    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < total { "..." } else { "" };

    let before: String = chars[start..match_start.min(total)].iter().collect();
    let matched: String = chars[match_start.min(total)..match_end.min(total)]
        .iter()
        .collect();
    let after: String = chars[match_end.min(total)..end].iter().collect();

    format!("{prefix}{before}<mark>{matched}</mark>{after}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::SearchDocument;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create an in-RAM Tantivy index with some documents.
    fn create_test_index(docs: &[(&str, &str, &str)]) -> SearchIndex {
        let idx = SearchIndex::open_in_ram().expect("create index");
        for (session_id, role, content) in docs {
            let doc = SearchDocument {
                session_id: session_id.to_string(),
                project: "test-project".to_string(),
                branch: "main".to_string(),
                model: "opus".to_string(),
                role: role.to_string(),
                content: content.to_string(),
                turn_number: 1,
                timestamp: 1710000000,
                skills: vec![],
            };
            idx.index_session(session_id, &[doc]).expect("index");
        }
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");
        idx
    }

    /// Helper: create temp JSONL files for grep testing.
    fn create_test_jsonl_files(dir: &std::path::Path, entries: &[(&str, &str)]) -> Vec<JsonlFile> {
        entries
            .iter()
            .map(|(session_id, content)| {
                let path = dir.join(format!("{session_id}.jsonl"));
                fs::write(&path, content).unwrap();
                JsonlFile {
                    path,
                    session_id: session_id.to_string(),
                    project: "test-project".to_string(),
                    project_path: dir.to_string_lossy().to_string(),
                    modified_at: 1710000000,
                }
            })
            .collect()
    }

    #[test]
    fn test_tantivy_hit_returns_tantivy_engine() {
        let idx = create_test_index(&[("s1", "user", "deploy to production tonight")]);
        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &[], &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Tantivy);
        assert_eq!(result.response.total_sessions, 1);
        // Tantivy path should NOT set search_engine
        assert!(result.response.search_engine.is_none());
    }

    #[test]
    fn test_tantivy_miss_falls_back_to_grep() {
        let tmp = TempDir::new().unwrap();
        let idx = create_test_index(&[("s1", "user", "deploy to production")]);
        let files = create_test_jsonl_files(
            tmp.path(),
            &[(
                "s2",
                "{\"role\":\"user\",\"content\":\"hook 嘅 payload 本身冇問題\"}\n",
            )],
        );

        let opts = UnifiedSearchOptions {
            query: "嘅 payload".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert!(result.response.total_sessions > 0);
        assert_eq!(result.response.search_engine.as_deref(), Some("grep"));
    }

    #[test]
    fn test_cjk_text_found_via_grep_fallback() {
        let tmp = TempDir::new().unwrap();
        let idx = create_test_index(&[]); // empty index
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"自動部署到生產環境完成\"}\n")],
        );

        let opts = UnifiedSearchOptions {
            query: "部署".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
        let snippet = &result.response.sessions[0].top_match.snippet;
        assert!(
            snippet.contains("<mark>"),
            "snippet should have highlight: {snippet}"
        );
    }

    #[test]
    fn test_no_index_goes_straight_to_grep() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(tmp.path(), &[("s1", "{\"msg\":\"hello world\"}\n")]);

        let opts = UnifiedSearchOptions {
            query: "hello".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(None, &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
    }

    #[test]
    fn test_both_empty_returns_zero_results() {
        let idx = create_test_index(&[]);
        let opts = UnifiedSearchOptions {
            query: "nonexistent".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &[], &opts).unwrap();
        assert_eq!(result.response.total_sessions, 0);
    }

    #[test]
    fn test_regex_metacharacters_escaped_for_grep() {
        let tmp = TempDir::new().unwrap();
        let idx = create_test_index(&[]);
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"auth.*middleware pattern\"}\n")],
        );

        // User types literal "auth.*middleware" — should find it literally,
        // NOT interpret .* as regex "any characters"
        let opts = UnifiedSearchOptions {
            query: "auth.*middleware".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
    }

    #[test]
    fn test_grep_results_normalized_to_session_hit_shape() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[(
                "s1",
                "{\"content\":\"line one match\"}\n{\"content\":\"line two match\"}\n",
            )],
        );

        let opts = UnifiedSearchOptions {
            query: "match".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(None, &files, &opts).unwrap();

        assert_eq!(result.response.sessions.len(), 1);
        let session = &result.response.sessions[0];
        assert_eq!(session.session_id, "s1");
        assert_eq!(session.match_count, 2);
        assert_eq!(session.matches.len(), 2);
        assert_eq!(session.best_score, 1.0);
        assert!(session.branch.is_none());
        assert!(session.top_match.snippet.contains("<mark>match</mark>"));
    }

    #[test]
    fn test_regex_escape_for_literal() {
        assert_eq!(regex_escape_for_literal("hello"), "hello");
        assert_eq!(regex_escape_for_literal("a.*b"), "a\\.\\*b");
        assert_eq!(regex_escape_for_literal("fn()"), "fn\\(\\)");
        assert_eq!(regex_escape_for_literal("[test]"), "\\[test\\]");
        assert_eq!(regex_escape_for_literal("a|b"), "a\\|b");
        // CJK passes through unchanged
        assert_eq!(regex_escape_for_literal("部署"), "部署");
    }
}
