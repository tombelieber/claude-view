//! Integration tests for POST /interact + GET /interaction endpoints.
//!
//! Uses the AppState test factory with in-memory DB. Exercises state management
//! logic (side-map lookup, clear on resolve). Sidecar HTTP forwarding is not
//! tested here — that's covered by e2e.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use claude_view_types::{InteractionBlock, InteractionVariant, PendingInteractionMeta};
use serde_json::json;
use tower::ServiceExt;

use crate::state::AppState;

/// Build an Axum router with only the interact routes, backed by the given state.
fn test_router(state: std::sync::Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .nest("/api", super::router())
        .with_state(state)
}

/// Helper: create a test AppState with an in-memory DB.
async fn test_state() -> std::sync::Arc<AppState> {
    let db = claude_view_db::Database::new_in_memory()
        .await
        .expect("in-memory DB");
    AppState::new(db)
}

/// Helper: test AppState whose sidecar delivery is pointed at `base_url`
/// (a mock sidecar or a known-dead URL).
async fn test_state_with_sidecar(base_url: String) -> std::sync::Arc<AppState> {
    let db = claude_view_db::Database::new_in_memory()
        .await
        .expect("in-memory DB");
    AppState::builder(db)
        .with_sidecar(std::sync::Arc::new(
            crate::sidecar::SidecarManager::with_base_url(base_url),
        ))
        .build()
}

