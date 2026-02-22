use std::collections::HashMap;
use std::time::Instant;

use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::snippet::SnippetGenerator;
use tantivy::schema::Value;
use tantivy::{DocAddress, TantivyDocument, Term};
use tracing::debug;

use crate::types::{MatchHit, SearchResponse, SessionHit};
use crate::{SearchError, SearchIndex};

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

/// A parsed qualifier extracted from the query string.
#[derive(Debug, Clone)]
struct Qualifier {
    key: String,
    value: String,
}

/// Check if the query string is a bare UUID (session ID).
/// Pattern: 8-4-4-4-12 hex digits, case-insensitive, whitespace-trimmed.
fn is_session_id(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.len() != 36 {
        return false;
    }
    let bytes = trimmed.as_bytes();
    // Check hyphens at positions 8, 13, 18, 23
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return false;
    }
    // Check all other chars are hex digits
    trimmed.chars().enumerate().all(|(i, c)| {
        i == 8 || i == 13 || i == 18 || i == 23 || c.is_ascii_hexdigit()
    })
}

/// Parse a raw query string into text query + qualifiers.
///
/// Qualifiers are `key:value` pairs. Supported keys:
/// `project`, `branch`, `model`, `role`, `skill`.
///
/// Everything that is not a qualifier becomes the text query.
fn parse_query_string(raw: &str) -> (String, Vec<Qualifier>) {
    let mut qualifiers = Vec::new();
    let mut text_parts = Vec::new();

    let known_keys = ["project", "branch", "model", "role", "skill"];

    // Tokenize respecting quoted strings
    let tokens = tokenize_query(raw);

    for token in tokens {
        if let Some(colon_pos) = token.find(':') {
            let key = &token[..colon_pos];
            let value = &token[colon_pos + 1..];
            if known_keys.contains(&key) && !value.is_empty() {
                qualifiers.push(Qualifier {
                    key: key.to_string(),
                    value: value.to_string(),
                });
                continue;
            }
        }
        text_parts.push(token);
    }

    let text_query = text_parts.join(" ");
    (text_query, qualifiers)
}

