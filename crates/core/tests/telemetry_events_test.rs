// crates/core/tests/telemetry_events_test.rs
//
// The closed event taxonomy IS the privacy guarantee: a caller can only
// emit a fixed enum variant, never a free-form string, so a file path /
// prompt / project name is structurally unrepresentable in a payload.
// These tests pin the stable wire strings (PostHog dashboards depend on
// them) and the round-trip.

use claude_view_core::telemetry_events::{
    ActionId, Surface, EVENT_APP_ACTIVE, EVENT_FEATURE_ACTION, EVENT_FEATURE_OPENED,
    EVENT_FIRST_FEATURE_USED, EVENT_SCALE_MILESTONE, EVENT_SERVER_STARTED,
};

#[test]
fn surface_serializes_to_stable_snake_case() {
    assert_eq!(
        serde_json::to_string(&Surface::LiveMonitor).unwrap(),
        "\"live_monitor\""
    );
    assert_eq!(
        serde_json::to_string(&Surface::SessionDetail).unwrap(),
        "\"session_detail\""
    );
    assert_eq!(
        serde_json::to_string(&Surface::SystemMonitor).unwrap(),
        "\"system_monitor\""
    );
}

#[test]
fn surface_roundtrips_all_variants() {
    for s in [
        Surface::LiveMonitor,
        Surface::Chat,
        Surface::History,
        Surface::SessionDetail,
        Surface::Search,
        Surface::Analytics,
        Surface::Activity,
        Surface::Reports,
        Surface::Prompts,
        Surface::Teams,
        Surface::Workflows,
        Surface::Plugins,
        Surface::Memory,
        Surface::SystemMonitor,
        Surface::Insights,
        Surface::Settings,
    ] {
        let json = serde_json::to_string(&s).unwrap();
        let back: Surface = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}

#[test]
fn arbitrary_string_is_rejected_not_coerced() {
    // A payload claiming an out-of-allowlist surface/action must FAIL to
    // deserialize — the server rejects it rather than inventing data.
    let r: Result<Surface, _> = serde_json::from_str("\"/Users/secret/path\"");
    assert!(
        r.is_err(),
        "arbitrary string must not deserialize to a Surface"
    );
    let r2: Result<ActionId, _> = serde_json::from_str("\"rm_-rf\"");
    assert!(r2.is_err());
}

#[test]
fn action_ids_are_snake_case() {
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
    assert_eq!(EVENT_FEATURE_ACTION, "feature_action");
}
