// crates/server/src/routes/marketplace_refresh.rs
//! Marketplace refresh tracking — tracks batch refresh operations for
//! marketplace plugins with status transitions and TTL eviction.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

const STALENESS_TTL: Duration = Duration::from_secs(5 * 60);
const EVICTION_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub enum RefreshStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RefreshOp {
    pub status: RefreshStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RefreshAllRequest {
    pub names: Option<Vec<String>>,
}

#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RefreshAllResponse {
    pub count: usize,
}

#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RefreshStatusResponse {
    pub active: bool,
    pub ops: HashMap<String, RefreshOp>,
}

struct RefreshInner {
    ops: HashMap<String, RefreshOp>,
    batch_active: bool,
    batch_started_at: Option<Instant>,
    completed_at: Option<Instant>,
}

pub struct MarketplaceRefreshTracker {
    inner: Mutex<RefreshInner>,
}

impl Default for MarketplaceRefreshTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketplaceRefreshTracker {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(RefreshInner {
                ops: HashMap::new(),
                batch_active: false,
                batch_started_at: None,
                completed_at: None,
            }),
        }
    }

    /// Returns true if a batch is active AND started less than 5 minutes ago.
    pub fn is_active(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if !inner.batch_active {
            return false;
        }
        match inner.batch_started_at {
            Some(started) => started.elapsed() < STALENESS_TTL,
            None => false,
        }
    }

    /// Begin a new refresh batch. Clears previous ops, sets all names to Queued.
    pub fn start_batch(&self, names: &[String]) {
        let mut inner = self.inner.lock().unwrap();
        inner.ops.clear();
        for name in names {
            inner.ops.insert(
                name.clone(),
                RefreshOp {
                    status: RefreshStatus::Queued,
                    error: None,
                },
            );
        }
        inner.batch_active = true;
        inner.batch_started_at = Some(Instant::now());
        inner.completed_at = None;
    }

    pub fn set_running(&self, name: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(op) = inner.ops.get_mut(name) {
            op.status = RefreshStatus::Running;
        }
    }

    pub fn set_completed(&self, name: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(op) = inner.ops.get_mut(name) {
            op.status = RefreshStatus::Completed;
        }
    }

    pub fn set_failed(&self, name: &str, error: String) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(op) = inner.ops.get_mut(name) {
            op.status = RefreshStatus::Failed;
            op.error = Some(error);
        }
    }

    /// Mark batch as finished, record completion time.
    pub fn finish_batch(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.batch_active = false;
        inner.completed_at = Some(Instant::now());
    }

    /// Returns a snapshot of the current state. Evicts ops if the batch
    /// completed more than 30 seconds ago.
    pub fn status_snapshot(&self) -> RefreshStatusResponse {
        let mut inner = self.inner.lock().unwrap();

        // Evict if completed_at is older than EVICTION_TTL
        if let Some(completed_at) = inner.completed_at {
            if completed_at.elapsed() >= EVICTION_TTL {
                inner.ops.clear();
                inner.completed_at = None;
            }
        }

        RefreshStatusResponse {
            active: inner.batch_active
                && inner
                    .batch_started_at
                    .map_or(false, |s| s.elapsed() < STALENESS_TTL),
            ops: inner.ops.clone(),
        }
    }

    /// Test helper: manually set batch_started_at for staleness testing.
    #[allow(dead_code)]
    #[cfg(test)]
    fn set_batch_started_at(&self, instant: Instant) {
        self.inner.lock().unwrap().batch_started_at = Some(instant);
    }

    /// Test helper: manually set completed_at for eviction testing.
    #[allow(dead_code)]
    #[cfg(test)]
    fn set_completed_at(&self, instant: Instant) {
        self.inner.lock().unwrap().completed_at = Some(instant);
    }
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// CLI JSON shape for `claude plugin marketplace list --json`.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliMarketplace {
    name: String,
}

/// Fetch marketplace names from the CLI.
async fn fetch_marketplace_names() -> Result<Vec<String>, ApiError> {
    let json =
        super::plugins::run_claude_plugin_in(&["marketplace", "list", "--json"], None, 30).await?;
    let markets: Vec<CliMarketplace> = serde_json::from_str(&json).unwrap_or_default();
    Ok(markets.into_iter().map(|m| m.name).collect())
}

/// POST /api/plugins/marketplaces/refresh-all
///
/// Starts a batch refresh of all (or specified) marketplaces.
/// Returns 409 if a batch is already active.
async fn refresh_all(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RefreshAllRequest>,
) -> ApiResult<Json<RefreshAllResponse>> {
    if state.marketplace_refresh.is_active() {
        return Err(ApiError::Conflict(
            "A marketplace refresh is already in progress.".into(),
        ));
    }

    let names = match req.names {
        Some(n) if !n.is_empty() => n,
        _ => fetch_marketplace_names().await?,
    };

    if names.is_empty() {
        return Ok(Json(RefreshAllResponse { count: 0 }));
    }

    let count = names.len();
    state.marketplace_refresh.start_batch(&names);

    // Spawn background orchestrator
    let tracker = state.marketplace_refresh.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        // Acquire marketplace lock so we don't conflict with individual marketplace actions
        let _guard = super::plugins::get_marketplace_lock().lock().await;

        let mut handles = Vec::new();
        for name in names {
            let tracker = tracker.clone();
            let handle = tokio::spawn(async move {
                tracker.set_running(&name);
                let result = super::plugins::run_claude_plugin_in(
                    &["marketplace", "update", &name],
                    None,
                    60,
                )
                .await;
                match result {
                    Ok(_) => tracker.set_completed(&name),
                    Err(e) => tracker.set_failed(&name, e.to_string()),
                }
            });
            handles.push(handle);
        }

        futures_util::future::join_all(handles).await;
        tracker.finish_batch();
        super::plugins::invalidate_plugin_cache(&state_clone).await;
    });

    Ok(Json(RefreshAllResponse { count }))
}

