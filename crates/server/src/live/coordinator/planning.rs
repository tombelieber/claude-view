//! Side-effect planning — pure, captures data before mutation clears it.

use crate::live::mutation::types::{
    InteractionAction, LifecycleEvent, SessionMutation, SideEffect,
};
use crate::live::state::LiveSession;

/// Plan side effects that need to happen after the mutation.
/// Called BEFORE the mutation so we can capture data that may be cleared.
pub fn plan_side_effects(
    session_id: &str,
    session: &LiveSession,
    mutation: &SessionMutation,
    _now: i64,
) -> Vec<SideEffect> {
    let mut effects = Vec::new();

    match mutation {
        SessionMutation::Lifecycle(LifecycleEvent::End { .. }) => {
            // Capture hook events before End clears them
            if !session.hook.hook_events.is_empty() {
                effects.push(SideEffect::PersistHookEvents {
                    session_id: session_id.to_string(),
                    events: session.hook.hook_events.clone(),
                });
            }
            effects.push(SideEffect::RemoveAccumulator {
                session_id: session_id.to_string(),
            });
            effects.push(SideEffect::CleanHookEventChannel {
                session_id: session_id.to_string(),
            });
        }
        SessionMutation::Lifecycle(LifecycleEvent::Start { .. }) => {
            effects.push(SideEffect::CreateAccumulator {
                session_id: session_id.to_string(),
            });
        }
        SessionMutation::Interaction(InteractionAction::Set {
            meta, full_data, ..
        }) => {
            effects.push(SideEffect::SetInteractionData {
                request_id: meta.request_id.clone(),
                data: full_data.clone(),
            });
        }
        SessionMutation::Interaction(InteractionAction::Clear { request_id }) => {
            effects.push(SideEffect::ClearInteractionData {
                request_id: request_id.clone(),
            });
        }
        _ => {}
    }

    effects
}
