//! Tests for `apply_lifecycle`.

use super::apply::apply_lifecycle;
use crate::live::mutation::types::{LifecycleEvent, SubEntityEvent};
use crate::live::state::{AgentState, AgentStateGroup, HookEvent, HookFields, SessionStatus};

fn make_hook_fields() -> HookFields {
    HookFields::default()
}

fn make_autonomous_state(state: &str, label: &str) -> AgentState {
    AgentState {
        group: AgentStateGroup::Autonomous,
        state: state.into(),
        label: label.into(),
        context: None,
    }
}

#[test]
fn prompt_increments_turn_count() {
    let mut hook = make_hook_fields();
    assert_eq!(hook.turn_count, 0);

    let event = LifecycleEvent::Prompt {
        text: "Hello world".into(),
        pid: None,
    };
    apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(hook.turn_count, 1);

    apply_lifecycle(&mut hook, &event, 1001);
    assert_eq!(hook.turn_count, 2);
}

#[test]
fn prompt_sets_title_if_empty() {
    let mut hook = make_hook_fields();
    assert!(hook.title.is_empty());

    let event = LifecycleEvent::Prompt {
        text: "Fix the bug in main.rs".into(),
        pid: None,
    };
    apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(hook.title, "Fix the bug in main.rs");

    // Second prompt should NOT overwrite existing title
    let event2 = LifecycleEvent::Prompt {
        text: "Also update tests".into(),
        pid: None,
    };
    apply_lifecycle(&mut hook, &event2, 1001);
    assert_eq!(hook.title, "Fix the bug in main.rs");
}

#[test]
fn prompt_transitions_to_autonomous_thinking() {
    let mut hook = make_hook_fields();
    // Start in NeedsYou/idle (typical post-Stop state)
    hook.agent_state = AgentState {
        group: AgentStateGroup::NeedsYou,
        state: "idle".into(),
        label: "Waiting".into(),
        context: None,
    };

    let event = LifecycleEvent::Prompt {
        text: "Fix the bug".into(),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);

    assert_eq!(result, Some(SessionStatus::Working));
    assert_eq!(hook.agent_state.state, "thinking");
    assert!(matches!(
        hook.agent_state.group,
        AgentStateGroup::Autonomous
    ));
}

#[test]
fn end_clears_hook_events_and_sets_done() {
    let mut hook = make_hook_fields();
    hook.hook_events.push(HookEvent {
        timestamp: 1,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "Reading".into(),
        group: "autonomous".into(),
        context: None,
        source: "hook".into(),
    });
    assert!(!hook.hook_events.is_empty());

    let result = apply_lifecycle(&mut hook, &LifecycleEvent::End { reason: None }, 2000);
    assert_eq!(result, Some(SessionStatus::Done));
    assert!(hook.hook_events.is_empty());
    assert_eq!(hook.agent_state.state, "session_ended");
}

#[test]
fn end_with_reason_includes_reason_in_label() {
    let mut hook = make_hook_fields();
    let result = apply_lifecycle(
        &mut hook,
        &LifecycleEvent::End {
            reason: Some("clear".into()),
        },
        2000,
    );
    assert_eq!(result, Some(SessionStatus::Done));
    assert_eq!(hook.agent_state.label, "Session ended (clear)");
}

