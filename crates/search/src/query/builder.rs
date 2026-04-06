/// Query building: translates parsed query text + qualifiers into Tantivy queries.
use std::ops::Bound;

use chrono::NaiveDate;

use tantivy::query::{
    BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, RangeQuery, TermQuery,
};
use tantivy::schema::IndexRecordOption;
use tantivy::Term;

use crate::SearchIndex;

use super::parsing::{parse_query_string, tokenize_text_terms, Qualifier};

/// Boost weights for multi-signal scoring.
/// Invariant: PHRASE > EXACT > FUZZY (verified by integration tests).
/// These are starting values — tune based on real session data.
const PHRASE_BOOST: f32 = 3.0;
const EXACT_BOOST: f32 = 1.5;
const FUZZY_BOOST: f32 = 0.5;

/// The result of building a Tantivy query from a raw query string.
/// Contains the combined query, the extracted text portion (for snippet
/// generation), and the tokens derived from the text portion.
pub(crate) struct BuiltQuery {
    /// The combined Tantivy query (text signals + qualifier filters).
    pub query: BooleanQuery,
    /// The free-text portion of the raw query (qualifiers removed).
    pub text_query: String,
}

impl SearchIndex {
    /// Parse a raw query string (with optional scope) and build a Tantivy
    /// `BooleanQuery` combining text signals and qualifier filters.
    ///
    /// Returns `None` if the query is empty (no text, no qualifiers).
    pub(crate) fn build_query(&self, query_str: &str, scope: Option<&str>) -> Option<BuiltQuery> {
        let (text_query, mut qualifiers) = parse_query_string(query_str);

        // Add scope qualifiers (may contain multiple: "project:X branch:Y")
        if let Some(scope_str) = scope {
            let (_, scope_qualifiers) = parse_query_string(scope_str);
            qualifiers.extend(scope_qualifiers);
        }

        // Build the combined query
        let mut sub_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        // Text query (the main BM25-scored part) — multi-signal: phrase + exact + fuzzy
        if !text_query.trim().is_empty() {
            if let Some(text_combined) = self.build_text_signals(&text_query) {
                sub_queries.push((Occur::Must, Box::new(text_combined)));
            }
        }

        // Qualifier term queries
        self.build_qualifier_queries(&qualifiers, &mut sub_queries);

        // If no query components at all, return None
        if sub_queries.is_empty() {
            return None;
        }

        Some(BuiltQuery {
            query: BooleanQuery::new(sub_queries),
            text_query,
        })
    }

    /// Build multi-signal text query: phrase + exact + fuzzy, all as Should.
    /// Returns `None` if the text produces no tokens.
    fn build_text_signals(&self, text_query: &str) -> Option<BooleanQuery> {
        let tokens = tokenize_text_terms(text_query);
        if tokens.is_empty() {
            return None;
        }

        let mut text_signals: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        // Signal 1: Exact phrase (highest weight, only for 2+ terms)
        if tokens.len() >= 2 {
            let phrase_terms: Vec<Term> = tokens
                .iter()
                .map(|t| Term::from_field_text(self.content_field, t))
                .collect();
            let phrase_query = PhraseQuery::new(phrase_terms);
            text_signals.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(phrase_query), PHRASE_BOOST)),
            ));
        }

        // Signal 2: All exact terms present (BM25 scored)
        {
            let exact_term_queries: Vec<(Occur, Box<dyn Query>)> = tokens
                .iter()
                .map(|t| {
                    let term = Term::from_field_text(self.content_field, t);
                    (
                        Occur::Must,
                        Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs))
                            as Box<dyn Query>,
                    )
                })
                .collect();
            let exact_query = BooleanQuery::new(exact_term_queries);
            text_signals.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(exact_query), EXACT_BOOST)),
            ));
        }

        // Signal 3: Fuzzy terms (typo tolerance, lowest weight)
        {
            let fuzzy_term_queries: Vec<(Occur, Box<dyn Query>)> = tokens
                .iter()
                .map(|t| {
                    let term = Term::from_field_text(self.content_field, t);
                    (
                        Occur::Must,
                        Box::new(FuzzyTermQuery::new(term, 1, true)) as Box<dyn Query>,
                    )
                })
                .collect();
            let fuzzy_query = BooleanQuery::new(fuzzy_term_queries);
            text_signals.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(fuzzy_query), FUZZY_BOOST)),
            ));
        }

        if text_signals.is_empty() {
            None
        } else {
            Some(BooleanQuery::new(text_signals))
        }
    }

    /// Translate qualifier structs into Tantivy sub-queries and append them
    /// to `sub_queries`.
    fn build_qualifier_queries(
        &self,
        qualifiers: &[Qualifier],
        sub_queries: &mut Vec<(Occur, Box<dyn Query>)>,
    ) {
        for qual in qualifiers {
            let (field, is_text) = match qual.key.as_str() {
                "project" => (self.project_field, false),
                "branch" => (self.branch_field, false),
                "model" => (self.model_field, true), // TEXT field: tokenized, needs lowercase
                "role" => (self.role_field, false),
                "skill" => (self.skills_field, false),
                "session" => {
                    let term = Term::from_field_text(self.session_id_field, &qual.value);
                    sub_queries.push((
                        Occur::Must,
                        Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
                    ));
                    continue;
                }
                "after" => {
                    if let Ok(date) = NaiveDate::parse_from_str(&qual.value, "%Y-%m-%d") {
                        let ts = date
                            .and_hms_opt(0, 0, 0)
                            .map(|dt| dt.and_utc().timestamp())
                            .unwrap_or(0);
                        let range = RangeQuery::new_i64_bounds(
                            "timestamp".to_string(),
                            Bound::Excluded(ts),
                            Bound::Unbounded,
                        );
                        sub_queries.push((Occur::Must, Box::new(range)));
                    } else {
                        tracing::warn!(qualifier = "after", value = %qual.value, "invalid date format, expected YYYY-MM-DD");
                    }
                    continue;
                }
                "before" => {
                    if let Ok(date) = NaiveDate::parse_from_str(&qual.value, "%Y-%m-%d") {
                        let ts = date
                            .and_hms_opt(0, 0, 0)
                            .map(|dt| dt.and_utc().timestamp())
                            .unwrap_or(0);
                        let range = RangeQuery::new_i64_bounds(
                            "timestamp".to_string(),
                            Bound::Unbounded,
                            Bound::Excluded(ts),
                        );
                        sub_queries.push((Occur::Must, Box::new(range)));
                    } else {
                        tracing::warn!(qualifier = "before", value = %qual.value, "invalid date format, expected YYYY-MM-DD");
                    }
                    continue;
                }
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
                for token in lowered
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|t| !t.is_empty())
                {
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
    }
}
