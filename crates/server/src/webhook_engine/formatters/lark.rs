//! Lark interactive card formatter for webhook payloads.

use super::WebhookPayload;
use serde_json::{json, Value};

/// Format a webhook payload as a Lark interactive card.
pub fn format(payload: &WebhookPayload) -> Value {
    let title = event_title(&payload.event_type);
    let mut elements: Vec<Value> = Vec::new();

    // Project info
    elements.push(div(&format!("**Project:** {}", payload.data.project)));

    // Model (if present)
    if let Some(model) = payload
        .data
        .model_display_name
        .as_ref()
        .or(payload.data.model.as_ref())
    {
        elements.push(div(&format!("**Model:** {model}")));
    }

    // Status/state
    elements.push(div(&format!("**Status:** {}", payload.data.agent_state)));

    // Duration + cost line (for ended sessions)
    let mut metrics = Vec::new();
    if let Some(secs) = payload.data.duration_secs {
        metrics.push(format!("**Duration:** {}", format_duration(secs)));
    }
    if let Some(cost) = payload.data.cost_usd {
        metrics.push(format!("**Cost:** ${:.2}", cost));
    }
    if let Some(turns) = payload.data.turn_count {
        metrics.push(format!("**Turns:** {turns}"));
    }
    if !metrics.is_empty() {
        elements.push(div(&metrics.join("  |  ")));
    }

    // Error info
    if let Some(ref error) = payload.data.error {
        elements.push(json!({ "tag": "hr" }));
        elements.push(div(&format!("⚠️ **Error:** {error}")));
        if let Some(ref details) = payload.data.error_details {
            elements.push(div(&format!("```\n{details}\n```")));
        }
    }

    // Git branch
    if let Some(ref branch) = payload.data.git_branch {
        elements.push(div(&format!("**Branch:** {branch}")));
    }

    // View button
    if let Some(ref url) = payload.data.web_url {
        elements.push(json!({ "tag": "hr" }));
        elements.push(json!({
            "tag": "action",
            "actions": [{
                "tag": "button",
                "text": { "content": "View Session", "tag": "plain_text" },
                "type": "primary",
                "url": url
            }]
        }));
    }

    json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "content": title, "tag": "plain_text" }
            },
            "elements": elements
        }
    })
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

fn div(content: &str) -> Value {
    json!({
        "tag": "div",
        "text": { "content": content, "tag": "lark_md" }
    })
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
    fn started_event_has_correct_title() {
        let payload = test_payload("session.started");
        let card = format(&payload);
        let title = card["card"]["header"]["title"]["content"].as_str().unwrap();
        assert_eq!(title, "Session Started");
    }

    #[test]
    fn ended_event_includes_duration_and_cost() {
        let mut payload = test_payload("session.ended");
        payload.data.duration_secs = Some(252);
        payload.data.cost_usd = Some(0.35);
        let card = format(&payload);
        let elements = card["card"]["elements"].as_array().unwrap();
        let texts: Vec<String> = elements
            .iter()
            .filter_map(|e| e["text"]["content"].as_str())
            .map(String::from)
            .collect();
        let metrics_line = texts.iter().find(|t| t.contains("Duration")).unwrap();
        assert!(metrics_line.contains("4m 12s"), "got: {metrics_line}");
        assert!(metrics_line.contains("$0.35"), "got: {metrics_line}");
    }

    #[test]
    fn error_event_shows_error() {
        let mut payload = test_payload("session.error");
        payload.data.error = Some("API rate limit".into());
        let card = format(&payload);
        let json_str = serde_json::to_string(&card).unwrap();
        assert!(json_str.contains("API rate limit"));
    }

    #[test]
    fn view_button_has_correct_url() {
        let mut payload = test_payload("session.started");
        payload.data.web_url = Some("https://example.com/sessions/sess-1".into());
        let card = format(&payload);
        let json_str = serde_json::to_string(&card).unwrap();
        assert!(json_str.contains("https://example.com/sessions/sess-1"));
        assert!(json_str.contains("View Session"));
    }

    #[test]
    fn missing_optional_fields_dont_appear() {
        let payload = test_payload("session.started");
        let card = format(&payload);
        let json_str = serde_json::to_string(&card).unwrap();
        assert!(!json_str.contains("Duration"));
        assert!(!json_str.contains("Cost"));
        assert!(!json_str.contains("Error"));
    }

    #[test]
    fn msg_type_is_interactive() {
        let payload = test_payload("session.started");
        let card = format(&payload);
        assert_eq!(card["msg_type"].as_str().unwrap(), "interactive");
    }
}
