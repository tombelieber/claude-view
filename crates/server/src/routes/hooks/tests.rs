//! Integration tests for the hook handler.
//!
//! These need full app context (AppState, coordinator, HTTP) so they live
//! separately from the unit tests in each submodule.

use super::*;
use crate::live::state::{AgentState, AgentStateGroup, HookFields, SessionStatus};
use tower::ServiceExt;

pub(super) fn minimal_payload(event: &str) -> HookPayload {
    HookPayload {
        session_id: "test-session".into(),
        hook_event_name: event.into(),
        cwd: None,
        transcript_path: None,
        permission_mode: None,
        tool_name: None,
        tool_input: None,
        tool_response: None,
        tool_use_id: None,
        error: None,
        is_interrupt: None,
        agent_type: None,
        agent_id: None,
        reason: None,
        task_id: None,
        task_subject: None,
        task_description: None,
        stop_hook_active: None,
        agent_transcript_path: None,
        teammate_name: None,
        team_name: None,
        source: None,
        prompt: None,
        notification_type: None,
        message: None,
        model: None,
        permission_suggestions: None,
        trigger: None,
        custom_instructions: None,
        name: None,
        last_assistant_message: None,
        compact_summary: None,
        error_details: None,
        title: None,
        old_cwd: None,
        new_cwd: None,
        file_path: None,
        file_event: None,
        worktree_path: None,
        memory_type: None,
        load_reason: None,
        globs: None,
        trigger_file_path: None,
        parent_file_path: None,
        mcp_server_name: None,
        mode: None,
        url: None,
        elicitation_id: None,
        requested_schema: None,
        action: None,
        content: None,
        extra: std::collections::HashMap::new(),
    }
}

/// Create a LiveSession in autonomous/acting state for integration tests.
fn make_autonomous_session(id: &str) -> crate::live::state::LiveSession {
    crate::live::state::LiveSession {
        id: id.to_string(),
        status: SessionStatus::Working,
        started_at: Some(1000),
        closed_at: None,
        control: None,
        model: None,
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 0,
        statusline: crate::live::state::StatuslineFields::default(),
        hook: HookFields {
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            pid: None,
            title: "Test session".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 5,
            last_activity_at: 1000,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
            hook_events: Vec::new(),
        },
        jsonl: crate::live::state::JsonlFields {
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            ..crate::live::state::JsonlFields::default()
        },
    }
}

// =========================================================================
// Helpers
// =========================================================================

