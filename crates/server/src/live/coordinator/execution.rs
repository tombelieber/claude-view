//! Side-effect execution — async IO, called after the write lock is dropped.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};
use tracing::debug;

use super::types::MutationContext;
use crate::live::mutation::types::SideEffect;
use crate::live::state::HookEvent;

/// Execute a single side effect. Called after the write lock is dropped.
pub async fn execute_side_effect(ctx: &MutationContext<'_>, effect: &SideEffect) {
    match effect {
        SideEffect::RemoveAccumulator { session_id } => {
            if let Some(mgr) = ctx.live_manager {
                mgr.remove_accumulator(session_id).await;
            }
        }
        SideEffect::EnrichFromJsonl {
            session_id: _,
            file_path,
        } => {
            if let Some(mgr) = ctx.live_manager {
                mgr.process_jsonl_update(file_path).await;
            }
        }
        SideEffect::CreateAccumulator { session_id } => {
            if let Some(mgr) = ctx.live_manager {
                mgr.create_accumulator_for_hook(session_id).await;
            }
        }
        SideEffect::CleanHookEventChannel { session_id } => {
            let mut channels = ctx.hook_event_channels.write().await;
            channels.remove(session_id.as_str());
        }
        SideEffect::CleanTranscriptDedup { path } => {
            let mut map = ctx.transcript_to_session.write().await;
            map.remove(path);
        }
        SideEffect::PersistHookEvents { session_id, events } => {
            let rows: Vec<_> = events.iter().map(|e| e.to_row()).collect();
            if let Err(e) =
                claude_view_db::hook_events_queries::insert_hook_events(ctx.db, session_id, &rows)
                    .await
            {
                tracing::warn!(
                    session_id,
                    error = %e,
                    "Failed to persist hook events"
                );
            } else {
                debug!(
                    session_id,
                    count = rows.len(),
                    "Persisted hook events to DB"
                );
            }
        }
        SideEffect::SavePidBinding { session_id, pid } => {
            debug!(
                session_id,
                pid, "Would save PID binding (handled by snapshot writer)"
            );
        }
        SideEffect::EvictSession { session_id, reason } => {
            debug!(session_id, reason, "Would evict session");
            // Future: remove from session map + broadcast Removed
        }
    }
}

// ---------------------------------------------------------------------------
// Hook event WS forwarding
// ---------------------------------------------------------------------------

/// Forward a hook event to the per-session WS broadcast channel, if any
/// listeners are subscribed. Non-blocking — drops event if no listeners.
pub async fn forward_hook_event_to_ws(
    channels: &Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>>,
    session_id: &str,
    event: Option<HookEvent>,
) {
    if let Some(event) = event {
        let channels = channels.read().await;
        if let Some(tx) = channels.get(session_id) {
            let _ = tx.send(event);
        }
    }
}
