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
    // Create guard EAGERLY (before Sse is returned) so subscriber_count never
    // drops back to 0 during the window between fetch_add and the first stream poll.
    // async_stream::stream! expands to `async move { ... }`, so _guard is captured
    // by move into the stream's state. It is NOT dropped when monitor_stream()
    // returns — only when the stream future is dropped (client disconnect, shutdown,
    // or broadcast close). Re-binding _guard inside the stream makes the move
    // explicit and prevents the compiler from treating it as unused.
    //
    // BUG HISTORY: placing the guard in the outer fn scope dropped it the moment
    // monitor_stream() returned Sse<>, decrementing subscribers to 0 and causing
    // the polling task to exit on its next 2s tick. Data froze after the init event.
    let _guard = SubscriberGuard(subscribers.clone());

    let mut rx = tx.subscribe();

    let stream = async_stream::stream! {
        // Re-bind so the move into this async block is explicit.
        let _guard = _guard;

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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    // -------------------------------------------------------------------------
    // SubscriberGuard lifecycle
    // -------------------------------------------------------------------------

    /// Guard decrements the shared counter exactly once on drop.
    #[test]
    fn subscriber_guard_decrements_count_on_drop() {
        let count = Arc::new(AtomicUsize::new(1));
        {
            let _guard = SubscriberGuard(count.clone());
            // Guard created — counter unchanged (SubscriberGuard only decrements)
            assert_eq!(count.load(Ordering::SeqCst), 1);
        } // guard dropped here
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    /// Two guards for two concurrent subscribers: each drop decrements once.
    #[test]
    fn two_subscriber_guards_decrement_independently() {
        let count = Arc::new(AtomicUsize::new(2));
        let g1 = SubscriberGuard(count.clone());
        let g2 = SubscriberGuard(count.clone());

        drop(g1);
        assert_eq!(count.load(Ordering::SeqCst), 1, "first drop: 2 → 1");
        drop(g2);
        assert_eq!(count.load(Ordering::SeqCst), 0, "second drop: 1 → 0");
    }

    // -------------------------------------------------------------------------
    // Polling task lifecycle
    // -------------------------------------------------------------------------

    /// Polling task emits at least one snapshot while subscriber_count > 0,
    /// then stops broadcasting after the count is set to 0.
    ///
    /// Regression guard for: guard placed in outer async fn scope → dropped on
    /// handler return → subscriber_count hits 0 → polling task exits → data freezes.
    #[tokio::test]
    async fn polling_task_stops_when_subscriber_count_reaches_zero() {
        use crate::live::monitor::{start_polling_task, MonitorEvent};
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        let (tx, mut rx) = broadcast::channel::<MonitorEvent>(32);
        let count = Arc::new(AtomicUsize::new(1)); // simulate one connected client
        let sessions = Arc::new(RwLock::new(HashMap::new()));

        start_polling_task(tx.clone(), count.clone(), sessions);

        // Must receive at least one snapshot while subscribed (within 5s to account
        // for sysinfo's initial CPU baseline sleep of ~200ms + 2s poll interval).
        let received = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await;
        assert!(
            received.is_ok(),
            "Should receive a MonitorEvent::Snapshot while subscriber_count > 0"
        );

        // Signal disconnect: set count to 0 (what SubscriberGuard::drop does).
        count.store(0, Ordering::Relaxed);

        // Drain any events already buffered before the task noticed count == 0.
        while rx.try_recv().is_ok() {}

        // Wait long enough for the polling task to complete one full 2s interval
        // and observe subscriber_count == 0, then exit its loop.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // After the task exits, no new events should arrive.
        assert!(
            rx.try_recv().is_err(),
            "Polling task should have stopped after subscriber_count reached 0 — \
             data would freeze if this fails (the original bug)"
        );
    }

    /// Polling task continues broadcasting while subscriber_count stays above 0
    /// — verifies the task does NOT stop prematurely.
    ///
    /// Regression guard for: guard dropped too early → count = 0 → task exits
    /// after first snapshot (the original bug in disguise).
    #[tokio::test]
    async fn polling_task_keeps_running_while_subscriber_count_is_nonzero() {
        use crate::live::monitor::{start_polling_task, MonitorEvent};
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        let (tx, mut rx) = broadcast::channel::<MonitorEvent>(32);
        let count = Arc::new(AtomicUsize::new(1));
        let sessions = Arc::new(RwLock::new(HashMap::new()));

        start_polling_task(tx.clone(), count.clone(), sessions);

        // Receive two distinct snapshots to prove the task polls more than once.
        let first = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("first snapshot within 5s");
        assert!(first.is_ok(), "first snapshot should be Ok");

        let second = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("second snapshot within 5s");
        assert!(
            second.is_ok(),
            "second snapshot should be Ok — polling task must not exit prematurely"
        );

        count.store(0, Ordering::Relaxed); // clean up
    }
}
