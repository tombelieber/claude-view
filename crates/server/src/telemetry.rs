// crates/server/src/telemetry.rs
//! PostHog telemetry client for fire-and-forget HTTP capture.
//!
//! Consent is controlled by an `AtomicBool` — safe for AppState (Arc-wrapped,
//! immutable after construction). Tracking calls that arrive before `set_enabled(true)`
//! are silently dropped; no events are queued for later delivery.

use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Production PostHog capture endpoint. Injectable for tests only via
/// [`TelemetryClient::with_capture_url`]; [`TelemetryClient::new`] always
/// targets this, so production emission is byte-identical.
const POSTHOG_CAPTURE_URL: &str = "https://us.i.posthog.com/capture/";

/// Server binary version, baked in at compile time. Stamped on every
/// captured event as `app_version` so PostHog dashboards can break down
/// rollout / adoption by release without depending on a runtime config.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Inject the two server-side super-properties — `source: "server"` and
/// `app_version: <CARGO_PKG_VERSION>` — onto every captured payload.
/// Non-object inputs pass through unchanged (matches existing track()
/// behavior for malformed callers).
fn enrich_properties(mut properties: Value) -> Value {
    if let Some(obj) = properties.as_object_mut() {
        obj.insert("source".to_string(), Value::String("server".to_string()));
        obj.insert(
            "app_version".to_string(),
            Value::String(APP_VERSION.to_string()),
        );
    }
    properties
}

/// PostHog telemetry client.
///
/// Cheaply cloneable: all fields are reference-counted or `Clone`.
/// The `enabled` flag is shared across clones via `Arc<AtomicBool>`, so
/// a `set_enabled(true)` on any clone is immediately visible to all copies.
#[derive(Clone)]
pub struct TelemetryClient {
    http: reqwest::Client,
    pub(crate) api_key: String,
    pub(crate) anonymous_id: String,
    capture_url: Arc<str>,
    enabled: Arc<AtomicBool>,
}

impl TelemetryClient {
    pub fn new(api_key: &str, anonymous_id: &str) -> Self {
        Self::with_capture_url(api_key, anonymous_id, POSTHOG_CAPTURE_URL)
    }

    /// Test seam: redirect captures to a mock endpoint so emission can be
    /// asserted hermetically. Production code uses [`TelemetryClient::new`],
    /// which always targets PostHog.
    pub fn with_capture_url(api_key: &str, anonymous_id: &str, capture_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.to_string(),
            anonymous_id: anonymous_id.to_string(),
            capture_url: Arc::from(capture_url),
            enabled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn track(&self, event: &str, properties: Value) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let http = self.http.clone();
        let api_key = self.api_key.clone();
        let anonymous_id = self.anonymous_id.clone();
        let capture_url = self.capture_url.clone();
        let event = event.to_string();
        tokio::spawn(async move {
            let props = enrich_properties(properties);
            let payload = serde_json::json!({
                "api_key": api_key,
                "event": event,
                "distinct_id": anonymous_id,
                "properties": props,
            });
            let _ = http.post(capture_url.as_ref()).json(&payload).send().await;
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

    #[test]
    fn enrich_properties_adds_source_and_app_version() {
        let enriched = enrich_properties(serde_json::json!({"feature": "search"}));
        assert_eq!(enriched["source"], "server");
        assert_eq!(enriched["app_version"], APP_VERSION);
        assert_eq!(enriched["feature"], "search");
    }

    #[test]
    fn enrich_properties_overrides_caller_supplied_source_and_version() {
        let enriched = enrich_properties(
            serde_json::json!({"source": "spoofed", "app_version": "0.0.0", "kept": 1}),
        );
        assert_eq!(enriched["source"], "server");
        assert_eq!(enriched["app_version"], APP_VERSION);
        assert_eq!(enriched["kept"], 1);
    }

    #[test]
    fn enrich_properties_passes_non_object_through_unchanged() {
        assert!(enrich_properties(serde_json::json!("str")).is_string());
        assert!(enrich_properties(serde_json::json!(42)).is_number());
        assert!(enrich_properties(serde_json::json!(null)).is_null());
        assert!(enrich_properties(serde_json::json!([1, 2])).is_array());
    }

    #[test]
    fn app_version_constant_matches_cargo_pkg_version() {
        assert_eq!(APP_VERSION, env!("CARGO_PKG_VERSION"));
        assert!(!APP_VERSION.is_empty());
    }
}
