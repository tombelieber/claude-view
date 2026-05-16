use axum::body::Body;
use axum::http::{Request, StatusCode};
use claude_view_db::Database;
use tower::ServiceExt;
use wiremock::matchers::{method, path as wpath};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_app_with_telemetry_path(db: Database, config_path: std::path::PathBuf) -> axum::Router {
    claude_view_server::create_app_with_telemetry_path(db, config_path)
}

#[tokio::test]
async fn consent_enable_returns_enabled_status() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = Database::new_in_memory().await.unwrap();
    let app = test_app_with_telemetry_path(db, dir.path().join("telemetry.json"));
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"enabled":true}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "enabled");
}

#[tokio::test]
async fn consent_disable_returns_disabled_status() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = Database::new_in_memory().await.unwrap();
    let app = test_app_with_telemetry_path(db, dir.path().join("telemetry.json"));
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"enabled":false}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "disabled");
}

async fn post_consent(config_path: &std::path::Path, enabled: bool) {
    let db = Database::new_in_memory().await.unwrap();
    let app = test_app_with_telemetry_path(db, config_path.to_path_buf());
    let body = format!(r#"{{"enabled":{enabled}}}"#);
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn read_config(config_path: &std::path::Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(config_path).unwrap();
    serde_json::from_str(&raw).unwrap()
}

/// `install_reported` flips to true on the first consented start and is
/// idempotent across server restarts + consent toggles — so the `installed`
/// ("acquired") event can fire at most once per persistent anonymous_id.
#[tokio::test]
async fn installed_event_fires_once_and_persists_across_restarts() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("telemetry.json");

    // First consented start (each call rebuilds the app = simulates restart).
    post_consent(&config_path, true).await;
    let after_first = read_config(&config_path);
    assert_eq!(after_first["install_reported"], true);
    let first_consent_at = after_first["consent_given_at"].clone();
    assert!(first_consent_at.is_string());

    // Toggle off, then on again — must NOT re-arm the install signal, and
    // must preserve the original first-consent timestamp.
    post_consent(&config_path, false).await;
    post_consent(&config_path, true).await;
    let after_toggle = read_config(&config_path);
    assert_eq!(after_toggle["install_reported"], true);
    assert_eq!(after_toggle["consent_given_at"], first_consent_at);
}

async fn post_consent_with_client(
    config_path: &std::path::Path,
    enabled: bool,
    telemetry: claude_view_server::telemetry::TelemetryClient,
) {
    let db = Database::new_in_memory().await.unwrap();
    let app = claude_view_server::create_app_with_telemetry_client(
        db,
        config_path.to_path_buf(),
        telemetry,
    );
    let body = format!(r#"{{"enabled":{enabled}}}"#);
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), StatusCode::OK);
}

/// Counts `installed` events POSTed to the mock, retrying because `track()`
/// is fire-and-forget on a detached task (no fixed sleep — deadline-bounded).
async fn installed_count(mock: &MockServer, distinct_id: &str) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let reqs = mock.received_requests().await.unwrap_or_default();
        let count = reqs
            .iter()
            .filter_map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
            .filter(|b| b["event"] == "installed" && b["distinct_id"] == distinct_id)
            .count();
        // Once consent_given (always emitted on first enable) has landed, any
        // `installed` from that same call has been spawned too, so the count
        // is final.
        let consent_landed = reqs
            .iter()
            .filter_map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
            .any(|b| b["event"] == "telemetry_consent_given");
        if (consent_landed && count >= 1) || std::time::Instant::now() >= deadline {
            return count;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Over-the-wire proof that `installed` is emitted to PostHog exactly once —
/// closes the gap that the persistence test can't (test apps had no telemetry
/// client, so `client.track("installed")` was never executed). Verifies the
/// real payload shape and that consent off→on never re-fires it.
#[tokio::test]
async fn installed_event_emitted_to_posthog_exactly_once() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wpath("/capture/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;

    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("telemetry.json");
    let cfg = claude_view_core::telemetry_config::TelemetryConfig::new_undecided();
    let anon = cfg.anonymous_id.clone();
    claude_view_core::telemetry_config::write_telemetry_config(&config_path, &cfg).unwrap();

    let capture = format!("{}/capture/", mock.uri());
    let client = claude_view_server::telemetry::TelemetryClient::with_capture_url(
        "phc_test", &anon, &capture,
    );
    client.set_enabled(true);

    post_consent_with_client(&config_path, true, client.clone()).await;
    post_consent_with_client(&config_path, false, client.clone()).await;
    post_consent_with_client(&config_path, true, client.clone()).await;

    assert_eq!(
        installed_count(&mock, &anon).await,
        1,
        "`installed` must hit PostHog exactly once across consent toggles"
    );

    // Payload shape: the metric is only trustworthy if it carries the
    // breakdown dimensions the dashboard segments by.
    let reqs = mock.received_requests().await.unwrap();
    let installed = reqs
        .iter()
        .filter_map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
        .find(|b| b["event"] == "installed")
        .expect("an `installed` event was POSTed");
    assert_eq!(installed["distinct_id"], anon);
    let props = &installed["properties"];
    assert!(
        props["install_source"].is_string(),
        "install_source present"
    );
    assert!(props["version"].is_string(), "version present");
    assert!(props["platform"].is_string(), "platform present");
    assert!(
        props["$set_once"]["installed_at"].is_string(),
        "installed_at stamped once on the person profile"
    );
}

#[tokio::test]
async fn consent_invalid_body_returns_422() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = Database::new_in_memory().await.unwrap();
    let app = test_app_with_telemetry_path(db, dir.path().join("telemetry.json"));
    let req = Request::builder()
        .uri("/api/telemetry/consent")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"bad":"field"}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
