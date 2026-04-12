//! Route handlers for session interaction endpoints.
//!
//! - GET  /sessions/{session_id}/interaction — fetch full interaction data
//! - POST /sessions/{session_id}/interact   — resolve an interaction

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::error::{ApiError, ApiResult};
use crate::live::mutation::types::{InteractionAction, SessionMutation};
use crate::state::AppState;
use claude_view_types::{InteractRequest, InteractionBlock, SessionOwnership};

/// Extract request_id from any InteractRequest variant.
fn extract_request_id(req: &InteractRequest) -> &str {
    match req {
        InteractRequest::Permission { request_id, .. }
        | InteractRequest::Question { request_id, .. }
        | InteractRequest::Plan { request_id, .. }
        | InteractRequest::Elicitation { request_id, .. } => request_id,
    }
}

// ════════════════════════════════════════════════════════════════════
// GET /sessions/{session_id}/interaction
// ════════════════════════════════════════════════════════════════════

/// Fetch the full interaction data for a session's pending interaction.
///
/// 1. Look up session in live_sessions — 404 if not found
/// 2. Read pending_interaction — 404 if None (no pending interaction)
/// 3. Look up full data in interaction_data side-map — 404 if not found
#[utoipa::path(
    get,
    path = "/api/sessions/{session_id}/interaction",
    tag = "sessions",
    params(("session_id" = String, Path, description = "Session UUID")),
    responses(
        (status = 200, description = "Full interaction data"),
        (status = 404, description = "Session or interaction not found"),
    )
)]
pub async fn get_interaction_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<InteractionBlock>> {
    // Step 1: session exists?
    let request_id = {
        let sessions = state.live_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| ApiError::NotFound(format!("Session not found: {session_id}")))?;

        // Step 2: pending interaction?
        let pending = session.pending_interaction.as_ref().ok_or_else(|| {
            ApiError::NotFound(format!("No pending interaction for session: {session_id}"))
        })?;

        pending.request_id.clone()
    };

    // Step 3: full data in side-map
    let data = state.interaction_data.read().await;
    let block = data.get(&request_id).ok_or_else(|| {
        ApiError::NotFound(format!(
            "Interaction data not found for request_id: {request_id}"
        ))
    })?;

    Ok(Json(block.clone()))
}

// ════════════════════════════════════════════════════════════════════
// POST /sessions/{session_id}/interact
// ════════════════════════════════════════════════════════════════════

/// Resolve a pending interaction (permission, question, plan, elicitation).
///
/// 1. Parse InteractRequest from body
/// 2. Look up session in live_sessions — 404 if not found
/// 3. Resolve ownership — 400 if Observed
/// 4. Check interaction_data side-map — 409 if request_id not found (stale)
/// 5. For SDK: forward to sidecar HTTP API
/// 6. Clear pending interaction via coordinator
#[utoipa::path(
    post,
    path = "/api/sessions/{session_id}/interact",
    tag = "sessions",
    params(("session_id" = String, Path, description = "Session UUID")),
    responses(
        (status = 200, description = "Interaction resolved"),
        (status = 400, description = "Observed session or invalid request"),
        (status = 404, description = "Session not found"),
        (status = 409, description = "Request ID stale or already resolved"),
    )
)]
pub async fn interact_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(req): Json<InteractRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let request_id = extract_request_id(&req).to_string();

    // Step 2: session exists? Resolve ownership.
    let ownership = {
        let sessions = state.live_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| ApiError::NotFound(format!("Session not found: {session_id}")))?;

        session.ownership.clone()
    };

    // Step 3: check ownership tier
    let ownership = ownership.ok_or_else(|| {
        ApiError::BadRequest("Session has no resolved ownership — cannot interact".to_string())
    })?;

    match &ownership {
        SessionOwnership::Observed { .. } => {
            return Err(ApiError::BadRequest(
                "Cannot interact with an observed session — no control channel available"
                    .to_string(),
            ));
        }
        SessionOwnership::Sdk { .. } | SessionOwnership::Tmux { .. } => {
            // Allowed — continue
        }
    }

    // Step 4: check side-map for request_id (stale check)
    {
        let data = state.interaction_data.read().await;
        if !data.contains_key(&request_id) {
            return Err(ApiError::Conflict(format!(
                "Interaction request_id not found (stale or already resolved): {request_id}"
            )));
        }
    }

    // Step 5: dispatch based on ownership
    let sidecar_forwarded = match &ownership {
        SessionOwnership::Sdk { control_id, .. } => {
            match forward_to_sidecar(&state, control_id, &req).await {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!(
                        session_id,
                        request_id = %request_id,
                        error = %e,
                        "Sidecar forward failed — state cleared but sidecar not notified"
                    );
                    false
                }
            }
        }
        SessionOwnership::Tmux { .. } => {
            // TODO(Task 10+): forward keystroke via tmux send-keys.
            // For v1, we just clear the server-side state.
            true
        }
        SessionOwnership::Observed { .. } => unreachable!("guarded above"),
    };

    // Step 6: clear pending interaction regardless of sidecar result.
    // The user made their decision — don't leave stale pending state.
    clear_pending_interaction(&state, &session_id, &request_id).await;

    // Always return 200 — the interaction is resolved from the server's
    // perspective. The sidecarForwarded flag lets the frontend know if
    // the ConversationView WS path needs to deliver the response instead.
    Ok(Json(serde_json::json!({
        "resolved": true,
        "requestId": request_id,
        "sessionId": session_id,
        "sidecarForwarded": sidecar_forwarded
    })))
}

