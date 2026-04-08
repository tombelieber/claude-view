//! Small helpers used by `apply_lifecycle`.

use crate::live::state::{AgentState, HookFields, SessionStatus};

/// Derive SessionStatus from AgentState. No heuristics — purely structural.
///
/// Re-exported here so mutation callers don't need to reach into `state.rs`.
pub fn status_from_agent_state(state: &AgentState) -> SessionStatus {
    crate::live::state::status_from_agent_state(state)
}

/// Bind PID if not already set. Extracted to avoid repetition.
pub(super) fn bind_pid(hook: &mut HookFields, pid: Option<u32>) {
    if hook.pid.is_none() {
        if let Some(p) = pid {
            hook.pid = Some(p);
        }
    }
}
