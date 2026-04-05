//! V1-hardening M2.2 — TaskSupervisor.
//!
//! All server background tasks funnel through this supervisor so that panics
//! are caught, logged, and counted via metrics — and critical tasks can be
//! restarted automatically with exponential backoff.
//!
//! Problem (before): the server called `tokio::spawn` in 17+ places across
//! main.rs, lib.rs, and live/manager/*. None were held in a `JoinSet`; if
//! any task panicked, the process kept running with a silently-degraded
//! subsystem (e.g. file watcher dead → no live updates ever again, with
//! nothing logged beyond the panic's own message vanishing into the ether).
//!
//! Solution: wrap every spawn in `TaskSupervisor::spawn` which:
//!   1. Catches panics via `AssertUnwindSafe(…).catch_unwind()`.
//!   2. Logs a structured record with the task name.
//!   3. Increments `task_panic_total{task_name=…}` counter.
//!   4. For `Critical` tasks, restarts with exponential backoff (1s → 60s).
//!   5. For `Fatal` tasks, triggers graceful shutdown via the shutdown watch
//!      channel (does NOT call `process::exit` — that would bypass Drop
//!      handlers and orphan the sidecar process).
//!
//! Precedent: Erlang/OTP supervisors, tokio-graceful-shutdown, every Erlang
//! VM ever run in production.

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use futures_util::FutureExt;
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// How the supervisor should react when a task panics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskCriticality {
    /// Panic → log + metrics. No restart. Appropriate for one-shot work
    /// like initial indexing or best-effort background jobs (telemetry
    /// pings, eager sidecar start).
    BestEffort,

    /// Panic → log + metrics + restart with exponential backoff
    /// (1 s → 2 s → 4 s → … capped at 60 s). Backoff resets to 1 s after
    /// the task runs for 60 s without panicking. Appropriate for
    /// long-running loops that MUST stay up (reconciler, drain_loop,
    /// classify handler, snapshot writer).
    Critical,

    /// Panic → log + metrics + signal graceful shutdown via the
    /// supervisor's shutdown watch channel. Appropriate for the single
    /// task that holds the irreplaceable OS resource (e.g. the filesystem
    /// watcher). The process tears down via the normal shutdown path so
    /// the sidecar, terminal WSs, and open sessions all clean up.
    Fatal,
}

/// Maximum backoff between restart attempts for `Critical` tasks.
const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// If a `Critical` task runs for at least this long without panicking,
/// its backoff resets to the initial 1-second delay.
const BACKOFF_RESET_WINDOW: Duration = Duration::from_secs(60);

/// The supervisor tracks every spawn it owns and provides a single
/// `shutdown_all` entrypoint for graceful teardown.
pub struct TaskSupervisor {
    /// Broadcast channel used to trigger shutdown from a Fatal panic.
    /// Owned elsewhere (`main`); we only hold the sender-side clone.
    shutdown_tx: watch::Sender<bool>,
    /// Holds `JoinHandle`s so we can abort + await on shutdown.
    handles: tokio::sync::Mutex<Vec<JoinHandle<()>>>,
}

impl TaskSupervisor {
    pub fn new(shutdown_tx: watch::Sender<bool>) -> Arc<Self> {
        Arc::new(Self {
            shutdown_tx,
            handles: tokio::sync::Mutex::new(Vec::new()),
        })
    }

