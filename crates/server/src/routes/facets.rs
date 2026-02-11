// crates/server/src/routes/facets.rs
//! Facet ingest and query endpoints.
//!
//! - GET  /facets/ingest/stream  — SSE stream of facet ingest progress
//! - POST /facets/ingest/trigger — Trigger facet ingest from cache
//! - GET  /facets/stats          — Aggregate facet statistics
//! - GET  /facets/badges         — Quality badges for a batch of sessions
//! - GET  /facets/pattern-alert  — Check for negative satisfaction patterns

use axum::{
    extract::{Query, State},
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use crate::facet_ingest::{run_facet_ingest, IngestStatus};
use crate::state::AppState;

// ============================================================================
// Request / Response Types
// ============================================================================

/// Query parameters for GET /facets/badges.
#[derive(Debug, Deserialize)]
pub struct BadgesQuery {
    /// Comma-separated list of session IDs.
    ids: String,
}

/// Response for GET /facets/stats.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacetStatsResponse {
    pub total_with_facets: i64,
    pub total_without_facets: i64,
    pub achievement_rate: f64,
    pub frustrated_count: i64,
    pub satisfied_or_above_count: i64,
    pub friction_session_count: i64,
}

/// Per-session badge data returned in the badges map.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBadge {
    pub outcome: Option<String>,
    pub satisfaction: Option<String>,
}

/// Response for GET /facets/pattern-alert.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternAlertResponse {
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
}

/// Response for POST /facets/ingest/trigger.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerResponse {
    pub status: String,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// GET /api/facets/ingest/stream — SSE stream polling FacetIngestState atomics
