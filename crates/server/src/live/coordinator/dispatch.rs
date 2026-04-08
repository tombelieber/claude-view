//! Mutation dispatch — pure, no IO.
//!
//! Routes a `SessionMutation` to the appropriate apply function and handles
//! cross-source field merging (model, context_window_tokens, turn_count).

use crate::live::mutation::apply_control::apply_control;
use crate::live::mutation::apply_lifecycle::apply_lifecycle;
use crate::live::mutation::apply_reconcile::apply_reconcile;
use crate::live::mutation::apply_statusline::apply_statusline;
use crate::live::mutation::types::{LifecycleEvent, SessionMutation};
use crate::live::state::{LiveSession, SessionStatus};

/// Dispatch a mutation to the appropriate apply function, returning any
/// status change. Handles cross-source fields (model) inline.
pub fn apply_mutation_to_session(
    session: &mut LiveSession,
    mutation: &SessionMutation,
    now: i64,
) -> Option<SessionStatus> {
    match mutation {
        SessionMutation::Statusline(ref payload) => {
            apply_statusline(&mut session.statusline, payload);
            // Cross-source: context_window_tokens (derived from current_usage)
            if let Some(ref cw) = payload.context_window {
                if let Some(ref usage) = cw.current_usage {
                    let fill = usage.input_tokens.unwrap_or(0)
                        + usage.cache_creation_input_tokens.unwrap_or(0)
                        + usage.cache_read_input_tokens.unwrap_or(0);
                    if fill > 0 {
                        session.context_window_tokens = fill;
                    }
                }
            }
            // Cross-source: model — timestamp-guarded, empty-string rejected
            if let Some(ref m) = payload.model {
                if now >= session.model_set_at {
                    if let Some(ref id) = m.id {
                        if !id.is_empty() {
                            session.model = Some(id.clone());
                            session.model_set_at = now;
                        }
                    }
                    if let Some(ref dn) = m.display_name {
                        if !dn.is_empty() {
                            session.model_display_name = Some(dn.clone());
                        }
                    }
                }
            }
            None
        }
        SessionMutation::Lifecycle(event) => apply_lifecycle(&mut session.hook, event, now),
        SessionMutation::Reconcile(data) => {
            apply_reconcile(&mut session.jsonl, data);
            // Cross-source: model (only if newer than current)
            if let Some(ref m) = data.model {
                if now >= session.model_set_at {
                    session.model = Some(m.clone());
                    session.model_set_at = now;
                }
            }
            if let Some(ref md) = data.model_display_name {
                session.model_display_name = Some(md.clone());
            }
            // Cross-source: context_window_tokens
            if let Some(cwt) = data.context_window_tokens {
                session.context_window_tokens = cwt;
            }
            // Cross-source: turn_count (JSONL may have it)
            if let Some(tc) = data.turn_count {
                if tc > session.hook.turn_count {
                    session.hook.turn_count = tc;
                }
            }
            None
        }
        SessionMutation::Control(action) => {
            apply_control(&mut session.control, action);
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Common post-mutation bookkeeping
// ---------------------------------------------------------------------------

use crate::live::mutation::types::CommonPostMutation;

/// Extract common post-mutation fields from the mutation.
pub fn common_post_mutation(
    mutation: &SessionMutation,
    caller_pid: Option<u32>,
    now: i64,
) -> CommonPostMutation {
    // Extract PID from lifecycle events, falling back to caller-provided PID.
    // `mutation` is borrowed, so `pid` fields are `&Option<u32>`.
    let bind_pid = match mutation {
        SessionMutation::Lifecycle(LifecycleEvent::Start { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Prompt { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::StateChange { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Stop { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::StopFailure { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Compacted { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::CwdChanged { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::Observability { pid, .. })
        | SessionMutation::Lifecycle(LifecycleEvent::SubagentStarted { pid, .. }) => {
            (*pid).or(caller_pid)
        }
        _ => caller_pid,
    };

    CommonPostMutation {
        bind_pid,
        update_activity_at: Some(now),
    }
}
