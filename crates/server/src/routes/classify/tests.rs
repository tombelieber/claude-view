//! Tests for the classification API.

use axum::Router;

use super::job::actual_cost_cents_from_total;
use super::router;
use super::types::*;
use crate::state::AppState;
use claude_view_core::{SessionInfo, ToolCounts};
use claude_view_db::Database;
use tower::ServiceExt;

fn make_unclassified_session(id: &str, modified_at: i64) -> SessionInfo {
    SessionInfo {
        id: id.to_string(),
        project: "project-a".to_string(),
        project_path: "/home/user/project-a".to_string(),
        display_name: "project-a".to_string(),
        git_root: None,
        file_path: format!("/tmp/{id}.jsonl"),
        modified_at,
        size_bytes: 1024,
        preview: "Preview".to_string(),
        last_message: "Last message".to_string(),
        files_touched: vec![],
        skills_used: vec![],
        tool_counts: ToolCounts::default(),
        message_count: 1,
        turn_count: 1,
        summary: None,
        git_branch: None,
        is_sidechain: false,
        deep_indexed: false,
        total_input_tokens: None,
        total_output_tokens: None,
        total_cache_read_tokens: None,
        total_cache_creation_tokens: None,
        turn_count_api: Some(1),
        primary_model: None,
        user_prompt_count: 1,
        api_call_count: 0,
        tool_call_count: 0,
        files_read: vec![],
        files_edited: vec![],
        files_read_count: 0,
        files_edited_count: 0,
        reedited_files_count: 0,
        duration_seconds: 0,
        commit_count: 0,
        thinking_block_count: 0,
        turn_duration_avg_ms: None,
        turn_duration_max_ms: None,
        api_error_count: 0,
        compaction_count: 0,
        agent_spawn_count: 0,
        bash_progress_count: 0,
        hook_progress_count: 0,
        mcp_progress_count: 0,
        parse_version: 0,
        lines_added: 0,
        lines_removed: 0,
        loc_source: 0,
        category_l1: None,
        category_l2: None,
        category_l3: None,
        category_confidence: None,
        category_source: None,
        classified_at: None,
        total_task_time_seconds: None,
        longest_task_seconds: None,
        longest_task_preview: None,
        first_message_at: Some(modified_at),
        total_cost_usd: None,
        slug: None,
        entrypoint: None,
    }
}

#[test]
fn test_router_creation() {
    let _router = router();
}

#[test]
fn test_classify_request_deserialize() {
    let json = r#"{"mode": "unclassified"}"#;
    let req: ClassifyRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.mode, "unclassified");
    assert!(!req.dry_run);
}

#[test]
fn test_classify_request_dry_run() {
    let json = r#"{"mode": "all", "dryRun": true}"#;
    let req: ClassifyRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.mode, "all");
    assert!(req.dry_run);
}

#[test]
fn test_classify_response_serialize() {
    let resp = ClassifyResponse {
        job_id: 42,
        total_sessions: 100,
        status: "running".to_string(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"jobId\":42"));
    assert!(json.contains("\"totalSessions\":100"));
}

#[test]
fn test_cancel_response_serialize() {
    let resp = CancelResponse {
        job_id: 1,
        classified: 50,
        status: "cancelled".to_string(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"jobId\":1"));
    assert!(json.contains("\"classified\":50"));
}

#[test]
fn test_classify_status_response_serialize() {
    let resp = ClassifyStatusResponse {
        status: "idle".to_string(),
        job_id: None,
        progress: None,
        last_run: None,
        error: None,
        total_sessions: 500,
        classified_sessions: 400,
        unclassified_sessions: 100,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"status\":\"idle\""));
    assert!(json.contains("\"totalSessions\":500"));
    assert!(!json.contains("\"jobId\"")); // Should be skipped when None
}

#[tokio::test]
async fn test_start_classification_empty_db() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);

    let app = Router::new().nest("/api", router()).with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/classify")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"mode":"unclassified"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 because no sessions exist
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_status_idle() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);

    let app = Router::new().nest("/api", router()).with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/classify/status")
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
    assert_eq!(json["status"], "idle");
    assert_eq!(json["totalSessions"], 0);
}

#[tokio::test]
async fn test_cancel_when_not_running() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);

    let app = Router::new().nest("/api", router()).with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/classify/cancel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 because no job is running
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_classify_single_response_serialize() {
    let resp = ClassifySingleResponse {
        session_id: "sess-123".to_string(),
        category_l1: "code_work".to_string(),
        category_l2: "feature".to_string(),
        category_l3: "new-component".to_string(),
        confidence: 0.92,
        was_cached: false,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"sessionId\":\"sess-123\""));
    assert!(json.contains("\"categoryL1\":\"code_work\""));
    assert!(json.contains("\"wasCached\":false"));
}

#[tokio::test]
async fn test_classify_single_session_not_found() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);

    let app = Router::new().nest("/api", router()).with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/classify/single/nonexistent-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 404 because session doesn't exist
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_start_classification_dry_run_returns_scope_only_and_creates_no_job() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();
    let session = make_unclassified_session("sess-dry-run", now);
    db.insert_session(&session, "project-a", "Project A")
        .await
        .unwrap();

    let app = Router::new()
        .nest("/api", router())
        .with_state(AppState::new(db.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/classify")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"mode":"unclassified","dryRun":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "dry_run");
    assert_eq!(json["jobId"], 0);
    assert_eq!(json["totalSessions"], 1);

    let jobs_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM classification_jobs")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        jobs_count.0, 0,
        "dry run must not create classification job"
    );
}

#[test]
fn test_actual_cost_cents_from_total_null_when_unknown_or_invalid() {
    assert_eq!(actual_cost_cents_from_total(1.23, false), None);
    assert_eq!(actual_cost_cents_from_total(f64::NAN, true), None);
    assert_eq!(actual_cost_cents_from_total(f64::INFINITY, true), None);
    assert_eq!(actual_cost_cents_from_total(1.0e20, true), None);
}

#[test]
fn test_actual_cost_cents_from_total_known_finite() {
    assert_eq!(actual_cost_cents_from_total(0.0, true), Some(0));
    assert_eq!(actual_cost_cents_from_total(1.234, true), Some(123));
}
