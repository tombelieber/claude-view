//! JSONL-first session routes (v2).
//!
//! These routes serve session data from the in-memory `SessionCatalog`
//! and on-demand JSONL parsing via `session_stats`, replacing the
//! SQLite mirror entirely.
//!
//! Route map:
//!   GET /sessions            — list sessions (catalog + on-demand stats)
//!   GET /sessions/:id        — session detail (full JSONL parse)
//!   GET /sessions/:id/turns  — turn list (JSONL on-demand)
//!   GET /projects            — project summaries (from catalog)
//!   GET /projects/:id/sessions — paginated sessions for a project

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use claude_view_core::jsonl_reader::MinLine;
use claude_view_core::pricing::{calculate_cost, TokenUsage};
use claude_view_core::session_catalog::{CatalogRow, Filter, Sort};
use claude_view_core::session_stats::{self, SessionStats};
use claude_view_core::{ProjectSummary, SessionInfo, SessionsPage, ToolCounts};

use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session_detail))
        .route("/sessions/{id}/turns", get(get_session_turns))
        .route("/projects", get(list_projects))
        .route("/projects/{id}/sessions", get(list_project_sessions))
}

// ---- Query params ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListParams {
    project: Option<String>,
    #[serde(default = "default_list_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    sort: Option<String>,
    /// Unix timestamp — only include sessions modified after this time.
    #[serde(default)]
    time_after: Option<i64>,
    /// Unix timestamp — only include sessions modified before this time.
    #[serde(default)]
    time_before: Option<i64>,
}

fn default_list_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSessionsParams {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    #[serde(default = "default_sort")]
    sort: String,
    /// Accepted but not yet implemented in v2 — branch filtering requires
    /// JSONL parsing per session which is deferred to a later phase.
    #[serde(default)]
    #[allow(dead_code)]
    branch: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    include_sidechains: bool,
}

fn default_limit() -> i64 {
    50
}
fn default_sort() -> String {
    "recent".to_string()
}

// ---- Response types ----

#[derive(Debug, Clone, serde::Serialize)]
struct TurnItem {
    seq: u32,
    model: Option<String>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

// ---- Conversion helpers ----

/// Build a `SessionInfo` from catalog metadata + JSONL-extracted stats.
fn build_session_info(
    row: &CatalogRow,
    stats: &SessionStats,
    pricing: &HashMap<String, claude_view_core::pricing::ModelPricing>,
) -> SessionInfo {
    // Compute cost from per-model token breakdowns
    let total_cost_usd = compute_total_cost(&stats.per_model_tokens, pricing);

    // Parse first_message_at ISO string to epoch
    let first_message_at = stats
        .first_message_at
        .as_deref()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp());

    SessionInfo {
        id: row.id.clone(),
        project: row.project_id.clone(),
        project_path: project_path_from_id(&row.project_id),
        display_name: display_name_from_project(&row.project_id),
        file_path: row.file_path.to_string_lossy().to_string(),
        modified_at: row.mtime,
        size_bytes: row.bytes,
        // Stats-derived fields
        total_input_tokens: Some(stats.total_input_tokens),
        total_output_tokens: Some(stats.total_output_tokens),
        total_cache_read_tokens: Some(stats.cache_read_tokens),
        total_cache_creation_tokens: Some(stats.cache_creation_tokens),
        turn_count: stats.turn_count as usize,
        turn_count_api: Some(stats.turn_count as u64),
        message_count: (stats.turn_count + stats.user_prompt_count) as usize,
        primary_model: stats.primary_model.clone(),
        tool_call_count: stats.tool_call_count,
        thinking_block_count: stats.thinking_block_count,
        files_read_count: stats.files_read_count,
        files_edited_count: stats.files_edited_count,
        duration_seconds: stats.duration_seconds,
        first_message_at,
        preview: stats.preview.clone(),
        last_message: stats.last_message.clone(),
        user_prompt_count: stats.user_prompt_count,
        api_call_count: stats.turn_count,
        agent_spawn_count: stats.agent_spawn_count,
        api_error_count: stats.api_error_count,
        total_cost_usd: if total_cost_usd > 0.0 {
            Some(total_cost_usd)
        } else {
            None
        },
        tool_counts: ToolCounts {
            read: stats.files_read_count as usize,
            edit: stats.files_edited_count as usize,
            bash: stats.bash_count as usize,
            write: 0,
        },
        ..Default::default()
    }
}

