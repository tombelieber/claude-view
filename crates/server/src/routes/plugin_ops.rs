// crates/server/src/routes/plugin_ops.rs
//! Queued plugin operations — accepts mutations immediately, processes serially
//! in a background task. Replaces the old try_lock/409 pattern.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::Json;
use serde::Serialize;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use ts_rs::TS;

const DEFAULT_EVICTION_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub enum OpStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginOp {
    pub id: String,
    pub action: String,
    pub name: String,
    pub status: OpStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[ts(skip)]
    #[serde(skip)]
    completed_at: Option<Instant>,
}

struct Inner {
    pending: VecDeque<PluginOp>,
    active: Vec<PluginOp>,
    eviction_ttl: Duration,
}

pub struct PluginOpQueue {
    inner: Mutex<Inner>,
}

impl Default for PluginOpQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginOpQueue {
    pub fn new() -> Self {
        Self::with_eviction_ttl(DEFAULT_EVICTION_TTL)
    }

    pub fn with_eviction_ttl(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(Inner {
                pending: VecDeque::new(),
                active: Vec::new(),
                eviction_ttl: ttl,
            }),
        }
    }

    pub fn enqueue(
        &self,
        action: &str,
        name: &str,
        scope: Option<&str>,
        project_path: Option<&str>,
    ) -> PluginOp {
        let op = PluginOp {
            id: uuid::Uuid::new_v4().to_string(),
            action: action.to_string(),
            name: name.to_string(),
            status: OpStatus::Queued,
            scope: scope.map(String::from),
            project_path: project_path.map(String::from),
            message: None,
            error: None,
            completed_at: None,
        };
        let snapshot = op.clone();
        self.inner.lock().unwrap().pending.push_back(op);
        snapshot
    }

    pub fn take_next(&self) -> Option<PluginOp> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(mut op) = inner.pending.pop_front() {
            op.status = OpStatus::Running;
            let snapshot = op.clone();
            inner.active.push(op);
            Some(snapshot)
        } else {
            None
        }
    }

    pub fn complete_op(&self, id: &str, result: Result<String, String>) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(op) = inner.active.iter_mut().find(|o| o.id == id) {
            match result {
                Ok(msg) => {
                    op.status = OpStatus::Completed;
                    op.message = if msg.is_empty() { None } else { Some(msg) };
                }
                Err(err) => {
                    op.status = OpStatus::Failed;
                    op.error = Some(err);
                }
            }
            op.completed_at = Some(Instant::now());
        }
    }

    pub fn list_ops(&self) -> Vec<PluginOp> {
        let mut inner = self.inner.lock().unwrap();
        let ttl = inner.eviction_ttl;
        let now = Instant::now();

        // Lazy eviction: remove completed/failed ops older than TTL
        inner.active.retain(|op| {
            if let Some(completed_at) = op.completed_at {
                now.duration_since(completed_at) < ttl
            } else {
                true // running ops are never evicted
            }
        });

        let mut ops: Vec<PluginOp> = Vec::new();
        // Pending first (FIFO order), then active (running/completed/failed)
        for op in &inner.pending {
            ops.push(op.clone());
        }
        for op in &inner.active {
            ops.push(op.clone());
        }
        ops
    }
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// POST /api/plugins/ops — Enqueue a plugin mutation, return immediately.
#[utoipa::path(post, path = "/api/plugins/ops", tag = "plugins",
    request_body = crate::routes::plugins::PluginActionRequest,
    responses(
        (status = 200, description = "Operation enqueued", body = PluginOp),
    )
)]
pub async fn enqueue_op(
    State(state): State<Arc<crate::state::AppState>>,
    Json(req): Json<super::plugins::PluginActionRequest>,
) -> crate::error::ApiResult<Json<PluginOp>> {
    super::plugins::validate_plugin_name(&req.name)?;
    super::plugins::validate_scope(&req.scope)?;
    if !super::plugins::VALID_ACTIONS.contains(&req.action.as_str()) {
        return Err(crate::error::ApiError::BadRequest(format!(
            "Invalid action: {}. Must be one of: {}",
            req.action,
            super::plugins::VALID_ACTIONS.join(", ")
        )));
    }

    let op = state.plugin_op_queue.enqueue(
        &req.action,
        &req.name,
        req.scope.as_deref(),
        req.project_path.as_deref(),
    );
    state.plugin_op_notify.notify_one();
    Ok(Json(op))
}

/// GET /api/plugins/ops — List all queued/running/completed ops.
#[utoipa::path(get, path = "/api/plugins/ops", tag = "plugins",
    responses(
        (status = 200, description = "All plugin operations", body = Vec<PluginOp>),
    )
)]
pub async fn list_ops_handler(State(state): State<Arc<crate::state::AppState>>) -> Json<Vec<PluginOp>> {
    Json(state.plugin_op_queue.list_ops())
}

pub fn router() -> axum::Router<Arc<crate::state::AppState>> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/plugins/ops", post(enqueue_op))
        .route("/plugins/ops", get(list_ops_handler))
}

