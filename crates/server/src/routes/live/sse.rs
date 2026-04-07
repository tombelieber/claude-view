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
/// | Event name          | When emitted                           |
/// |---------------------|----------------------------------------|
/// | `summary`           | On connect, and when a client lags     |
/// | `session_discovered`| New session detected                   |
/// | `session_updated`   | Session state changed                  |
/// | `session_closed`    | Session process exited (recently closed) |
/// | `session_completed` | Session ended                          |
/// | `heartbeat`         | Every 15 seconds to keep connection    |
///
/// On initial connection, the server sends the current summary followed by
/// all active sessions so the client can hydrate immediately without a
/// separate REST call.
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
        // 1. On connect: send current summary + all active sessions
        {
            let map = sessions.read().await;
            let pc = live_manager.as_ref().map(|m| m.process_count()).unwrap_or(0);
            let summary = build_summary(&map, pc);
            match serde_json::to_string(&summary) {
                Ok(data) => yield Ok(Event::default().event("summary").data(data)),
                Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
            }
            for session in map.values() {
                let event_name = if session.closed_at.is_some() {
                    "session_closed"
                } else {
                    "session_discovered"
                };
                match serde_json::to_string(session) {
                    Ok(data) => yield Ok(Event::default().event(event_name).data(data)),
                    Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                }
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
                                SessionEvent::SessionDiscovered { .. } => "session_discovered",
                                SessionEvent::SessionUpdated { .. } => "session_updated",
                                SessionEvent::SessionClosed { .. } => "session_closed",
                                SessionEvent::SessionCompleted { .. } => "session_completed",
                                SessionEvent::Summary { .. } => "summary",
                            };
                            match serde_json::to_string(&session_event) {
                                Ok(data) => yield Ok(Event::default().event(event_name).data(data)),
                                Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "SSE client lagged by {} events, re-sending all sessions",
                                n
                            );
                            // Re-send full state (same as initial connect) so the
                            // client recovers from any missed discover/complete events.
                            let map = sessions.read().await;
                            let pc = live_manager.as_ref().map(|m| m.process_count()).unwrap_or(0);
                            let summary = build_summary(&map, pc);
                            match serde_json::to_string(&summary) {
                                Ok(data) => yield Ok(Event::default().event("summary").data(data)),
                                Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                            }
                            for session in map.values() {
                                let event_name = if session.closed_at.is_some() {
                                    "session_closed"
                                } else {
                                    "session_discovered"
                                };
                                match serde_json::to_string(session) {
                                    Ok(data) => yield Ok(Event::default().event(event_name).data(data)),
                                    Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                                }
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