fn compute_total_cost(
    per_model: &HashMap<String, TokenUsage>,
    pricing: &HashMap<String, claude_view_core::pricing::ModelPricing>,
) -> f64 {
    per_model
        .iter()
        .map(|(model, tokens)| {
            let breakdown = calculate_cost(tokens, Some(model.as_str()), pricing);
            breakdown.total_usd
        })
        .sum()
}

/// Return project_id as the path. Encoding is lossy (`/`, `@`, `.` all become `-`),
/// so decoding is ambiguous. CWD evidence from JSONL would be needed for accuracy.
/// For now, return the encoded name verbatim — the frontend handles it.
fn project_path_from_id(project_id: &str) -> String {
    project_id.to_string()
}

/// Extract a display name from the project_id. Uses the last segment heuristic,
/// which works for most projects (e.g. "-Users-dev-my-project" → "my-project").
/// Falls back to the full encoded name.
fn display_name_from_project(project_id: &str) -> String {
    // The project dir is the last path component. In the encoded form,
    // the last `-` that separates a path component is indistinguishable
    // from a `-` in a dirname. Best heuristic: use the raw encoded name.
    project_id.to_string()
}

// ---- Handlers ----

async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Json<SessionsPage> {
    let filter = Filter {
        project_id: params.project,
        min_last_ts: params.time_after,
        max_last_ts: params.time_before,
    };
    let sort = match params.sort.as_deref() {
        Some("oldest") => Sort::LastTsAsc,
        _ => Sort::LastTsDesc,
    };

    // Get all matching rows for total count, then paginate
    let all_rows = state.session_catalog.list(&filter, sort, usize::MAX);
    let total = all_rows.len();
    let page_rows: Vec<&CatalogRow> = all_rows
        .iter()
        .skip(params.offset)
        .take(params.limit)
        .collect();

    let pricing = &state.pricing;
    let sessions: Vec<SessionInfo> = page_rows
        .iter()
        .filter_map(|row| {
            let stats = session_stats::extract_stats(&row.file_path, row.is_compressed).ok()?;
            Some(build_session_info(row, &stats, pricing))
        })
        .collect();

    Json(SessionsPage { sessions, total })
}

