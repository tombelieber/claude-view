use std::sync::Arc;
use axum::{extract::State, response::Json, routing::post, Router};
use serde::Deserialize;

use crate::live::state::{AgentState, AgentStateGroup, SignalSource};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct HookPayload {
    pub session_id: String,
    pub hook_event_name: String,
    pub cwd: Option<String>,
    pub transcript_path: Option<String>,
    pub permission_mode: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_response: Option<serde_json::Value>,
    pub tool_use_id: Option<String>,
    pub error: Option<String>,
    pub is_interrupt: Option<bool>,
    pub agent_type: Option<String>,
    pub agent_id: Option<String>,
    pub reason: Option<String>,
    pub task_id: Option<String>,
    pub task_subject: Option<String>,
    pub task_description: Option<String>,
    pub stop_hook_active: Option<bool>,
    pub agent_transcript_path: Option<String>,
    pub teammate_name: Option<String>,
    pub team_name: Option<String>,
    pub source: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/live/hook", post(handle_hook))
}

async fn handle_hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HookPayload>,
) -> Json<serde_json::Value> {
    let agent_state = resolve_state_from_hook(&payload);

    tracing::info!(
        session_id = %payload.session_id,
        event = %payload.hook_event_name,
        state = %agent_state.state,
        group = ?agent_state.group,
        "Hook event received"
    );

    state.state_resolver.update_from_hook(&payload.session_id, agent_state.clone()).await;

    // Also update the live session map if this session exists
    {
        let mut sessions = state.live_sessions.write().await;
        if let Some(session) = sessions.get_mut(&payload.session_id) {
            session.agent_state = agent_state;
            let _ = state.live_tx.send(crate::live::state::SessionEvent::SessionUpdated {
                session: session.clone(),
            });
        }
    }

    Json(serde_json::json!({ "ok": true }))
}

fn resolve_state_from_hook(payload: &HookPayload) -> AgentState {
    match payload.hook_event_name.as_str() {
        "PostToolUse" => match payload.tool_name.as_deref() {
            Some("AskUserQuestion") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_input".into(),
                label: "Asked you a question".into(),
                confidence: 0.99,
                source: SignalSource::Hook,
                context: payload.tool_input.clone(),
            },
            Some("ExitPlanMode") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_approval".into(),
                label: "Plan ready for review".into(),
                confidence: 0.99,
                source: SignalSource::Hook,
                context: None,
            },
            _ => AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: format!("Used {}", payload.tool_name.as_deref().unwrap_or("tool")),
                confidence: 0.9,
                source: SignalSource::Hook,
                context: None,
            },
        },
        "PostToolUseFailure" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "error".into(),
            label: format!("Failed: {}", payload.tool_name.as_deref().unwrap_or("tool")),
            confidence: 0.95,
            source: SignalSource::Hook,
            context: payload.error.as_ref().map(|e| serde_json::json!({"error": e})),
        },
        "PermissionRequest" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "needs_permission".into(),
            label: format!("Needs permission: {}", payload.tool_name.as_deref().unwrap_or("tool")),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        "Stop" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Waiting for your next prompt".into(),
            confidence: 0.8,
            source: SignalSource::Hook,
            context: None,
        },
        "SubagentStart" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: format!("Running {} subagent", payload.agent_type.as_deref().unwrap_or("unknown")),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        "SubagentStop" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("Subagent {} finished", payload.agent_type.as_deref().unwrap_or("unknown")),
            confidence: 0.9,
            source: SignalSource::Hook,
            context: None,
        },
        "SessionEnd" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session closed".into(),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        "TaskCompleted" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "task_complete".into(),
            label: payload.task_subject.clone().unwrap_or_else(|| "Task completed".into()),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        _ => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("Event: {}", payload.hook_event_name),
            confidence: 0.5,
            source: SignalSource::Hook,
            context: None,
        },
    }
}