/// GET /api/plugins/marketplaces/refresh-status
async fn refresh_status(State(state): State<Arc<AppState>>) -> Json<RefreshStatusResponse> {
    Json(state.marketplace_refresh.status_snapshot())
}

pub fn router() -> axum::Router<Arc<AppState>> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/plugins/marketplaces/refresh-all", post(refresh_all))
        .route("/plugins/marketplaces/refresh-status", get(refresh_status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_is_not_active() {
        let tracker = MarketplaceRefreshTracker::new();
        assert!(!tracker.is_active());
    }

    #[test]
    fn start_batch_sets_queued_and_active() {
        let tracker = MarketplaceRefreshTracker::new();
        let names = vec!["plugin-a".to_string(), "plugin-b".to_string()];
        tracker.start_batch(&names);

        assert!(tracker.is_active());

        let snap = tracker.status_snapshot();
        assert!(snap.active);
        assert_eq!(snap.ops.len(), 2);
        assert_eq!(snap.ops["plugin-a"].status, RefreshStatus::Queued);
        assert_eq!(snap.ops["plugin-b"].status, RefreshStatus::Queued);
        assert!(snap.ops["plugin-a"].error.is_none());
    }

    #[test]
    fn status_transitions_queued_to_running_to_completed() {
        let tracker = MarketplaceRefreshTracker::new();
        tracker.start_batch(&["pkg".to_string()]);

        // Queued -> Running
        tracker.set_running("pkg");
        let snap = tracker.status_snapshot();
        assert_eq!(snap.ops["pkg"].status, RefreshStatus::Running);

        // Running -> Completed
        tracker.set_completed("pkg");
        let snap = tracker.status_snapshot();
        assert_eq!(snap.ops["pkg"].status, RefreshStatus::Completed);
        assert!(snap.ops["pkg"].error.is_none());
    }

    #[test]
    fn status_transitions_queued_to_running_to_failed() {
        let tracker = MarketplaceRefreshTracker::new();
        tracker.start_batch(&["bad-pkg".to_string()]);

        tracker.set_running("bad-pkg");
        tracker.set_failed("bad-pkg", "network timeout".to_string());

        let snap = tracker.status_snapshot();
        assert_eq!(snap.ops["bad-pkg"].status, RefreshStatus::Failed);
        assert_eq!(
            snap.ops["bad-pkg"].error.as_deref(),
            Some("network timeout")
        );
    }

    #[test]
    fn finish_batch_sets_inactive() {
        let tracker = MarketplaceRefreshTracker::new();
        tracker.start_batch(&["x".to_string()]);
        assert!(tracker.is_active());

        tracker.finish_batch();
        assert!(!tracker.is_active());
    }

    #[test]
    fn is_active_returns_false_after_staleness() {
        let tracker = MarketplaceRefreshTracker::new();
        tracker.start_batch(&["stale".to_string()]);

        // Manually set batch_started_at to 6 minutes ago
        let six_min_ago = Instant::now() - Duration::from_secs(6 * 60);
        tracker.set_batch_started_at(six_min_ago);

        // is_active() should return false despite batch_active=true
        assert!(!tracker.is_active());

        // status_snapshot should also report active=false
        let snap = tracker.status_snapshot();
        assert!(!snap.active);
    }

    #[test]
    fn status_snapshot_evicts_after_ttl() {
        let tracker = MarketplaceRefreshTracker::new();
        tracker.start_batch(&["evict-me".to_string()]);
        tracker.set_completed("evict-me");
        tracker.finish_batch();

        // Before eviction: ops still visible
        let snap = tracker.status_snapshot();
        assert_eq!(snap.ops.len(), 1);

        // Manually set completed_at to 31 seconds ago
        let past = Instant::now() - Duration::from_secs(31);
        tracker.set_completed_at(past);

        // After eviction: ops should be cleared
        let snap = tracker.status_snapshot();
        assert!(snap.ops.is_empty());
    }

    #[test]
    fn start_batch_resets_previous_ops() {
        let tracker = MarketplaceRefreshTracker::new();

        // First batch
        tracker.start_batch(&["old-a".to_string(), "old-b".to_string()]);
        tracker.set_completed("old-a");
        tracker.set_completed("old-b");
        tracker.finish_batch();

        // Second batch — previous ops should be cleared
        tracker.start_batch(&["new-x".to_string()]);

        let snap = tracker.status_snapshot();
        assert_eq!(snap.ops.len(), 1);
        assert!(snap.ops.contains_key("new-x"));
        assert!(!snap.ops.contains_key("old-a"));
        assert!(!snap.ops.contains_key("old-b"));
    }

    #[test]
    fn default_impl_matches_new() {
        let tracker = MarketplaceRefreshTracker::default();
        assert!(!tracker.is_active());
        let snap = tracker.status_snapshot();
        assert!(!snap.active);
        assert!(snap.ops.is_empty());
    }

    #[test]
    fn set_on_unknown_name_is_noop() {
        let tracker = MarketplaceRefreshTracker::new();
        tracker.start_batch(&["known".to_string()]);

        // Operations on unknown names should not panic
        tracker.set_running("unknown");
        tracker.set_completed("unknown");
        tracker.set_failed("unknown", "err".to_string());

        // Known op is unaffected
        let snap = tracker.status_snapshot();
        assert_eq!(snap.ops["known"].status, RefreshStatus::Queued);
    }
}
