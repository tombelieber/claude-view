//! POST /api/live/statusline handler and router.

use axum::{extract::State, response::Json, routing::post, Router};
use std::sync::Arc;

use crate::live::mutation::types::SessionMutation;
use crate::state::AppState;

use super::types::StatuslinePayload;

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

    // Step 1: Transcript dedup -- acquire + release transcript lock BEFORE coordinator.
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

    // -- Debug log: full raw payload before it moves into coordinator --
    #[cfg(debug_assertions)]
    let debug_line = serde_json::to_string(&payload).unwrap_or_default();

    // Step 2: Delegate to coordinator (parse -> buffer-or-apply -> broadcast).
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
            None, // statusline has its own session creation path
            None,
        )
        .await;

    // -- Append to debug log (fire-and-forget, non-blocking) --
    #[cfg(debug_assertions)]
    if let Some(ref log) = state.debug_statusline_log {
        log.append(debug_line);
    }

    Json(serde_json::json!({ "ok": true }))
}
