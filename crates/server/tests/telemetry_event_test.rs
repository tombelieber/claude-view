// crates/server/tests/telemetry_event_test.rs
//
// POST /api/telemetry/event is the server-side ingress for web journey
// events. It exists so (a) ad-blockers on the PostHog domain can't blind
// us and (b) the closed-enum privacy guarantee is enforced server-side: a
// path/prompt/free-form string fails to deserialize and is rejected, not
// coerced.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use claude_view_db::Database;
use tower::ServiceExt;
use wiremock::matchers::{method, path as wpath};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn count_events(mock: &MockServer, event: &str) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let reqs = mock.received_requests().await.unwrap_or_default();
        let count = reqs
            .iter()
            .filter_map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
            .filter(|b| b["event"] == event)
            .count();
        if count >= 1 || std::time::Instant::now() >= deadline {
            return count;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn post_event(body: &str) -> Request<Body> {
    Request::builder()
        .uri("/api/telemetry/event")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn page_viewed_is_forwarded_to_posthog() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wpath("/capture/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("telemetry.json");
    let capture = format!("{}/capture/", mock.uri());
    let client = claude_view_server::telemetry::TelemetryClient::with_capture_url(
        "phc_test", "anon-1", &capture,
    );
    client.set_enabled(true);
    let db = Database::new_in_memory().await.unwrap();
    let app = claude_view_server::create_app_with_telemetry_client(db, config_path, client);

    let res = app
        .clone()
        .oneshot(post_event(r#"{"event":"page_viewed","route":"search"}"#))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(count_events(&mock, "page_viewed").await, 1);
}

#[tokio::test]
async fn unknown_route_is_rejected_not_coerced() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("telemetry.json");
    let client = claude_view_server::telemetry::TelemetryClient::with_capture_url(
        "phc_test",
        "anon-2",
        "http://127.0.0.1:1/capture/",
    );
    client.set_enabled(true);
    let db = Database::new_in_memory().await.unwrap();
    let app = claude_view_server::create_app_with_telemetry_client(db, config_path, client);

    // A path masquerading as a route MUST be rejected by the type system.
    let res = app
        .oneshot(post_event(
            r#"{"event":"page_viewed","route":"/Users/secret/project"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "closed enum must reject arbitrary strings"
    );
}

#[tokio::test]
async fn disabled_telemetry_accepts_but_does_not_forward() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wpath("/capture/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("telemetry.json");
    let capture = format!("{}/capture/", mock.uri());
    let client = claude_view_server::telemetry::TelemetryClient::with_capture_url(
        "phc_test", "anon-3", &capture,
    );
    client.set_enabled(false); // opted out / source build
    let db = Database::new_in_memory().await.unwrap();
    let app = claude_view_server::create_app_with_telemetry_client(db, config_path, client);

    let res = app
        .oneshot(post_event(
            r#"{"event":"feature_action","action":"chat_message_sent"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(
        mock.received_requests().await.unwrap_or_default().len(),
        0,
        "disabled telemetry must not forward anything"
    );
}

#[tokio::test]
async fn first_feature_opened_also_emits_activation_once() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wpath("/capture/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("telemetry.json");
    let capture = format!("{}/capture/", mock.uri());
    let client = claude_view_server::telemetry::TelemetryClient::with_capture_url(
        "phc_test", "anon-4", &capture,
    );
    client.set_enabled(true);
    let db = Database::new_in_memory().await.unwrap();
    let app = claude_view_server::create_app_with_telemetry_client(db, config_path.clone(), client);

    // First feature ever opened → feature_opened + first_feature_used.
    let res = app
        .clone()
        .oneshot(post_event(
            r#"{"event":"feature_opened","feature":"live_monitor"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(count_events(&mock, "first_feature_used").await, 1);
    let cfg = claude_view_core::telemetry_config::read_telemetry_config(&config_path);
    assert_eq!(cfg.first_feature_used.as_deref(), Some("live_monitor"));
}
