// crates/server/src/routes/turns.rs
//! Per-turn breakdown endpoint for historical sessions.
//!
//! `GET /api/sessions/{id}/turns` re-parses the JSONL file on demand to extract
//! per-turn data (wall-clock duration, CC duration, prompt preview). This avoids
//! storing per-turn data in the DB for rarely-accessed detail views.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use memchr::memmem;
use serde::Serialize;
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ============================================================================
// Response Types
// ============================================================================

/// A single turn in the session breakdown.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TurnInfo {
    /// 1-based turn index.
    pub index: u32,
    /// Unix timestamp (seconds) when the turn started (user prompt).
    #[ts(type = "number")]
    pub started_at: i64,
    /// Wall-clock seconds from turn start to turn end (last message before next turn or EOF).
    #[ts(type = "number")]
    pub wall_clock_seconds: i64,
    /// Claude Code reported turn duration in milliseconds (from `turn_duration` system message).
    /// Null if no `turn_duration` message follows this turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub cc_duration_ms: Option<u64>,
    /// First 60 characters of the user prompt text.
    pub prompt_preview: String,
}

// ============================================================================
// JSONL Scanning
// ============================================================================

/// Pre-compiled SIMD finders for turn scanning.
struct TurnFinders {
    type_user: memmem::Finder<'static>,
    type_assistant: memmem::Finder<'static>,
    type_system: memmem::Finder<'static>,
    tool_result: memmem::Finder<'static>,
    turn_duration: memmem::Finder<'static>,
    timestamp_key: memmem::Finder<'static>,
    content_key: memmem::Finder<'static>,
}

impl TurnFinders {
    fn new() -> Self {
        Self {
            type_user: memmem::Finder::new(b"\"type\":\"user\""),
            type_assistant: memmem::Finder::new(b"\"type\":\"assistant\""),
            type_system: memmem::Finder::new(b"\"type\":\"system\""),
            tool_result: memmem::Finder::new(b"\"tool_result\""),
            turn_duration: memmem::Finder::new(b"\"turn_duration\""),
            timestamp_key: memmem::Finder::new(b"\"timestamp\""),
            content_key: memmem::Finder::new(b"\"content\""),
        }
    }
}

/// Returns true if the user message content looks like a system/hook message
/// rather than a real user prompt. Mirrors `is_system_user_content` in
/// `indexer_parallel.rs`.
fn is_system_user_content(content: &str) -> bool {
    let trimmed = content.trim();
    trimmed.is_empty()
        || trimmed.starts_with("<local-command-caveat>")
        || trimmed.starts_with("<command-name>")
        || trimmed.starts_with("<command-message>")
        || trimmed.starts_with("<local-command-stdout>")
        || trimmed.starts_with("<system-reminder>")
        || trimmed.starts_with("{\"type\":\"tool_result\"")
}

/// Extract the first text content from a user line's raw bytes.
///
/// For user lines, content is either a JSON string (real prompt) or a JSON array
/// (tool_result continuation). We only care about string content.
fn extract_user_content(parsed: &serde_json::Value) -> Option<String> {
    let msg = parsed.get("message")?;
    let content = msg.get("content")?;

    match content {
        serde_json::Value::String(s) => Some(s.clone()),
        // Array content = tool_result blocks; handled by tool_result finder
        _ => None,
    }
}

/// Extract timestamp as unix seconds from a parsed JSONL line.
///
/// Claude Code uses either:
/// - ISO 8601 string: `"timestamp": "2026-01-28T10:00:00Z"`
/// - Unix epoch number: `"timestamp": 1700000000`
fn extract_timestamp(parsed: &serde_json::Value) -> Option<i64> {
    let ts = parsed.get("timestamp")?;
    match ts {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => {
            // Parse ISO 8601 to unix timestamp
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp())
                .or_else(|| {
                    // Try without timezone (assume UTC)
                    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                        .ok()
                        .map(|ndt| ndt.and_utc().timestamp())
                })
        }
        _ => None,
    }
}

/// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
fn truncate_preview(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

/// Scan JSONL file and extract per-turn breakdown.
///
/// Uses SIMD pre-filtering to avoid JSON parsing lines that aren't interesting.
fn scan_turns(data: &[u8]) -> Vec<TurnInfo> {
    let finders = TurnFinders::new();

    // Intermediate state for building turns
    struct PendingTurn {
        index: u32,
        started_at: i64,
        prompt_preview: String,
        cc_duration_ms: Option<u64>,
    }

    let mut turns: Vec<TurnInfo> = Vec::new();
    let mut current_turn: Option<PendingTurn> = None;
    let mut last_timestamp: Option<i64> = None;
    let mut turn_counter: u32 = 0;

    for raw_line in data.split(|&b| b == b'\n') {
        if raw_line.is_empty() {
            continue;
        }

        let is_user = finders.type_user.find(raw_line).is_some();
        let is_assistant = finders.type_assistant.find(raw_line).is_some();
        let is_system = finders.type_system.find(raw_line).is_some();

        // We only need to parse lines that are user, assistant, or system
        if !is_user && !is_assistant && !is_system {
            continue;
        }

        // Check for turn_duration system message (SIMD pre-filter)
        if is_system && finders.turn_duration.find(raw_line).is_some() {
            if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                if let Some(ms) = parsed.get("durationMs").and_then(|v| v.as_u64()) {
                    // Attach duration to current turn
                    if let Some(ref mut turn) = current_turn {
                        turn.cc_duration_ms = Some(ms);
                    }
                }
                // Update last_timestamp from system message too
                if let Some(ts) = extract_timestamp(&parsed) {
                    last_timestamp = Some(ts);
                }
            }
            continue;
        }

        // For user lines: check if it's a real user turn start
        if is_user {
            // Skip tool_result continuations (SIMD check)
            if finders.tool_result.find(raw_line).is_some() {
                // Still extract timestamp for wall-clock tracking
                if finders.timestamp_key.find(raw_line).is_some() {
                    if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                        if let Some(ts) = extract_timestamp(&parsed) {
                            last_timestamp = Some(ts);
                        }
                    }
                }
                continue;
            }

            // Parse to check content
            if finders.content_key.find(raw_line).is_some() {
                if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                    let ts = extract_timestamp(&parsed);

                    // Check if this is a system-injected user message
                    if let Some(content) = extract_user_content(&parsed) {
                        if is_system_user_content(&content) {
                            // Not a real turn, but update timestamp
                            if let Some(t) = ts {
                                last_timestamp = Some(t);
                            }
                            continue;
                        }

                        // Check isMeta flag
                        if parsed.get("isMeta").and_then(|v| v.as_bool()).unwrap_or(false) {
                            if let Some(t) = ts {
                                last_timestamp = Some(t);
                            }
                            continue;
                        }

                        // This is a real user turn start.
                        // Close the previous turn if one was open.
                        if let Some(pending) = current_turn.take() {
                            let end_ts = last_timestamp.unwrap_or(pending.started_at);
                            turns.push(TurnInfo {
                                index: pending.index,
                                started_at: pending.started_at,
                                wall_clock_seconds: (end_ts - pending.started_at).max(0),
                                cc_duration_ms: pending.cc_duration_ms,
                                prompt_preview: pending.prompt_preview,
                            });
                        }

                        turn_counter += 1;
                        let started_at = ts.unwrap_or(0);
                        let preview = truncate_preview(&content, 60);

                        current_turn = Some(PendingTurn {
                            index: turn_counter,
                            started_at,
                            prompt_preview: preview,
                            cc_duration_ms: None,
                        });

                        if let Some(t) = ts {
                            last_timestamp = Some(t);
                        }
                    } else {
                        // Content is array (tool_result) or missing — not a turn start
                        if let Some(t) = ts {
                            last_timestamp = Some(t);
                        }
                    }
                }
            }
            continue;
        }

        // For assistant/system lines: just track timestamp for wall-clock end
        if finders.timestamp_key.find(raw_line).is_some() {
            if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                if let Some(ts) = extract_timestamp(&parsed) {
                    last_timestamp = Some(ts);
                }
            }
        }
    }

    // Close the last turn
    if let Some(pending) = current_turn.take() {
        let end_ts = last_timestamp.unwrap_or(pending.started_at);
        turns.push(TurnInfo {
            index: pending.index,
            started_at: pending.started_at,
            wall_clock_seconds: (end_ts - pending.started_at).max(0),
            cc_duration_ms: pending.cc_duration_ms,
            prompt_preview: pending.prompt_preview,
        });
    }

    turns
}

// ============================================================================
// Route Handler
// ============================================================================

