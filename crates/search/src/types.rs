use serde::Serialize;
use ts_rs::TS;

/// Response from a full-text search query across all sessions.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// The original query string.
    pub query: String,
    /// Number of distinct sessions matching the query.
    pub total_sessions: usize,
    /// Total number of individual message-level matches.
    pub total_matches: usize,
    /// Time spent executing the search, in milliseconds.
    pub elapsed_ms: f64,
    /// Session-grouped results, sorted by best BM25 score descending.
    pub sessions: Vec<SessionHit>,
}

/// A session that contains one or more search matches.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionHit {
    pub session_id: String,
    pub project: String,
    pub branch: Option<String>,
    /// Unix timestamp (seconds) of the most recent match in this session.
    pub modified_at: i64,
    /// How many individual messages matched in this session.
    pub match_count: usize,
    /// BM25 score of the best-scoring match in this session.
    pub best_score: f32,
    /// The single best-scoring match (for collapsed view).
    pub top_match: MatchHit,
    /// All matches in this session (for expanded view).
    pub matches: Vec<MatchHit>,
}

/// A single message-level search match with a highlighted snippet.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct MatchHit {
    /// "user", "assistant", or "tool"
    pub role: String,
    /// 1-based turn number within the conversation.
    pub turn_number: u64,
    /// Snippet with `<mark>` tags highlighting matched terms.
    pub snippet: String,
    /// Unix timestamp (seconds) of this message. 0 if unknown.
    pub timestamp: i64,
}