/// every 200ms. Emits "progress" events repeatedly, then a terminal "done"
/// event when the ingest reaches Complete, Error, or NoCacheFound.
pub async fn facet_ingest_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let ingest = state.facet_ingest.clone();

    let stream = async_stream::stream! {
        loop {
            let status = ingest.status();
            let data = serde_json::json!({
                "status": status.as_str(),
                "total": ingest.total(),
                "ingested": ingest.ingested(),
                "newFacets": ingest.new_facets(),
            });
            yield Ok(Event::default().event("progress").data(data.to_string()));

            if matches!(status, IngestStatus::Complete | IngestStatus::Error | IngestStatus::NoCacheFound) {
                yield Ok(Event::default().event("done").data(data.to_string()));
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    };

    Sse::new(stream)
}

/// POST /api/facets/ingest/trigger — Start facet ingest from the Claude Code
/// insights cache. Returns immediately with `{"status": "started"}` or
/// `{"status": "already_running"}` if an ingest is already in progress.
pub async fn trigger_facet_ingest(
    State(state): State<Arc<AppState>>,
) -> Json<TriggerResponse> {
    if state.facet_ingest.is_running() {
        return Json(TriggerResponse {
            status: "already_running".to_string(),
        });
    }

    // Clone what we need for the spawned task
    let db = state.db.clone();
    let ingest = state.facet_ingest.clone();

    tokio::spawn(async move {
        if let Err(e) = run_facet_ingest(&db, &ingest, None).await {
            tracing::error!(error = %e, "Facet ingest failed");
        }
    });

    Json(TriggerResponse {
        status: "started".to_string(),
    })
}

/// GET /api/facets/stats — Aggregate statistics across all session facets.
pub async fn facet_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FacetStatsResponse>, axum::http::StatusCode> {
    match state.db.get_facet_aggregate_stats().await {
        Ok(stats) => Ok(Json(FacetStatsResponse {
            total_with_facets: stats.total_with_facets,
            total_without_facets: stats.total_without_facets,
            achievement_rate: stats.achievement_rate,
            frustrated_count: stats.frustrated_count,
            satisfied_or_above_count: stats.satisfied_or_above_count,
            friction_session_count: stats.friction_session_count,
        })),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get facet aggregate stats");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/facets/badges?ids=id1,id2,id3 — Quality badges (outcome +
/// satisfaction) for the requested session IDs. Returns a JSON map keyed
/// by session ID.
pub async fn facet_badges(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BadgesQuery>,
) -> Result<Json<HashMap<String, SessionBadge>>, axum::http::StatusCode> {
    let ids: Vec<String> = query
        .ids
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    match state.db.get_session_quality_badges(&ids).await {
        Ok(rows) => {
            let mut map = HashMap::new();
            for (session_id, outcome, satisfaction) in rows {
                map.insert(
                    session_id,
                    SessionBadge {
                        outcome,
                        satisfaction,
                    },
                );
            }
            Ok(Json(map))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get session quality badges");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/facets/pattern-alert — Check the most recent sessions for a
/// negative satisfaction pattern. Returns `{pattern, count, tip}` if a
/// pattern is detected, or `{pattern: null}` otherwise.
pub async fn pattern_alert(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PatternAlertResponse>, axum::http::StatusCode> {
    match state.db.get_pattern_alert().await {
        Ok(Some((pattern, count, tip))) => Ok(Json(PatternAlertResponse {
            pattern: Some(pattern),
            count: Some(count),
            tip: Some(tip),
        })),
        Ok(None) => Ok(Json(PatternAlertResponse {
            pattern: None,
            count: None,
            tip: None,
        })),
        Err(e) => {
            tracing::error!(error = %e, "Failed to check pattern alert");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// Router
// ============================================================================

/// Build the facets sub-router.
///
/// Routes:
/// - `GET  /facets/ingest/stream`  — SSE stream of facet ingest progress
/// - `POST /facets/ingest/trigger` — Trigger facet ingest
/// - `GET  /facets/stats`          — Aggregate facet statistics
/// - `GET  /facets/badges`         — Quality badges for sessions
/// - `GET  /facets/pattern-alert`  — Negative satisfaction pattern detection
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/facets/ingest/stream", get(facet_ingest_stream))
        .route("/facets/ingest/trigger", post(trigger_facet_ingest))
        .route("/facets/stats", get(facet_stats))
        .route("/facets/badges", get(facet_badges))
        .route("/facets/pattern-alert", get(pattern_alert))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let _router = router();
    }

    #[test]
    fn test_badges_query_deserialize() {
        let q = BadgesQuery {
            ids: "abc,def,ghi".to_string(),
        };
        let ids: Vec<String> = q
            .ids
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], "abc");
    }

    #[test]
    fn test_pattern_alert_response_serialize_some() {
        let resp = PatternAlertResponse {
            pattern: Some("frustrated".to_string()),
            count: Some(4),
            tip: Some("Try smaller sessions".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"pattern\":\"frustrated\""));
        assert!(json.contains("\"count\":4"));
        assert!(json.contains("\"tip\""));
    }

    #[test]
    fn test_pattern_alert_response_serialize_none() {
        let resp = PatternAlertResponse {
            pattern: None,
            count: None,
            tip: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"pattern\":null"));
        // count and tip should be omitted (skip_serializing_if)
        assert!(!json.contains("\"count\""));
        assert!(!json.contains("\"tip\""));
    }

    #[test]
    fn test_trigger_response_serialize() {
        let resp = TriggerResponse {
            status: "started".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"started\""));
    }

    #[test]
    fn test_facet_stats_response_serialize() {
        let resp = FacetStatsResponse {
            total_with_facets: 100,
            total_without_facets: 50,
            achievement_rate: 75.5,
            frustrated_count: 3,
            satisfied_or_above_count: 80,
            friction_session_count: 15,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"totalWithFacets\":100"));
        assert!(json.contains("\"achievementRate\":75.5"));
    }

    #[tokio::test]
    async fn test_facet_stats_endpoint() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/facets/stats")
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
        assert_eq!(json["totalWithFacets"], 0);
        assert_eq!(json["totalWithoutFacets"], 0);
    }

    #[tokio::test]
    async fn test_facet_badges_endpoint_empty() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/facets/badges?ids=abc,def")
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
        // No facets in DB, so empty map
        assert!(json.as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_pattern_alert_endpoint_empty() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/facets/pattern-alert")
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
        assert!(json["pattern"].is_null());
    }

    #[tokio::test]
    async fn test_trigger_ingest_endpoint() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/facets/ingest/trigger")
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
        assert_eq!(json["status"], "started");
    }

    #[tokio::test]
    async fn test_sse_ingest_stream_returns_event_stream() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/facets/ingest/stream")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // The stream is infinite (Idle doesn't terminate) so we only check
        // status code and content-type — don't consume the body.
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            content_type.contains("text/event-stream"),
            "Expected text/event-stream, got: {}",
            content_type
        );
    }
}