/// GET /api/sessions/{id}/turns — Per-turn breakdown for a historical session.
pub async fn get_session_turns(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<Vec<TurnInfo>>> {
    // Resolve JSONL file path via DB
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    // Read + parse in blocking thread (file I/O)
    let turns = tokio::task::spawn_blocking(move || {
        let data = std::fs::read(&path).map_err(|e| {
            ApiError::Internal(format!("Failed to read session file: {}", e))
        })?;
        Ok::<Vec<TurnInfo>, ApiError>(scan_turns(&data))
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Task join error: {}", e)))??;

    Ok(Json(turns))
}

// ============================================================================
// Router
// ============================================================================

/// Create the turns routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/sessions/{id}/turns", get(get_session_turns))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_core::{SessionInfo, ToolCounts};
    use vibe_recall_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    fn make_session(id: &str, file_path: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: "test-project".to_string(),
            project_path: "/home/user/test-project".to_string(),
            file_path: file_path.to_string(),
            modified_at: 1700000000,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Last msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
        }
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_turns_not_found() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/nonexistent/turns").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_turns_file_gone() {
        let db = test_db().await;
        let session = make_session("turns-gone", "/nonexistent/path.jsonl");
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-gone/turns").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_turns_empty_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("empty.jsonl");
        std::fs::write(&session_file, "").unwrap();

        let session = make_session("turns-empty", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-empty/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(turns.is_empty(), "Empty session should return empty array");
    }

    #[tokio::test]
    async fn test_get_turns_valid_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("multi_turn.jsonl");

        // Two-turn session with turn_duration system messages
        let jsonl = r#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"Read and fix auth.rs"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":"2026-01-28T10:01:00Z","message":{"role":"assistant","content":[{"type":"text","text":"I'll read the file first."}]}}
{"type":"user","uuid":"u2","parentUuid":"a1","timestamp":"2026-01-28T10:02:00Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"fn authenticate() { todo!() }"}]}}
{"type":"assistant","uuid":"a2","parentUuid":"u2","timestamp":"2026-01-28T10:03:00Z","message":{"role":"assistant","content":[{"type":"text","text":"I've fixed it."}]}}
{"type":"system","uuid":"s1","timestamp":"2026-01-28T10:03:05Z","subtype":"turn_duration","durationMs":180000,"isMeta":true}
{"type":"user","uuid":"u3","timestamp":"2026-01-28T10:05:00Z","message":{"role":"user","content":"Now run the tests please"}}
{"type":"assistant","uuid":"a3","parentUuid":"u3","timestamp":"2026-01-28T10:07:00Z","message":{"role":"assistant","content":[{"type":"text","text":"All tests pass!"}]}}
{"type":"system","uuid":"s2","timestamp":"2026-01-28T10:07:05Z","subtype":"turn_duration","durationMs":120000,"isMeta":true}
"#;
        std::fs::write(&session_file, jsonl).unwrap();

        let session = make_session("turns-ok", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-ok/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();

        assert_eq!(turns.len(), 2, "Should detect 2 turns");

        // Turn 1
        assert_eq!(turns[0]["index"], 1);
        assert!(turns[0]["startedAt"].is_number());
        assert!(turns[0]["wallClockSeconds"].is_number());
        assert_eq!(turns[0]["ccDurationMs"], 180000);
        assert_eq!(turns[0]["promptPreview"], "Read and fix auth.rs");

        // Turn 2
        assert_eq!(turns[1]["index"], 2);
        assert_eq!(turns[1]["ccDurationMs"], 120000);
        assert_eq!(turns[1]["promptPreview"], "Now run the tests please");

        // Wall clock for turn 1: from 10:00:00 to 10:05:00 (next turn) = 300 seconds
        // But actually turn 1 ends at the last message before turn 2 starts.
        // The turn_duration system message at 10:03:05 is the last message before turn 2 at 10:05:00.
        let wall1 = turns[0]["wallClockSeconds"].as_i64().unwrap();
        // Turn 1: started 10:00:00, last timestamp before turn 2 is 10:03:05 = 185 seconds
        assert_eq!(wall1, 185, "Wall clock should be from turn start to last message before next turn");

        // Wall clock for turn 2: from 10:05:00 to 10:07:05 = 125 seconds
        let wall2 = turns[1]["wallClockSeconds"].as_i64().unwrap();
        assert_eq!(wall2, 125, "Wall clock for last turn should extend to EOF");
    }

    #[tokio::test]
    async fn test_get_turns_response_shape() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("shape.jsonl");

        let jsonl = r#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"Hello world"}}
{"type":"assistant","uuid":"a1","timestamp":"2026-01-28T10:00:30Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
"#;
        std::fs::write(&session_file, jsonl).unwrap();

        let session = make_session("turns-shape", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-shape/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();

        assert_eq!(turns.len(), 1);
        let turn = &turns[0];

        // Verify all expected fields are present
        assert!(turn.get("index").is_some(), "Must have index");
        assert!(turn.get("startedAt").is_some(), "Must have startedAt");
        assert!(turn.get("wallClockSeconds").is_some(), "Must have wallClockSeconds");
        // ccDurationMs is optional — should be absent (not null) when no turn_duration message
        assert!(turn.get("ccDurationMs").is_none(), "ccDurationMs should be absent when not available");
        assert!(turn.get("promptPreview").is_some(), "Must have promptPreview");
    }

    #[tokio::test]
    async fn test_get_turns_prompt_preview_truncation() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("truncate.jsonl");

        let long_prompt = "a".repeat(100); // 100 chars, should be truncated to 60 + "..."
        let jsonl = format!(
            r#"{{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{{"role":"user","content":"{}"}}}}"#,
            long_prompt,
        );
        // Add a trailing newline
        let jsonl = format!("{}\n", jsonl);
        std::fs::write(&session_file, jsonl).unwrap();

        let session = make_session("turns-trunc", session_file.to_str().unwrap());
        db.insert_session(&session, "test-project", "Test Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/turns-trunc/turns").await;

        assert_eq!(status, StatusCode::OK);
        let turns: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();

        assert_eq!(turns.len(), 1);
        let preview = turns[0]["promptPreview"].as_str().unwrap();
        // 60 chars + "..." = 63 total
        assert_eq!(preview.len(), 63, "Preview should be 60 chars + '...'");
        assert!(preview.ends_with("..."), "Truncated preview should end with '...'");
    }

    // ========================================================================
    // Unit Tests for scan_turns
    // ========================================================================

    #[test]
    fn test_scan_turns_empty() {
        let turns = scan_turns(b"");
        assert!(turns.is_empty());
    }

    #[test]
    fn test_scan_turns_skips_system_prefixes() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"message":{"role":"user","content":"Hello world"}}
{"type":"assistant","uuid":"a1","timestamp":1700000060,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
{"type":"user","uuid":"u2","timestamp":1700000100,"message":{"role":"user","content":"<system-reminder>Context refresh</system-reminder>"}}
{"type":"assistant","uuid":"a2","timestamp":1700000120,"message":{"role":"assistant","content":[{"type":"text","text":"Ok."}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(turns.len(), 1, "System-prefix user messages should not start a new turn");
        assert_eq!(turns[0].prompt_preview, "Hello world");
        // Wall clock: 1700000000 → 1700000120 = 120s
        assert_eq!(turns[0].wall_clock_seconds, 120);
    }

    #[test]
    fn test_scan_turns_skips_tool_result_continuations() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"message":{"role":"user","content":"Fix the bug"}}
{"type":"assistant","uuid":"a1","timestamp":1700000030,"message":{"role":"assistant","content":[{"type":"tool_use","id":"tu1","name":"Read","input":{}}]}}
{"type":"user","uuid":"u2","timestamp":1700000035,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"file contents"}]}}
{"type":"assistant","uuid":"a2","timestamp":1700000060,"message":{"role":"assistant","content":[{"type":"text","text":"Fixed!"}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(turns.len(), 1, "Tool result continuation should not start a new turn");
        assert_eq!(turns[0].prompt_preview, "Fix the bug");
    }

    #[test]
    fn test_scan_turns_with_turn_duration() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"message":{"role":"user","content":"Hello"}}
{"type":"assistant","uuid":"a1","timestamp":1700000030,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
{"type":"system","uuid":"s1","timestamp":1700000035,"subtype":"turn_duration","durationMs":30000,"isMeta":true}
"#;
        let turns = scan_turns(data);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].cc_duration_ms, Some(30000));
        assert_eq!(turns[0].wall_clock_seconds, 35); // 1700000000 → 1700000035
    }

    #[test]
    fn test_scan_turns_iso_timestamps() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"Hello"}}
{"type":"assistant","uuid":"a1","timestamp":"2026-01-28T10:07:00Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].wall_clock_seconds, 420); // 7 minutes
    }

    #[test]
    fn test_scan_turns_skips_meta_user_messages() {
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"isMeta":true,"message":{"role":"user","content":"System init"}}
{"type":"user","uuid":"u2","timestamp":1700000010,"message":{"role":"user","content":"Real prompt"}}
{"type":"assistant","uuid":"a1","timestamp":1700000060,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
"#;
        let turns = scan_turns(data);
        assert_eq!(turns.len(), 1, "isMeta user message should not start a turn");
        assert_eq!(turns[0].prompt_preview, "Real prompt");
    }

    #[test]
    fn test_truncate_preview_short() {
        assert_eq!(truncate_preview("short", 60), "short");
    }

    #[test]
    fn test_truncate_preview_exact() {
        let s = "a".repeat(60);
        assert_eq!(truncate_preview(&s, 60), s);
    }

    #[test]
    fn test_truncate_preview_long() {
        let s = "a".repeat(100);
        let result = truncate_preview(&s, 60);
        assert_eq!(result.len(), 63); // 60 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_trims_whitespace() {
        assert_eq!(truncate_preview("  hello  ", 60), "hello");
    }
}