async fn send_session_start(
    app: &axum::Router,
    session_id: &str,
    cwd: &str,
    pid: Option<u32>,
) -> axum::http::StatusCode {
    let body = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": "SessionStart",
        "cwd": cwd,
    });
    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/live/hook")
        .header("content-type", "application/json");
    if let Some(pid) = pid {
        req = req.header("x-claude-pid", pid.to_string());
    }
    let response = app
        .clone()
        .oneshot(
            req.body(axum::body::Body::from(
                serde_json::to_string(&body).unwrap(),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

async fn send_session_end(
    app: &axum::Router,
    session_id: &str,
    pid: Option<u32>,
) -> axum::http::StatusCode {
    let body = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": "SessionEnd",
    });
    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/live/hook")
        .header("content-type", "application/json");
    if let Some(pid) = pid {
        req = req.header("x-claude-pid", pid.to_string());
    }
    let response = app
        .clone()
        .oneshot(
            req.body(axum::body::Body::from(
                serde_json::to_string(&body).unwrap(),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

async fn send_hook_event(app: &axum::Router, body: serde_json::Value) -> axum::http::StatusCode {
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

// =========================================================================
// Serde / payload tests
// =========================================================================

#[test]
fn serde_flatten_captures_unknown_fields() {
    let json = serde_json::json!({
        "session_id": "test-123",
        "hook_event_name": "FutureEvent",
        "brand_new_field": "surprise",
        "nested_data": {"key": "value"}
    });
    let payload: HookPayload = serde_json::from_value(json).unwrap();
    assert_eq!(payload.session_id, "test-123");
    assert_eq!(payload.hook_event_name, "FutureEvent");
    assert_eq!(
        payload.extra.get("brand_new_field"),
        Some(&serde_json::json!("surprise")),
        "Unknown string field must be captured in extra HashMap"
    );
    assert_eq!(
        payload.extra.get("nested_data"),
        Some(&serde_json::json!({"key": "value"})),
        "Unknown nested object must be captured in extra HashMap"
    );
    assert!(
        !payload.extra.contains_key("session_id"),
        "Known fields must not leak into extra"
    );
}

// =========================================================================
// Integration tests — full HTTP handler
// =========================================================================

#[tokio::test]
async fn session_start_missing_cwd_buffers_without_creating_session() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());
    let body = serde_json::json!({
        "session_id": "missing-cwd-start",
        "hook_event_name": "SessionStart"
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    assert!(
        sessions.get("missing-cwd-start").is_none(),
        "SessionStart without cwd must not create a live session"
    );
}

#[tokio::test]
async fn buffered_events_promote_on_session_start_with_cwd() {
    let session_id = "promote-on-valid-cwd";
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    let body_buffered = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "git status"}
    });
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&body_buffered).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(state.live_sessions.read().await.get(session_id).is_none());

    let body_start = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": "SessionStart",
        "cwd": "/tmp/promoted-project"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&body_start).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get(session_id).expect("session should be created");
    assert_eq!(session.jsonl.project_path, "/tmp/promoted-project");
    assert_eq!(session.hook.hook_events.len(), 2);
    assert_eq!(session.hook.hook_events[0].event_name, "PreToolUse");
    assert_eq!(session.hook.hook_events[1].event_name, "SessionStart");
}

#[tokio::test]
async fn session_end_for_unknown_session_returns_ok() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    let body_end = serde_json::json!({
        "session_id": "unknown-session-end",
        "hook_event_name": "SessionEnd"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&body_end).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(state
        .live_sessions
        .read()
        .await
        .get("unknown-session-end")
        .is_none());
}

#[tokio::test]
async fn existing_session_receives_hook_events() {
    let session_id = "existing-session-hooks";
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(session_id.to_string(), make_autonomous_session(session_id));
    }

    let body = serde_json::json!({
        "session_id": session_id,
        "hook_event_name": "UserPromptSubmit",
        "cwd": "/tmp/promoted-project",
        "prompt": "Continue"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get(session_id).expect("session should exist");
    assert_eq!(session.hook.hook_events.len(), 1);
    assert_eq!(session.hook.hook_events[0].event_name, "UserPromptSubmit");
}

#[tokio::test]
async fn task_completed_hook_event_records_actual_session_group() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            make_autonomous_session("test-session"),
        );
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "TaskCompleted",
            "task_id": "task-1",
            "task_subject": "Fix login bug"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook.hook_events.len(), 1);
    let event = &session.hook.hook_events[0];
    assert_eq!(event.event_name, "TaskCompleted");
    assert_eq!(event.label, "Fix login bug");
    assert_eq!(event.group, "autonomous");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
}

#[tokio::test]
async fn subagent_stop_hook_event_records_actual_session_group() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            make_autonomous_session("test-session"),
        );
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "SubagentStop",
            "agent_type": "code-explorer",
            "agent_id": "agent-1"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook.hook_events.len(), 1);
    assert_eq!(session.hook.hook_events[0].group, "autonomous");
}

#[tokio::test]
async fn teammate_idle_hook_event_records_actual_session_group() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            make_autonomous_session("test-session"),
        );
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "TeammateIdle",
            "teammate_name": "researcher"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook.hook_events.len(), 1);
    assert_eq!(session.hook.hook_events[0].group, "autonomous");
}

#[tokio::test]
async fn state_changing_event_hook_event_records_new_group() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            make_autonomous_session("test-session"),
        );
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "PreToolUse",
            "tool_name": "AskUserQuestion",
            "tool_input": {"question": "Which approach?"}
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook.hook_events.len(), 1);
    assert_eq!(session.hook.hook_events[0].group, "needs_you");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::NeedsYou
    ));
}

#[tokio::test]
async fn post_tool_use_does_not_override_compacting() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut session = make_autonomous_session("test-session");
        session.hook.agent_state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "compacting".into(),
            label: "Auto-compacting context...".into(),
            context: None,
        };
        session.status = SessionStatus::Working;
        session.hook.current_activity = "Auto-compacting context...".into();
        let mut sessions = state.live_sessions.write().await;
        sessions.insert("test-session".to_string(), session);
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook.agent_state.state, "compacting");
    assert_eq!(session.hook.current_activity, "Auto-compacting context...");
    assert_eq!(session.status, SessionStatus::Working);
}