/// Tokenize a query string, preserving quoted phrases as single tokens.
fn tokenize_query(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = raw.chars().peekable();
    let mut current = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '"' => {
                // Start of a quoted phrase — consume until closing quote
                chars.next(); // consume opening quote
                let mut phrase = String::from("\"");
                loop {
                    match chars.next() {
                        Some('"') => {
                            phrase.push('"');
                            break;
                        }
                        Some(c) => phrase.push(c),
                        None => {
                            // Unterminated quote — treat as regular text
                            phrase.push('"');
                            break;
                        }
                    }
                }
                // Flush any accumulated text before the quote
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(phrase);
            }
            ' ' | '\t' => {
                chars.next();
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => {
                chars.next();
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
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
    ) -> Result<SearchResponse, SearchError> {
        let start = Instant::now();

        // Fast path: if the query is a bare UUID, do an exact session_id lookup
        let trimmed_query = query_str.trim();
        if is_session_id(trimmed_query) {
            let session_id = trimmed_query.to_lowercase();
            let term = Term::from_field_text(self.session_id_field, &session_id);
            let term_query = TermQuery::new(term, IndexRecordOption::Basic);
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

            return Ok(SearchResponse {
                query: query_str.to_string(),
                total_sessions: 1,
                total_matches: matches.len(),
                elapsed_ms,
                sessions: vec![SessionHit {
                    session_id,
                    project,
                    branch: if branch.is_empty() { None } else { Some(branch) },
                    modified_at: latest_timestamp,
                    match_count: matches.len(),
                    best_score: 1.0,
                    top_match,
                    matches,
                }],
            });
        }

        let (text_query, mut qualifiers) = parse_query_string(query_str);

        // Add scope qualifiers (may contain multiple: "project:X branch:Y")
        if let Some(scope_str) = scope {
            let (_, scope_qualifiers) = parse_query_string(scope_str);
            qualifiers.extend(scope_qualifiers);
        }

        // Build the combined query
        let mut sub_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        // Text query (the main BM25-scored part)
        if !text_query.trim().is_empty() {
            // Check if the query is a quoted phrase
            let trimmed = text_query.trim();
            if trimmed.starts_with('"') && trimmed.ends_with('"') {
                // Quoted phrase: use standard query parser (exact phrase match)
                let query_parser =
                    tantivy::query::QueryParser::for_index(&self.index, vec![self.content_field]);
                let parsed = query_parser.parse_query(trimmed)?;
                sub_queries.push((Occur::Must, parsed));
            } else {
                // Unquoted: apply fuzzy matching per term (Levenshtein distance=1)
                let tokens: Vec<&str> = trimmed.split_whitespace()
                    .filter(|t| !t.is_empty())
                    .collect();

                if tokens.len() == 1 {
                    // Single term: fuzzy match
                    let term = Term::from_field_text(self.content_field, &tokens[0].to_lowercase());
                    let fuzzy_query = FuzzyTermQuery::new(term, 1, true);
                    sub_queries.push((Occur::Must, Box::new(fuzzy_query)));
                } else {
                    // Multiple terms: each must match (fuzzy), combined with Must
                    let mut term_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
                    for token in &tokens {
                        let term = Term::from_field_text(self.content_field, &token.to_lowercase());
                        let fuzzy_query = FuzzyTermQuery::new(term, 1, true);
                        term_queries.push((Occur::Must, Box::new(fuzzy_query)));
                    }
                    sub_queries.push((Occur::Must, Box::new(BooleanQuery::new(term_queries))));
                }
            }
        }

        // Qualifier term queries
        for qual in &qualifiers {
            let (field, is_text) = match qual.key.as_str() {
                "project" => (self.project_field, false),
                "branch" => (self.branch_field, false),
                "model" => (self.model_field, true),  // TEXT field: tokenized, needs lowercase
                "role" => (self.role_field, false),
                "skill" => (self.skills_field, false),
                _ => continue,
            };

            if is_text {
                // TEXT fields are tokenized — the value may contain multiple tokens
                // (e.g. "claude-opus-4-6" → ["claude", "opus", "4", "6"]).
                // We create a TermQuery for each token, all joined with Must,
                // so "opus" matches and "claude-opus-4-6" also matches.
                let lowered = qual.value.to_lowercase();
                let mut token_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
                // Split on non-alphanumeric to mirror Tantivy's default tokenizer
                for token in lowered.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()) {
                    let term = Term::from_field_text(field, token);
                    let term_query = TermQuery::new(term, IndexRecordOption::Basic);
                    token_queries.push((Occur::Must, Box::new(term_query)));
                }
                if token_queries.len() == 1 {
                    sub_queries.push(token_queries.pop().unwrap());
                } else if !token_queries.is_empty() {
                    sub_queries.push((Occur::Must, Box::new(BooleanQuery::new(token_queries))));
                }
            } else {
                // STRING fields store exact values — single TermQuery
                let term = Term::from_field_text(field, &qual.value);
                let term_query = TermQuery::new(term, IndexRecordOption::Basic);
                sub_queries.push((Occur::Must, Box::new(term_query)));
            }
        }

        // If no query components at all, return empty
        if sub_queries.is_empty() {
            return Ok(SearchResponse {
                query: query_str.to_string(),
                total_sessions: 0,
                total_matches: 0,
                elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                sessions: vec![],
            });
        }

        let combined_query = BooleanQuery::new(sub_queries);

        let searcher = self.reader.searcher();

        // Two-phase search: Count gets true total, TopDocs gets scored results.
        // Local tool — total doc count is small, no artificial caps needed.
        let total_matches_all = searcher.search(&combined_query, &Count)?;

        // Fetch all matching docs for accurate session grouping and ranking.
        let top_docs = searcher.search(&combined_query, &TopDocs::with_limit(total_matches_all.max(1)))?;

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

        // Build a snippet generator for the text query
        // (only if we have a text query to highlight)
        let snippet_gen = if !text_query.trim().is_empty() {
            let query_parser =
                tantivy::query::QueryParser::for_index(&self.index, vec![self.content_field]);
            let parsed = query_parser.parse_query(&text_query)?;
            SnippetGenerator::create(&searcher, &*parsed, self.content_field).ok()
        } else {
            None
        };

        // Sort sessions by best score descending
        let mut session_entries: Vec<(String, Vec<(f32, DocAddress)>)> =
            session_groups.into_iter().collect();
        session_entries.sort_by(|a, b| {
            let best_a = a.1.iter().map(|(s, _)| *s).fold(f32::NEG_INFINITY, f32::max);
            let best_b = b.1.iter().map(|(s, _)| *s).fold(f32::NEG_INFINITY, f32::max);
            best_b
                .partial_cmp(&best_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply offset and limit at the session level
        let paginated: Vec<_> = session_entries
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        // Build SessionHit for each group
        let mut sessions = Vec::with_capacity(paginated.len());

        for (session_id, mut scored_docs) in paginated {
            // Sort docs within session by score descending
            scored_docs.sort_by(|a, b| {
                b.0.partial_cmp(&a.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

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
                let snippet = match &snippet_gen {
                    Some(gen) => {
                        let snip = gen.snippet_from_doc(&retrieved);
                        // Tantivy's to_html() wraps matches in <b> tags;
                        // frontend expects <mark> tags for highlight styling
                        let html = snip.to_html()
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
            });
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_plain_text() {
        let (text, quals) = parse_query_string("JWT authentication");
        assert_eq!(text, "JWT authentication");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_parse_query_with_qualifiers() {
        let (text, quals) = parse_query_string("project:claude-view auth token");
        assert_eq!(text, "auth token");
        assert_eq!(quals.len(), 1);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "claude-view");
    }

    #[test]
    fn test_parse_query_multiple_qualifiers() {
        let (text, quals) = parse_query_string("project:myapp role:user fix bug");
        assert_eq!(text, "fix bug");
        assert_eq!(quals.len(), 2);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "myapp");
        assert_eq!(quals[1].key, "role");
        assert_eq!(quals[1].value, "user");
    }

    #[test]
    fn test_parse_query_quoted_phrase() {
        let (text, quals) = parse_query_string("\"JWT authentication\" project:myapp");
        assert_eq!(text, "\"JWT authentication\"");
        assert_eq!(quals.len(), 1);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "myapp");
    }

    #[test]
    fn test_parse_query_unknown_qualifier_treated_as_text() {
        let (text, quals) = parse_query_string("unknown:value search text");
        assert_eq!(text, "unknown:value search text");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_parse_query_qualifier_empty_value() {
        // `project:` with no value should be treated as text, not a qualifier
        let (text, quals) = parse_query_string("project: search text");
        assert_eq!(text, "project: search text");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_tokenize_preserves_quoted_strings() {
        let tokens = tokenize_query("hello \"world foo\" bar");
        assert_eq!(tokens, vec!["hello", "\"world foo\"", "bar"]);
    }

    #[test]
    fn test_tokenize_unterminated_quote() {
        let tokens = tokenize_query("hello \"world foo");
        assert_eq!(tokens, vec!["hello", "\"world foo\""]);
    }

    #[test]
    fn test_is_session_id_valid_uuid() {
        assert!(is_session_id("136ed96f-913d-4a1a-91a9-5e651469b2a0"));
    }

    #[test]
    fn test_is_session_id_uppercase() {
        assert!(is_session_id("136ED96F-913D-4A1A-91A9-5E651469B2A0"));
    }

    #[test]
    fn test_is_session_id_with_whitespace() {
        assert!(is_session_id("  136ed96f-913d-4a1a-91a9-5e651469b2a0  "));
    }

    #[test]
    fn test_is_session_id_plain_text() {
        assert!(!is_session_id("JWT authentication"));
    }

    #[test]
    fn test_is_session_id_with_qualifier() {
        assert!(!is_session_id("project:claude-view auth"));
    }

    #[test]
    fn test_is_session_id_partial_uuid() {
        assert!(!is_session_id("136ed96f-913d"));
    }

    #[test]
    fn test_is_session_id_empty() {
        assert!(!is_session_id(""));
    }

    #[test]
    fn test_parse_query_session_id_detected() {
        // Verify that is_session_id is true for a UUID query
        assert!(is_session_id("136ed96f-913d-4a1a-91a9-5e651469b2a0"));
        // And that normal text is not
        let (text, quals) = parse_query_string("136ed96f-913d-4a1a-91a9-5e651469b2a0");
        assert_eq!(text, "136ed96f-913d-4a1a-91a9-5e651469b2a0");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_parse_scope_multiple_qualifiers() {
        // This is the exact format the frontend sends as the scope parameter
        let (text, quals) = parse_query_string("project:claude-view branch:main");
        assert!(text.is_empty());
        assert_eq!(quals.len(), 2);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "claude-view");
        assert_eq!(quals[1].key, "branch");
        assert_eq!(quals[1].value, "main");
    }

    #[test]
    fn test_parse_scope_single_qualifier() {
        let (text, quals) = parse_query_string("project:claude-view");
        assert!(text.is_empty());
        assert_eq!(quals.len(), 1);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "claude-view");
    }

    #[test]
    fn test_parse_scope_with_all_qualifier_types() {
        let (text, quals) =
            parse_query_string("project:myapp branch:dev model:claude-opus-4-6 role:assistant");
        assert!(text.is_empty());
        assert_eq!(quals.len(), 4);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[1].key, "branch");
        assert_eq!(quals[2].key, "model");
        assert_eq!(quals[3].key, "role");
    }
}
