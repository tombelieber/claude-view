//! SSE stream endpoint for real-time session events.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, Sse},
};

use crate::live::state::SessionEvent;
use crate::state::AppState;

use super::summary::build_summary;

/// GET /api/live/stream -- SSE stream of real-time session events.
///
/// # Events
///
/// | Event name            | When emitted                           |
/// |-----------------------|----------------------------------------|
/// | `snapshot`            | On connect and lag recovery (full state)|
/// | `session_upsert`      | Session created or updated             |
/// | `session_remove`      | Session removed from active map        |
/// | `cli_session_created` | CLI session created                    |
/// | `cli_session_updated` | CLI session status changed             |
/// | `cli_session_removed` | CLI session killed                     |
/// | `heartbeat`           | Every 15 seconds to keep connection    |
///
/// On initial connection the server sends a single `snapshot` event containing
/// the summary, all active sessions, and recently closed sessions so the client
/// can hydrate immediately without a separate REST call.
#[utoipa::path(get, path = "/api/live/stream", tag = "live",
    responses(
        (status = 200, description = "SSE stream of live session events", content_type = "text/event-stream"),
    )
)]
pub async fn live_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.live_tx.subscribe();
    let sessions = state.live_sessions.clone();
    let live_manager = state.live_manager.clone();
    let mut shutdown = state.shutdown.clone();

    let stream = async_stream::stream! {
        // 1. On connect: send snapshot (summary + all active sessions + recently closed)
        {
            let map = sessions.read().await;
            let pc = live_manager.as_ref().map(|m| m.process_count()).unwrap_or(0);
            let summary = build_summary(&map, pc);
            let active: Vec<&crate::live::state::LiveSession> = map.values().collect();
            let closed: Vec<crate::live::state::LiveSession> = {
                let ring = state.closed_ring.read().await;
                ring.iter().cloned().collect()
            };
            let snapshot = serde_json::json!({
                "summary": summary,
                "sessions": active,
                "recentlyClosed": closed,
            });
            match serde_json::to_string(&snapshot) {
                Ok(data) => yield Ok(Event::default().event("snapshot").data(data)),
                Err(e) => tracing::error!("failed to serialize snapshot: {e}"),
            }
        }

        // 2. Stream events from broadcast channel with heartbeat
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(session_event) => {
                            let event_name = match &session_event {
                                SessionEvent::SessionUpsert { .. } => "session_upsert",
                                SessionEvent::SessionRemove { .. } => "session_remove",
                                SessionEvent::CliSessionCreated { .. } => "cli_session_created",
                                SessionEvent::CliSessionUpdated { .. } => "cli_session_updated",
                                SessionEvent::CliSessionRemoved { .. } => "cli_session_removed",
                            };
                            // No enrichment needed — ownership is a stored field
                            // in the session record. Just serialize directly.
                            match serde_json::to_string(&session_event) {
                                Ok(data) => yield Ok(Event::default().event(event_name).data(data)),
                                Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "SSE client lagged by {} events, re-sending snapshot",
                                n
                            );
                            // Re-send full snapshot (same as initial connect) so the
                            // client recovers from any missed events.
                            let map = sessions.read().await;
                            let pc = live_manager.as_ref().map(|m| m.process_count()).unwrap_or(0);
                            let summary = build_summary(&map, pc);
                            let active: Vec<&crate::live::state::LiveSession> = map.values().collect();
                            let closed: Vec<crate::live::state::LiveSession> = {
                                let ring = state.closed_ring.read().await;
                                ring.iter().cloned().collect()
                            };
                            let snapshot = serde_json::json!({
                                "summary": summary,
                                "sessions": active,
                                "recentlyClosed": closed,
                            });
                            match serde_json::to_string(&snapshot) {
                                Ok(data) => yield Ok(Event::default().event("snapshot").data(data)),
                                Err(e) => tracing::error!("failed to serialize snapshot: {e}"),
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat_interval.tick() => {
                    yield Ok(Event::default().event("heartbeat").data("{}"));
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
            }
        }
    };

    Sse::new(stream)
}