// =========================================================================
// PID uniqueness + ghost session tests
// =========================================================================

#[tokio::test]
async fn pid_uniqueness_evicts_ghost_session_on_same_pid() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    let status = send_session_start(&app, "session-a", "/tmp/proj", Some(99999)).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    {
        let sessions = state.live_sessions.read().await;
        let a = sessions.get("session-a").expect("session-a must exist");
        assert_eq!(a.hook.pid, Some(99999));
    }

    let status = send_session_start(&app, "session-b", "/tmp/proj2", Some(99999)).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    {
        let sessions = state.live_sessions.read().await;
        let b = sessions.get("session-b").expect("session-b must exist");
        assert_eq!(b.hook.pid, Some(99999));
        assert_ne!(b.status, SessionStatus::Done);
        // Note: eviction requires live_manager (reap_session). In test AppState
        // with live_manager=None, the eviction handler is a no-op. The eviction
        // behavior is tested via the reaper unit tests in manager/reaper.rs.
    }
}

#[tokio::test]
async fn pid_uniqueness_new_session_created_alongside_stale() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    {
        let mut sessions = state.live_sessions.write().await;
        let mut real = make_autonomous_session("real-old");
        real.hook.pid = Some(99998);
        real.jsonl.file_path = "/tmp/real.jsonl".into();
        real.hook.turn_count = 5;
        sessions.insert("real-old".into(), real);
    }

    send_session_start(&app, "real-new", "/tmp/proj", Some(99998)).await;

    let sessions = state.live_sessions.read().await;
    // New session is created
    assert!(sessions.get("real-new").is_some());
    // Note: stale session eviction requires live_manager (reap_session).
    // With live_manager=None, old session stays. Real eviction tested in
    // manager/reaper.rs unit tests.
}

#[tokio::test]
async fn pid_uniqueness_does_not_evict_different_pid() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    send_session_start(&app, "session-x", "/tmp/proj-x", Some(10001)).await;
    send_session_start(&app, "session-y", "/tmp/proj-y", Some(10002)).await;

    let sessions = state.live_sessions.read().await;
    assert!(sessions.get("session-x").is_some());
    assert!(sessions.get("session-y").is_some());
    assert_ne!(sessions["session-x"].status, SessionStatus::Done);
    assert_ne!(sessions["session-y"].status, SessionStatus::Done);
}

#[tokio::test]
async fn pid_uniqueness_skips_done_sessions() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    send_session_start(&app, "done-a", "/tmp/done", Some(20001)).await;
    send_session_end(&app, "done-a", Some(20001)).await;
    {
        let sessions = state.live_sessions.read().await;
        assert_eq!(
            sessions.get("done-a").expect("done-a must exist").status,
            SessionStatus::Done
        );
    }

    send_session_start(&app, "done-b", "/tmp/done2", Some(20001)).await;

    let sessions = state.live_sessions.read().await;
    assert!(sessions.get("done-b").is_some());
    assert_eq!(
        sessions
            .get("done-a")
            .expect("done-a must still exist")
            .status,
        SessionStatus::Done
    );
}

#[tokio::test]
async fn pid_uniqueness_skips_sidecar_sessions() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    {
        let mut sessions = state.live_sessions.write().await;
        let mut session = make_autonomous_session("sidecar-session");
        session.hook.pid = Some(30001);
        session.control = Some(crate::live::state::ControlBinding {
            control_id: "ctrl-123".into(),
            bound_at: 1000,
            cancel: tokio_util::sync::CancellationToken::new(),
        });
        sessions.insert("sidecar-session".into(), session);
    }

    send_session_start(&app, "new-session", "/tmp/proj", Some(30001)).await;

    let sessions = state.live_sessions.read().await;
    let sidecar = sessions
        .get("sidecar-session")
        .expect("sidecar must survive");
    assert_ne!(sidecar.status, SessionStatus::Done);
    assert!(sidecar.control.is_some());
    assert!(sessions.get("new-session").is_some());
}

