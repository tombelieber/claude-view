//! Raw JSON formatter — outputs the canonical payload as-is.

use super::WebhookPayload;

pub fn format(payload: &WebhookPayload) -> serde_json::Value {
    serde_json::to_value(payload).unwrap()
}
