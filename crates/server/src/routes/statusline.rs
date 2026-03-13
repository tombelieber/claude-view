//! POST /api/live/statusline — receive per-turn statusline JSON from Claude Code.
//!
//! Claude Code pipes the full statusline JSON to our wrapper script on every
//! assistant turn. The wrapper forwards it here. We extract ground-truth fields
//! that can't be reliably derived from JSONL parsing:
//!   - context_window.context_window_size  (real max: 200K or 1M)
//!   - context_window.used_percentage      (authoritative %, no math needed)
//!   - context_window.current_usage        (current turn input tokens)
//!   - cost.total_cost_usd                 (Claude Code's own cost calculation)
//!   - model.id                            (current model, catches mid-session switches)

use axum::{extract::State, response::Json, routing::post, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::live::state::LiveSession;
use crate::state::AppState;

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslinePayload {
    pub session_id: String,
    pub model: Option<StatuslineModel>,
    pub cwd: Option<String>,
    pub workspace: Option<StatuslineWorkspace>,
    pub cost: Option<StatuslineCost>,
    pub context_window: Option<StatuslineContextWindow>,
    pub exceeds_200k_tokens: Option<bool>,
    pub transcript_path: Option<String>,
    pub version: Option<String>,
    pub output_style: Option<StatuslineOutputStyle>,
    pub vim: Option<StatuslineVim>,
    pub agent: Option<StatuslineAgent>,
    pub worktree: Option<StatuslineWorktree>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineModel {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineContextWindow {
    /// The real context window limit in tokens (200_000 or 1_000_000).
    /// This is the authoritative value — no guessing needed.
    pub context_window_size: Option<u32>,
    /// Pre-computed percentage used (0.0–100.0). Null early in session.
    pub used_percentage: Option<f64>,
    /// Pre-computed percentage remaining (0.0–100.0).
    pub remaining_percentage: Option<f64>,
    /// Current turn token usage breakdown. Claude Code sends this as an object
    /// with per-category token counts, not a single integer.
    pub current_usage: Option<StatuslineCurrentUsage>,
    /// Cumulative input tokens across the session.
    pub total_input_tokens: Option<u64>,
    /// Cumulative output tokens across the session.
    pub total_output_tokens: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineCurrentUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineCost {
    /// Claude Code's own total cost calculation in USD.
    pub total_cost_usd: Option<f64>,
    /// Total wall-clock session duration in milliseconds.
    pub total_duration_ms: Option<u64>,
    /// API-only duration in milliseconds.
    pub total_api_duration_ms: Option<u64>,
    /// Total lines added across the session.
    pub total_lines_added: Option<u64>,
    /// Total lines removed across the session.
    pub total_lines_removed: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineWorkspace {
    pub current_dir: Option<String>,
    pub project_dir: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineOutputStyle {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineVim {
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineAgent {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatuslineWorktree {
    pub name: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
    pub original_cwd: Option<String>,
    pub original_branch: Option<String>,
}

/// Apply statusline payload fields to a live session.
/// Pure function — no IO, no branching, just field mapping.
/// Testable independently of the Axum handler.
pub fn apply_statusline(session: &mut LiveSession, payload: &StatuslinePayload) {
    // Context window
    if let Some(ref cw) = payload.context_window {
        if let Some(size) = cw.context_window_size {
            session.statusline_context_window_size = Some(size);
        }
        if let Some(pct) = cw.used_percentage {
            session.statusline_used_pct = Some(pct as f32);
        }
        if let Some(ref usage) = cw.current_usage {
            let fill = usage.input_tokens.unwrap_or(0)
                + usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.cache_read_input_tokens.unwrap_or(0);
            if fill > 0 {
                session.context_window_tokens = fill;
            }
            session.statusline_input_tokens = usage.input_tokens;
            session.statusline_output_tokens = usage.output_tokens;
            session.statusline_cache_read_tokens = usage.cache_read_input_tokens;
            session.statusline_cache_creation_tokens = usage.cache_creation_input_tokens;
        }
    }

    // Model
    if let Some(ref m) = payload.model {
        if let Some(ref id) = m.id {
            if !id.is_empty() {
                session.model = Some(id.clone());
            }
        }
        if let Some(ref dn) = m.display_name {
            if !dn.is_empty() {
                session.model_display_name = Some(dn.clone());
            }
        }
    }

    // Cost
    if let Some(ref cost) = payload.cost {
        if let Some(usd) = cost.total_cost_usd {
            if usd > 0.0 {
                session.statusline_cost_usd = Some(usd);
            }
        }
        session.statusline_total_duration_ms = cost.total_duration_ms;
        session.statusline_api_duration_ms = cost.total_api_duration_ms;
        session.statusline_lines_added = cost.total_lines_added;
        session.statusline_lines_removed = cost.total_lines_removed;
    }

    // Workspace
    if let Some(ref ws) = payload.workspace {
        session.statusline_cwd = ws.current_dir.clone();
        session.statusline_project_dir = ws.project_dir.clone();
    } else if let Some(ref cwd) = payload.cwd {
        session.statusline_cwd = Some(cwd.clone());
    }

    // Top-level scalars
    session.statusline_version = payload.version.clone();
    session.exceeds_200k_tokens = payload.exceeds_200k_tokens;
    session.statusline_transcript_path = payload.transcript_path.clone();

    // Raw blob for debug endpoint
    session.statusline_raw = serde_json::to_value(payload).ok();
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/live/statusline", post(handle_statusline))
}

async fn handle_statusline(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StatuslinePayload>,
) -> Json<serde_json::Value> {
    // Step 1: Check transcript dedup FIRST (acquire + release transcript lock).
    // Lock ordering: transcript_to_session → live_sessions → accumulators.
    let dedup_action = if let Some(ref tp) = payload.transcript_path {
        let transcript_path = std::path::PathBuf::from(tp);
        let mut tmap = state.transcript_to_session.write().await;
        if let Some(existing_id) = tmap.get(&transcript_path) {
            if existing_id != &payload.session_id {
                Some(existing_id.clone())
            } else {
                None
            }
        } else {
            tmap.insert(transcript_path, payload.session_id.clone());
            None
        }
        // tmap lock dropped here
    } else {
        None
    };

    // Step 2: Now acquire sessions lock (no other lock held)
    let mut sessions = state.live_sessions.write().await;

    if let Some(older_id) = dedup_action {
        // Merge: apply statusline to older session, remove newer one
        if let Some(_newer) = sessions.remove(&payload.session_id) {
            if let Some(older) = sessions.get_mut(&older_id) {
                apply_statusline(older, &payload);
                tracing::info!(
                    older_id = %older_id,
                    newer_id = %payload.session_id,
                    "Merged duplicate session via transcript_path dedup"
                );
            }
        } else if let Some(older) = sessions.get_mut(&older_id) {
            // Newer session not in map yet — just apply to the older one
            apply_statusline(older, &payload);
            tracing::debug!(
                older_id = %older_id,
                newer_id = %payload.session_id,
                "Statusline applied to existing session (newer not yet registered)"
            );
        }
    } else if let Some(session) = sessions.get_mut(&payload.session_id) {
        apply_statusline(session, &payload);
        tracing::debug!(
            session_id = %payload.session_id,
            "Statusline update applied"
        );
    } else {
        tracing::debug!(
            session_id = %payload.session_id,
            "Statusline received for unknown session (not yet live)"
        );
    }

    Json(serde_json::json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_current_usage_as_object() {
        let json = serde_json::json!({
            "session_id": "abc-123",
            "context_window": {
                "context_window_size": 200000,
                "used_percentage": 42.5,
                "current_usage": {
                    "input_tokens": 8500,
                    "output_tokens": 1200,
                    "cache_creation_input_tokens": 5000,
                    "cache_read_input_tokens": 2000
                }
            }
        });
        let payload: StatuslinePayload = serde_json::from_value(json).unwrap();
        let cw = payload.context_window.unwrap();
        let usage = cw.current_usage.unwrap();
        assert_eq!(usage.input_tokens, Some(8500));
        assert_eq!(usage.output_tokens, Some(1200));
        assert_eq!(usage.cache_creation_input_tokens, Some(5000));
        assert_eq!(usage.cache_read_input_tokens, Some(2000));
    }

    #[test]
    fn computes_context_fill_from_usage_breakdown() {
        let usage = StatuslineCurrentUsage {
            input_tokens: Some(8500),
            output_tokens: Some(1200),
            cache_creation_input_tokens: Some(5000),
            cache_read_input_tokens: Some(2000),
        };
        // fill = input + cache_creation + cache_read (output excluded)
        let fill = usage.input_tokens.unwrap_or(0)
            + usage.cache_creation_input_tokens.unwrap_or(0)
            + usage.cache_read_input_tokens.unwrap_or(0);
        assert_eq!(fill, 15500);
    }

    #[test]
    fn missing_optional_usage_fields_default_to_zero() {
        let usage = StatuslineCurrentUsage {
            input_tokens: Some(8500),
            output_tokens: None,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        let fill = usage.input_tokens.unwrap_or(0)
            + usage.cache_creation_input_tokens.unwrap_or(0)
            + usage.cache_read_input_tokens.unwrap_or(0);
        assert_eq!(fill, 8500);
    }

    #[test]
    fn deserializes_null_current_usage() {
        let json = serde_json::json!({
            "session_id": "abc-123",
            "context_window": {
                "context_window_size": 200000,
                "used_percentage": 10.0,
                "current_usage": null
            }
        });
        let payload: StatuslinePayload = serde_json::from_value(json).unwrap();
        let cw = payload.context_window.unwrap();
        assert!(cw.current_usage.is_none());
    }

    #[test]
    fn deserializes_missing_context_window_fields() {
        let json = serde_json::json!({
            "session_id": "abc-123",
            "context_window": {
                "context_window_size": 1000000
            }
        });
        let payload: StatuslinePayload = serde_json::from_value(json).unwrap();
        let cw = payload.context_window.unwrap();
        assert_eq!(cw.context_window_size, Some(1000000));
        assert!(cw.used_percentage.is_none());
        assert!(cw.current_usage.is_none());
    }

    #[test]
    fn deserializes_cost_zero_present() {
        let json = serde_json::json!({
            "session_id": "abc-123",
            "cost": {
                "total_cost_usd": 0.0
            }
        });
        let payload: StatuslinePayload = serde_json::from_value(json).unwrap();
        let cost = payload.cost.unwrap();
        assert_eq!(cost.total_cost_usd, Some(0.0));
        // Handler guards > 0.0, so 0.0 should NOT be stored
    }

    #[test]
    fn deserializes_minimal_payload() {
        let json = serde_json::json!({
            "session_id": "abc-123"
        });
        let payload: StatuslinePayload = serde_json::from_value(json).unwrap();
        assert_eq!(payload.session_id, "abc-123");
        assert!(payload.model.is_none());
        assert!(payload.context_window.is_none());
        assert!(payload.cost.is_none());
    }

    #[test]
    fn all_zero_usage_yields_zero_fill() {
        let usage = StatuslineCurrentUsage {
            input_tokens: Some(0),
            output_tokens: Some(500),
            cache_creation_input_tokens: Some(0),
            cache_read_input_tokens: Some(0),
        };
        let fill = usage.input_tokens.unwrap_or(0)
            + usage.cache_creation_input_tokens.unwrap_or(0)
            + usage.cache_read_input_tokens.unwrap_or(0);
        // fill=0 means the handler's `if fill > 0` guard prevents update
        assert_eq!(fill, 0);
    }

    #[test]
    fn deserializes_full_statusline_payload() {
        let json = serde_json::json!({
            "session_id": "abc-123",
            "model": { "id": "claude-opus-4-6", "display_name": "Opus" },
            "cwd": "/Users/dev/project",
            "workspace": { "current_dir": "/Users/dev/project", "project_dir": "/Users/dev/project" },
            "cost": {
                "total_cost_usd": 1.23,
                "total_duration_ms": 45000,
                "total_api_duration_ms": 30000,
                "total_lines_added": 156,
                "total_lines_removed": 23
            },
            "context_window": {
                "context_window_size": 1000000,
                "used_percentage": 42.5,
                "remaining_percentage": 57.5,
                "total_input_tokens": 425000,
                "total_output_tokens": 12000,
                "current_usage": {
                    "input_tokens": 8500,
                    "output_tokens": 1200,
                    "cache_creation_input_tokens": 5000,
                    "cache_read_input_tokens": 2000
                }
            },
            "exceeds_200k_tokens": true,
            "transcript_path": "/Users/dev/.claude/projects/abc/sessions/abc-123.jsonl",
            "version": "1.0.42",
            "output_style": { "name": "concise" },
            "vim": { "mode": "normal" },
            "agent": { "name": "code-reviewer" },
            "worktree": {
                "name": "feature-x",
                "path": "/tmp/worktree-x",
                "branch": "feature/x",
                "original_cwd": "/Users/dev/project",
                "original_branch": "main"
            }
        });
        let payload: StatuslinePayload = serde_json::from_value(json).unwrap();
        assert_eq!(payload.session_id, "abc-123");
        assert_eq!(
            payload.model.as_ref().unwrap().display_name.as_deref(),
            Some("Opus")
        );
        assert_eq!(payload.cwd.as_deref(), Some("/Users/dev/project"));
        let ws = payload.workspace.as_ref().unwrap();
        assert_eq!(ws.current_dir.as_deref(), Some("/Users/dev/project"));
        assert_eq!(ws.project_dir.as_deref(), Some("/Users/dev/project"));
        let cost = payload.cost.as_ref().unwrap();
        assert_eq!(cost.total_cost_usd, Some(1.23));
        assert_eq!(cost.total_duration_ms, Some(45000));
        assert_eq!(cost.total_api_duration_ms, Some(30000));
        assert_eq!(cost.total_lines_added, Some(156));
        assert_eq!(cost.total_lines_removed, Some(23));
        let cw = payload.context_window.as_ref().unwrap();
        assert_eq!(cw.remaining_percentage, Some(57.5));
        assert_eq!(payload.exceeds_200k_tokens, Some(true));
        assert_eq!(
            payload.transcript_path.as_deref(),
            Some("/Users/dev/.claude/projects/abc/sessions/abc-123.jsonl")
        );
        assert_eq!(payload.version.as_deref(), Some("1.0.42"));
        assert_eq!(
            payload.output_style.as_ref().unwrap().name.as_deref(),
            Some("concise")
        );
        assert_eq!(
            payload.vim.as_ref().unwrap().mode.as_deref(),
            Some("normal")
        );
        assert_eq!(
            payload.agent.as_ref().unwrap().name.as_deref(),
            Some("code-reviewer")
        );
        let wt = payload.worktree.as_ref().unwrap();
        assert_eq!(wt.name.as_deref(), Some("feature-x"));
        assert_eq!(wt.original_branch.as_deref(), Some("main"));
    }

    #[test]
    fn extra_fields_captured_by_serde_flatten() {
        let json = serde_json::json!({
            "session_id": "abc-123",
            "some_future_field": "hello",
            "another_new_thing": { "nested": true }
        });
        let payload: StatuslinePayload = serde_json::from_value(json).unwrap();
        assert_eq!(payload.session_id, "abc-123");
        assert_eq!(payload.extra["some_future_field"], "hello");
        assert!(payload.extra["another_new_thing"]["nested"]
            .as_bool()
            .unwrap());
    }

    #[test]
    fn apply_statusline_maps_all_fields() {
        use crate::live::state::test_live_session;
        let mut session = test_live_session("test-1");

        let payload = StatuslinePayload {
            session_id: "test-1".into(),
            model: Some(StatuslineModel {
                id: Some("claude-opus-4-6".into()),
                display_name: Some("Opus".into()),
            }),
            cwd: Some("/Users/dev/project".into()),
            workspace: Some(StatuslineWorkspace {
                current_dir: Some("/Users/dev/project".into()),
                project_dir: Some("/Users/dev/project".into()),
            }),
            cost: Some(StatuslineCost {
                total_cost_usd: Some(1.23),
                total_duration_ms: Some(45000),
                total_api_duration_ms: Some(30000),
                total_lines_added: Some(156),
                total_lines_removed: Some(23),
            }),
            context_window: Some(StatuslineContextWindow {
                context_window_size: Some(1_000_000),
                used_percentage: Some(42.5),
                remaining_percentage: Some(57.5),
                total_input_tokens: Some(425000),
                total_output_tokens: Some(12000),
                current_usage: Some(StatuslineCurrentUsage {
                    input_tokens: Some(8500),
                    output_tokens: Some(1200),
                    cache_creation_input_tokens: Some(5000),
                    cache_read_input_tokens: Some(2000),
                }),
            }),
            exceeds_200k_tokens: Some(true),
            transcript_path: Some("/path/to/transcript.jsonl".into()),
            version: Some("1.0.42".into()),
            output_style: Some(StatuslineOutputStyle {
                name: Some("concise".into()),
            }),
            vim: Some(StatuslineVim {
                mode: Some("normal".into()),
            }),
            agent: Some(StatuslineAgent {
                name: Some("code-reviewer".into()),
            }),
            worktree: Some(StatuslineWorktree {
                name: Some("feature-x".into()),
                path: Some("/tmp/wt".into()),
                branch: Some("feature/x".into()),
                original_cwd: Some("/Users/dev".into()),
                original_branch: Some("main".into()),
            }),
            extra: std::collections::HashMap::new(),
        };

        apply_statusline(&mut session, &payload);

        assert_eq!(session.model_display_name.as_deref(), Some("Opus"));
        assert_eq!(session.statusline_context_window_size, Some(1_000_000));
        assert_eq!(session.statusline_used_pct, Some(42.5f32));
        assert_eq!(session.statusline_cost_usd, Some(1.23));
        assert_eq!(
            session.statusline_cwd.as_deref(),
            Some("/Users/dev/project")
        );
        assert_eq!(
            session.statusline_project_dir.as_deref(),
            Some("/Users/dev/project")
        );
        assert_eq!(session.statusline_total_duration_ms, Some(45000));
        assert_eq!(session.statusline_api_duration_ms, Some(30000));
        assert_eq!(session.statusline_lines_added, Some(156));
        assert_eq!(session.statusline_lines_removed, Some(23));
        assert_eq!(session.statusline_input_tokens, Some(8500));
        assert_eq!(session.statusline_output_tokens, Some(1200));
        assert_eq!(session.statusline_cache_read_tokens, Some(2000));
        assert_eq!(session.statusline_cache_creation_tokens, Some(5000));
        assert_eq!(session.statusline_version.as_deref(), Some("1.0.42"));
        assert_eq!(session.exceeds_200k_tokens, Some(true));
        assert_eq!(
            session.statusline_transcript_path.as_deref(),
            Some("/path/to/transcript.jsonl")
        );
    }
}
