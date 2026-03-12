//! System monitor endpoints (SSE + REST).
//!
//! - `GET /monitor/stream`   -- SSE stream of periodic resource snapshots
//! - `GET /monitor/snapshot` -- One-shot JSON snapshot of current resources

use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, Sse},
    response::Json,
    routing::get,
    Router,
};

use crate::live::monitor::{
    collect_snapshot, collect_system_info, start_polling_task, MonitorEvent, ResourceSnapshot,
    SystemInfo,
};
use crate::state::AppState;

/// RAII guard that decrements the monitor subscriber count when dropped.
///
/// Guarantees the count is always decremented even if the SSE stream future
/// is dropped mid-way (e.g. client disconnect during the initial snapshot).
struct SubscriberGuard(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for SubscriberGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
        tracing::debug!("monitor SSE client disconnected");
    }
}

/// Build the monitor sub-router.
///
/// Routes:
/// - `GET /monitor/stream`   - SSE stream of resource snapshots (lazy polling)
/// - `GET /monitor/snapshot`  - One-shot JSON snapshot
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/monitor/stream", get(monitor_stream))
        .route("/monitor/snapshot", get(monitor_snapshot))
}

/// SSE response type combining system info (sent once) and periodic snapshots.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorInit {
    system_info: SystemInfo,
    snapshot: ResourceSnapshot,
}

/// GET /api/monitor/stream -- SSE stream of periodic resource snapshots.
///
/// # Lazy observer pattern
///
/// On first subscriber (0 → 1), spawns a background polling task that collects
/// system resources every 2 seconds. On last unsubscribe (1 → 0), the polling
/// task exits on its next tick. This avoids wasting CPU when no one is watching.
///
/// # Events
///
/// | Event name  | When emitted                       |
/// |-------------|------------------------------------|
/// | `init`      | On connect: system info + snapshot  |
/// | `snapshot`  | Every 2s: resource snapshot          |
/// | `heartbeat` | Every 15s: keepalive                 |
async fn monitor_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let tx = state.monitor_tx.clone();
    let subscribers = state.monitor_subscribers.clone();
    let live_sessions = state.live_sessions.clone();
    let mut shutdown = state.shutdown.clone();

    // Lazy start: if 0 → 1, spawn the polling task
    let prev = subscribers.fetch_add(1, Ordering::SeqCst);
    if prev == 0 {
        start_polling_task(tx.clone(), subscribers.clone(), live_sessions.clone());
    }
    // Guard lives outside the stream so it's dropped on any exit path —
    // normal completion, client disconnect, or panic.
    let _guard = SubscriberGuard(subscribers.clone());

    let mut rx = tx.subscribe();

    let stream = async_stream::stream! {
        // 1. Send init event with system info + first snapshot
        let system_info = collect_system_info();
        let first_snapshot = {
            let sessions = live_sessions.read().await;
            let sessions_clone = sessions.clone();
            drop(sessions);
            let mut sys = sysinfo::System::new_all();
            tokio::task::spawn_blocking(move || {
                // Brief sleep so sysinfo CPU delta is non-zero on first measurement
                std::thread::sleep(Duration::from_millis(200));
                collect_snapshot(&mut sys, &sessions_clone)
            })
            .await
            .unwrap_or_else(|_| ResourceSnapshot {
                timestamp: chrono::Utc::now().timestamp(),
                cpu_percent: 0.0,
                memory_used_bytes: 0,
                memory_total_bytes: 0,
                disk_used_bytes: 0,
                disk_total_bytes: 0,
                top_processes: Vec::new(),
                session_resources: Vec::new(),
            })
        };

        let init = MonitorInit {
            system_info,
            snapshot: first_snapshot,
        };
        match serde_json::to_string(&init) {
            Ok(data) => yield Ok(Event::default().event("init").data(data)),
            Err(e) => tracing::error!("failed to serialize monitor init: {e}"),
        }

        // 2. Stream snapshots from broadcast channel
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(MonitorEvent::Snapshot(snapshot)) => {
                            match serde_json::to_string(&snapshot) {
                                Ok(data) => yield Ok(Event::default().event("snapshot").data(data)),
                                Err(e) => tracing::error!("failed to serialize snapshot: {e}"),
                            }
                        }
                        Ok(MonitorEvent::ProcessTree(tree)) => {
                            match serde_json::to_string(&tree) {
                                Ok(data) => yield Ok(Event::default().event("process_tree").data(data)),
                                Err(e) => tracing::error!("failed to serialize process_tree: {e}"),
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("monitor SSE client lagged by {n} events");
                            // Just continue — next snapshot will arrive shortly
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat_interval.tick() => {
                    yield Ok(Event::default().event("heartbeat").data("{}"));
                }
                _ = shutdown.changed() => {
                    break;
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("{}"),
    )
}

/// GET /api/monitor/snapshot -- One-shot JSON snapshot of current resources.
async fn monitor_snapshot(State(state): State<Arc<AppState>>) -> Json<ResourceSnapshot> {
    let sessions = {
        let map = state.live_sessions.read().await;
        map.clone()
    };
    let mut sys = sysinfo::System::new_all();
    let snapshot = tokio::task::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        collect_snapshot(&mut sys, &sessions)
    })
    .await
    .unwrap_or_else(|_| ResourceSnapshot {
        timestamp: chrono::Utc::now().timestamp(),
        cpu_percent: 0.0,
        memory_used_bytes: 0,
        memory_total_bytes: 0,
        disk_used_bytes: 0,
        disk_total_bytes: 0,
        top_processes: Vec::new(),
        session_resources: Vec::new(),
    });
    Json(snapshot)
}
