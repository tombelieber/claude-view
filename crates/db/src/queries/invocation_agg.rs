//! Shared helpers for aggregating `session_stats.invocation_counts` JSON
//! blobs across a set of sessions — the CQRS replacement for the
//! `invocations` table.
//!
//! Readers project the JSON map into a per-key total, then filter by
//! `ToolKind` (deduced from the key prefix) and return top-N rankings.
//!
//! Key format (see `claude_view_core::session_stats::invocation_key`):
//! - built-in/MCP tools → raw tool name (e.g. `"Bash"`, `"mcp__plugin_X__tool_Y"`)
//! - skills → `"Skill:<input.skill>"`
//! - agents → `"Task:<input.subagent_type>"` / `"Agent:<input.subagent_type>"`

use std::collections::HashMap;

use sqlx::{Pool, Sqlite};

use crate::DbResult;

/// Classification of an `invocation_counts` key — derived from the prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Plain built-in tool call (e.g. `Bash`, `Read`, `Write`).
    Tool,
    /// `Skill` tool call with an `input.skill` sub-name.
    Skill,
    /// `Task` / `Agent` tool call with an `input.subagent_type` sub-name.
    Agent,
    /// MCP tool call (raw name begins with `mcp__`).
    McpTool,
}

/// Classify an `invocation_counts` key against the kind encoding.
pub fn classify_key(key: &str) -> ToolKind {
    if let Some(sub) = key.strip_prefix("Skill:") {
        // Empty sub → fall back to tool kind (the key is "Skill" alone
        // only when the parser couldn't extract `input.skill`).
        if !sub.is_empty() {
            return ToolKind::Skill;
        }
    }
    if key.starts_with("Task:") || key.starts_with("Agent:") {
        return ToolKind::Agent;
    }
    if key.starts_with("mcp__") {
        return ToolKind::McpTool;
    }
    ToolKind::Tool
}

/// Extract the display name (right-hand side for `:sub`-prefixed keys).
pub fn display_name(key: &str) -> &str {
    key.split_once(':').map(|(_, rhs)| rhs).unwrap_or(key)
}

/// Fetch `(session_id, invocation_counts_json)` rows for every session in
/// `valid_sessions` whose `first_message_at` falls in `[start_ts, end_ts]`.
async fn fetch_range_jsonl(
    pool: &Pool<Sqlite>,
    start_ts: i64,
    end_ts: i64,
) -> DbResult<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT s.id, ss.invocation_counts
           FROM valid_sessions s
           JOIN session_stats ss ON ss.session_id = s.id
           WHERE s.first_message_at >= ?1 AND s.first_message_at <= ?2"#,
    )
    .bind(start_ts)
    .bind(end_ts)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch every session's `invocation_counts` blob unfiltered.
async fn fetch_all_jsonl(pool: &Pool<Sqlite>) -> DbResult<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT s.id, ss.invocation_counts
           FROM valid_sessions s
           JOIN session_stats ss ON ss.session_id = s.id"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Aggregate per-key totals across the provided `(session, json)` rows.
/// Parse errors and empty blobs are silently skipped.
fn fold_totals(rows: Vec<(String, String)>) -> HashMap<String, i64> {
    let mut totals: HashMap<String, i64> = HashMap::new();
    for (_session_id, json) in rows {
        let per_session: HashMap<String, u64> = serde_json::from_str(&json).unwrap_or_default();
        for (key, count) in per_session {
            *totals.entry(key).or_default() += count as i64;
        }
    }
    totals
}

/// Load aggregated per-key totals across all sessions — no range filter.
pub async fn load_invocation_totals(pool: &Pool<Sqlite>) -> DbResult<HashMap<String, i64>> {
    let rows = fetch_all_jsonl(pool).await?;
    Ok(fold_totals(rows))
}

/// Load aggregated per-key totals for sessions in `[start_ts, end_ts]`.
pub async fn load_invocation_totals_in_range(
    pool: &Pool<Sqlite>,
    start_ts: i64,
    end_ts: i64,
) -> DbResult<HashMap<String, i64>> {
    let rows = fetch_range_jsonl(pool, start_ts, end_ts).await?;
    Ok(fold_totals(rows))
}

/// Top `limit` display names for a given `ToolKind`, ordered by total
/// count descending with lexicographic tie-break.
pub fn top_n_by_prefix(totals: &HashMap<String, i64>, kind: ToolKind, limit: i64) -> Vec<String> {
    let mut filtered: Vec<(&str, i64)> = totals
        .iter()
        .filter(|(k, _)| classify_key(k) == kind)
        .map(|(k, v)| (display_name(k), *v))
        .collect();
    filtered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    filtered
        .into_iter()
        .take(limit.max(0) as usize)
        .map(|(name, _)| name.to_string())
        .collect()
}

/// Aggregate summary across the entire `session_stats.invocation_counts`
/// corpus — used by `get_stats_overview`.
pub struct InvocationAggregate {
    pub total_invocations: i64,
    pub unique_invocables: i64,
}

/// Compute `(total, unique)` across all sessions.
pub async fn aggregate_all(pool: &Pool<Sqlite>) -> DbResult<InvocationAggregate> {
    let totals = load_invocation_totals(pool).await?;
    let total_invocations: i64 = totals.values().sum();
    let unique_invocables = totals.len() as i64;
    Ok(InvocationAggregate {
        total_invocations,
        unique_invocables,
    })
}
