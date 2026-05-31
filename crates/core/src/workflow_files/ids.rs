//! Identifier validation + matching. These guard every path built from
//! caller-supplied session/run/agent ids, rejecting traversal before any read.

use super::types::WorkflowArtifactError;

pub(crate) fn validate_session_id(session_id: &str) -> Result<(), WorkflowArtifactError> {
    validate_id("session ID", session_id, false)
}

pub(crate) fn validate_run_id(run_id: &str) -> Result<(), WorkflowArtifactError> {
    if !run_id.starts_with("wf_") {
        return Err(WorkflowArtifactError::InvalidIdentifier("run ID"));
    }
    validate_id("run ID", run_id, true)
}

pub(crate) fn validate_agent_id(agent_id: &str) -> Result<(), WorkflowArtifactError> {
    validate_id("agent ID", agent_id, true)
}

fn validate_id(
    label: &'static str,
    value: &str,
    allow_underscore: bool,
) -> Result<(), WorkflowArtifactError> {
    let valid = !value.is_empty()
        && value.len() <= 160
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || (allow_underscore && ch == '_'));
    valid
        .then_some(())
        .ok_or(WorkflowArtifactError::InvalidIdentifier(label))
}

/// Two agent ids match if equal, or equal after stripping a single `agent-`
/// prefix on either side (summary uses the bare id; files use `agent-<id>`).
pub(crate) fn ids_match(a: &str, b: &str) -> bool {
    a == b || a.strip_prefix("agent-") == Some(b) || b.strip_prefix("agent-") == Some(a)
}
