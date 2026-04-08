/// Search execution: runs Tantivy queries, groups by session, sorts, generates snippets.
use std::collections::HashMap;
use std::time::Instant;

use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, Occur, PhraseQuery, Query, TermQuery};
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::snippet::SnippetGenerator;
use tantivy::{DocAddress, TantivyDocument, Term};
use tracing::debug;

use crate::types::{MatchHit, SearchResponse, SessionHit};
use crate::{SearchError, SearchIndex};

use super::parsing::{is_session_id, tokenize_text_terms};

/// Truncate a string to at most `max_bytes` bytes, respecting UTF-8 char boundaries.
fn truncate_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Find the last char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

impl SearchIndex {
    /// Execute a full-text search query.
    ///
    /// - `query_str`: Raw query string, may contain qualifiers like `project:foo`
    ///   and quoted phrases like `"exact match"`.
    /// - `scope`: Optional scope filter, e.g. `"project:claude-view"`.
    /// - `limit`: Maximum number of session groups to return.
    /// - `offset`: Number of session groups to skip (for pagination).
    pub fn search(
        &self,
        query_str: &str,
        scope: Option<&str>,
        limit: usize,
        offset: usize,
        skip_snippets: bool,
    ) -> Result<SearchResponse, SearchError> {
        let start = Instant::now();

        // Fast path: if the query is a bare UUID, do an exact session_id lookup
        let trimmed_query = query_str.trim();
        if is_session_id(trimmed_query) {
            return self.search_by_session_id(query_str, trimmed_query, skip_snippets, start);
        }

        // Build the Tantivy query from the raw string
        let built = match self.build_query(query_str, scope) {
            Some(b) => b,
            None => {
                return Ok(SearchResponse {
                    query: query_str.to_string(),
                    total_sessions: 0,
                    total_matches: 0,
                    elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                    sessions: vec![],
                });
            }
        };

        let searcher = self.reader.searcher();

        // Two-phase search: Count gets true total, TopDocs gets top scored results.
        let total_matches_all = searcher.search(&built.query, &Count)?;

        // Fetch ALL matching docs so session grouping gives a true total_sessions count.
        // Without this, paginated queries under-count total sessions.
        let fetch_limit = total_matches_all.max(1);
        let top_docs = searcher.search(&built.query, &TopDocs::with_limit(fetch_limit))?;

        // Group by session_id
        let mut session_groups: HashMap<String, Vec<(f32, DocAddress)>> = HashMap::new();

        for (score, doc_addr) in &top_docs {
            let retrieved: TantivyDocument = searcher.doc(*doc_addr)?;
            let session_id = retrieved
                .get_first(self.session_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            session_groups
                .entry(session_id)
                .or_default()
                .push((*score, *doc_addr));
        }

        let total_sessions_all = session_groups.len();

        if skip_snippets {
            let sessions: Vec<SessionHit> = session_groups
                .into_keys()
                .map(|session_id| SessionHit {
                    session_id,
                    project: String::new(),
                    branch: None,
                    modified_at: 0,
                    match_count: 0,
                    best_score: 0.0,
                    top_match: MatchHit {
                        role: String::new(),
                        turn_number: 0,
                        snippet: String::new(),
                        timestamp: 0,
                    },
                    matches: vec![],
                    engines: vec!["tantivy".to_string()],
                })
                .collect();
            return Ok(SearchResponse {
                query: query_str.to_string(),
                total_sessions: total_sessions_all,
                total_matches: total_matches_all,
                elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                sessions,
            });
        }

        // Build snippet generator from text query
        let snippet_gen = self.build_snippet_generator(&searcher, &built.text_query);

        // Sort and paginate session groups
        let paginated = self.sort_and_paginate(session_groups, &searcher, offset, limit);

        // Build SessionHit for each group
        let sessions = self.build_session_hits(paginated, &searcher, &snippet_gen)?;

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        debug!(
            query = query_str,
            total_sessions = total_sessions_all,
            total_matches = total_matches_all,
            elapsed_ms = elapsed_ms,
            "search completed"
        );

        Ok(SearchResponse {
            query: query_str.to_string(),
            total_sessions: total_sessions_all,
            total_matches: total_matches_all,
            elapsed_ms,
            sessions,
        })
    }

    /// Fast path for bare UUID queries: exact session_id lookup.
    fn search_by_session_id(
        &self,
        query_str: &str,
        trimmed_query: &str,
        skip_snippets: bool,
        start: Instant,
    ) -> Result<SearchResponse, SearchError> {
        let session_id = trimmed_query.to_lowercase();
        let term = Term::from_field_text(self.session_id_field, &session_id);
        let term_query = tantivy::query::TermQuery::new(term, IndexRecordOption::Basic);
        let searcher = self.reader.searcher();
        let top_docs = searcher.search(&term_query, &TopDocs::with_limit(1000))?;

        if top_docs.is_empty() {
            return Ok(SearchResponse {
                query: query_str.to_string(),
                total_sessions: 0,
                total_matches: 0,
                elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                sessions: vec![],
            });
        }

        if skip_snippets {
            return Ok(SearchResponse {
                query: query_str.to_string(),
                total_sessions: 1,
                total_matches: top_docs.len(),
                elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                sessions: vec![SessionHit {
                    session_id: session_id.clone(),
                    project: String::new(),
                    branch: None,
                    modified_at: 0,
                    match_count: top_docs.len(),
                    best_score: 1.0,
                    top_match: MatchHit {
                        role: String::new(),
                        turn_number: 0,
                        snippet: String::new(),
                        timestamp: 0,
                    },
                    matches: vec![],
                    engines: vec!["tantivy".to_string()],
                }],
            });
        }

        // Build matches from all docs in this session
        let mut matches = Vec::with_capacity(top_docs.len());
        let mut project = String::new();
        let mut branch = String::new();
        let mut latest_timestamp: i64 = 0;

        for (_score, doc_addr) in &top_docs {
            let retrieved: TantivyDocument = searcher.doc(*doc_addr)?;

            let role = retrieved
                .get_first(self.role_field)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let turn_number = retrieved
                .get_first(self.turn_number_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let timestamp = retrieved
                .get_first(self.timestamp_field)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let snippet = retrieved
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .map(|s| truncate_utf8(s, 200))
                .unwrap_or_default();

            if timestamp > latest_timestamp {
                latest_timestamp = timestamp;
            }
            if project.is_empty() {
                project = retrieved
                    .get_first(self.project_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                branch = retrieved
                    .get_first(self.branch_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }

            matches.push(MatchHit {
                role,
                turn_number,
                snippet,
                timestamp,
            });
        }

        // Sort by turn number ascending for session ID lookups
        matches.sort_by_key(|m| m.turn_number);

        let top_match = matches.first().cloned().unwrap_or(MatchHit {
            role: String::new(),
            turn_number: 0,
            snippet: String::new(),
            timestamp: 0,
        });

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        debug!(
            query = query_str,
            session_id = session_id,
            matches = matches.len(),
            elapsed_ms = elapsed_ms,
            "session ID lookup completed"
        );

        Ok(SearchResponse {
            query: query_str.to_string(),
            total_sessions: 1,
            total_matches: matches.len(),
            elapsed_ms,
            sessions: vec![SessionHit {
                session_id,
                project,
                branch: if branch.is_empty() {
                    None
                } else {
                    Some(branch)
                },
                modified_at: latest_timestamp,
                match_count: matches.len(),
                best_score: 1.0,
                top_match,
                matches,
                engines: vec!["tantivy".to_string()],
            }],
        })
    }

    /// Build a snippet generator from the text portion of the query.
    ///
    /// IMPORTANT: Do NOT use FuzzyTermQuery for snippets — Tantivy's
    /// SnippetGenerator calls query_terms() which is a no-op for FuzzyTermQuery
    /// (inherits empty default from Query trait). Only exact terms highlight.
    fn build_snippet_generator(
        &self,
        searcher: &tantivy::Searcher,
        text_query: &str,
    ) -> Option<SnippetGenerator> {
        if text_query.trim().is_empty() {
            return None;
        }

        let tokens = tokenize_text_terms(text_query);
        if tokens.is_empty() {
            return None;
        }

        let snippet_query: Box<dyn Query> = if tokens.len() >= 2 {
            // Combine PhraseQuery + individual TermQueries so snippets
            // highlight both exact phrases and scattered term matches.
            let mut snippet_signals: Vec<(Occur, Box<dyn Query>)> = Vec::new();
            let phrase_terms: Vec<Term> = tokens
                .iter()
                .map(|t| Term::from_field_text(self.content_field, t))
                .collect();
            snippet_signals.push((Occur::Should, Box::new(PhraseQuery::new(phrase_terms))));
            for t in &tokens {
                let term = Term::from_field_text(self.content_field, t);
                snippet_signals.push((
                    Occur::Should,
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
                ));
            }
            Box::new(BooleanQuery::new(snippet_signals))
        } else {
            // Single term
            let term = Term::from_field_text(self.content_field, &tokens[0]);
            Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs))
        };

        SnippetGenerator::create(searcher, &*snippet_query, self.content_field).ok()
    }

    /// Sort session groups by score (bucketed) then recency, and apply pagination.
    fn sort_and_paginate(
        &self,
        session_groups: HashMap<String, Vec<(f32, DocAddress)>>,
        searcher: &tantivy::Searcher,
        offset: usize,
        limit: usize,
    ) -> Vec<(String, Vec<(f32, DocAddress)>)> {
        let mut session_entries: Vec<(String, Vec<(f32, DocAddress)>)> =
            session_groups.into_iter().collect();

        // Pre-compute max timestamps per session to avoid repeated doc reads
        // inside the sort comparator (O(N*M) reads vs O(N*logN*M) in-sort reads).
        let session_max_ts: HashMap<String, i64> = session_entries
            .iter()
            .map(|(sid, docs)| {
                let max_ts = docs
                    .iter()
                    .filter_map(|(_, addr)| {
                        let d: TantivyDocument = searcher.doc(*addr).ok()?;
                        d.get_first(self.timestamp_field)?.as_i64()
                    })
                    .max()
                    .unwrap_or(0);
                (sid.clone(), max_ts)
            })
            .collect();

        // Sort by composite key: quantized score bucket (descending) then
        // timestamp (descending). Quantizing scores into buckets of 0.1 gives
        // a stable recency tiebreak for near-equal scores without violating
        // transitivity (which the previous "scores close" heuristic did,
        // causing panics in Rust 1.81+ sort validation).
        session_entries.sort_by(|a, b| {
            let best_a =
                a.1.iter()
                    .map(|(s, _)| *s)
                    .fold(f32::NEG_INFINITY, f32::max);
            let best_b =
                b.1.iter()
                    .map(|(s, _)| *s)
                    .fold(f32::NEG_INFINITY, f32::max);

            // Quantize into buckets of 0.1 so near-equal scores share a bucket
            // and get tiebroken by recency. This preserves transitivity.
            let bucket_a = (best_a * 10.0).round() as i64;
            let bucket_b = (best_b * 10.0).round() as i64;

            match bucket_b.cmp(&bucket_a) {
                std::cmp::Ordering::Equal => {
                    // Same score bucket — newer session first
                    let ts_a = session_max_ts.get(&a.0).copied().unwrap_or(0);
                    let ts_b = session_max_ts.get(&b.0).copied().unwrap_or(0);
                    ts_b.cmp(&ts_a)
                }
                ord => ord,
            }
        });

        // Apply offset and limit at the session level
        session_entries
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect()
    }

    /// Build `SessionHit` structs from sorted, paginated session groups.
    fn build_session_hits(
        &self,
        paginated: Vec<(String, Vec<(f32, DocAddress)>)>,
        searcher: &tantivy::Searcher,
        snippet_gen: &Option<SnippetGenerator>,
    ) -> Result<Vec<SessionHit>, SearchError> {
        let mut sessions = Vec::with_capacity(paginated.len());

        for (session_id, mut scored_docs) in paginated {
            // Sort docs within session by score descending
            scored_docs.sort_by(|a, b| b.0.total_cmp(&a.0));

            let mut matches = Vec::with_capacity(scored_docs.len());
            let mut best_score = f32::NEG_INFINITY;
            let mut project = String::new();
            let mut branch = String::new();
            let mut latest_timestamp: i64 = 0;

            for (score, doc_addr) in &scored_docs {
                let retrieved: TantivyDocument = searcher.doc(*doc_addr)?;

                let role = retrieved
                    .get_first(self.role_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let turn_number = retrieved
                    .get_first(self.turn_number_field)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                let timestamp = retrieved
                    .get_first(self.timestamp_field)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                // Generate snippet
                let snippet = match snippet_gen {
                    Some(gen) => {
                        let snip = gen.snippet_from_doc(&retrieved);
                        // Tantivy's to_html() wraps matches in <b> tags;
                        // frontend expects <mark> tags for highlight styling
                        let html = snip
                            .to_html()
                            .replace("<b>", "<mark>")
                            .replace("</b>", "</mark>");
                        if html.is_empty() {
                            // If no highlight, fall back to first 200 chars of content
                            retrieved
                                .get_first(self.content_field)
                                .and_then(|v| v.as_str())
                                .map(|s| truncate_utf8(s, 200))
                                .unwrap_or_default()
                        } else {
                            html
                        }
                    }
                    None => {
                        // No text query — just return truncated content
                        retrieved
                            .get_first(self.content_field)
                            .and_then(|v| v.as_str())
                            .map(|s| truncate_utf8(s, 200))
                            .unwrap_or_default()
                    }
                };

                if *score > best_score {
                    best_score = *score;
                }

                if timestamp > latest_timestamp {
                    latest_timestamp = timestamp;
                }

                // Capture project and branch from first doc
                if project.is_empty() {
                    project = retrieved
                        .get_first(self.project_field)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    branch = retrieved
                        .get_first(self.branch_field)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                }

                matches.push(MatchHit {
                    role,
                    turn_number,
                    snippet,
                    timestamp,
                });
            }

            let top_match = matches.first().cloned().unwrap_or(MatchHit {
                role: String::new(),
                turn_number: 0,
                snippet: String::new(),
                timestamp: 0,
            });

            sessions.push(SessionHit {
                session_id,
                project,
                branch: if branch.is_empty() {
                    None
                } else {
                    Some(branch)
                },
                modified_at: latest_timestamp,
                match_count: matches.len(),
                best_score,
                top_match,
                matches,
                engines: vec!["tantivy".to_string()],
            });
        }

        Ok(sessions)
    }
}
