// crates/search/src/unified.rs
//! Session search over raw JSONL files.
//!
//! Grep is the session text search engine: exact substring matching, CJK-safe,
//! and no persistent index lifecycle to manage.

use crate::grep::{grep_files, GrepOptions, JsonlFile};
use crate::types::{MatchHit, SearchResponse, SessionHit};

/// Which engine produced the search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEngine {
    /// Results came from grep over raw JSONL files.
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
    /// Session groups to skip.
    pub offset: usize,
    /// Skip snippet generation — retained for API compatibility.
    pub skip_snippets: bool,
}

/// Extended search response with engine metadata.
pub struct UnifiedSearchResult {
    pub response: SearchResponse,
    pub engine: SearchEngine,
}

#[derive(Debug, thiserror::Error)]
pub enum UnifiedSearchError {
    #[error("Grep error: {0}")]
    Grep(#[from] crate::grep::GrepError),
}

/// Run session search using grep over raw JSONL files.
///
/// Caller is responsible for wrapping this in `spawn_blocking` if needed.
pub fn unified_search(
    jsonl_files: &[JsonlFile],
    opts: &UnifiedSearchOptions,
) -> Result<UnifiedSearchResult, UnifiedSearchError> {
    if opts.limit == 0 {
        return Ok(UnifiedSearchResult {
            response: SearchResponse {
                query: opts.query.clone(),
                total_sessions: 0,
                total_matches: 0,
                elapsed_ms: 0.0,
                sessions: vec![],
            },
            engine: SearchEngine::Grep,
        });
    }

    if !jsonl_files.is_empty() {
        let grep_opts = GrepOptions {
            pattern: regex_escape_for_literal(&opts.query),
            case_sensitive: false,
            whole_word: false,
            limit: opts
                .limit
                .saturating_add(opts.offset)
                .saturating_mul(10)
                .min(100_000),
        };

        let grep_resp = grep_files(jsonl_files, &grep_opts)?;
        let mut sessions: Vec<SessionHit> = grep_resp
            .results
            .into_iter()
            .map(|hit| {
                let match_count = hit.matches.len();
                let top_match = if opts.skip_snippets {
                    MatchHit {
                        role: "unknown".to_string(),
                        turn_number: 0,
                        snippet: String::new(),
                        timestamp: hit.modified_at,
                    }
                } else {
                    hit.matches
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
                        })
                };
                let matches: Vec<MatchHit> = if opts.skip_snippets {
                    Vec::new()
                } else {
                    hit.matches
                        .iter()
                        .map(|m| MatchHit {
                            role: "unknown".to_string(),
                            turn_number: 0,
                            snippet: truncate_and_highlight(&m.content, m.match_start, m.match_end),
                            timestamp: hit.modified_at,
                        })
                        .collect()
                };
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
        let sessions = sessions
            .into_iter()
            .skip(opts.offset)
            .take(opts.limit)
            .collect();

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

    Ok(UnifiedSearchResult {
        response: SearchResponse {
            query: opts.query.clone(),
            total_sessions: 0,
            total_matches: 0,
            elapsed_ms: 0.0,
            sessions: vec![],
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
    use std::fs;
    use tempfile::TempDir;

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

    #[test]
    fn test_grep_only_searches_jsonl() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"deploy to production\"}\n", 1710000000)],
        );

        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
            skip_snippets: false,
        };
        let result = unified_search(&files, &opts).unwrap();

        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
        assert_eq!(result.response.sessions[0].session_id, "s1");
        assert_eq!(result.response.sessions[0].engines, vec!["grep"]);
    }

    /// CJK search works via grep over the raw JSONL line.
    #[test]
    fn test_cjk_found_by_grep() {
        let tmp = TempDir::new().unwrap();
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
        let result = unified_search(&files, &opts).unwrap();

        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
        let snippet = &result.response.sessions[0].top_match.snippet;
        assert!(
            snippet.contains("<mark>"),
            "snippet should have highlight: {snippet}"
        );
    }

    /// Results are sorted newest-first.
    #[test]
    fn test_sorted_by_recency() {
        let tmp = TempDir::new().unwrap();
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
        let result = unified_search(&files, &opts).unwrap();

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

    #[test]
    fn test_offset_and_limit_apply_after_session_grouping() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[
                ("s1", "{\"content\":\"deploy\"}\n", 100),
                ("s2", "{\"content\":\"deploy\"}\n", 200),
                ("s3", "{\"content\":\"deploy\"}\n", 300),
            ],
        );
        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 1,
            offset: 1,
            skip_snippets: false,
        };
        let result = unified_search(&files, &opts).unwrap();

        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 3);
        assert_eq!(result.response.sessions.len(), 1);
        assert_eq!(result.response.sessions[0].session_id, "s2");
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
