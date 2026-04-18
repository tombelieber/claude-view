//! Per-session fsnotify debouncer.
//!
//! Coalesces a burst of `FileEvent::Modified` events for the same
//! session into a single re-index after `DEBOUNCE_MS` of quiet. Each
//! session has at most one in-flight scheduled task; rescheduling
//! aborts the previous one so we never accumulate a backlog of stale
//! re-index attempts on hot files.
//!
//! ## Why per-session, not global
//!
//! A global "wait N ms then drain everything" timer would couple every
//! session's latency to the slowest-writing file in the tree. Per-key
//! debouncing keeps each session's re-index latency bounded by D1 +
//! parse cost (~500 ms + ~1 ms for p95 file sizes).
//!
//! ## Cancellation safety
//!
//! `JoinHandle::abort()` interrupts the debounce sleep but **cannot**
//! interrupt an `index_session` call that has already started — that
//! call runs to completion (UPSERT-safe). The next event for the same
//! session simply schedules a fresh debounce after the running call
//! returns. Worst case: one extra UPSERT on a busy session. That's
//! the design's accepted trade for keeping the fast-path lock-free.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Per-entry tracking state. The `gen` field disambiguates "my own
/// cleanup" from "someone else rescheduled while I was running" — the
/// task that owns a generation only removes the map entry if the
/// stored generation still matches its own.
#[derive(Debug)]
struct Pending {
    gen: u64,
    handle: JoinHandle<()>,
}

/// Per-key debouncer. Generic over the key type so tests can use
/// short string keys; production use threads `String` (session UUIDs).
#[derive(Debug)]
pub struct Debouncer<K> {
    delay: Duration,
    in_flight: Arc<Mutex<HashMap<K, Pending>>>,
    next_gen: Arc<AtomicU64>,
}

impl<K> Debouncer<K>
where
    K: Eq + std::hash::Hash + Clone + Send + 'static,
{
    /// Create a new debouncer with the given coalesce window.
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            next_gen: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Schedule `task` to run after `delay` quiet time on `key`. If a
    /// previous task for `key` is still pending, it is aborted first.
    ///
    /// `task` is a `FnOnce` returning a future. It is built fresh on
    /// each schedule call (so each scheduling captures its own context
    /// — file path, hash, etc.).
    pub async fn schedule<F, Fut>(&self, key: K, task: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send,
    {
        let delay = self.delay;
        let map = self.in_flight.clone();
        let key_for_task = key.clone();
        let my_gen = self.next_gen.fetch_add(1, Ordering::Relaxed);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            task().await;
            // Cleanup: remove the entry IFF its generation still
            // matches mine. If a newer schedule replaced me, its
            // generation is strictly greater and we leave it alone.
            // This keeps the map bounded by the steady-state count of
            // active sessions, not by lifetime cumulative schedules.
            let mut guard = map.lock().await;
            if let Some(existing) = guard.get(&key_for_task) {
                if existing.gen == my_gen {
                    guard.remove(&key_for_task);
                }
            }
        });

        let mut guard = self.in_flight.lock().await;
        if let Some(prev) = guard.insert(
            key,
            Pending {
                gen: my_gen,
                handle,
            },
        ) {
            prev.handle.abort();
        }
    }

    /// How many sessions currently have a pending debounce task.
    /// Reported by the orchestrator's metrics.
    pub async fn pending_count(&self) -> usize {
        self.in_flight.lock().await.len()
    }
}

impl<K> Default for Debouncer<K>
where
    K: Eq + std::hash::Hash + Clone + Send + 'static,
{
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}

#[cfg(test)]
mod tests {
    //! Tests use a real (multi-thread) tokio runtime instead of the
    //! `start_paused` pattern: spawned tasks need multiple polls to
    //! progress under a paused clock, and getting that orchestration
    //! right is more brittle than just running with short real delays.
    //! Wall-time per test is ~150 ms — comfortable on every dev box
    //! and reliable in CI.

    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Schedule once; the task runs exactly once after the delay.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn single_schedule_runs_after_delay() {
        let counter = Arc::new(AtomicUsize::new(0));
        let debouncer: Debouncer<&'static str> = Debouncer::new(Duration::from_millis(50));

        let c = counter.clone();
        debouncer
            .schedule("sess-1", move || async move {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // Before the delay elapses nothing has run.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // Past the delay + execution: exactly one increment.
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// Reschedule before the delay elapses; only the latest task runs.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rescheduling_aborts_pending_task() {
        let counter = Arc::new(AtomicUsize::new(0));
        let debouncer: Debouncer<&'static str> = Debouncer::new(Duration::from_millis(150));

        for _ in 0..5 {
            let c = counter.clone();
            debouncer
                .schedule("hot-session", move || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                })
                .await;
            // Each schedule arrives 30 ms after the previous — well
            // inside the 150 ms window — so each one aborts its
            // predecessor before its sleep elapses.
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        // Wait past the debounce window of the LAST schedule.
        tokio::time::sleep(Duration::from_millis(250)).await;

        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "only the last scheduled task in a burst should run"
        );
    }

    /// Two distinct keys debounce independently.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn distinct_keys_run_independently() {
        let a_counter = Arc::new(AtomicUsize::new(0));
        let b_counter = Arc::new(AtomicUsize::new(0));
        let debouncer: Debouncer<&'static str> = Debouncer::new(Duration::from_millis(50));

        let ca = a_counter.clone();
        let cb = b_counter.clone();
        debouncer
            .schedule("a", move || async move {
                ca.fetch_add(1, Ordering::SeqCst);
            })
            .await;
        debouncer
            .schedule("b", move || async move {
                cb.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        assert_eq!(a_counter.load(Ordering::SeqCst), 1);
        assert_eq!(b_counter.load(Ordering::SeqCst), 1);
    }

    /// `pending_count` reflects in-flight scheduled tasks.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pending_count_tracks_scheduled_keys() {
        let debouncer: Debouncer<&'static str> = Debouncer::new(Duration::from_millis(50));

        debouncer.schedule("a", || async {}).await;
        debouncer.schedule("b", || async {}).await;
        debouncer.schedule("c", || async {}).await;
        assert_eq!(debouncer.pending_count().await, 3);

        // After the delay + post-task cleanup, the map shrinks.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(debouncer.pending_count().await, 0);
    }
}
