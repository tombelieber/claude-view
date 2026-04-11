//! Interaction event detection for the sidecar WS relay tap.
//!
//! Inspects JSON messages flowing sidecar -> client and extracts
//! `PendingInteractionMeta` + `InteractionBlock` when the message is
//! a user-facing interaction event (permission, question, plan, elicitation).
//!
//! Pure function — no IO, no locks. Called from `sidecar_proxy::relay_websocket`.

use claude_view_types::{InteractionBlock, InteractionVariant, PendingInteractionMeta};

/// Try to extract interaction metadata from a sidecar JSON message.
///
/// Returns `Some((meta, block))` for interaction events, `None` for all others.
/// Never panics — malformed JSON or missing fields gracefully return `None`.
pub fn try_extract_interaction(text: &str) -> Option<(PendingInteractionMeta, InteractionBlock)> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let msg_type = json.get("type")?.as_str()?;
    let request_id = json.get("requestId")?.as_str()?.to_string();

    let (variant, preview) = match msg_type {
        "permission_request" => {
            let tool = json
                .get("toolName")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            (InteractionVariant::Permission, tool.to_string())
        }
        "ask_question" => {
            let preview = json
                .get("questions")
                .and_then(|q| q.as_array())
                .and_then(|arr| arr.first())
                .and_then(|q| q.get("question"))
                .and_then(|q| q.as_str())
                .unwrap_or("Question")
                .to_string();
            (InteractionVariant::Question, preview)
        }
        "plan_approval" => {
            let plan_text = json
                .get("planData")
                .and_then(|p| p.get("plan"))
                .and_then(|p| p.as_str())
                .unwrap_or("Plan approval");
            let preview = if plan_text.len() > 200 {
                // Truncate at char boundary (plan text is UTF-8)
                let mut end = 200;
                while !plan_text.is_char_boundary(end) && end > 0 {
                    end -= 1;
                }
                plan_text[..end].to_string()
            } else {
                plan_text.to_string()
            };
            (InteractionVariant::Plan, preview)
        }
        "elicitation" => {
            let prompt = json
                .get("prompt")
                .and_then(|p| p.as_str())
                .unwrap_or("Elicitation");
            (InteractionVariant::Elicitation, prompt.to_string())
        }
        _ => return None,
    };

    let meta = PendingInteractionMeta {
        variant,
        request_id: request_id.clone(),
        preview: preview.clone(),
    };

    let block = InteractionBlock {
        id: format!("interaction-{request_id}"),
        variant,
        request_id: Some(request_id),
        resolved: false,
        historical_source: None,
        data: json,
    };

    Some((meta, block))
}

