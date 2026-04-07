//! Search and query execution for the prompt index.

use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, Occur, QueryParser, RangeQuery, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Value};
use tantivy::snippet::SnippetGenerator;
use tantivy::Term;

use crate::SearchError;

use super::types::{PromptHit, PromptSearchIndex, PromptSearchParams, PromptSearchResponse};

impl PromptSearchIndex {
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
