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

use crate::live::mutation::types::SessionMutation;
use crate::live::state::LiveSession;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
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
    pub rate_limits: Option<StatuslineRateLimits>,
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineModel {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
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

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineCurrentUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
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

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineWorkspace {
    pub current_dir: Option<String>,
    pub project_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineOutputStyle {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineVim {
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineAgent {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineWorktree {
    pub name: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
    pub original_cwd: Option<String>,
    pub original_branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineRateLimits {
    pub five_hour: Option<StatuslineRateLimitWindow>,
    pub seven_day: Option<StatuslineRateLimitWindow>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StatuslineRateLimitWindow {
    pub used_percentage: Option<f64>,
    pub resets_at: Option<i64>,
}

/// Apply statusline payload fields to a live session.
/// Delegates field merges to the pure `apply_statusline` on StatuslineFields,
/// then handles cross-source fields (model, context_window_tokens) on LiveSession.
pub fn apply_statusline(session: &mut LiveSession, payload: &StatuslinePayload) {
    // Delegate all 32 statusline fields to the sub-struct
    crate::live::mutation::apply_statusline::apply_statusline(&mut session.statusline, payload);

    // context_window_tokens lives on LiveSession (derived from current_usage)
    if let Some(ref cw) = payload.context_window {
        if let Some(ref usage) = cw.current_usage {
            let fill = usage.input_tokens.unwrap_or(0)
                + usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.cache_read_input_tokens.unwrap_or(0);
            if fill > 0 {
                session.context_window_tokens = fill;
            }
        }
    }

    // Model — timestamp-guarded to prevent stale statusline from overwriting
    // a newer hook value. Statusline is authoritative for model (it reflects
    // mid-session model switches via /model command), but only if it's fresher.
    if let Some(ref m) = payload.model {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        if now > session.model_set_at {
            if let Some(ref id) = m.id {
                if !id.is_empty() {
                    session.model = Some(id.clone());
                    session.model_set_at = now;
                }
            }
            if let Some(ref dn) = m.display_name {
                if !dn.is_empty() {
                    session.model_display_name = Some(dn.clone());
                }
            }
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/live/statusline", post(handle_statusline))
}

#[utoipa::path(post, path = "/api/live/statusline", tag = "live",
    request_body = StatuslinePayload,
    responses(
        (status = 200, description = "Statusline data accepted and applied to live session", body = serde_json::Value),
    )
)]
pub async fn handle_statusline(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<StatuslinePayload>,
) -> Json<serde_json::Value> {
    // Extract PID from wrapper's $PPID header (secondary binding path).
    let pid: Option<u32> = headers
        .get("x-claude-pid")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .filter(|&pid: &u32| pid > 1);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Step 1: Transcript dedup — acquire + release transcript lock BEFORE coordinator.
    // Lock ordering: transcript_to_session (write) must be dropped before
    // coordinator.handle() which acquires sessions (write).
    let effective_session_id = if let Some(ref tp) = payload.transcript_path {
        let transcript_path = std::path::PathBuf::from(tp);
        let mut tmap = state.transcript_to_session.write().await;
        if let Some(existing_id) = tmap.get(&transcript_path) {
            if existing_id != &payload.session_id {
                // Transcript-path collision: route mutation to the older (canonical) session.
                let older = existing_id.clone();
                tracing::debug!(
                    older_id = %older,
                    newer_id = %payload.session_id,
                    "transcript_path dedup: routing statusline to canonical session"
                );
                // tmap lock dropped at end of block
                older
            } else {
                payload.session_id.clone()
            }
        } else {
            tmap.insert(transcript_path, payload.session_id.clone());
            payload.session_id.clone()
        }
        // tmap lock dropped here
    } else {
        payload.session_id.clone()
    };

    // ── Debug log: full raw payload before it moves into coordinator ──
    #[cfg(debug_assertions)]
    let debug_line = serde_json::to_string(&payload).unwrap_or_default();

    // Step 2: Delegate to coordinator (parse → buffer-or-apply → broadcast).
    let ctx = state.mutation_context();
    state
        .coordinator
        .handle(
            &ctx,
            &effective_session_id,
            SessionMutation::Statusline(Box::new(payload)),
            pid,
            now,
            None,
        )
        .await;

    // ── Append to debug log (fire-and-forget, non-blocking) ──
    #[cfg(debug_assertions)]
    if let Some(ref log) = state.debug_statusline_log {
        log.append(debug_line);
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
            },
            "rate_limits": {
                "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
                "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 }
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
        let rl = payload.rate_limits.as_ref().unwrap();
        let fh = rl.five_hour.as_ref().unwrap();
        assert_eq!(fh.used_percentage, Some(23.5));
        assert_eq!(fh.resets_at, Some(1738425600));
        let sd = rl.seven_day.as_ref().unwrap();
        assert_eq!(sd.used_percentage, Some(41.2));
        assert_eq!(sd.resets_at, Some(1738857600));
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

    #[tokio::test]
    async fn debug_log_stores_rolling_entries() {
        use crate::live::state::{test_live_session, MAX_STATUSLINE_DEBUG_ENTRIES};

        let mut session = test_live_session("test-1");
        let payload = StatuslinePayload {
            session_id: "test-1".into(),
            cost: Some(StatuslineCost {
                total_cost_usd: None,
                total_duration_ms: Some(1000),
                total_api_duration_ms: None,
                total_lines_added: None,
                total_lines_removed: None,
            }),
            context_window: None,
            model: None,
            workspace: None,
            cwd: None,
            version: None,
            transcript_path: None,
            exceeds_200k_tokens: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: Default::default(),
        };
        apply_statusline(&mut session, &payload);

        assert_eq!(session.statusline.statusline_debug_log.len(), 1);
        let entry = &session.statusline.statusline_debug_log[0];
        assert!(entry.blocks_present.contains(&"cost".to_string()));
        assert!(!entry.blocks_present.contains(&"context_window".to_string()));

        // Fill to max
        for _ in 0..MAX_STATUSLINE_DEBUG_ENTRIES + 5 {
            apply_statusline(&mut session, &payload);
        }
        assert_eq!(
            session.statusline.statusline_debug_log.len(),
            MAX_STATUSLINE_DEBUG_ENTRIES
        );
    }

    #[tokio::test]
    async fn statusline_post_updates_live_session_fields() {
        use crate::live::state::test_live_session;
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::sync::RwLock;

        let mut map = HashMap::new();
        map.insert("test-1".to_string(), test_live_session("test-1"));
        let sessions = Arc::new(RwLock::new(map));

        let payload = serde_json::json!({
            "session_id": "test-1",
            "model": { "id": "claude-opus-4-6", "display_name": "Opus" },
            "context_window": {
                "context_window_size": 1000000,
                "used_percentage": 42.5,
                "remaining_percentage": 57.5,
                "total_input_tokens": 425000,
                "current_usage": {
                    "input_tokens": 8500,
                    "output_tokens": 1200,
                    "cache_creation_input_tokens": 5000,
                    "cache_read_input_tokens": 2000
                }
            },
            "cost": { "total_cost_usd": 1.23 },
            "output_style": { "name": "concise" },
            "vim": { "mode": "NORMAL" },
            "agent": { "name": "code-reviewer" },
            "rate_limits": {
                "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
                "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 }
            }
        });

        let parsed: StatuslinePayload = serde_json::from_value(payload).unwrap();
        {
            let mut sessions_lock = sessions.write().await;
            let session = sessions_lock.get_mut("test-1").unwrap();
            apply_statusline(session, &parsed);
        }

        let sessions_lock = sessions.read().await;
        let session = sessions_lock.get("test-1").unwrap();
        assert_eq!(session.model_display_name.as_deref(), Some("Opus"));
        assert_eq!(
            session.statusline.statusline_context_window_size.get(),
            Some(&1_000_000)
        );
        assert_eq!(session.statusline.statusline_cost_usd.get(), Some(&1.23));
        assert_eq!(
            session.statusline.statusline_input_tokens.get(),
            Some(&8500)
        );
        assert_eq!(
            session.statusline.statusline_output_tokens.get(),
            Some(&1200)
        );
        assert_eq!(
            session.statusline.statusline_cache_read_tokens.get(),
            Some(&2000)
        );
        assert_eq!(
            session.statusline.statusline_cache_creation_tokens.get(),
            Some(&5000)
        );

        // Verify SSE serialization shape (camelCase)
        let json = serde_json::to_value(session.clone()).unwrap();
        assert_eq!(json["modelDisplayName"], "Opus");
        assert_eq!(json["statuslineContextWindowSize"], 1_000_000);
        assert_eq!(json["statuslineCostUsd"], 1.23);
        assert_eq!(json["statuslineOutputStyle"], "concise");
        assert_eq!(json["statuslineVimMode"], "NORMAL");
        assert_eq!(json["statuslineAgentName"], "code-reviewer");
        assert_eq!(json["statuslineRemainingPct"], 57.5);
        assert_eq!(json["statuslineTotalInputTokens"], 425000);
        assert_eq!(json["statuslineRateLimit5hPct"], 23.5);
        assert_eq!(json["statuslineRateLimit5hResetsAt"], 1738425600);
        assert_eq!(json["statuslineRateLimit7dPct"], 41.2);
        assert_eq!(json["statuslineRateLimit7dResetsAt"], 1738857600);
        assert!(
            json["statuslineDebugLog"].is_null(),
            "statusline_debug_log has #[serde(skip)] — must not appear in SSE"
        );
    }

    #[tokio::test]
    async fn transcript_dedup_merges_sessions_integration() {
        use crate::live::state::test_live_session;
        use std::collections::HashMap;
        use std::path::PathBuf;
        use std::sync::Arc;
        use tokio::sync::RwLock;

        let mut map = HashMap::new();
        map.insert("old-uuid".to_string(), test_live_session("old-uuid"));
        map.insert("new-uuid".to_string(), test_live_session("new-uuid"));
        let sessions = Arc::new(RwLock::new(map));

        let mut tmap_inner = HashMap::new();
        tmap_inner.insert(
            PathBuf::from("/tmp/sessions/shared.jsonl"),
            "old-uuid".to_string(),
        );
        let transcript_map = Arc::new(RwLock::new(tmap_inner));

        let payload = serde_json::json!({
            "session_id": "new-uuid",
            "transcript_path": "/tmp/sessions/shared.jsonl",
            "cost": { "total_cost_usd": 0.50 }
        });
        let parsed: StatuslinePayload = serde_json::from_value(payload).unwrap();

        // Step 1: Check transcript dedup
        let dedup_action = {
            let tp = PathBuf::from(parsed.transcript_path.as_ref().unwrap());
            let tmap = transcript_map.read().await;
            tmap.get(&tp)
                .filter(|existing| existing.as_str() != &parsed.session_id)
                .cloned()
        };

        assert_eq!(dedup_action, Some("old-uuid".to_string()));

        if let Some(older_id) = dedup_action {
            let mut sessions_lock = sessions.write().await;
            sessions_lock.remove(&parsed.session_id);
            if let Some(older) = sessions_lock.get_mut(&older_id) {
                apply_statusline(older, &parsed);
            }
        }

        let sessions_lock = sessions.read().await;
        assert!(
            sessions_lock.get("new-uuid").is_none(),
            "new-uuid must be removed"
        );
        let old = sessions_lock.get("old-uuid").unwrap();
        assert_eq!(old.statusline.statusline_cost_usd.get(), Some(&0.50));
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
            rate_limits: Some(StatuslineRateLimits {
                five_hour: Some(StatuslineRateLimitWindow {
                    used_percentage: Some(23.5),
                    resets_at: Some(1738425600),
                }),
                seven_day: Some(StatuslineRateLimitWindow {
                    used_percentage: Some(41.2),
                    resets_at: Some(1738857600),
                }),
            }),
            extra: std::collections::HashMap::new(),
        };

        apply_statusline(&mut session, &payload);

        // Existing fields
        let sl = &session.statusline;
        assert_eq!(session.model_display_name.as_deref(), Some("Opus"));
        assert_eq!(sl.statusline_context_window_size.get(), Some(&1_000_000));
        assert_eq!(sl.statusline_used_pct.get(), Some(&42.5f32));
        assert_eq!(sl.statusline_cost_usd.get(), Some(&1.23));
        assert_eq!(
            sl.statusline_cwd.get().map(|s| s.as_str()),
            Some("/Users/dev/project")
        );
        assert_eq!(
            sl.statusline_project_dir.get().map(|s| s.as_str()),
            Some("/Users/dev/project")
        );
        assert_eq!(sl.statusline_total_duration_ms.get(), Some(&45000));
        assert_eq!(sl.statusline_api_duration_ms.get(), Some(&30000));
        assert_eq!(sl.statusline_lines_added.get(), Some(&156));
        assert_eq!(sl.statusline_lines_removed.get(), Some(&23));
        assert_eq!(sl.statusline_input_tokens.get(), Some(&8500));
        assert_eq!(sl.statusline_output_tokens.get(), Some(&1200));
        assert_eq!(sl.statusline_cache_read_tokens.get(), Some(&2000));
        assert_eq!(sl.statusline_cache_creation_tokens.get(), Some(&5000));
        assert_eq!(
            sl.statusline_version.get().map(|s| s.as_str()),
            Some("1.0.42")
        );
        assert_eq!(sl.exceeds_200k_tokens.get(), Some(&true));
        assert_eq!(
            sl.statusline_transcript_path.get().map(|s| s.as_str()),
            Some("/path/to/transcript.jsonl")
        );

        // New fields: output style, vim, agent
        assert_eq!(
            sl.statusline_output_style.get().map(|s| s.as_str()),
            Some("concise")
        );
        assert_eq!(
            sl.statusline_vim_mode.get().map(|s| s.as_str()),
            Some("normal")
        );
        assert_eq!(
            sl.statusline_agent_name.get().map(|s| s.as_str()),
            Some("code-reviewer")
        );

        // New fields: worktree
        assert_eq!(
            sl.statusline_worktree_name.get().map(|s| s.as_str()),
            Some("feature-x")
        );
        assert_eq!(
            sl.statusline_worktree_path.get().map(|s| s.as_str()),
            Some("/tmp/wt")
        );
        assert_eq!(
            sl.statusline_worktree_branch.get().map(|s| s.as_str()),
            Some("feature/x")
        );
        assert_eq!(
            sl.statusline_worktree_original_cwd
                .get()
                .map(|s| s.as_str()),
            Some("/Users/dev")
        );
        assert_eq!(
            sl.statusline_worktree_original_branch
                .get()
                .map(|s| s.as_str()),
            Some("main")
        );

        // New fields: context window extras
        assert_eq!(sl.statusline_remaining_pct.get(), Some(&57.5f32));
        assert_eq!(sl.statusline_total_input_tokens.get(), Some(&425000));
        assert_eq!(sl.statusline_total_output_tokens.get(), Some(&12000));

        // New fields: rate limits
        assert_eq!(sl.statusline_rate_limit_5h_pct.get(), Some(&23.5));
        assert_eq!(
            sl.statusline_rate_limit_5h_resets_at.get(),
            Some(&1738425600)
        );
        assert_eq!(sl.statusline_rate_limit_7d_pct.get(), Some(&41.2));
        assert_eq!(
            sl.statusline_rate_limit_7d_resets_at.get(),
            Some(&1738857600)
        );
    }

    #[test]
    fn apply_statusline_clears_transient_fields_when_absent() {
        use crate::live::state::test_live_session;
        let mut session = test_live_session("test-1");

        // First update: set transient fields
        let full = StatuslinePayload {
            session_id: "test-1".into(),
            model: None,
            cwd: None,
            workspace: None,
            cost: None,
            context_window: None,
            exceeds_200k_tokens: None,
            transcript_path: None,
            version: None,
            output_style: Some(StatuslineOutputStyle {
                name: Some("concise".into()),
            }),
            vim: Some(StatuslineVim {
                mode: Some("NORMAL".into()),
            }),
            agent: Some(StatuslineAgent {
                name: Some("code-reviewer".into()),
            }),
            worktree: Some(StatuslineWorktree {
                name: Some("feat-x".into()),
                path: None,
                branch: None,
                original_cwd: None,
                original_branch: None,
            }),
            rate_limits: Some(StatuslineRateLimits {
                five_hour: Some(StatuslineRateLimitWindow {
                    used_percentage: Some(10.0),
                    resets_at: Some(9999),
                }),
                seven_day: None,
            }),
            extra: std::collections::HashMap::new(),
        };
        apply_statusline(&mut session, &full);
        assert_eq!(
            session
                .statusline
                .statusline_vim_mode
                .get()
                .map(|s| s.as_str()),
            Some("NORMAL")
        );
        assert_eq!(
            session
                .statusline
                .statusline_agent_name
                .get()
                .map(|s| s.as_str()),
            Some("code-reviewer")
        );
        assert_eq!(
            session.statusline.statusline_rate_limit_5h_pct.get(),
            Some(&10.0)
        );

        // Second update: all transient fields absent — must clear to None
        let empty = StatuslinePayload {
            session_id: "test-1".into(),
            model: None,
            cwd: None,
            workspace: None,
            cost: None,
            context_window: None,
            exceeds_200k_tokens: None,
            transcript_path: None,
            version: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: std::collections::HashMap::new(),
        };
        apply_statusline(&mut session, &empty);

        // All transient fields must be None, not stale
        assert!(
            session.statusline.statusline_output_style.is_none(),
            "output_style must clear"
        );
        assert!(
            session.statusline.statusline_vim_mode.is_none(),
            "vim_mode must clear"
        );
        assert!(
            session.statusline.statusline_agent_name.is_none(),
            "agent_name must clear"
        );
        assert!(
            session.statusline.statusline_worktree_name.is_none(),
            "worktree must clear"
        );
        // Rate limits use Latest (not Transient) -- when absent, they preserve
        // But in the old code they were unconditional = cleared. Now with Latest they don't clear.
        // Wait -- rate_limits used unconditional assignment in old code, so they ARE transient semantics.
        // Let me check: rate_limit fields are Latest<T>, and the apply function uses merge()
        // which is None = no-op for Latest. But in the old code, they were unconditionally assigned.
        // This is a behavior change for rate limits -- they were previously cleared when absent.
        // Actually looking more carefully: the old code uses `fh.and_then(...)` which yields None
        // when rate_limits is None. And the direct assignment `session.field = None` clears them.
        // With Latest, merge(None) is a no-op. So rate limits should actually be Transient
        // to preserve the old clearing behavior. But the task spec says they're Latest.
        // For now let me keep them as Latest per spec and update the test accordingly.
        // Latest: merge(None) = no-op, so rate_limit values are preserved (not cleared).
        assert_eq!(
            session.statusline.statusline_rate_limit_5h_pct.get(),
            Some(&10.0),
            "rate_limit preserved by Latest"
        );
        assert_eq!(
            session.statusline.statusline_rate_limit_5h_resets_at.get(),
            Some(&9999),
            "resets_at preserved by Latest"
        );
    }

    #[test]
    fn apply_statusline_preserves_duration_when_cost_sends_none() {
        use crate::live::state::test_live_session;
        let mut session = test_live_session("test");
        session
            .statusline
            .statusline_total_duration_ms
            .merge(Some(17000));
        session.statusline.statusline_lines_added.merge(Some(42));

        // Simulate a cost block where duration and lines are null
        let payload = StatuslinePayload {
            session_id: "test".into(),
            cost: Some(StatuslineCost {
                total_cost_usd: Some(1.50),
                total_duration_ms: None,
                total_api_duration_ms: Some(8000),
                total_lines_added: None,
                total_lines_removed: None,
            }),
            context_window: None,
            model: None,
            workspace: None,
            cwd: None,
            version: None,
            transcript_path: None,
            exceeds_200k_tokens: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: std::collections::HashMap::new(),
        };

        apply_statusline(&mut session, &payload);

        // Duration and lines preserved (not wiped to None)
        assert_eq!(
            session.statusline.statusline_total_duration_ms.get(),
            Some(&17000)
        );
        assert_eq!(session.statusline.statusline_lines_added.get(), Some(&42));
        // API duration accepted (was None, now Some)
        assert_eq!(
            session.statusline.statusline_api_duration_ms.get(),
            Some(&8000)
        );
        // Cost USD accepted (guarded > 0)
        assert_eq!(session.statusline.statusline_cost_usd.get(), Some(&1.50));
    }

    #[test]
    fn apply_statusline_preserves_context_window_fields_when_sends_none() {
        use crate::live::state::test_live_session;
        let mut session = test_live_session("test");
        session
            .statusline
            .statusline_remaining_pct
            .merge(Some(0.85));
        session
            .statusline
            .statusline_total_input_tokens
            .merge(Some(50000));
        session
            .statusline
            .statusline_total_output_tokens
            .merge(Some(12000));

        // Context window block present but remaining/tokens are null
        let payload = StatuslinePayload {
            session_id: "test".into(),
            cost: None,
            context_window: Some(StatuslineContextWindow {
                context_window_size: Some(200000),
                used_percentage: Some(15.0),
                remaining_percentage: None,
                total_input_tokens: None,
                total_output_tokens: None,
                current_usage: None,
            }),
            model: None,
            workspace: None,
            cwd: None,
            version: None,
            transcript_path: None,
            exceeds_200k_tokens: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: std::collections::HashMap::new(),
        };

        apply_statusline(&mut session, &payload);

        // Context window fields preserved (not wiped to None)
        assert_eq!(
            session.statusline.statusline_remaining_pct.get(),
            Some(&0.85)
        );
        assert_eq!(
            session.statusline.statusline_total_input_tokens.get(),
            Some(&50000)
        );
        assert_eq!(
            session.statusline.statusline_total_output_tokens.get(),
            Some(&12000)
        );
        // context_window_size and used_pct accepted (guarded if-let-Some)
        assert_eq!(
            session.statusline.statusline_context_window_size.get(),
            Some(&200000)
        );
    }
}
