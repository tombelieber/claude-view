//! Live session classification for connect/resume gate logic.

use super::core::LiveSession;

/// Check if a process with the given PID is still alive.
///
/// Uses `kill(pid, 0)` which checks process existence without sending a signal.
/// Returns `false` for PIDs <= 1 (kernel/init) to guard against reparented processes.
pub fn is_pid_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    // SAFETY: kill with signal 0 does not send a signal, only checks existence.
    // Returns 0 if process exists and we have permission, -1 with ESRCH if not.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// What to do with a session that's in the live_sessions map.
///
/// Extracted from control.rs so the gate logic is reusable across routes.
/// Design rule: **only return `Block` for states the Agent SDK fundamentally
/// cannot handle.** Process liveness is NOT such a state -- the SDK creates
/// a new CLI process from session history, so dead-PID sessions resume fine.
#[derive(Debug)]
pub enum LiveSessionAction {
    /// Session not tracked by the live monitor -> proceed to SDK resume.
    ResumeNew,
    /// Session tracked but process is dead -> proceed to SDK resume (new process).
    ResumeDeadProcess,
    /// Session has an active PID but no control binding -> proceed to SDK resume.
    ResumeAlive,
    /// Session is already controlled -> reuse the existing binding.
    ReuseExisting {
        control_id: String,
        cancel: tokio_util::sync::CancellationToken,
    },
}

impl PartialEq for LiveSessionAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ResumeNew, Self::ResumeNew)
            | (Self::ResumeDeadProcess, Self::ResumeDeadProcess)
            | (Self::ResumeAlive, Self::ResumeAlive) => true,
            (
                Self::ReuseExisting { control_id: a, .. },
                Self::ReuseExisting { control_id: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

/// Decide what action to take for a live session connect request.
///
/// Pure function (no side effects) so it can be unit-tested directly.
pub fn classify_live_session(session: Option<&LiveSession>) -> LiveSessionAction {
    match session {
        None => LiveSessionAction::ResumeNew,
        Some(s) if s.hook.pid.is_none() || !is_pid_alive(s.hook.pid.unwrap_or(0)) => {
            LiveSessionAction::ResumeDeadProcess
        }
        Some(s) if s.control.is_some() => {
            let ctl = s.control.as_ref().unwrap();
            LiveSessionAction::ReuseExisting {
                control_id: ctl.control_id.clone(),
                cancel: ctl.cancel.clone(),
            }
        }
        _ => LiveSessionAction::ResumeAlive,
    }
}

#[cfg(test)]
mod tests {
    use super::super::core::{test_live_session, ControlBinding};
    use super::*;

    #[test]
    fn classify_none_returns_resume_new() {
        assert_eq!(classify_live_session(None), LiveSessionAction::ResumeNew);
    }

    /// Regression: dead-PID sessions MUST proceed to resume, never block.
    #[test]
    fn classify_dead_pid_returns_resume_not_block() {
        let mut session = test_live_session("dead-pid-session");
        session.hook.pid = Some(999_999);
        let action = classify_live_session(Some(&session));
        assert_eq!(
            action,
            LiveSessionAction::ResumeDeadProcess,
            "Dead-PID session must resume, not block -- SDK creates a new CLI process"
        );
    }

    #[test]
    fn classify_no_pid_returns_resume_dead() {
        let mut session = test_live_session("no-pid-session");
        session.hook.pid = None;
        let action = classify_live_session(Some(&session));
        assert_eq!(action, LiveSessionAction::ResumeDeadProcess);
    }

    #[test]
    fn classify_alive_pid_no_control_returns_resume_alive() {
        let mut session = test_live_session("alive-session");
        session.hook.pid = Some(std::process::id());
        session.control = None;
        let action = classify_live_session(Some(&session));
        assert_eq!(action, LiveSessionAction::ResumeAlive);
    }

    #[test]
    fn classify_already_controlled_returns_reuse() {
        let mut session = test_live_session("controlled-session");
        session.hook.pid = Some(std::process::id());
        session.control = Some(ControlBinding {
            control_id: "ctl-123".to_string(),
            bound_at: 0,
            cancel: tokio_util::sync::CancellationToken::new(),
        });
        let action = classify_live_session(Some(&session));
        match action {
            LiveSessionAction::ReuseExisting { control_id, .. } => {
                assert_eq!(control_id, "ctl-123");
            }
            other => panic!("Expected ReuseExisting, got {other:?}"),
        }
    }
}
