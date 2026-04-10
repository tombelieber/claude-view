//! Slack Block Kit formatter for webhook payloads.

use super::WebhookPayload;
use serde_json::{json, Value};

/// Format a webhook payload as Slack Block Kit blocks.
pub fn format(payload: &WebhookPayload) -> Value {
    let title = event_title(&payload.event_type);
    let mut blocks: Vec<Value> = Vec::new();

    // Header
    blocks.push(json!({
        "type": "header",
        "text": { "type": "plain_text", "text": title }
    }));

    // Session info section
    let mut fields: Vec<Value> = Vec::new();
    fields.push(mrkdwn(&format!("*Project:* {}", payload.data.project)));

    if let Some(model) = payload
        .data
        .model_display_name
        .as_ref()
        .or(payload.data.model.as_ref())
    {
        fields.push(mrkdwn(&format!("*Model:* {model}")));
    }

    fields.push(mrkdwn(&format!("*Status:* {}", payload.data.agent_state)));

    if let Some(secs) = payload.data.duration_secs {
        fields.push(mrkdwn(&format!("*Duration:* {}", format_duration(secs))));
    }
    if let Some(cost) = payload.data.cost_usd {
        fields.push(mrkdwn(&format!("*Cost:* ${:.2}", cost)));
    }
    if let Some(turns) = payload.data.turn_count {
        fields.push(mrkdwn(&format!("*Turns:* {turns}")));
    }
    if let Some(ref branch) = payload.data.git_branch {
        fields.push(mrkdwn(&format!("*Branch:* {branch}")));
    }

    blocks.push(json!({
        "type": "section",
        "fields": fields
    }));

    // Error section
    if let Some(ref error) = payload.data.error {
        blocks.push(json!({ "type": "divider" }));
        let mut error_text = format!("⚠️ *Error:* {error}");
        if let Some(ref details) = payload.data.error_details {
            error_text.push_str(&format!("\n```{details}```"));
        }
        blocks.push(json!({
            "type": "section",
            "text": { "type": "mrkdwn", "text": error_text }
        }));
    }

    // Action button
    if let Some(ref url) = payload.data.web_url {
        blocks.push(json!({
            "type": "actions",
            "elements": [{
                "type": "button",
                "text": { "type": "plain_text", "text": "View Session" },
                "url": url
            }]
        }));
    }

    json!({ "blocks": blocks })
}

fn event_title(event_type: &str) -> String {
    match event_type {
        "session.started" => "Session Started".into(),
        "session.ended" => "Session Ended".into(),
        "session.error" => "Session Error".into(),
        "session.updated" => "Session Updated".into(),
        other => other.to_string(),
    }
}

fn mrkdwn(text: &str) -> Value {
    json!({ "type": "mrkdwn", "text": text })
}

fn format_duration(secs: i64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webhook_engine::formatters::{WebhookEventData, WebhookPayload};

    fn test_payload(event_type: &str) -> WebhookPayload {
        WebhookPayload {
            id: "evt_test".into(),
            event_type: event_type.into(),
            timestamp: 1000,
            data: WebhookEventData {
                session_id: "sess-1".into(),
                project: "claude-view".into(),
                project_path: "/tmp/test".into(),
                model: Some("claude-sonnet-4-6".into()),
                model_display_name: Some("Sonnet".into()),
                status: "Working".into(),
                duration_secs: None,
                cost_usd: None,
                turn_count: None,
                edit_count: None,
                git_branch: None,
                entrypoint: None,
                session_kind: None,
                agent_state: "Working".into(),
                error: None,
                error_details: None,
                web_url: None,
            },
        }
    }

    #[test]
    fn started_event_has_header() {
        let payload = test_payload("session.started");
        let result = format(&payload);
        let blocks = result["blocks"].as_array().unwrap();
        let header = &blocks[0];
        assert_eq!(header["type"], "header");
        assert_eq!(header["text"]["text"], "Session Started");
    }

    #[test]
    fn ended_event_includes_duration_and_cost() {
        let mut payload = test_payload("session.ended");
        payload.data.duration_secs = Some(252);
        payload.data.cost_usd = Some(0.35);
        let result = format(&payload);
        let json_str = serde_json::to_string(&result).unwrap();
        assert!(json_str.contains("4m 12s"));
        assert!(json_str.contains("$0.35"));
    }

    #[test]
    fn error_event_shows_error_section() {
        let mut payload = test_payload("session.error");
        payload.data.error = Some("API rate limit".into());
        let result = format(&payload);
        let json_str = serde_json::to_string(&result).unwrap();
        assert!(json_str.contains("API rate limit"));
    }

    #[test]
    fn view_button_has_url() {
        let mut payload = test_payload("session.started");
        payload.data.web_url = Some("https://example.com/sessions/sess-1".into());
        let result = format(&payload);
        let json_str = serde_json::to_string(&result).unwrap();
        assert!(json_str.contains("View Session"));
        assert!(json_str.contains("https://example.com/sessions/sess-1"));
    }

    #[test]
    fn missing_fields_not_in_output() {
        let payload = test_payload("session.started");
        let result = format(&payload);
        let json_str = serde_json::to_string(&result).unwrap();
        assert!(!json_str.contains("Duration"));
        assert!(!json_str.contains("Cost"));
        assert!(!json_str.contains("Error"));
    }

    #[test]
    fn has_blocks_key() {
        let payload = test_payload("session.started");
        let result = format(&payload);
        assert!(result["blocks"].is_array());
    }
}