/// Check if a sidecar message signals end-of-turn (clears pending interactions).
pub fn is_turn_end(text: &str) -> bool {
    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    matches!(
        json.get("type").and_then(|t| t.as_str()),
        Some("turn_complete" | "turn_error")
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── permission_request ────────────────────────────────────────────

    #[test]
    fn extracts_permission_request() {
        let msg = json!({
            "type": "permission_request",
            "requestId": "req-001",
            "toolName": "Bash",
            "command": "rm -rf /tmp/test"
        });
        let (meta, block) = try_extract_interaction(&msg.to_string()).unwrap();

        assert!(matches!(meta.variant, InteractionVariant::Permission));
        assert_eq!(meta.request_id, "req-001");
        assert_eq!(meta.preview, "Bash");

        assert_eq!(block.id, "interaction-req-001");
        assert!(matches!(block.variant, InteractionVariant::Permission));
        assert_eq!(block.request_id.as_deref(), Some("req-001"));
        assert!(!block.resolved);
        assert!(block.historical_source.is_none());
    }

    #[test]
    fn permission_request_unknown_tool_fallback() {
        let msg = json!({
            "type": "permission_request",
            "requestId": "req-002"
        });
        let (meta, _) = try_extract_interaction(&msg.to_string()).unwrap();
        assert_eq!(meta.preview, "unknown");
    }

    // ── ask_question ─────────────────────────────────────────────────

    #[test]
    fn extracts_ask_question() {
        let msg = json!({
            "type": "ask_question",
            "requestId": "req-010",
            "questions": [
                { "question": "What should I name the file?" },
                { "question": "Where to put it?" }
            ]
        });
        let (meta, block) = try_extract_interaction(&msg.to_string()).unwrap();

        assert!(matches!(meta.variant, InteractionVariant::Question));
        assert_eq!(meta.request_id, "req-010");
        assert_eq!(meta.preview, "What should I name the file?");
        assert!(matches!(block.variant, InteractionVariant::Question));
    }

    #[test]
    fn ask_question_empty_questions_fallback() {
        let msg = json!({
            "type": "ask_question",
            "requestId": "req-011",
            "questions": []
        });
        let (meta, _) = try_extract_interaction(&msg.to_string()).unwrap();
        assert_eq!(meta.preview, "Question");
    }

    #[test]
    fn ask_question_missing_questions_fallback() {
        let msg = json!({
            "type": "ask_question",
            "requestId": "req-012"
        });
        let (meta, _) = try_extract_interaction(&msg.to_string()).unwrap();
        assert_eq!(meta.preview, "Question");
    }

    // ── plan_approval ────────────────────────────────────────────────

    #[test]
    fn extracts_plan_approval() {
        let msg = json!({
            "type": "plan_approval",
            "requestId": "req-020",
            "planData": {
                "plan": "Step 1: Create files\nStep 2: Write tests"
            }
        });
        let (meta, block) = try_extract_interaction(&msg.to_string()).unwrap();

        assert!(matches!(meta.variant, InteractionVariant::Plan));
        assert_eq!(meta.request_id, "req-020");
        assert_eq!(meta.preview, "Step 1: Create files\nStep 2: Write tests");
        assert!(matches!(block.variant, InteractionVariant::Plan));
    }

    #[test]
    fn plan_approval_truncates_to_200_chars() {
        let long_plan = "a".repeat(300);
        let msg = json!({
            "type": "plan_approval",
            "requestId": "req-021",
            "planData": { "plan": long_plan }
        });
        let (meta, _) = try_extract_interaction(&msg.to_string()).unwrap();
        assert_eq!(meta.preview.len(), 200);
    }

    #[test]
    fn plan_approval_missing_plan_data_fallback() {
        let msg = json!({
            "type": "plan_approval",
            "requestId": "req-022"
        });
        let (meta, _) = try_extract_interaction(&msg.to_string()).unwrap();
        assert_eq!(meta.preview, "Plan approval");
    }

    // ── elicitation ──────────────────────────────────────────────────

    #[test]
    fn extracts_elicitation() {
        let msg = json!({
            "type": "elicitation",
            "requestId": "req-030",
            "prompt": "Enter your API key:"
        });
        let (meta, block) = try_extract_interaction(&msg.to_string()).unwrap();

        assert!(matches!(meta.variant, InteractionVariant::Elicitation));
        assert_eq!(meta.request_id, "req-030");
        assert_eq!(meta.preview, "Enter your API key:");
        assert!(matches!(block.variant, InteractionVariant::Elicitation));
    }

    #[test]
    fn elicitation_missing_prompt_fallback() {
        let msg = json!({
            "type": "elicitation",
            "requestId": "req-031"
        });
        let (meta, _) = try_extract_interaction(&msg.to_string()).unwrap();
        assert_eq!(meta.preview, "Elicitation");
    }

    // ── non-interaction messages ──────────────────────────────────────

    #[test]
    fn returns_none_for_non_interaction_type() {
        let msg = json!({
            "type": "assistant_text",
            "requestId": "req-099",
            "text": "Hello world"
        });
        assert!(try_extract_interaction(&msg.to_string()).is_none());
    }

    #[test]
    fn returns_none_for_malformed_json() {
        assert!(try_extract_interaction("not json at all {{{").is_none());
    }

    #[test]
    fn returns_none_for_missing_request_id() {
        let msg = json!({
            "type": "permission_request",
            "toolName": "Bash"
        });
        assert!(try_extract_interaction(&msg.to_string()).is_none());
    }

    #[test]
    fn returns_none_for_missing_type() {
        let msg = json!({
            "requestId": "req-100",
            "toolName": "Bash"
        });
        assert!(try_extract_interaction(&msg.to_string()).is_none());
    }

    // ── turn end detection ───────────────────────────────────────────

    #[test]
    fn detects_turn_complete() {
        let msg = json!({ "type": "turn_complete" });
        assert!(is_turn_end(&msg.to_string()));
    }

    #[test]
    fn detects_turn_error() {
        let msg = json!({ "type": "turn_error" });
        assert!(is_turn_end(&msg.to_string()));
    }

    #[test]
    fn non_turn_end_returns_false() {
        let msg = json!({ "type": "assistant_text" });
        assert!(!is_turn_end(&msg.to_string()));
    }

    #[test]
    fn malformed_json_is_not_turn_end() {
        assert!(!is_turn_end("not json"));
    }

    // ── block stores full payload ────────────────────────────────────

    #[test]
    fn block_data_contains_full_original_payload() {
        let msg = json!({
            "type": "permission_request",
            "requestId": "req-040",
            "toolName": "Write",
            "filePath": "/tmp/test.txt",
            "extra_field": 42
        });
        let (_, block) = try_extract_interaction(&msg.to_string()).unwrap();
        assert_eq!(block.data["extra_field"], 42);
        assert_eq!(block.data["filePath"], "/tmp/test.txt");
    }
}
