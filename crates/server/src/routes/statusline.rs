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
use serde::Deserialize;
use std::sync::Arc;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StatuslinePayload {
    pub session_id: String,
    pub model: Option<StatuslineModel>,
    pub context_window: Option<StatuslineContextWindow>,
    pub cost: Option<StatuslineCost>,
}

#[derive(Debug, Deserialize)]
pub struct StatuslineModel {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StatuslineContextWindow {
    /// The real context window limit in tokens (200_000 or 1_000_000).
    /// This is the authoritative value — no guessing needed.
    pub context_window_size: Option<u32>,
    /// Pre-computed percentage used (0.0–100.0). Null early in session.
    pub used_percentage: Option<f64>,
    /// Current turn token usage breakdown. Claude Code sends this as an object
    /// with per-category token counts, not a single integer.
    pub current_usage: Option<StatuslineCurrentUsage>,
    /// Cumulative input tokens across the session.
    pub total_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct StatuslineCurrentUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct StatuslineCost {
    /// Claude Code's own total cost calculation in USD.
    pub total_cost_usd: Option<f64>,
    /// Total wall-clock session duration in milliseconds.
    pub total_duration_ms: Option<u64>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/live/statusline", post(handle_statusline))
}

async fn handle_statusline(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StatuslinePayload>,
) -> Json<serde_json::Value> {
    let mut sessions = state.live_sessions.write().await;

    if let Some(session) = sessions.get_mut(&payload.session_id) {
        // Update context window ground-truth fields
        if let Some(ref cw) = payload.context_window {
            if let Some(size) = cw.context_window_size {
                session.statusline_context_window_size = Some(size);
            }
            if let Some(pct) = cw.used_percentage {
                session.statusline_used_pct = Some(pct as f32);
            }
            if let Some(ref usage) = cw.current_usage {
                // Context fill = input + cache_creation + cache_read
                // (output tokens don't occupy input context space)
                let fill = usage.input_tokens.unwrap_or(0)
                    + usage.cache_creation_input_tokens.unwrap_or(0)
                    + usage.cache_read_input_tokens.unwrap_or(0);
                if fill > 0 {
                    session.context_window_tokens = fill;
                }
            }
        }

        // Update model if we got a more current value
        if let Some(ref m) = payload.model {
            if let Some(ref id) = m.id {
                if !id.is_empty() {
                    session.model = Some(id.clone());
                }
            }
        }

        // Update cost ground-truth
        if let Some(ref cost) = payload.cost {
            if let Some(usd) = cost.total_cost_usd {
                if usd > 0.0 {
                    session.statusline_cost_usd = Some(usd);
                }
            }
        }

        tracing::debug!(
            session_id = %payload.session_id,
            context_window_size = ?payload.context_window.as_ref().and_then(|c| c.context_window_size),
            used_pct = ?payload.context_window.as_ref().and_then(|c| c.used_percentage),
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
}
