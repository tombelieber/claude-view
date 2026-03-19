// crates/server/src/telemetry.rs
//! PostHog telemetry client for fire-and-forget HTTP capture.
//!
//! Consent is controlled by an `AtomicBool` — safe for AppState (Arc-wrapped,
//! immutable after construction). Tracking calls that arrive before `set_enabled(true)`
//! are silently dropped; no events are queued for later delivery.

use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct TelemetryClient {
    http: reqwest::Client,
    pub(crate) api_key: String,
    pub(crate) anonymous_id: String,
    enabled: AtomicBool,
}

impl TelemetryClient {
    pub fn new(api_key: &str, anonymous_id: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.to_string(),
            anonymous_id: anonymous_id.to_string(),
            enabled: AtomicBool::new(false),
        }
    }

    pub fn track(&self, event: &str, properties: Value) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let http = self.http.clone();
        let api_key = self.api_key.clone();
        let anonymous_id = self.anonymous_id.clone();
        let event = event.to_string();
        tokio::spawn(async move {
            let mut props = properties;
            if let Some(obj) = props.as_object_mut() {
                obj.insert("source".to_string(), Value::String("server".to_string()));
            }
            let payload = serde_json::json!({
                "api_key": api_key,
                "event": event,
                "distinct_id": anonymous_id,
                "properties": props,
            });
            let _ = http
                .post("https://us.i.posthog.com/capture/")
                .json(&payload)
                .send()
                .await;
        });
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_does_nothing_when_disabled() {
        let client = TelemetryClient::new("phc_test", "anon-id");
        client.set_enabled(false);
        client.track("test_event", serde_json::json!({}));
    }

    #[test]
    fn set_enabled_toggles_state() {
        let client = TelemetryClient::new("phc_test", "anon-id");
        assert!(!client.is_enabled());
        client.set_enabled(true);
        assert!(client.is_enabled());
        client.set_enabled(false);
        assert!(!client.is_enabled());
    }

    #[test]
    fn new_creates_client_with_correct_fields() {
        let client = TelemetryClient::new("phc_key123", "uuid-456");
        assert_eq!(client.api_key, "phc_key123");
        assert_eq!(client.anonymous_id, "uuid-456");
        assert!(!client.is_enabled());
    }

    #[test]
    fn track_with_non_object_properties_does_not_panic() {
        let client = TelemetryClient::new("phc_test", "anon-id");
        client.set_enabled(false);
        client.track("test_event", serde_json::json!("string_value"));
        client.track("test_event", serde_json::json!(42));
        client.track("test_event", serde_json::json!(null));
        client.track("test_event", serde_json::json!([1, 2, 3]));
    }
}
