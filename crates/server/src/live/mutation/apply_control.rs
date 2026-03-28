//! Pure function: apply a ControlAction to the session's control binding.
//!
//! Binary bind/unbind — no IO, no locks, no broadcasts.

use crate::live::mutation::types::ControlAction;
use crate::live::state::ControlBinding;

/// Apply a control action to the session's control binding.
///
/// Pure function — no IO, no locks. The caller is responsible for
/// broadcasting SSE events and any cleanup on unbind.
pub fn apply_control(control: &mut Option<ControlBinding>, action: &ControlAction) {
    match action {
        ControlAction::Bind(binding) => {
            *control = Some(binding.clone());
        }
        ControlAction::Unbind(_control_id) => {
            *control = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::ControlBinding;

    fn make_binding(id: &str) -> ControlBinding {
        ControlBinding {
            control_id: id.to_string(),
            bound_at: 1000,
            cancel: tokio_util::sync::CancellationToken::new(),
        }
    }

    #[test]
    fn bind_sets_control() {
        let mut control: Option<ControlBinding> = None;
        let binding = make_binding("ctrl-123");

        apply_control(&mut control, &ControlAction::Bind(binding));
        assert!(control.is_some());
        assert_eq!(control.as_ref().unwrap().control_id, "ctrl-123");
    }

    #[test]
    fn unbind_clears_control() {
        let mut control: Option<ControlBinding> = Some(make_binding("ctrl-123"));
        assert!(control.is_some());

        apply_control(&mut control, &ControlAction::Unbind("ctrl-123".into()));
        assert!(control.is_none());
    }
}