async fn get_session_detail(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionInfo>, StatusCode> {
    let row = state
        .session_catalog
        .get(&session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let stats = session_stats::extract_stats(&row.file_path, row.is_compressed)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let pricing = &state.pricing;
    Ok(Json(build_session_info(&row, &stats, pricing)))
}

async fn get_session_turns(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<Vec<TurnItem>>, StatusCode> {
    let row = state
        .session_catalog
        .get(&session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let lines: Vec<MinLine> =
        claude_view_core::jsonl_reader::read_all(&row.file_path, row.is_compressed)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut turns = Vec::new();
    let mut seq: u32 = 0;
    let mut last_by_id: HashMap<String, usize> = HashMap::new();

    // Collect last occurrence of each message ID
    for (i, line) in lines.iter().enumerate() {
        if line.line_type.as_deref() != Some("assistant") {
            continue;
        }
        let Some(ref msg) = line.message else {
            continue;
        };
        if let Some(ref mid) = msg.id {
            last_by_id.insert(mid.clone(), i);
        }
    }

    let last_indices: HashSet<usize> = last_by_id.values().copied().collect();
    let mut seen = HashSet::new();

    for (i, line) in lines.iter().enumerate() {
        if line.line_type.as_deref() != Some("assistant") {
            continue;
        }
        let Some(ref msg) = line.message else {
            continue;
        };

        // Only process the last occurrence of each message ID
        if let Some(ref mid) = msg.id {
            if !last_indices.contains(&i) {
                continue;
            }
            if !seen.insert(mid.clone()) {
                continue;
            }
        }

        turns.push(TurnItem {
            seq,
            model: msg.model.clone(),
            input_tokens: msg.usage.as_ref().and_then(|u| u.input_tokens),
            output_tokens: msg.usage.as_ref().and_then(|u| u.output_tokens),
        });
        seq += 1;
    }

    Ok(Json(turns))
}

async fn list_projects(State(state): State<Arc<AppState>>) -> Json<Vec<ProjectSummary>> {
    let project_counts = state.session_catalog.projects();

    let mut summaries: Vec<ProjectSummary> = project_counts
        .into_iter()
        .map(|(project_id, session_count)| {
            let path = project_path_from_id(&project_id);
            let display_name = display_name_from_project(&project_id);
            // Get the most recent mtime across sessions in this project
            let last_activity = state
                .session_catalog
                .list(&Filter::by_project(&project_id), Sort::LastTsDesc, 1)
                .first()
                .map(|r| r.mtime);

            ProjectSummary {
                name: project_id,
                display_name,
                path,
                session_count,
                active_count: 0,
                last_activity_at: last_activity,
                is_archived: false,
            }
        })
        .collect();

    summaries.sort_unstable_by(|a, b| {
        b.last_activity_at
            .unwrap_or(0)
            .cmp(&a.last_activity_at.unwrap_or(0))
    });
    Json(summaries)
}

async fn list_project_sessions(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    Query(params): Query<ProjectSessionsParams>,
) -> Json<SessionsPage> {
    let filter = Filter::by_project(&project_id);
    let sort = match params.sort.as_str() {
        "oldest" => Sort::LastTsAsc,
        _ => Sort::LastTsDesc,
    };

    // Get total count (unfiltered for this project)
    let all_rows = state.session_catalog.list(&filter, sort, usize::MAX);
    let total = all_rows.len();

    // Apply pagination
    let offset = params.offset.max(0) as usize;
    let limit = params.limit.max(1) as usize;
    let page_rows: Vec<&CatalogRow> = all_rows.iter().skip(offset).take(limit).collect();

    let pricing = &state.pricing;
    let sessions: Vec<SessionInfo> = page_rows
        .iter()
        .filter_map(|row| {
            let stats = session_stats::extract_stats(&row.file_path, row.is_compressed).ok()?;
            Some(build_session_info(row, &stats, pricing))
        })
        .collect();

    Json(SessionsPage { sessions, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_path_returns_encoded_name() {
        // Encoding is lossy — return encoded name verbatim
        assert_eq!(
            project_path_from_id("-Users-dev-proj-demo-app"),
            "-Users-dev-proj-demo-app"
        );
        assert_eq!(project_path_from_id(""), "");
    }

    #[test]
    fn test_build_session_info_from_stats() {
        let row = CatalogRow {
            id: "test-session".to_string(),
            file_path: "/tmp/test.jsonl".into(),
            is_compressed: false,
            bytes: 1024,
            mtime: 1712000000,
            project_id: "-Users-dev-myproject".to_string(),
            first_ts: None,
            last_ts: None,
        };
        let stats = SessionStats {
            total_input_tokens: 5000,
            total_output_tokens: 2000,
            cache_read_tokens: 1000,
            cache_creation_tokens: 500,
            turn_count: 5,
            user_prompt_count: 3,
            tool_call_count: 7,
            thinking_block_count: 2,
            files_read_count: 3,
            files_edited_count: 2,
            bash_count: 2,
            duration_seconds: 120,
            primary_model: Some("claude-opus-4-6".to_string()),
            preview: "Fix the bug".to_string(),
            last_message: "Done".to_string(),
            first_message_at: Some("2026-04-01T10:00:00Z".to_string()),
            ..Default::default()
        };

        let info = build_session_info(&row, &stats, &HashMap::new());

        assert_eq!(info.id, "test-session");
        assert_eq!(info.project, "-Users-dev-myproject");
        assert_eq!(info.total_input_tokens, Some(5000));
        assert_eq!(info.total_output_tokens, Some(2000));
        assert_eq!(info.turn_count, 5);
        assert_eq!(info.tool_call_count, 7);
        assert_eq!(info.duration_seconds, 120);
        assert_eq!(info.primary_model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(info.preview, "Fix the bug");
        // 2026-04-01T10:00:00Z epoch
        assert_eq!(info.first_message_at, Some(1775037600));
    }
}
