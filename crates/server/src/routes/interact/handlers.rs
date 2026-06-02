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
use crate::routes::interact::delivery::{self, DeliveryOutcome};
use crate::state::AppState;
use claude_view_types::{InteractRequest, InteractionBlock};

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
///
/// **Phase 3 PR 3.7 — no cutover.** Interaction data is live-only state
/// (pending-interaction + interaction_data side-map) maintained by the
/// live session manager. It is not derivable from JSONL or
/// `session_stats` — those reflect the historical record, while this
/// endpoint serves in-flight permission/question/plan prompts. The
/// 2026-04-17 design doc §5.1 contemplated using
/// `session_stats.last_ts` for cache validation, but the current
/// handler has no cache surface that would benefit. Response shape is
/// pinned by the openapi_compatibility test.
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
/// 1. Look up session in live_sessions — 404 if not found
/// 2. Require an SDK control channel — 400 if observed (read-only mirror) or
///    tmux-only (a CLI session is driven by forking, never by writing into it)
/// 3. Check interaction_data side-map — 409 if request_id not found (stale)
/// 4. Deliver to the sidecar and await its ack, then:
///    - Delivered  → clear pending + 200
///    - Rejected   → 409, pending retained (SDK turn-end is authoritative)
///    - Failed     → 503, pending retained for retry
///
/// Pending state is cleared ONLY on confirmed delivery: a decision the agent
/// never received must never be reported as resolved (寧願唔顯示，都唔顯示錯嘅嘢).
#[utoipa::path(
    post,
    path = "/api/sessions/{session_id}/interact",
    tag = "sessions",
    params(("session_id" = String, Path, description = "Session UUID")),
    responses(
        (status = 200, description = "Interaction delivered and resolved"),
        (status = 400, description = "Observed/tmux-only session — no SDK control channel"),
        (status = 404, description = "Session not found"),
        (status = 409, description = "Request ID stale, or sidecar reported it already resolved"),
        (status = 503, description = "Delivery to the controlling sidecar could not be confirmed"),
    )
)]
pub async fn interact_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(req): Json<InteractRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let request_id = extract_request_id(&req).to_string();

    // Step 1: session exists? Clone and drop lock before async work.
    let session_clone = {
        let sessions = state.live_sessions.read().await;
        sessions
            .get(&session_id)
            .ok_or_else(|| ApiError::NotFound(format!("Session not found: {session_id}")))?
            .clone()
    };
    let ownership = crate::live::ownership::compute_ownership(&session_clone);

    // Step 2: require an SDK control channel. Observed sessions are read-only
    // mirrors of the CLI; a tmux-owned CLI session is driven by *forking* it
    // (take-over), never by injecting a response into the CLI's own lineage.
    if ownership.sdk.is_none() {
        return Err(ApiError::BadRequest(
            "Cannot interact with this session — no SDK control channel (take over to drive it)"
                .to_string(),
        ));
    }

    // Step 3: side-map must still hold this request_id (else stale / already resolved).
    {
        let data = state.interaction_data.read().await;
        if !data.contains_key(&request_id) {
            return Err(ApiError::Conflict(format!(
                "Interaction request_id not found (stale or already resolved): {request_id}"
            )));
        }
    }

    // Step 4: deliver to the controlling sidecar and confirm receipt.
    match delivery::deliver(&state, &session_id, &req).await {
        DeliveryOutcome::Delivered => {
            clear_pending_interaction(&state, &session_id, &request_id).await;
            Ok(Json(serde_json::json!({
                "resolved": true,
                "requestId": request_id,
                "sessionId": session_id,
            })))
        }
        DeliveryOutcome::Rejected(reason) => {
            // Sidecar reached, but the decision didn't apply (unknown/stale id).
            // The SDK's own turn-end clear is authoritative — don't clear here.
            Err(ApiError::Conflict(format!(
                "Interaction already resolved or unknown: {reason}"
            )))
        }
        DeliveryOutcome::Failed(reason) => {
            // Delivery unconfirmed — keep pending so the user can retry. The
            // decision is NOT silently lost or falsely reported as resolved.
            tracing::warn!(
                session_id,
                request_id = %request_id,
                reason = %reason,
                "Interaction delivery failed — pending retained for retry"
            );
            Err(ApiError::ServiceUnavailable(format!(
                "Could not deliver decision to the controlling agent: {reason}"
            )))
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
            None,
        )
        .await;

    // Also remove from side-map directly (coordinator SideEffect may not run
    // synchronously in all paths, so ensure immediate cleanup).
    state.interaction_data.write().await.remove(request_id);
}
