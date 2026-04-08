//! Tests for the live monitoring endpoints.

use std::collections::HashMap;

use crate::live::state::{
    AgentState, AgentStateGroup, HookFields, JsonlFields, LiveSession, SessionStatus,
};

use super::summary::build_summary;

/// Minimal LiveSession for tests with optional closed flag.
fn test_session(id: &str, closed: bool) -> LiveSession {
    let mut s = LiveSession {
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
        jsonl: JsonlFields {
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            ..JsonlFields::default()
        },
        session_kind: None,
        entrypoint: None,
    };
    if closed {
        s.status = SessionStatus::Done;
        s.closed_at = Some(1_700_000_000);
    }
    s
}

#[test]
fn test_build_summary_excludes_closed_sessions() {
    let mut map = HashMap::new();
    map.insert("active-1".into(), test_session("active-1", false));
    map.insert("active-2".into(), test_session("active-2", false));
    map.insert("closed-1".into(), test_session("closed-1", true));

    let summary = build_summary(&map, 2);

    assert_eq!(
        summary["autonomousCount"], 2,
        "closed session must not inflate autonomousCount"
    );
    assert_eq!(summary["needsYouCount"], 0);
    assert_eq!(
        summary["processCount"], 2,
        "processCount should be passed through"
    );
}

#[test]
fn test_build_summary_empty_map() {
    let map = HashMap::new();
    let summary = build_summary(&map, 0);

    assert_eq!(summary["autonomousCount"], 0);
    assert_eq!(summary["needsYouCount"], 0);
    assert_eq!(summary["totalCostTodayUsd"], 0.0);
    assert_eq!(summary["totalTokensToday"], 0);
}
