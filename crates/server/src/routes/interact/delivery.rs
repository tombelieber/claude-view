//! Interaction delivery — forward a user's decision to the sidecar and confirm receipt.
//!
//! The Rust server is the single delivery authority for the REST `/interact` path.
//! It POSTs the decision to the sidecar's `/api/sidecar/sessions/{id}/interact`
//! bridge and reads the `{ ok }` ack. The caller (`interact_handler`) clears pending
//! state ONLY on [`DeliveryOutcome::Delivered`] — never on a rejected or failed
//! delivery — so a decision the agent never received is reported honestly instead of
//! silently lost (寧願唔顯示，都唔顯示錯嘅嘢).
//!
//! Replaces the old `forward_to_sidecar`, which POSTed to a route that did not exist
//! (`/api/sessions/{control_id}/message`: wrong mount, wrong id, no such route) and
//! cleared pending state regardless of the 404 it always got.

use std::time::Duration;

use serde::Deserialize;

use crate::state::AppState;
use claude_view_types::InteractRequest;

/// Outcome of attempting to deliver an interaction decision to the sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryOutcome {
    /// Sidecar applied the decision (HTTP 2xx + `{ ok: true }`). Safe to clear pending.
    Delivered,
    /// Sidecar reached, but the decision was not applied — unknown/stale `requestId`
    /// (HTTP 2xx + `{ ok: false }`). Maps to 409; pending is NOT cleared (the SDK's
    /// own turn-end clear is authoritative).
    Rejected(String),
    /// Delivery could not be confirmed — sidecar unreachable, timed out, returned a
    /// non-2xx status, or there is no live control channel. Maps to 503; pending is
    /// NOT cleared so the user can retry.
    Failed(String),
}

#[derive(Deserialize)]
struct AckResponse {
    ok: bool,
    #[serde(default)]
    reason: Option<String>,
}

/// How long to wait for the sidecar to ack before declaring delivery failed.
/// Short by design: a dead sidecar should surface a Retry quickly, not hang the request.
const ACK_TIMEOUT: Duration = Duration::from_secs(3);

/// Deliver an interaction decision for `session_id` to the sidecar.
///
/// Performs lazy control recovery first (resuming the session in the sidecar if a
/// restart orphaned it), then POSTs the decision and interprets the ack. When there
/// is no `live_manager` (test factories) the lazy-recovery step is skipped and the
/// decision is POSTed directly.
pub async fn deliver(state: &AppState, session_id: &str, req: &InteractRequest) -> DeliveryOutcome {
    if let Some(live_manager) = state.live_manager.as_ref() {
        if let Err(e) = live_manager
            .ensure_session_control_alive(session_id, "interact")
            .await
        {
            return DeliveryOutcome::Failed(format!("control channel unavailable: {e}"));
        }
    }

    post_decision(
        &reqwest::Client::new(),
        state.sidecar.base_url(),
        session_id,
        req,
        ACK_TIMEOUT,
    )
    .await
}

/// Pure HTTP delivery: POST the decision and map the response to an outcome.
///
/// Free function (no `AppState`) so the full delivery contract is unit-testable
/// against a mock sidecar.
pub async fn post_decision(
    client: &reqwest::Client,
    sidecar_base: &str,
    session_id: &str,
    req: &InteractRequest,
    timeout: Duration,
) -> DeliveryOutcome {
    let url = format!("{sidecar_base}/api/sidecar/sessions/{session_id}/interact");
    let body = build_sidecar_message(req);

    let resp = match client.post(&url).json(&body).timeout(timeout).send().await {
        Ok(resp) => resp,
        Err(e) if e.is_timeout() => {
            return DeliveryOutcome::Failed(format!("sidecar ack timed out after {timeout:?}"));
        }
        Err(e) => return DeliveryOutcome::Failed(format!("sidecar unreachable: {e}")),
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return DeliveryOutcome::Failed(format!("sidecar returned {status}: {body}"));
    }

    match resp.json::<AckResponse>().await {
        Ok(ack) if ack.ok => DeliveryOutcome::Delivered,
        Ok(ack) => DeliveryOutcome::Rejected(
            ack.reason
                .unwrap_or_else(|| "interaction not applied".into()),
        ),
        Err(e) => DeliveryOutcome::Failed(format!("malformed sidecar ack: {e}")),
    }
}

/// Build the sidecar-compatible JSON body from an `InteractRequest`.
/// Same wire shape the sidecar's interaction-resolver parses.
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
        } => serde_json::json!({
            "type": "question_response",
            "requestId": request_id,
            "answers": answers
        }),
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
        } => serde_json::json!({
            "type": "elicitation_response",
            "requestId": request_id,
            "response": response
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, routing::post, Json, Router};

    fn permission_req() -> InteractRequest {
        InteractRequest::Permission {
            request_id: "req-1".to_string(),
            allowed: true,
            updated_permissions: None,
        }
    }

    /// Spawn a mock sidecar that answers the interact route with `(status, body)`.
    /// Returns its base URL (e.g. `http://127.0.0.1:54321`).
    async fn spawn_mock_sidecar(status: StatusCode, body: serde_json::Value) -> String {
        let app = Router::new().route(
            "/api/sidecar/sessions/{id}/interact",
            post(move || {
                let body = body.clone();
                async move { (status, Json(body)) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn delivered_when_ack_ok_true() {
        let base = spawn_mock_sidecar(StatusCode::OK, serde_json::json!({ "ok": true })).await;
        let outcome = post_decision(
            &reqwest::Client::new(),
            &base,
            "sess-1",
            &permission_req(),
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(outcome, DeliveryOutcome::Delivered);
    }

    #[tokio::test]
    async fn rejected_when_ack_ok_false() {
        let base = spawn_mock_sidecar(
            StatusCode::OK,
            serde_json::json!({ "ok": false, "reason": "Unknown permission requestId" }),
        )
        .await;
        let outcome = post_decision(
            &reqwest::Client::new(),
            &base,
            "sess-1",
            &permission_req(),
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            outcome,
            DeliveryOutcome::Rejected("Unknown permission requestId".to_string())
        );
    }

    #[tokio::test]
    async fn failed_on_non_2xx() {
        let base = spawn_mock_sidecar(
            StatusCode::INTERNAL_SERVER_ERROR,
            serde_json::json!({ "error": "boom" }),
        )
        .await;
        let outcome = post_decision(
            &reqwest::Client::new(),
            &base,
            "sess-1",
            &permission_req(),
            Duration::from_secs(2),
        )
        .await;
        assert!(matches!(outcome, DeliveryOutcome::Failed(_)));
    }

    #[tokio::test]
    async fn failed_when_sidecar_unreachable() {
        // Bind then drop a listener to obtain a definitely-closed port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let base = format!("http://{addr}");

        let outcome = post_decision(
            &reqwest::Client::new(),
            &base,
            "sess-1",
            &permission_req(),
            Duration::from_secs(2),
        )
        .await;
        assert!(matches!(outcome, DeliveryOutcome::Failed(_)));
    }
}
