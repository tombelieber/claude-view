// crates/core/tests/telemetry_events_test.rs
//
// The closed event taxonomy IS the privacy guarantee: a caller can only
// emit a fixed enum variant, never a free-form string, so a file path /
// prompt / project name is structurally unrepresentable in a payload.
// These tests pin the stable wire strings (PostHog dashboards depend on
// them) and the round-trip.

use claude_view_core::telemetry_events::{
    ActionId, FeatureId, RouteId, EVENT_APP_ACTIVE, EVENT_FEATURE_ACTION, EVENT_FEATURE_OPENED,
    EVENT_FIRST_FEATURE_USED, EVENT_PAGE_VIEWED, EVENT_SCALE_MILESTONE, EVENT_SERVER_STARTED,
};

#[test]
fn feature_id_serializes_to_stable_snake_case() {
    assert_eq!(
        serde_json::to_string(&FeatureId::LiveMonitor).unwrap(),
        "\"live_monitor\""
    );
    assert_eq!(
        serde_json::to_string(&FeatureId::OnDeviceAi).unwrap(),
        "\"on_device_ai\""
    );
    assert_eq!(
        serde_json::to_string(&FeatureId::OpenInIde).unwrap(),
        "\"open_in_ide\""
    );
}

#[test]
fn feature_id_roundtrips() {
    for f in [
        FeatureId::LiveMonitor,
        FeatureId::Chat,
        FeatureId::Search,
        FeatureId::Analytics,
        FeatureId::AgentInternals,
        FeatureId::Plans,
        FeatureId::Prompts,
        FeatureId::Teams,
        FeatureId::SystemMonitor,
        FeatureId::OnDeviceAi,
        FeatureId::Workflows,
        FeatureId::OpenInIde,
        FeatureId::Share,
        FeatureId::Settings,
    ] {
        let s = serde_json::to_string(&f).unwrap();
        let back: FeatureId = serde_json::from_str(&s).unwrap();
        assert_eq!(back, f);
    }
}

#[test]
fn unknown_variant_is_rejected_not_coerced() {
    // A payload claiming an out-of-allowlist feature must FAIL to
    // deserialize — the server rejects it rather than inventing data.
    let r: Result<FeatureId, _> = serde_json::from_str("\"/Users/secret/path\"");
    assert!(
        r.is_err(),
        "arbitrary string must not deserialize to a FeatureId"
    );
    let r2: Result<ActionId, _> = serde_json::from_str("\"rm_-rf\"");
    assert!(r2.is_err());
}

#[test]
fn route_and_action_ids_are_snake_case() {
    assert_eq!(
        serde_json::to_string(&RouteId::SessionDetail).unwrap(),
        "\"session_detail\""
    );
    assert_eq!(
        serde_json::to_string(&ActionId::ChatMessageSent).unwrap(),
        "\"chat_message_sent\""
    );
    assert_eq!(
        serde_json::to_string(&ActionId::ShareLinkCreated).unwrap(),
        "\"share_link_created\""
    );
}

#[test]
fn event_name_constants_are_stable() {
    assert_eq!(EVENT_SERVER_STARTED, "server_started");
    assert_eq!(EVENT_APP_ACTIVE, "app_active");
    assert_eq!(EVENT_FIRST_FEATURE_USED, "first_feature_used");
    assert_eq!(EVENT_FEATURE_OPENED, "feature_opened");
    assert_eq!(EVENT_SCALE_MILESTONE, "scale_milestone");
    assert_eq!(EVENT_PAGE_VIEWED, "page_viewed");
    assert_eq!(EVENT_FEATURE_ACTION, "feature_action");
}