/// Forward the interaction resolution to the sidecar HTTP API.
async fn forward_to_sidecar(
    state: &AppState,
    control_id: &str,
    req: &InteractRequest,
) -> Result<(), String> {
    let sidecar_base = state
        .sidecar
        .ensure_running()
        .await
        .map_err(|e| format!("Sidecar not available: {e}"))?;

    let url = format!("{sidecar_base}/api/sessions/{control_id}/message");

    // Build the sidecar message from the InteractRequest.
    let sidecar_body = build_sidecar_message(req);

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&sidecar_body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Sidecar HTTP request failed: {e}"))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(format!("Sidecar returned {status}: {body}"))
    }
}

/// Build a sidecar-compatible JSON message from an InteractRequest.
fn build_sidecar_message(req: &InteractRequest) -> serde_json::Value {
    match req {
        InteractRequest::Permission {
            request_id,
            allowed,
            updated_permissions,
        } => {
            let mut msg = serde_json::json!({
                "type": "permission_response",
                "requestId": request_id,
                "allowed": allowed
            });
            if let Some(perms) = updated_permissions {
                msg["updatedPermissions"] = serde_json::json!(perms);
            }
            msg
        }
        InteractRequest::Question {
            request_id,
            answers,
        } => {
            serde_json::json!({
                "type": "question_response",
                "requestId": request_id,
                "answers": answers
            })
        }
        InteractRequest::Plan {
            request_id,
            approved,
            feedback,
            bypass_permissions,
        } => {
            let mut msg = serde_json::json!({
                "type": "plan_response",
                "requestId": request_id,
                "approved": approved
            });
            if let Some(fb) = feedback {
                msg["feedback"] = serde_json::json!(fb);
            }
            if let Some(bp) = bypass_permissions {
                msg["bypassPermissions"] = serde_json::json!(bp);
            }
            msg
        }
        InteractRequest::Elicitation {
            request_id,
            response,
        } => {
            serde_json::json!({
                "type": "elicitation_response",
                "requestId": request_id,
                "response": response
            })
        }
    }
}

/// Clear the pending interaction from both the session and the side-map.
/// Uses the coordinator for the session mutation and direct write for the side-map.
async fn clear_pending_interaction(state: &AppState, session_id: &str, request_id: &str) {
    // Clear via coordinator (updates session + broadcasts SSE)
    let ctx = state.mutation_context();
    let now = chrono::Utc::now().timestamp();
    state
        .coordinator
        .handle(
            &ctx,
            session_id,
            SessionMutation::Interaction(InteractionAction::Clear {
                request_id: request_id.to_string(),
            }),
            None, // no pid
            now,
            None, // no hook event
            None, // no cwd
            None, // no transcript_path
        )
        .await;

    // Also remove from side-map directly (coordinator SideEffect may not run
    // synchronously in all paths, so ensure immediate cleanup).
    state.interaction_data.write().await.remove(request_id);
}