// ---------------------------------------------------------------------------
// Executor — bridges the queue worker to the CLI helpers in plugins.rs
// ---------------------------------------------------------------------------

/// Execute a single plugin operation by calling the CLI.
/// Used as the executor function for `spawn_op_worker`.
pub async fn execute_plugin_op(
    state: &crate::state::AppState,
    op: PluginOp,
) -> Result<String, String> {
    let mut args: Vec<&str> = vec![&op.action, &op.name];
    let scope_str;
    if op.action == "install" || op.action == "uninstall" {
        if let Some(ref scope) = op.scope {
            scope_str = scope.clone();
            args.push("--scope");
            args.push(&scope_str);
        }
    }

    let cwd = op.project_path.as_deref();
    match super::plugins::run_claude_plugin_in(&args, cwd, 30).await {
        Ok(stdout) => {
            super::plugins::invalidate_plugin_cache(state).await;
            let msg = stdout.trim().to_string();
            Ok(msg)
        }
        Err(e) => Err(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Background worker
// ---------------------------------------------------------------------------

/// Spawn a background task that serially processes queued plugin operations.
///
/// The worker sleeps on `notify` until woken, then drains all pending ops.
/// Each op is executed via the injected `executor` function.
pub fn spawn_op_worker<F, Fut>(
    queue: Arc<PluginOpQueue>,
    notify: Arc<Notify>,
    executor: F,
) -> JoinHandle<()>
where
    F: Fn(PluginOp) -> Fut + Send + 'static,
    Fut: Future<Output = Result<String, String>> + Send,
{
    tokio::spawn(async move {
        loop {
            // Drain all pending ops before sleeping
            while let Some(op) = queue.take_next() {
                let id = op.id.clone();
                let result = executor(op).await;
                queue.complete_op(&id, result);
            }
            // Sleep until new work arrives
            notify.notified().await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Core queue tests
    // -----------------------------------------------------------------------

    #[test]
    fn enqueue_returns_id_with_queued_status() {
        let queue = PluginOpQueue::new();
        let op = queue.enqueue("install", "superpowers", Some("user"), None);

        assert!(!op.id.is_empty(), "op id must be non-empty");
        assert_eq!(op.status, OpStatus::Queued);
        assert_eq!(op.action, "install");
        assert_eq!(op.name, "superpowers");
    }

    #[test]
    fn multiple_enqueues_never_rejected() {
        let queue = PluginOpQueue::new();

        let op1 = queue.enqueue("install", "plugin-a", None, None);
        let op2 = queue.enqueue("update", "plugin-b", None, None);
        let op3 = queue.enqueue("uninstall", "plugin-c", Some("user"), None);

        // All three accepted — no 409 rejection
        assert_eq!(op1.status, OpStatus::Queued);
        assert_eq!(op2.status, OpStatus::Queued);
        assert_eq!(op3.status, OpStatus::Queued);

        // All have unique IDs
        assert_ne!(op1.id, op2.id);
        assert_ne!(op2.id, op3.id);
    }

    #[test]
    fn list_ops_returns_all_ops() {
        let queue = PluginOpQueue::new();
        queue.enqueue("install", "a", None, None);
        queue.enqueue("update", "b", None, None);

        let ops = queue.list_ops();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].name, "a");
        assert_eq!(ops[1].name, "b");
    }

    #[test]
    fn take_next_returns_fifo_order() {
        let queue = PluginOpQueue::new();
        queue.enqueue("install", "first", None, None);
        queue.enqueue("install", "second", None, None);

        let next = queue.take_next();
        assert!(next.is_some());
        assert_eq!(next.unwrap().name, "first");

        let next = queue.take_next();
        assert!(next.is_some());
        assert_eq!(next.unwrap().name, "second");

        let next = queue.take_next();
        assert!(next.is_none(), "queue should be empty");
    }

    #[test]
    fn take_next_marks_op_as_running() {
        let queue = PluginOpQueue::new();
        queue.enqueue("install", "plugin-x", None, None);

        let taken = queue.take_next().unwrap();
        assert_eq!(taken.status, OpStatus::Running);

        // list_ops should reflect the running status
        let ops = queue.list_ops();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].status, OpStatus::Running);
    }

    #[test]
    fn complete_op_marks_success() {
        let queue = PluginOpQueue::new();
        let enqueued = queue.enqueue("install", "superpowers", None, None);

        let _taken = queue.take_next().unwrap();
        queue.complete_op(&enqueued.id, Ok("Installed successfully".into()));

        let ops = queue.list_ops();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].status, OpStatus::Completed);
        assert_eq!(ops[0].message.as_deref(), Some("Installed successfully"));
        assert!(ops[0].error.is_none());
    }

    #[test]
    fn complete_op_marks_failure() {
        let queue = PluginOpQueue::new();
        let enqueued = queue.enqueue("install", "bad-plugin", None, None);

        let _taken = queue.take_next().unwrap();
        queue.complete_op(&enqueued.id, Err("Network timeout".into()));

        let ops = queue.list_ops();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].status, OpStatus::Failed);
        assert!(ops[0].message.is_none());
        assert_eq!(ops[0].error.as_deref(), Some("Network timeout"));
    }

    #[test]
    fn completed_ops_evicted_after_ttl() {
        let queue = PluginOpQueue::with_eviction_ttl(std::time::Duration::from_millis(50));
        let enqueued = queue.enqueue("install", "temp", None, None);

        let _taken = queue.take_next().unwrap();
        queue.complete_op(&enqueued.id, Ok("done".into()));

        // Immediately visible
        assert_eq!(queue.list_ops().len(), 1);

        // Wait past TTL
        std::thread::sleep(std::time::Duration::from_millis(80));

        // Evicted on next read
        assert_eq!(queue.list_ops().len(), 0);
    }

    #[test]
    fn queued_and_running_ops_not_evicted() {
        let queue = PluginOpQueue::with_eviction_ttl(std::time::Duration::from_millis(10));
        queue.enqueue("install", "waiting", None, None);

        std::thread::sleep(std::time::Duration::from_millis(30));

        // Queued ops must survive eviction regardless of age
        assert_eq!(queue.list_ops().len(), 1);
        assert_eq!(queue.list_ops()[0].status, OpStatus::Queued);
    }

    #[test]
    fn enqueue_preserves_scope_and_project_path() {
        let queue = PluginOpQueue::new();
        let _op = queue.enqueue(
            "uninstall",
            "old-plugin",
            Some("project"),
            Some("/home/user/proj"),
        );

        let ops = queue.list_ops();
        assert_eq!(ops[0].scope.as_deref(), Some("project"));
        assert_eq!(ops[0].project_path.as_deref(), Some("/home/user/proj"));
    }

    // -----------------------------------------------------------------------
    // Background worker tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn worker_processes_enqueued_op() {
        let queue = std::sync::Arc::new(PluginOpQueue::new());
        let notify = std::sync::Arc::new(tokio::sync::Notify::new());

        queue.enqueue("install", "test-plugin", None, None);

        let results = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let results_clone = results.clone();

        // Executor that records what it processed
        let executor = move |op: PluginOp| {
            let results = results_clone.clone();
            async move {
                results.lock().unwrap().push(op.name.clone());
                Ok(format!("Installed {}", op.name))
            }
        };

        let handle = spawn_op_worker(queue.clone(), notify.clone(), executor);
        notify.notify_one();

        // Give worker time to process
        tokio::time::sleep(Duration::from_millis(50)).await;
        handle.abort();

        let processed = results.lock().unwrap();
        assert_eq!(processed.len(), 1);
        assert_eq!(processed[0], "test-plugin");

        let ops = queue.list_ops();
        assert_eq!(ops[0].status, OpStatus::Completed);
        assert_eq!(ops[0].message.as_deref(), Some("Installed test-plugin"));
    }

    #[tokio::test]
    async fn worker_handles_executor_failure() {
        let queue = std::sync::Arc::new(PluginOpQueue::new());
        let notify = std::sync::Arc::new(tokio::sync::Notify::new());

        queue.enqueue("install", "broken-plugin", None, None);

        let executor = |_op: PluginOp| async { Err::<String, String>("CLI crashed".into()) };

        let handle = spawn_op_worker(queue.clone(), notify.clone(), executor);
        notify.notify_one();

        tokio::time::sleep(Duration::from_millis(50)).await;
        handle.abort();

        let ops = queue.list_ops();
        assert_eq!(ops[0].status, OpStatus::Failed);
        assert_eq!(ops[0].error.as_deref(), Some("CLI crashed"));
    }

    #[tokio::test]
    async fn worker_processes_multiple_ops_serially() {
        let queue = std::sync::Arc::new(PluginOpQueue::new());
        let notify = std::sync::Arc::new(tokio::sync::Notify::new());

        queue.enqueue("install", "first", None, None);
        queue.enqueue("install", "second", None, None);
        queue.enqueue("install", "third", None, None);

        let order = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let order_clone = order.clone();

        let executor = move |op: PluginOp| {
            let order = order_clone.clone();
            async move {
                order.lock().unwrap().push(op.name.clone());
                Ok(format!("done {}", op.name))
            }
        };

        let handle = spawn_op_worker(queue.clone(), notify.clone(), executor);
        notify.notify_one();

        tokio::time::sleep(Duration::from_millis(100)).await;
        handle.abort();

        let processed = order.lock().unwrap();
        assert_eq!(*processed, vec!["first", "second", "third"]);
    }

    #[tokio::test]
    async fn worker_wakes_on_notify_for_new_ops() {
        let queue = std::sync::Arc::new(PluginOpQueue::new());
        let notify = std::sync::Arc::new(tokio::sync::Notify::new());

        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count_clone = count.clone();

        let executor = move |_op: PluginOp| {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok("ok".into())
            }
        };

        let handle = spawn_op_worker(queue.clone(), notify.clone(), executor);

        // Enqueue after worker is running — worker should be sleeping, then wake on notify
        tokio::time::sleep(Duration::from_millis(20)).await;
        queue.enqueue("install", "late-arrival", None, None);
        notify.notify_one();

        tokio::time::sleep(Duration::from_millis(50)).await;
        handle.abort();

        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