/// Spawn a mock sidecar that answers the interact bridge with `(status, body)`.
/// Returns its base URL.
async fn spawn_mock_sidecar(status: StatusCode, body: serde_json::Value) -> String {
    let app = axum::Router::new().route(
        "/api/sidecar/sessions/{id}/interact",
        axum::routing::post(move || {
            let body = body.clone();
            async move { (status, axum::Json(body)) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// A base URL whose port is closed — delivery to it fails (connection refused).
async fn dead_sidecar_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{addr}")
}

/// Helper: insert a live session into the state's live_sessions map.
async fn insert_live_session(state: &AppState, id: &str) {
    let session = claude_view_server_live_state::core::test_live_session(id);
    state
        .live_sessions
        .write()
        .await
        .insert(id.to_string(), session);
}

/// Helper: insert a live session with SDK control binding.
/// compute_ownership() reads session.control, not session.ownership.
async fn insert_sdk_session(state: &AppState, id: &str, control_id: &str) {
    let mut session = claude_view_server_live_state::core::test_live_session(id);
    session.control = Some(claude_view_server_live_state::core::ControlBinding {
        control_id: control_id.to_string(),
        bound_at: 1000,
        bound_at_generation: 0,
        cancel: tokio_util::sync::CancellationToken::new(),
    });
    state
        .live_sessions
        .write()
        .await
        .insert(id.to_string(), session);
}

/// Helper: insert a live session with no control binding (observed).
/// No control, no CLI session → compute_ownership returns empty ownership.
async fn insert_observed_session(state: &AppState, id: &str) {
    let session = claude_view_server_live_state::core::test_live_session(id);
    // No control, no CLI session → compute_ownership will return empty ownership
    state
        .live_sessions
        .write()
        .await
        .insert(id.to_string(), session);
}

/// Helper: set a pending interaction on a live session + side-map entry.
async fn set_pending_interaction(state: &AppState, session_id: &str, request_id: &str) {
    // Set compact meta on the session
    {
        let mut sessions = state.live_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.pending_interaction = Some(PendingInteractionMeta {
                variant: InteractionVariant::Permission,
                request_id: request_id.to_string(),
                preview: "Allow Bash?".to_string(),
            });
        }
    }
    // Insert full data into side-map
    {
        let block = InteractionBlock {
            id: format!("interaction-{request_id}"),
            variant: InteractionVariant::Permission,
            request_id: Some(request_id.to_string()),
            resolved: false,
            historical_source: None,
            data: json!({
                "type": "permission_request",
                "requestId": request_id,
                "toolName": "Bash",
                "command": "echo hello"
            }),
        };
        state
            .interaction_data
            .write()
            .await
            .insert(request_id.to_string(), block);
    }
}

// ════════════════════════════════════════════════════════════════════
// GET /api/sessions/{id}/interaction
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn get_interaction_returns_404_when_no_session() {
    let state = test_state().await;
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions/nonexistent/interaction")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_interaction_returns_404_when_no_pending_interaction() {
    let state = test_state().await;
    insert_live_session(&state, "sess-001").await;

    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions/sess-001/interaction")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_interaction_returns_block_when_pending() {
    let state = test_state().await;
    insert_live_session(&state, "sess-002").await;
    set_pending_interaction(&state, "sess-002", "req-100").await;

    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions/sess-002/interaction")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let block: InteractionBlock = serde_json::from_slice(&body).unwrap();
    assert_eq!(block.request_id.as_deref(), Some("req-100"));
    assert!(matches!(block.variant, InteractionVariant::Permission));
    assert!(!block.resolved);
}

// ════════════════════════════════════════════════════════════════════
// POST /api/sessions/{id}/interact
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn post_interact_returns_404_when_session_not_found() {
    let state = test_state().await;
    let app = test_router(state);

    let body = json!({
        "variant": "permission",
        "requestId": "req-001",
        "allowed": true
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions/nonexistent/interact")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn post_interact_returns_409_when_request_id_not_in_side_map() {
    let state = test_state().await;
    insert_sdk_session(&state, "sess-003", "ctl-abc").await;
    // No pending interaction set — side-map is empty

    let app = test_router(state);
    let body = json!({
        "variant": "permission",
        "requestId": "req-stale",
        "allowed": true
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-003/interact")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn post_interact_returns_400_for_observed_sessions() {
    let state = test_state().await;
    insert_observed_session(&state, "sess-004").await;
    set_pending_interaction(&state, "sess-004", "req-200").await;

    let app = test_router(state);
    let body = json!({
        "variant": "permission",
        "requestId": "req-200",
        "allowed": true
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-004/interact")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn post_interact_clears_pending_when_delivered() {
    // Mock sidecar acks the decision → handler clears pending + returns 200.
    let base = spawn_mock_sidecar(StatusCode::OK, json!({ "ok": true })).await;
    let state = test_state_with_sidecar(base).await;
    insert_sdk_session(&state, "sess-005", "ctl-xyz").await;
    set_pending_interaction(&state, "sess-005", "req-300").await;

    let app = test_router(state.clone());
    let body = json!({ "variant": "permission", "requestId": "req-300", "allowed": true });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-005/interact")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["resolved"], true);
    assert_eq!(json["requestId"], "req-300");
    assert_eq!(json["sessionId"], "sess-005");
    // No sidecarForwarded field anymore — 200 means actually delivered.
    assert!(json.get("sidecarForwarded").is_none());

    // Pending cleared only because delivery was confirmed.
    {
        let sessions = state.live_sessions.read().await;
        assert!(sessions["sess-005"].pending_interaction.is_none());
    }
    assert!(!state.interaction_data.read().await.contains_key("req-300"));
}

#[tokio::test]
async fn post_interact_keeps_pending_when_delivery_fails() {
    // THE regression guard: a failed delivery must NEVER report resolved and
    // must NEVER clear pending (寧願唔顯示，都唔顯示錯嘅嘢). Sidecar is dead.
    let base = dead_sidecar_url().await;
    let state = test_state_with_sidecar(base).await;
    insert_sdk_session(&state, "sess-006", "ctl-dead").await;
    set_pending_interaction(&state, "sess-006", "req-301").await;

    let app = test_router(state.clone());
    let body = json!({ "variant": "permission", "requestId": "req-301", "allowed": true });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-006/interact")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    // Pending retained so the user can retry — decision not silently lost.
    {
        let sessions = state.live_sessions.read().await;
        assert!(
            sessions["sess-006"].pending_interaction.is_some(),
            "pending_interaction MUST be retained when delivery fails"
        );
    }
    assert!(
        state.interaction_data.read().await.contains_key("req-301"),
        "side-map entry MUST be retained when delivery fails"
    );
}

#[tokio::test]
async fn post_interact_keeps_pending_when_sidecar_rejects() {
    // Sidecar reached but reports the requestId unknown/stale → 409, no clear.
    let base = spawn_mock_sidecar(
        StatusCode::OK,
        json!({ "ok": false, "reason": "Unknown permission requestId" }),
    )
    .await;
    let state = test_state_with_sidecar(base).await;
    insert_sdk_session(&state, "sess-007", "ctl-rej").await;
    set_pending_interaction(&state, "sess-007", "req-302").await;

    let app = test_router(state.clone());
    let body = json!({ "variant": "permission", "requestId": "req-302", "allowed": false });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-007/interact")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
    // The SDK's own turn-end clear is authoritative — the handler does not clear.
    assert!(state.interaction_data.read().await.contains_key("req-302"));
}
