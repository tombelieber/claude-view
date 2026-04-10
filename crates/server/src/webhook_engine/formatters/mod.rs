//! Webhook payload formatters.
//!
//! Provides the canonical `WebhookPayload` type (Stripe convention),
//! `build_payload` to construct one from a `LiveSession`, and
//! `format_payload` to serialize it for a specific target platform.

pub mod raw;

use crate::webhook_engine::config::{WebhookEventType, WebhookFormat};
use claude_view_server_live_state::core::LiveSession;
use serde::Serialize;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Canonical webhook event payload (Stripe convention).
#[derive(Debug, Clone, Serialize)]
pub struct WebhookPayload {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub timestamp: i64,
    pub data: WebhookEventData,
}

/// Cherry-picked session data for webhook payloads.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEventData {
    pub session_id: String,
    pub project: String,
    pub project_path: String,
    pub model: Option<String>,
    pub model_display_name: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    pub entrypoint: Option<String>,
    pub session_kind: Option<String>,
    pub agent_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// Build a canonical payload from a LiveSession + event type.
pub fn build_payload(
    event_type: &WebhookEventType,
    session: &LiveSession,
    base_url: Option<&str>,
) -> WebhookPayload {
    let now = chrono::Utc::now().timestamp();
    let raw_uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
    let event_id = format!("evt_{}", &raw_uuid[..16]);

    let duration_secs = match (session.started_at, session.closed_at) {
        (Some(start), Some(end)) => Some(end - start),
        _ => None,
    };

    let web_url =
        base_url.map(|base| format!("{}/sessions/{}", base.trim_end_matches('/'), session.id));

    let type_str = match event_type {
        WebhookEventType::SessionStarted => "session.started",
        WebhookEventType::SessionEnded => "session.ended",
        WebhookEventType::SessionError => "session.error",
        WebhookEventType::SessionUpdated => "session.updated",
    };

    WebhookPayload {
        id: event_id,
        event_type: type_str.to_string(),
        timestamp: now,
        data: WebhookEventData {
            session_id: session.id.clone(),
            project: session.jsonl.project_display_name.clone(),
            project_path: session.jsonl.project_path.clone(),
            model: session.model.clone(),
            model_display_name: session.model_display_name.clone(),
            status: format!("{:?}", session.status),
            duration_secs,
            cost_usd: {
                let total = session.jsonl.cost.total_usd;
                if total > 0.0 {
                    Some(total)
                } else {
                    None
                }
            },
            turn_count: if session.hook.turn_count > 0 {
                Some(session.hook.turn_count as u32)
            } else {
                None
            },
            edit_count: if session.jsonl.edit_count > 0 {
                Some(session.jsonl.edit_count as u32)
            } else {
                None
            },
            git_branch: session.jsonl.git_branch.clone(),
            entrypoint: session.entrypoint.clone(),
            session_kind: session.session_kind.clone(),
            agent_state: session.hook.agent_state.label.clone(),
            error: session.hook.last_error.clone(),
            error_details: session.hook.last_error_details.clone(),
            web_url,
        },
    }
}

/// Format a canonical payload for the target platform.
pub fn format_payload(payload: &WebhookPayload, format: &WebhookFormat) -> serde_json::Value {
    match format {
        WebhookFormat::Raw => raw::format(payload),
        WebhookFormat::Lark => serde_json::to_value(payload).unwrap(), // placeholder until Task 7
        WebhookFormat::Slack => serde_json::to_value(payload).unwrap(), // placeholder until Task 7
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_server_live_state::core::test_live_session;

    #[test]
    fn build_payload_session_started() {
        let session = test_live_session("sess-1");
        let payload = build_payload(&WebhookEventType::SessionStarted, &session, None);
        assert_eq!(payload.event_type, "session.started");
        assert!(payload.id.starts_with("evt_"));
        assert_eq!(payload.data.session_id, "sess-1");
    }

    #[test]
    fn build_payload_session_ended_includes_duration() {
        let mut session = test_live_session("sess-2");
        session.started_at = Some(1000);
        session.closed_at = Some(1252); // 252 seconds
        let payload = build_payload(&WebhookEventType::SessionEnded, &session, None);
        assert_eq!(payload.event_type, "session.ended");
        assert_eq!(payload.data.duration_secs, Some(252));
    }

    #[test]
    fn build_payload_no_duration_without_closed_at() {
        let session = test_live_session("sess-3");
        let payload = build_payload(&WebhookEventType::SessionStarted, &session, None);
        assert!(payload.data.duration_secs.is_none());
    }

    #[test]
    fn build_payload_web_url_from_base() {
        let session = test_live_session("sess-4");
        let payload = build_payload(
            &WebhookEventType::SessionStarted,
            &session,
            Some("https://example.com"),
        );
        assert_eq!(
            payload.data.web_url,
            Some("https://example.com/sessions/sess-4".to_string())
        );
    }

    #[test]
    fn build_payload_no_web_url_without_base() {
        let session = test_live_session("sess-5");
        let payload = build_payload(&WebhookEventType::SessionStarted, &session, None);
        assert!(payload.data.web_url.is_none());
    }

    #[test]
    fn build_payload_skips_zero_cost() {
        let session = test_live_session("sess-6");
        let payload = build_payload(&WebhookEventType::SessionUpdated, &session, None);
        assert!(payload.data.cost_usd.is_none()); // 0.0 → None
    }

    #[test]
    fn raw_format_matches_serde() {
        let session = test_live_session("sess-7");
        let payload = build_payload(&WebhookEventType::SessionStarted, &session, None);
        let raw = raw::format(&payload);
        let direct = serde_json::to_value(&payload).unwrap();
        assert_eq!(raw, direct);
    }
}
