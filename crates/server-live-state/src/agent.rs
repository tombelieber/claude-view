//! Agent state types and status derivation.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The universal agent state -- driven by hooks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    /// Which UI group: NeedsYou or Autonomous
    pub group: AgentStateGroup,
    /// Sub-state within group (open string -- new states added freely)
    pub state: String,
    /// Human-readable label for the UI
    pub label: String,
    /// Optional context (tool input, error details, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum AgentStateGroup {
    NeedsYou,
    Autonomous,
}

/// Derive SessionStatus from AgentState. No heuristics -- purely structural.
pub fn status_from_agent_state(agent_state: &AgentState) -> super::SessionStatus {
    match agent_state.state.as_str() {
        "session_ended" => super::SessionStatus::Done,
        _ => match agent_state.group {
            AgentStateGroup::Autonomous => super::SessionStatus::Working,
            AgentStateGroup::NeedsYou => super::SessionStatus::Paused,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_from_autonomous_acting() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: "Working".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Working
        );
    }

    #[test]
    fn test_status_from_autonomous_thinking() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Thinking".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Working
        );
    }

    #[test]
    fn test_status_from_autonomous_delegating() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: "Running agent".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Working
        );
    }

    #[test]
    fn test_status_from_needs_you_idle() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Idle".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Paused
        );
    }

    #[test]
    fn test_status_from_needs_you_awaiting_input() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "awaiting_input".into(),
            label: "Asked a question".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Paused
        );
    }

    #[test]
    fn test_status_from_needs_you_needs_permission() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "needs_permission".into(),
            label: "Needs permission".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Paused
        );
    }

    #[test]
    fn test_status_from_session_ended() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Ended".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Done
        );
    }

    #[test]
    fn test_status_from_session_ended_autonomous_group() {
        // session_ended should always produce Done regardless of group
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "session_ended".into(),
            label: "Ended".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Done
        );
    }

    #[test]
    fn test_status_from_compacting() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "compacting".into(),
            label: "Auto-compacting context...".into(),
            context: None,
        };
        assert_eq!(
            status_from_agent_state(&state),
            super::super::SessionStatus::Working
        );
    }
}
