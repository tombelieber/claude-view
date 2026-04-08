/// Full-text search query pipeline: parsing, building, and execution.
///
/// - `parsing`: Tokenizes raw query strings, extracts qualifiers, detects session IDs.
/// - `builder`: Translates parsed queries into Tantivy `BooleanQuery` objects.
/// - `executor`: Runs queries, groups results by session, sorts, generates snippets.
///
/// Public API: `SearchIndex::search()` (defined in `executor`).
mod builder;
mod executor;
mod parsing;

#[cfg(test)]
mod tests;