#[test]
fn state_change_returns_status() {
    let mut hook = make_hook_fields();
    let state = make_autonomous_state("acting", "Working on task");

    let event = LifecycleEvent::StateChange {
        agent_state: state,
        event_name: "PreToolUse".into(),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(result, Some(SessionStatus::Working));
    assert_eq!(hook.agent_state.state, "acting");
}

#[test]
fn post_tool_use_during_compacting_skips_state_change() {
    let mut hook = make_hook_fields();
    // Set current state to compacting
    hook.agent_state = make_autonomous_state("compacting", "Compacting context");

    let new_state = make_autonomous_state("acting", "Working");
    let event = LifecycleEvent::StateChange {
        agent_state: new_state,
        event_name: "PostToolUse".into(),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);

    // Should return None (skipped) and state should remain "compacting"
    assert!(result.is_none());
    assert_eq!(hook.agent_state.state, "compacting");
}

#[test]
fn context_preserved_on_same_state() {
    let mut hook = make_hook_fields();
    // Set initial state with context
    hook.agent_state = AgentState {
        group: AgentStateGroup::Autonomous,
        state: "tool_use".into(),
        label: "Using Read".into(),
        context: Some(serde_json::json!({"file": "main.rs"})),
    };

    // Transition to same state but without context
    let new_state = AgentState {
        group: AgentStateGroup::Autonomous,
        state: "tool_use".into(),
        label: "Using Read".into(),
        context: None,
    };
    let event = LifecycleEvent::StateChange {
        agent_state: new_state,
        event_name: "PreToolUse".into(),
        pid: None,
    };
    apply_lifecycle(&mut hook, &event, 1000);

    // Context should be preserved from the previous state
    assert!(hook.agent_state.context.is_some());
    assert_eq!(
        hook.agent_state.context.as_ref().unwrap().to_string(),
        r#"{"file":"main.rs"}"#
    );
}

// ── New variant tests ──

#[test]
fn stop_clears_turn_and_stores_preview() {
    let mut hook = make_hook_fields();
    hook.current_turn_started_at = Some(999);

    let state = AgentState {
        group: AgentStateGroup::NeedsYou,
        state: "idle".into(),
        label: "Waiting".into(),
        context: None,
    };
    let event = LifecycleEvent::Stop {
        agent_state: state,
        last_assistant_message: Some("I've completed the task.".into()),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(result, Some(SessionStatus::Paused));
    assert!(hook.current_turn_started_at.is_none());
    assert_eq!(
        hook.last_assistant_preview.as_deref(),
        Some("I've completed the task.")
    );
}

#[test]
fn stop_failure_sets_error_state() {
    let mut hook = make_hook_fields();
    let event = LifecycleEvent::StopFailure {
        error: Some("rate_limit".into()),
        error_details: Some("429 Too Many Requests".into()),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(result, Some(SessionStatus::Paused));
    assert_eq!(hook.agent_state.state, "error");
    assert_eq!(hook.agent_state.group, AgentStateGroup::NeedsYou);
    assert_eq!(hook.last_error.as_deref(), Some("rate_limit"));
}

#[test]
fn stop_clears_previous_error() {
    let mut hook = make_hook_fields();
    hook.last_error = Some("rate_limit".into());

    let state = AgentState {
        group: AgentStateGroup::NeedsYou,
        state: "idle".into(),
        label: "Waiting".into(),
        context: None,
    };
    let event = LifecycleEvent::Stop {
        agent_state: state,
        last_assistant_message: None,
        pid: None,
    };
    apply_lifecycle(&mut hook, &event, 1000);
    assert!(hook.last_error.is_none());
}

#[test]
fn compacted_increments_count() {
    let mut hook = make_hook_fields();
    assert_eq!(hook.compact_count, 0);

    let event = LifecycleEvent::Compacted {
        trigger: Some("auto".into()),
        summary: Some("Summary of conversation".into()),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(result, Some(SessionStatus::Paused));
    assert_eq!(hook.compact_count, 1);
}

#[test]
fn observability_preserves_state() {
    let mut hook = make_hook_fields();
    hook.agent_state = make_autonomous_state("acting", "Running Bash");

    let event = LifecycleEvent::Observability {
        event_name: "FileChanged".into(),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(result, None); // no status change
    assert_eq!(hook.agent_state.state, "acting"); // preserved
}

#[test]
fn stop_truncates_last_assistant_preview_at_200_chars() {
    let mut hook = make_hook_fields();

    // Exactly 200 chars — should NOT be truncated
    let exact_200: String = "A".repeat(200);
    let state = AgentState {
        group: AgentStateGroup::NeedsYou,
        state: "idle".into(),
        label: "Waiting".into(),
        context: None,
    };
    let event = LifecycleEvent::Stop {
        agent_state: state.clone(),
        last_assistant_message: Some(exact_200.clone()),
        pid: None,
    };
    apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(
        hook.last_assistant_preview.as_ref().unwrap().len(),
        200,
        "Exactly 200 chars must be preserved in full"
    );

    // 201 chars — should be truncated to 200
    let over_200: String = "B".repeat(201);
    let event2 = LifecycleEvent::Stop {
        agent_state: state,
        last_assistant_message: Some(over_200),
        pid: None,
    };
    apply_lifecycle(&mut hook, &event2, 1001);
    let preview = hook.last_assistant_preview.as_ref().unwrap();
    assert_eq!(
        preview.len(),
        200,
        "201-char message must be truncated to 200"
    );
    assert!(preview.chars().all(|c| c == 'B'));
}

#[test]
fn stop_truncates_multibyte_chars_at_char_boundary() {
    let mut hook = make_hook_fields();

    // 201 multibyte chars (Chinese) — .take(200) counts chars, not bytes
    let long_msg: String = "你".repeat(201);
    let state = AgentState {
        group: AgentStateGroup::NeedsYou,
        state: "idle".into(),
        label: "Waiting".into(),
        context: None,
    };
    let event = LifecycleEvent::Stop {
        agent_state: state,
        last_assistant_message: Some(long_msg),
        pid: None,
    };
    apply_lifecycle(&mut hook, &event, 1000);
    let preview = hook.last_assistant_preview.as_ref().unwrap();
    assert_eq!(
        preview.chars().count(),
        200,
        "Multibyte: must truncate at 200 chars, not bytes"
    );
}

#[test]
fn subagent_started_pushes_and_updates_state() {
    let mut hook = make_hook_fields();
    assert!(hook.sub_agents.is_empty());

    let state = make_autonomous_state("delegating", "Running Explore agent");
    let event = LifecycleEvent::SubagentStarted {
        agent_state: state,
        agent_type: "Explore".into(),
        agent_id: Some("abc123".into()),
        pid: None,
    };
    let result = apply_lifecycle(&mut hook, &event, 1000);
    assert!(result.is_some());
    assert_eq!(hook.sub_agents.len(), 1);
    assert_eq!(hook.sub_agents[0].agent_type, "Explore");
    assert_eq!(hook.agent_state.state, "delegating");
}

#[test]
fn task_created_pushes_progress_item() {
    let mut hook = make_hook_fields();
    assert!(hook.progress_items.is_empty());

    let event = LifecycleEvent::SubEntity(SubEntityEvent::TaskCreated {
        task_id: "task-001".into(),
        subject: Some("Fix the bug".into()),
        description: Some("Fix auth flow".into()),
    });
    apply_lifecycle(&mut hook, &event, 1000);
    assert_eq!(hook.progress_items.len(), 1);
    assert_eq!(hook.progress_items[0].title, "Fix the bug");
}

#[test]
fn end_sweeps_running_subagents_as_orphaned() {
    use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};

    let mut hook = make_hook_fields();

    // Add 3 subagents: Running, Complete, Error
    hook.sub_agents = vec![
        SubAgentInfo {
            tool_use_id: "toolu_run".to_string(),
            agent_id: Some("agent1".to_string()),
            agent_type: "Explore".to_string(),
            description: "Still running".to_string(),
            status: SubAgentStatus::Running,
            started_at: 1000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: Some("Read".to_string()),
            error_reason: None,
        },
        SubAgentInfo {
            tool_use_id: "toolu_done".to_string(),
            agent_id: Some("agent2".to_string()),
            agent_type: "Edit".to_string(),
            description: "Completed".to_string(),
            status: SubAgentStatus::Complete,
            started_at: 1000,
            completed_at: Some(1050),
            duration_ms: Some(50000),
            tool_use_count: Some(10),
            model: Some("haiku".to_string()),
            input_tokens: Some(500),
            output_tokens: Some(200),
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: Some(0.001),
            current_activity: None,
            error_reason: None,
        },
        SubAgentInfo {
            tool_use_id: "toolu_err".to_string(),
            agent_id: None,
            agent_type: "Search".to_string(),
            description: "Already errored".to_string(),
            status: SubAgentStatus::Error,
            started_at: 1010,
            completed_at: Some(1020),
            duration_ms: None,
            tool_use_count: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: None,
            error_reason: None,
        },
    ];

    let result = apply_lifecycle(&mut hook, &LifecycleEvent::End { reason: None }, 2000);
    assert_eq!(result, Some(SessionStatus::Done));

    // Running subagent → Error with completed_at set and activity cleared
    assert_eq!(hook.sub_agents[0].status, SubAgentStatus::Error);
    assert_eq!(hook.sub_agents[0].completed_at, Some(2000));
    assert_eq!(hook.sub_agents[0].current_activity, None);

    // Complete subagent → unchanged
    assert_eq!(hook.sub_agents[1].status, SubAgentStatus::Complete);
    assert_eq!(hook.sub_agents[1].completed_at, Some(1050));
    assert_eq!(hook.sub_agents[1].cost_usd, Some(0.001));

    // Error subagent → unchanged (already finalized)
    assert_eq!(hook.sub_agents[2].status, SubAgentStatus::Error);
    assert_eq!(hook.sub_agents[2].completed_at, Some(1020));
}

#[test]
fn subagent_complete_sets_completed_at_and_clears_activity() {
    use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};

    let mut hook = make_hook_fields();

    // Add a running subagent with current_activity
    hook.sub_agents = vec![SubAgentInfo {
        tool_use_id: "toolu_run".to_string(),
        agent_id: Some("agent1".to_string()),
        agent_type: "Explore".to_string(),
        description: "Running agent".to_string(),
        status: SubAgentStatus::Running,
        started_at: 1000,
        completed_at: None,
        duration_ms: None,
        tool_use_count: None,
        model: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_creation_tokens: None,
        cost_usd: None,
        current_activity: Some("Grep".to_string()),
        error_reason: None,
    }];

    let event = LifecycleEvent::SubEntity(SubEntityEvent::SubagentComplete {
        agent_type: "Explore".into(),
        agent_id: Some("agent1".into()),
    });
    apply_lifecycle(&mut hook, &event, 1500);

    assert_eq!(hook.sub_agents[0].status, SubAgentStatus::Complete);
    assert_eq!(hook.sub_agents[0].completed_at, Some(1500));
    assert_eq!(hook.sub_agents[0].current_activity, None);
}
