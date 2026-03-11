// crates/search/src/unified.rs
//! Co-primary search: grep primary + Tantivy supplement.
//!
//! Grep is the primary text search engine (exact substring, regex, CJK).
//! Tantivy supplements with fuzzy matching and typo recovery.
//! Both engines always run; results are merged with grep as the base.

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
    /// Skip snippet generation — for callers that only need session IDs.
    pub skip_snippets: bool,
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

/// Run co-primary search: grep (primary) + Tantivy (supplement), both always run.
///
/// Grep is the primary engine — exact substring match, CJK-safe, zero warmup.
/// Tantivy supplements with fuzzy/BM25 results for typo recovery and scoring.
/// Results are merged with grep as the base; Tantivy fills gaps when grep is
/// under the limit.
///
/// Caller is responsible for wrapping this in `spawn_blocking` if needed.
pub fn unified_search(
    search_index: Option<&SearchIndex>,
    jsonl_files: &[JsonlFile],
    opts: &UnifiedSearchOptions,
) -> Result<UnifiedSearchResult, UnifiedSearchError> {
    // 1. Tantivy (PRIMARY) — always run if index available
    let tantivy_result = search_index.map(|idx| {
        idx.search(
            &opts.query,
            opts.scope.as_deref(),
            opts.limit,
            opts.offset,
            opts.skip_snippets,
        )
    });

    let tantivy_response: Option<SearchResponse> = match tantivy_result {
        Some(Ok(resp)) => Some(resp),
        Some(Err(e)) => {
            tracing::warn!(error = %e, "Tantivy search failed, will try grep fallback");
            None
        }
        None => None,
    };

    // 2. If Tantivy found results, return directly — grep does NOT run.
    if let Some(resp) = tantivy_response {
        if !resp.sessions.is_empty() {
            return Ok(UnifiedSearchResult {
                response: resp,
                engine: SearchEngine::Tantivy,
            });
        }
    }

    // 3. Grep (FALLBACK) — only runs when Tantivy returned 0 results
    if !jsonl_files.is_empty() {
        let grep_opts = GrepOptions {
            pattern: regex_escape_for_literal(&opts.query),
            case_sensitive: false,
            whole_word: false,
            limit: opts.limit.saturating_mul(10).min(100_000),
        };
        match grep_files(jsonl_files, &grep_opts) {
            Ok(grep_resp) => {
                let mut sessions: Vec<SessionHit> = grep_resp
                    .results
                    .into_iter()
                    .map(|hit| {
                        let match_count = hit.matches.len();
                        let top_match = hit
                            .matches
                            .first()
                            .map(|m| MatchHit {
                                role: "unknown".to_string(),
                                turn_number: 0,
                                snippet: truncate_and_highlight(
                                    &m.content,
                                    m.match_start,
                                    m.match_end,
                                ),
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
                                snippet: truncate_and_highlight(
                                    &m.content,
                                    m.match_start,
                                    m.match_end,
                                ),
                                timestamp: hit.modified_at,
                            })
                            .collect();
                        SessionHit {
                            session_id: hit.session_id,
                            project: hit.project,
                            branch: None,
                            modified_at: hit.modified_at,
                            match_count,
                            best_score: 0.0,
                            top_match,
                            matches,
                            engines: vec!["grep".to_string()],
                        }
                    })
                    .collect();

                sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
                let total_sessions = sessions.len();
                let total_matches: usize = sessions.iter().map(|s| s.match_count).sum();
                sessions.truncate(opts.limit);

                return Ok(UnifiedSearchResult {
                    response: SearchResponse {
                        query: opts.query.clone(),
                        total_sessions,
                        total_matches,
                        elapsed_ms: 0.0,
                        sessions,
                    },
                    engine: SearchEngine::Grep,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "Grep fallback also failed");
            }
        }
    }

    // 4. Both engines returned nothing
    Ok(UnifiedSearchResult {
        response: SearchResponse {
            query: opts.query.clone(),
            total_sessions: 0,
            total_matches: 0,
            elapsed_ms: 0.0,
            sessions: vec![],
        },
        engine: SearchEngine::Tantivy,
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
    /// `entries` is `(session_id, content, modified_at)`.
    fn create_test_jsonl_files(
        dir: &std::path::Path,
        entries: &[(&str, &str, i64)],
    ) -> Vec<JsonlFile> {
        entries
            .iter()
            .map(|(session_id, content, modified_at)| {
                let path = dir.join(format!("{session_id}.jsonl"));
                fs::write(&path, content).unwrap();
                JsonlFile {
                    path,
                    session_id: session_id.to_string(),
                    project: "test-project".to_string(),
                    project_path: dir.to_string_lossy().to_string(),
                    modified_at: *modified_at,
                }
            })
            .collect()
    }

    /// Tantivy is primary — only its results appear (grep doesn't run).
    #[test]
    fn test_tantivy_primary_returns_only_tantivy_results() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[
                ("s1", "{\"content\":\"deploy to production\"}\n", 1710000003),
                ("s2", "{\"content\":\"deploy staging\"}\n", 1710000002),
            ],
        );
        // Tantivy has only s3
        let idx = create_test_index(&[("s3", "user", "deploy to dev")]);

        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 2,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();

        // Tantivy found s3 — grep doesn't run
        assert_eq!(result.engine, SearchEngine::Tantivy);
        assert_eq!(result.response.total_sessions, 1);
        assert_eq!(result.response.sessions[0].session_id, "s3");
    }

    /// Grep finds nothing; Tantivy (fuzzy) supplements.
    #[test]
    fn test_tantivy_supplements_when_grep_misses() {
        let tmp = TempDir::new().unwrap();
        // Files contain "depploy" (typo) — grep for exact "deploy" misses
        let files = create_test_jsonl_files(
            tmp.path(),
            &[(
                "s1",
                "{\"content\":\"depploy to production\"}\n",
                1710000000,
            )],
        );
        // Tantivy has exact "deploy" indexed
        let idx = create_test_index(&[("s2", "user", "deploy to production tonight")]);

        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();

        // Grep misses (no exact "deploy" in files), Tantivy finds s2
        // Engine is Tantivy since grep_count == 0
        assert_eq!(result.engine, SearchEngine::Tantivy);
        assert!(result.response.total_sessions >= 1);
        let ids: Vec<&str> = result
            .response
            .sessions
            .iter()
            .map(|s| s.session_id.as_str())
            .collect();
        assert!(ids.contains(&"s2"), "Tantivy should supplement with s2");
    }

    /// CJK search works via grep (Tantivy doesn't tokenize CJK).
    #[test]
    fn test_cjk_found_by_grep() {
        let tmp = TempDir::new().unwrap();
        let idx = create_test_index(&[]); // empty index — Tantivy won't help
        let files = create_test_jsonl_files(
            tmp.path(),
            &[(
                "s1",
                "{\"content\":\"自動部署到生產環境完成\"}\n",
                1710000000,
            )],
        );

        let opts = UnifiedSearchOptions {
            query: "部署".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
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

    /// Tantivy-primary: when Tantivy finds the session, grep doesn't run.
    #[test]
    fn test_tantivy_primary_session_has_tantivy_engine_only() {
        let tmp = TempDir::new().unwrap();
        // JSONL file for grep (would find s1 if grep ran)
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"deploy to production\"}\n", 1710000000)],
        );
        // Same session ID indexed in Tantivy
        let idx = create_test_index(&[("s1", "user", "deploy to production tonight")]);

        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();

        assert_eq!(result.response.total_sessions, 1);
        let session = &result.response.sessions[0];
        assert_eq!(session.session_id, "s1");
        // Only Tantivy engine — grep never ran
        assert_eq!(session.engines, vec!["tantivy"]);
    }

    /// Results are sorted newest-first, with BM25 score as tiebreaker.
    #[test]
    fn test_sorted_by_recency_then_score() {
        let tmp = TempDir::new().unwrap();
        // Three sessions: s_old (ts=100), s_new (ts=200), s_mid (ts=150)
        let files = create_test_jsonl_files(
            tmp.path(),
            &[
                ("s_old", "{\"content\":\"hello world\"}\n", 100),
                ("s_new", "{\"content\":\"hello world\"}\n", 200),
                ("s_mid", "{\"content\":\"hello world\"}\n", 150),
            ],
        );

        let opts = UnifiedSearchOptions {
            query: "hello".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(None, &files, &opts).unwrap();

        assert_eq!(result.response.total_sessions, 3);
        let ids: Vec<&str> = result
            .response
            .sessions
            .iter()
            .map(|s| s.session_id.as_str())
            .collect();
        assert_eq!(ids[0], "s_new", "newest session first");
        assert_eq!(ids[1], "s_mid", "second newest second");
        assert_eq!(ids[2], "s_old", "oldest last");
    }

    /// No JSONL files but Tantivy works — engine tag is Tantivy.
    #[test]
    fn test_grep_error_returns_tantivy_alone() {
        let idx = create_test_index(&[("s1", "user", "hello from tantivy only")]);

        let opts = UnifiedSearchOptions {
            query: "hello".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        // Pass empty files — grep has nothing to search
        let result = unified_search(Some(&idx), &[], &opts).unwrap();

        // grep_count == 0 → engine reported as Tantivy
        assert_eq!(result.engine, SearchEngine::Tantivy);
        assert_eq!(result.response.total_sessions, 1);
        assert_eq!(result.response.sessions[0].engines, vec!["tantivy"]);
    }

    /// No Tantivy index (None) — grep works alone.
    #[test]
    fn test_no_index_grep_only() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"msg\":\"hello world\"}\n", 1710000000)],
        );

        let opts = UnifiedSearchOptions {
            query: "hello".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(None, &files, &opts).unwrap();

        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
        assert_eq!(result.response.sessions[0].engines, vec!["grep"]);
    }

    /// When Tantivy finds results, grep does NOT run. Only Tantivy sessions appear.
    #[test]
    fn test_tantivy_primary_wins_over_grep() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[
                ("s1", "{\"content\":\"deploy to production\"}\n", 1710000003),
                ("s2", "{\"content\":\"deploy staging\"}\n", 1710000002),
            ],
        );
        let idx = create_test_index(&[("s3", "user", "deploy to dev")]);
        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 2,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Tantivy);
        assert_eq!(result.response.total_sessions, 1);
        assert_eq!(result.response.sessions[0].session_id, "s3");
    }

    /// Engine tag is tantivy-only when Tantivy wins.
    #[test]
    fn test_tantivy_primary_only_tantivy_engine_tag() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"deploy to production\"}\n", 1710000000)],
        );
        let idx = create_test_index(&[("s1", "user", "deploy to production tonight")]);
        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.response.total_sessions, 1);
        assert_eq!(result.response.sessions[0].engines, vec!["tantivy"]);
    }

    /// Grep fallback when Tantivy returns 0 (CJK, empty index).
    #[test]
    fn test_grep_fallback_when_tantivy_returns_zero() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"自動部署到生產環境\"}\n", 1710000000)],
        );
        let idx = create_test_index(&[]); // empty index
        let opts = UnifiedSearchOptions {
            query: "部署".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.response.total_sessions, 1);
        assert_eq!(result.engine, SearchEngine::Grep);
    }

    /// Tantivy primary: find all sessions (not capped by grep's subset).
    #[test]
    fn test_tantivy_primary_grep_does_not_override() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[
                ("s1", "{\"content\":\"deploy to production\"}\n", 1710000003),
                ("s2", "{\"content\":\"deploy staging\"}\n", 1710000002),
            ],
        );
        let idx = create_test_index(&[
            ("s1", "user", "deploy to production"),
            ("s2", "user", "deploy staging"),
            ("s3", "user", "deploy to dev environment"),
        ]);
        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(
            result.response.total_sessions, 3,
            "Tantivy should find all 3"
        );
        let ids: Vec<&str> = result
            .response
            .sessions
            .iter()
            .map(|s| s.session_id.as_str())
            .collect();
        assert!(ids.contains(&"s3"), "Tantivy-only session s3 must appear");
        assert_eq!(result.engine, SearchEngine::Tantivy);
    }

    /// `regex_escape_for_literal` correctly escapes regex metacharacters.
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
