//! Regression tests for telemetry. These document invariants that must hold.
use axum::body::Body;
use axum::http::{Request, StatusCode};
use claude_view_db::Database;
use serial_test::serial;
use tower::ServiceExt;

/// Invariant: /api/config always includes a telemetry field.
#[tokio::test]
async fn config_response_always_includes_telemetry_field() {
    let db = Database::new_in_memory().await.unwrap();
    let app = claude_view_server::create_app(db);
    let req = Request::builder()
        .uri("/api/config")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        json.get("telemetry").is_some(),
        "Invariant: /api/config must always include 'telemetry' field"
    );
}

/// Invariant: Self-hosted builds always report disabled.
#[tokio::test]
async fn self_hosted_builds_report_telemetry_disabled() {
    let db = Database::new_in_memory().await.unwrap();
    let app = claude_view_server::create_app(db);
    let req = Request::builder()
        .uri("/api/config")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["telemetry"], "disabled",
        "Invariant: no compiled key → telemetry disabled"
    );
    assert!(
        json["posthogKey"].is_null(),
        "Invariant: no compiled key → null posthogKey"
    );
}

/// Invariant: The consent endpoint is wired and reachable.
#[tokio::test]
async fn consent_endpoint_is_reachable() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = Database::new_in_memory().await.unwrap();
    let app =
        claude_view_server::create_app_with_telemetry_path(db, dir.path().join("telemetry.json"));
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"enabled":true}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Invariant: consent endpoint must be wired (got 404)"
    );
}

/// Invariant: Milestone dedup prevents duplicate firing.
#[test]
fn milestone_dedup_prevents_duplicate_firing() {
    use claude_view_core::telemetry_config::check_milestone;
    assert_eq!(
        check_milestone(100, 100),
        None,
        "Invariant: same milestone must not refire"
    );
    assert_eq!(
        check_milestone(150, 100),
        None,
        "Invariant: between milestones must be None"
    );
    assert_eq!(
        check_milestone(500, 100),
        Some(500),
        "next milestone should fire"
    );
}

/// Invariant: CLAUDE_VIEW_TELEMETRY=0 always overrides file-level opt-in.
#[test]
#[serial]
fn env_var_kill_switch_overrides_file_opt_in() {
    use claude_view_core::telemetry_config::*;
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    std::env::set_var("CLAUDE_VIEW_TELEMETRY", "0");
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    std::env::remove_var("CLAUDE_VIEW_TELEMETRY");
    assert_eq!(
        status,
        TelemetryStatus::Disabled,
        "Invariant: CLAUDE_VIEW_TELEMETRY=0 must override file"
    );
}