    /// Spawn a supervised task.
    ///
    /// `factory` builds the future. It is called again on each restart
    /// for `Critical` tasks, so any state captured inside the closure
    /// must be `Clone` or `Arc`.
    ///
    /// `name` is used for structured logging and metrics. Keep it short
    /// and stable (it becomes a label value — not a message).
    pub fn spawn<F, Fut>(
        self: &Arc<Self>,
        name: &'static str,
        criticality: TaskCriticality,
        factory: F,
    ) where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let sup = Arc::clone(self);
        let factory = Arc::new(factory);
        let handle = tokio::spawn(async move {
            sup.run_supervised(name, criticality, factory).await;
        });
        // Record the handle so shutdown_all can abort it.
        if let Ok(mut guard) = self.handles.try_lock() {
            guard.push(handle);
        } else {
            // Contention only happens during shutdown; spawn the handle
            // without tracking it. This is a no-op risk because shutdown
            // will signal via the watch channel anyway.
            tracing::debug!(task = %name, "supervisor handles mutex contended; handle untracked");
        }
    }

    async fn run_supervised<F, Fut>(
        self: &Arc<Self>,
        name: &'static str,
        criticality: TaskCriticality,
        factory: Arc<F>,
    ) where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut backoff = Duration::from_secs(1);
        loop {
            let started = std::time::Instant::now();
            let fut = factory();
            let result = AssertUnwindSafe(fut).catch_unwind().await;
            match result {
                Ok(()) => {
                    tracing::debug!(task = %name, "supervised task completed normally");
                    metrics::counter!(
                        "supervised_task_completed_total",
                        "task" => name
                    )
                    .increment(1);
                    break;
                }
                Err(panic) => {
                    let msg = panic_message(&panic);
                    metrics::counter!(
                        "supervised_task_panic_total",
                        "task" => name
                    )
                    .increment(1);
                    tracing::error!(task = %name, panic = %msg, criticality = ?criticality,
                        "supervised task panicked");
                    match criticality {
                        TaskCriticality::BestEffort => break,
                        TaskCriticality::Critical => {
                            // If the task ran long enough, reset backoff.
                            if started.elapsed() >= BACKOFF_RESET_WINDOW {
                                backoff = Duration::from_secs(1);
                            }
                            tracing::warn!(task = %name, backoff_ms = backoff.as_millis() as u64,
                                "restarting supervised task after panic");
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(MAX_BACKOFF);
                            continue;
                        }
                        TaskCriticality::Fatal => {
                            tracing::error!(task = %name,
                                "FATAL supervised task panicked — triggering graceful shutdown");
                            // Signal shutdown via watch channel. Does NOT
                            // call process::exit (that would bypass Drop
                            // handlers and orphan the sidecar).
                            let _ = self.shutdown_tx.send(true);
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Await all tracked task handles, optionally aborting them if they
    /// don't return within `timeout`. Call this from the server's
    /// graceful-shutdown path.
    pub async fn shutdown_all(&self, timeout: Duration) {
        let mut handles = self.handles.lock().await;
        if handles.is_empty() {
            return;
        }
        tracing::info!(
            count = handles.len(),
            "awaiting supervised tasks for shutdown"
        );
        let drain = async {
            while let Some(h) = handles.pop() {
                let _ = h.await;
            }
        };
        match tokio::time::timeout(timeout, drain).await {
            Ok(()) => tracing::info!("all supervised tasks drained cleanly"),
            Err(_) => {
                tracing::warn!(
                    timeout_ms = timeout.as_millis() as u64,
                    "supervised tasks did not drain within timeout; aborting"
                );
                // `handles` is now partially drained; abort the rest.
                for h in handles.drain(..) {
                    h.abort();
                }
            }
        }
    }
}

/// Best-effort extraction of a panic message for logging.
fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_supervisor() -> (Arc<TaskSupervisor>, watch::Receiver<bool>) {
        let (tx, rx) = watch::channel(false);
        (TaskSupervisor::new(tx), rx)
    }

    #[tokio::test]
    async fn best_effort_does_not_restart_after_panic() {
        let (sup, _shutdown_rx) = make_supervisor();
        let runs = Arc::new(AtomicUsize::new(0));
        let runs_clone = runs.clone();
        sup.spawn("test_best_effort", TaskCriticality::BestEffort, move || {
            let runs = runs_clone.clone();
            async move {
                runs.fetch_add(1, Ordering::SeqCst);
                panic!("expected");
            }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        // BestEffort runs exactly once.
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn critical_restarts_with_backoff() {
        let (sup, _shutdown_rx) = make_supervisor();
        let runs = Arc::new(AtomicUsize::new(0));
        let runs_clone = runs.clone();
        sup.spawn("test_critical", TaskCriticality::Critical, move || {
            let runs = runs_clone.clone();
            async move {
                let n = runs.fetch_add(1, Ordering::SeqCst);
                // Panic on the first few, succeed on the 3rd.
                if n < 2 {
                    panic!("planned panic #{n}");
                }
                // Don't loop — let the supervisor break out.
            }
        });
        // First panic at t=0, restart at t=1s, second panic, restart at t=2s,
        // third attempt succeeds. Total ~3s; wait 4s.
        tokio::time::sleep(Duration::from_millis(3500)).await;
        let n = runs.load(Ordering::SeqCst);
        assert!(
            n >= 3,
            "expected ≥3 runs (panic, restart, panic, restart, ok), got {n}"
        );
    }

    #[tokio::test]
    async fn fatal_signals_shutdown() {
        let (sup, mut shutdown_rx) = make_supervisor();
        sup.spawn("test_fatal", TaskCriticality::Fatal, move || async move {
            panic!("fatal");
        });
        // Wait briefly for the shutdown signal.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            *shutdown_rx.borrow_and_update(),
            "shutdown watch should be true"
        );
    }

    #[tokio::test]
    async fn normal_completion_is_quiet() {
        let (sup, _shutdown_rx) = make_supervisor();
        let ran = Arc::new(AtomicUsize::new(0));
        let ran_clone = ran.clone();
        sup.spawn("test_normal", TaskCriticality::BestEffort, move || {
            let ran = ran_clone.clone();
            async move {
                ran.fetch_add(1, Ordering::SeqCst);
            }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(ran.load(Ordering::SeqCst), 1);
    }
}
