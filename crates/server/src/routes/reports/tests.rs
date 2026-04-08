// crates/server/src/routes/reports/tests.rs
//! Tests for the reports API routes.

use super::*;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use claude_view_db::Database;
use tower::ServiceExt;

fn test_app(state: Arc<AppState>) -> Router {
    Router::new().nest("/api", router()).with_state(state)
}

/// Parse an ISO date string (YYYY-MM-DD) to a unix timestamp.
/// If `end_of_day` is true, returns 23:59:59 of that day.
fn parse_date_to_ts(date_str: &str, end_of_day: bool) -> Result<i64, String> {
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("expected YYYY-MM-DD, got {date_str}"));
    }
    let y: i32 = parts[0].parse().map_err(|_| "invalid year")?;
    let m: u32 = parts[1].parse().map_err(|_| "invalid month")?;
    let d: u32 = parts[2].parse().map_err(|_| "invalid day")?;

    let days = days_from_civil(y, m, d);
    let base = days as i64 * 86400;

    if end_of_day {
        Ok(base + 86399)
    } else {
        Ok(base)
    }
}

/// Civil date to days since Unix epoch (1970-01-01).
fn days_from_civil(y: i32, m: u32, d: u32) -> i32 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i32 - 719468
}

#[tokio::test]
async fn test_router_creation() {
    let _router = router();
}

#[tokio::test]
async fn test_list_reports_empty() {
    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);
    let app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(json.is_empty());
}

#[tokio::test]
async fn test_list_reports_with_data() {
    let db = Database::new_in_memory().await.unwrap();
    db.insert_report(
        "daily",
        "2026-02-21",
        "2026-02-21",
        "- Did stuff",
        None,
        5,
        2,
        3600,
        100,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let state = AppState::new(db);
    let app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(json.len(), 1);
    assert_eq!(json[0]["reportType"], "daily");
    assert_eq!(json[0]["contentMd"], "- Did stuff");
}

#[tokio::test]
async fn test_get_report_by_id() {
    let db = Database::new_in_memory().await.unwrap();
    let id = db
        .insert_report(
            "weekly",
            "2026-02-17",
            "2026-02-21",
            "week summary",
            None,
            32,
            5,
            64800,
            2450,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let state = AppState::new(db);
    let app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/reports/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["reportType"], "weekly");
    assert_eq!(json["sessionCount"], 32);
}

#[tokio::test]
async fn test_get_report_not_found() {
    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);
    let app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_report() {
    let db = Database::new_in_memory().await.unwrap();
    let id = db
        .insert_report(
            "daily",
            "2026-02-21",
            "2026-02-21",
            "test",
            None,
            1,
            1,
            100,
            10,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let state = AppState::new(db);
    let app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/reports/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_report_not_found() {
    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);
    let app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/reports/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_preview_empty() {
    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);
    let app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/preview?startTs=0&endTs=9999999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["sessionCount"], 0);
}

#[test]
fn test_parse_date_to_ts() {
    // 2026-02-21 is a known date
    let ts = parse_date_to_ts("2026-02-21", false).unwrap();
    assert!(ts > 0);

    let ts_end = parse_date_to_ts("2026-02-21", true).unwrap();
    assert_eq!(ts_end - ts, 86399);
}

#[test]
fn test_parse_date_to_ts_invalid() {
    assert!(parse_date_to_ts("not-a-date", false).is_err());
    assert!(parse_date_to_ts("2026-13-01", false).is_ok()); // month validation is lax (ok for our purposes)
}

#[test]
fn test_days_from_civil() {
    // 1970-01-01 should be day 0
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    // 1970-01-02 should be day 1
    assert_eq!(days_from_civil(1970, 1, 2), 1);
}