#[tokio::test]
async fn ghost_session_new_session_created_on_same_pid() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    {
        let mut sessions = state.live_sessions.write().await;
        let mut ghost = make_autonomous_session("ghost-session");
        ghost.hook.pid = Some(40001);
        ghost.jsonl.file_path = String::new();
        ghost.hook.turn_count = 0;
        sessions.insert("ghost-session".into(), ghost);
    }

    send_session_start(&app, "real-session", "/tmp/proj", Some(40001)).await;

    let sessions = state.live_sessions.read().await;
    // New session is created
    assert!(sessions.get("real-session").is_some());
    // Note: ghost eviction requires live_manager (reap_session).
    // With live_manager=None, ghost stays. Real eviction tested in
    // manager/reaper.rs unit tests.
}

#[tokio::test]
async fn no_pid_means_no_eviction() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    send_session_start(&app, "with-pid", "/tmp/proj1", Some(50001)).await;
    send_session_start(&app, "no-pid", "/tmp/proj2", None).await;

    let sessions = state.live_sessions.read().await;
    assert!(sessions.get("with-pid").is_some());
    assert!(sessions.get("no-pid").is_some());
    assert_ne!(sessions["with-pid"].status, SessionStatus::Done);
}

#[tokio::test]
async fn same_session_id_same_pid_is_update_not_eviction() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    let app = crate::api_routes(state.clone());

    send_session_start(&app, "resume-me", "/tmp/proj", Some(60001)).await;
    send_session_start(&app, "resume-me", "/tmp/proj", Some(60001)).await;

    let sessions = state.live_sessions.read().await;
    let s = sessions.get("resume-me").expect("session must exist");
    assert_ne!(s.status, SessionStatus::Done);
    assert_eq!(s.hook.pid, Some(60001));
}

// =========================================================================
// Observability events must NOT change agent_state
// =========================================================================

#[tokio::test]
async fn instructions_loaded_preserves_agent_state() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert("obs-test".into(), make_autonomous_session("obs-test"));
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "obs-test",
            "hook_event_name": "InstructionsLoaded",
            "file_path": "/home/user/.claude/CLAUDE.md",
            "memory_type": "user",
            "load_reason": "session_start"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("obs-test").unwrap();
    assert_eq!(session.hook.agent_state.state, "acting");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
    assert!(session
        .hook
        .hook_events
        .iter()
        .any(|e| e.event_name == "InstructionsLoaded"));
}

#[tokio::test]
async fn config_change_preserves_agent_state() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert("obs-test".into(), make_autonomous_session("obs-test"));
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "obs-test",
            "hook_event_name": "ConfigChange",
            "file_path": "/home/user/.claude/settings.json"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("obs-test").unwrap();
    assert_eq!(session.hook.agent_state.state, "acting");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
}

#[tokio::test]
async fn file_changed_preserves_agent_state() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert("obs-test".into(), make_autonomous_session("obs-test"));
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "obs-test",
            "hook_event_name": "FileChanged",
            "file_path": "/tmp/test/src/main.rs",
            "event": "change"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("obs-test").unwrap();
    assert_eq!(session.hook.agent_state.state, "acting");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
}

#[tokio::test]
async fn worktree_create_preserves_agent_state() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert("obs-test".into(), make_autonomous_session("obs-test"));
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "obs-test",
            "hook_event_name": "WorktreeCreate",
            "cwd": "/tmp/test-worktree"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("obs-test").unwrap();
    assert_eq!(session.hook.agent_state.state, "acting");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
}

#[tokio::test]
async fn worktree_remove_preserves_agent_state() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert("obs-test".into(), make_autonomous_session("obs-test"));
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "obs-test",
            "hook_event_name": "WorktreeRemove",
            "worktree_path": "/tmp/test-worktree"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("obs-test").unwrap();
    assert_eq!(session.hook.agent_state.state, "acting");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
}

#[tokio::test]
async fn unknown_event_routes_to_observability_and_preserves_state() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert("obs-test".into(), make_autonomous_session("obs-test"));
    }

    let app = crate::api_routes(state.clone());
    let status = send_hook_event(
        &app,
        serde_json::json!({
            "session_id": "obs-test",
            "hook_event_name": "FutureClaudeCodeEvent",
            "some_new_field": "data"
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("obs-test").unwrap();
    assert_eq!(session.hook.agent_state.state, "acting");
    assert!(matches!(
        session.hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
    assert!(session
        .hook
        .hook_events
        .iter()
        .any(|e| e.event_name == "FutureClaudeCodeEvent"));
}
