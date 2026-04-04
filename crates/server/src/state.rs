// crates/server/src/state.rs
//! Application state for the Axum server.

use crate::auth::supabase::JwksCache;
use crate::cache::CachedUpstream;
use crate::classify_state::ClassifyState;
use crate::facet_ingest::FacetIngestState;
use crate::git_sync_state::GitSyncState;
use crate::indexing_state::IndexingState;
use crate::jobs::JobRunner;
use crate::live::buffer::PendingMutations;
use crate::live::coordinator::{MutationContext, SessionCoordinator};
use crate::live::debug_log::DebugEventLog;
use crate::live::manager::{LiveSessionManager, LiveSessionMap, TranscriptMap};
use crate::live::state::SessionEvent;
use crate::routes::marketplace_refresh::MarketplaceRefreshTracker;
use crate::routes::oauth::OAuthUsageResponse;
use crate::routes::plugin_ops::PluginOpQueue;
use crate::routes::plugins::CliAvailableResponse;
use crate::routes::statusline::StatuslinePayload;
use crate::sidecar::SidecarManager;
use crate::terminal_state::TerminalConnectionManager;
use claude_view_core::prompt_history::PromptStats;
use claude_view_core::Registry;
use claude_view_db::{Database, PricingTable};
use claude_view_search::prompt_index::PromptSearchIndex;
use claude_view_search::SearchIndex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::sync::{broadcast, OnceCell};

/// Cached identity from `claude auth status` (email, org, plan).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthIdentity {
    pub email: Option<String>,
    pub org_name: Option<String>,
    pub subscription_type: Option<String>,
    pub auth_method: Option<String>,
}

/// Configuration for the conversation sharing feature.
/// Only populated when SHARE_WORKER_URL and SHARE_VIEWER_URL are set.
pub struct ShareConfig {
    pub worker_url: String,
    pub viewer_url: String,
    pub http_client: reqwest::Client,
}

/// Type alias for the shared registry holder.
///
/// The registry is `None` until background indexing builds it, then `Some(Registry)`.
/// Uses `std::sync::RwLock` (not `tokio::sync::RwLock`) because:
/// - The registry is written exactly once from the background task
/// - Read operations are uncontended after the initial write
/// - No need to hold the lock across `.await` points
pub type RegistryHolder = Arc<RwLock<Option<Registry>>>;

/// Type alias for the runtime-swappable search index holder.
///
/// Follows the same `Arc<RwLock<Option<...>>>` pattern as `RegistryHolder`.
/// `None` means the index is unavailable (not yet opened, or mid-swap during clear-cache).
/// Wrapped in `Arc` so `clear_cache` can take-drop-recreate without blocking readers.
pub type SearchIndexHolder = Arc<RwLock<Option<Arc<SearchIndex>>>>;

/// Type alias for the prompt search index holder.
pub type PromptIndexHolder = Arc<RwLock<Option<Arc<PromptSearchIndex>>>>;

/// Type alias for the prompt stats holder.
pub type PromptStatsHolder = Arc<RwLock<Option<PromptStats>>>;

/// Type alias for the prompt templates holder.
pub type PromptTemplatesHolder =
    Arc<RwLock<Option<Vec<claude_view_core::prompt_templates::PromptTemplate>>>>;

/// Type alias for the cached list of detected IDEs.
///
/// Each entry is `(IdeInfo, resolved_command_path)`.
pub type AvailableIdesHolder = Vec<(crate::routes::ide::IdeInfo, String)>;

/// Shared application state accessible from all route handlers.
pub struct AppState {
    /// Server start time for uptime tracking.
    pub start_time: Instant,
    /// Database handle for session/project queries.
    pub db: Database,
    /// Shared indexing progress state (lock-free atomics).
    pub indexing: Arc<IndexingState>,
    /// Shared git-sync progress state (lock-free atomics, resettable).
    pub git_sync: Arc<GitSyncState>,
    /// Invocable registry (skills, commands, MCP tools, built-in tools).
    /// `None` until background indexing completes registry build.
    pub registry: RegistryHolder,
    /// Background job runner for long-running async tasks (classification, etc.)
    pub jobs: Arc<JobRunner>,
    /// Classification progress state (lock-free atomics for SSE streaming).
    pub classify: Arc<ClassifyState>,
    /// Facet ingest progress state (lock-free atomics for SSE streaming).
    pub facet_ingest: Arc<FacetIngestState>,
    /// Per-model pricing table for accurate cost calculation (immutable after init).
    pub pricing: Arc<PricingTable>,
    /// Live session state for Live Monitor (in-memory, not persisted).
    pub live_sessions: LiveSessionMap,
    /// Ephemeral recently-closed sessions — captured on reap, lost on restart.
    pub recently_closed: LiveSessionMap,
    /// Broadcast sender for live session SSE events.
    pub live_tx: broadcast::Sender<SessionEvent>,
    /// Directory where coaching rule files are stored (~/.claude/rules).
    pub rules_dir: PathBuf,
    /// WebSocket connection manager for live terminal monitoring.
    pub terminal_connections: Arc<TerminalConnectionManager>,
    /// Live session manager (for hook handler to create/remove accumulators).
    /// `None` in test factories that don't start the manager.
    pub live_manager: Option<Arc<LiveSessionManager>>,
    /// Full-text search index (Tantivy), runtime-swappable.
    /// `None` inside the RwLock until the index is initialized, or mid-swap during clear-cache.
    pub search_index: SearchIndexHolder,
    /// Shutdown signal receiver. When `true`, SSE streams should terminate cleanly.
    pub shutdown: tokio::sync::watch::Receiver<bool>,
    /// Per-session broadcast channels for hook events (WebSocket streaming).
    /// Key: session_id. Created on demand when a WS connects, cleaned up on SessionEnd.
    pub hook_event_channels: Arc<
        tokio::sync::RwLock<
            HashMap<String, tokio::sync::broadcast::Sender<crate::live::state::HookEvent>>,
        >,
    >,
    /// Node.js sidecar manager for Phase F interactive control.
    /// Lazy-started on first `/api/control/*` request.
    pub sidecar: Arc<SidecarManager>,
    /// Supabase JWKS cache for JWT validation (sharing feature).
    /// `None` when SUPABASE_URL is not set (auth disabled / dev mode).
    pub jwks: Option<Arc<tokio::sync::RwLock<JwksCache>>>,
    /// Sharing configuration (Worker URL, viewer URL, HTTP client).
    /// `None` when SHARE_WORKER_URL / SHARE_VIEWER_URL are not set.
    pub share: Option<ShareConfig>,
    /// Cached auth identity from `claude auth status` (lazy, one-shot).
    pub auth_identity: OnceCell<Option<AuthIdentity>>,
    /// Cached Anthropic OAuth usage response (5-min TTL).
    pub oauth_usage_cache: CachedUpstream<OAuthUsageResponse>,
    /// Cached `claude plugin list --available --json` response (5-min TTL).
    /// Shared by both `/api/plugins` and `/api/plugins/marketplaces`.
    pub(crate) plugin_cli_cache: CachedUpstream<CliAvailableResponse>,
    /// Parsed teams from ~/.claude/teams/ (read-only, loaded at startup).
    pub teams: Arc<crate::teams::TeamsStore>,
    /// Prompt history search index (Tantivy).
    pub prompt_index: PromptIndexHolder,
    /// Prompt history aggregate stats.
    pub prompt_stats: PromptStatsHolder,
    /// Detected prompt templates.
    pub prompt_templates: PromptTemplatesHolder,
    /// IDEs detected at startup (cached for the lifetime of the server).
    pub available_ides: AvailableIdesHolder,
    /// Broadcast sender for system monitor resource snapshots (SSE).
    pub monitor_tx: broadcast::Sender<crate::live::monitor::MonitorEvent>,
    /// Number of active SSE subscribers to the system monitor.
    /// When 0→1, the polling task starts. When 1→0, it stops.
    pub monitor_subscribers: Arc<AtomicUsize>,
    /// Unified process oracle receiver (latest-value semantics via tokio::watch).
    /// Both the monitor SSE and manager reconciliation loop read from this.
    pub oracle_rx: crate::live::process_oracle::OracleReceiver,
    /// Queued plugin operations (replaces the old try_lock/409 mutex pattern).
    pub plugin_op_queue: Arc<PluginOpQueue>,
    /// Notify channel to wake the plugin op worker when new ops are enqueued.
    pub plugin_op_notify: Arc<tokio::sync::Notify>,
    /// Marketplace refresh tracker for batch update operations.
    pub marketplace_refresh: Arc<MarketplaceRefreshTracker>,
    /// Transcript path → session ID map for dedup (used by statusline handler).
    /// Prevents duplicate sessions when Claude Code restarts with a new session ID
    /// but the same transcript file path.
    pub transcript_to_session: TranscriptMap,
    /// Buffer for statusline payloads arriving before session discovery.
    /// Drained when session is created (hooks.rs SessionStart or lazy creation).
    /// NOTE: Legacy — will be removed once hooks.rs is wired through coordinator (Task 10).
    pub pending_statusline: tokio::sync::Mutex<PendingMutations<StatuslinePayload>>,
    /// Single entry point for all session state mutations.
    /// Replaces scattered lock-acquire → mutate → broadcast patterns.
    /// Shared with `LiveSessionManager` when the manager is active.
    pub coordinator: Arc<SessionCoordinator>,
    /// PostHog telemetry client. `None` when no PostHog API key is compiled in.
    pub telemetry: Option<crate::telemetry::TelemetryClient>,
    /// Path to the telemetry config file (allows tests to use temp dirs).
    pub telemetry_config_path: std::path::PathBuf,
    /// Rolling JSONL debug log for statusline events (debug builds only).
    pub debug_statusline_log: Option<DebugEventLog>,
    /// Rolling JSONL debug log for hook events (debug builds only).
    pub debug_hooks_log: Option<DebugEventLog>,
    /// Rolling JSONL debug log for oMLX API calls (debug builds only).
    pub debug_omlx_log: Option<DebugEventLog>,
    /// Local LLM service (on-device AI).
    pub local_llm: Arc<crate::local_llm::LocalLlmService>,
    /// Connection registry for multiplexed session WebSockets (Stage 1+4).
    pub session_channels: Arc<crate::live::session_ws::registry::SessionChannelRegistry>,
}

/// Builder for constructing `AppState` with optional overrides.
///
/// Call `AppState::builder(db)` to start, chain `.with_*()` methods,
/// then `.build()` to produce an `Arc<AppState>`.
pub struct AppStateBuilder {
    db: Database,
    indexing: Option<Arc<IndexingState>>,
    registry: Option<RegistryHolder>,
    shutdown: Option<tokio::sync::watch::Receiver<bool>>,
}

impl AppStateBuilder {
    /// Override the `IndexingState`.
    pub fn with_indexing(mut self, indexing: Arc<IndexingState>) -> Self {
        self.indexing = Some(indexing);
        self
    }

    /// Override the shared registry holder.
    pub fn with_registry(mut self, registry: RegistryHolder) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Override the shutdown watch receiver.
    pub fn with_shutdown(mut self, shutdown: tokio::sync::watch::Receiver<bool>) -> Self {
        self.shutdown = Some(shutdown);
        self
    }

    /// Build the `AppState` wrapped in an `Arc`.
    pub fn build(self) -> Arc<AppState> {
        let (debug_statusline_log, debug_hooks_log, debug_omlx_log) = if cfg!(debug_assertions) {
            (
                Some(DebugEventLog::new(".debug/statusline.jsonl")),
                Some(DebugEventLog::new(".debug/hooks.jsonl")),
                Some(DebugEventLog::new(".debug/omlx.jsonl")),
            )
        } else {
            (None, None, None)
        };
        Arc::new(AppState {
            start_time: Instant::now(),
            db: self.db,
            indexing: self
                .indexing
                .unwrap_or_else(|| Arc::new(IndexingState::new())),
            git_sync: Arc::new(GitSyncState::new()),
            registry: self.registry.unwrap_or_else(|| Arc::new(RwLock::new(None))),
            jobs: Arc::new(JobRunner::new()),
            classify: Arc::new(ClassifyState::new()),
            facet_ingest: Arc::new(FacetIngestState::new()),
            pricing: Arc::new(claude_view_core::pricing::load_pricing()),
            live_sessions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            recently_closed: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            live_tx: broadcast::channel(256).0,
            rules_dir: dirs::home_dir()
                .expect("home dir exists")
                .join(".claude")
                .join("rules"),
            terminal_connections: Arc::new(TerminalConnectionManager::new()),
            live_manager: None,
            search_index: Arc::new(RwLock::new(None)),
            shutdown: self
                .shutdown
                .unwrap_or_else(|| tokio::sync::watch::channel(false).1),
            hook_event_channels: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            sidecar: Arc::new(SidecarManager::new()),
            jwks: None,
            share: None,
            auth_identity: OnceCell::new(),
            oauth_usage_cache: CachedUpstream::new(std::time::Duration::from_secs(300)),
            plugin_cli_cache: CachedUpstream::new(std::time::Duration::from_secs(300)),
            teams: Arc::new(crate::teams::TeamsStore::empty()),
            prompt_index: Arc::new(RwLock::new(None)),
            prompt_stats: Arc::new(RwLock::new(None)),
            prompt_templates: Arc::new(RwLock::new(None)),
            available_ides: Vec::new(),
            monitor_tx: broadcast::channel(64).0,
            monitor_subscribers: Arc::new(AtomicUsize::new(0)),
            oracle_rx: crate::live::process_oracle::stub(),
            plugin_op_queue: Arc::new(PluginOpQueue::new()),
            plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
            marketplace_refresh: Arc::new(MarketplaceRefreshTracker::new()),
            transcript_to_session: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            pending_statusline: tokio::sync::Mutex::new(PendingMutations::new(
                std::time::Duration::from_secs(120),
            )),
            coordinator: Arc::new(SessionCoordinator::new()),
            telemetry: None,
            telemetry_config_path: claude_view_core::telemetry_config::telemetry_config_path(),
            debug_statusline_log,
            debug_hooks_log,
            debug_omlx_log,
            local_llm: Arc::new(crate::local_llm::LocalLlmService::new(
                Arc::new(crate::local_llm::LocalLlmConfig::new_disabled()),
                Arc::new(crate::local_llm::LlmStatus::new(10710)),
            )),
            session_channels: Arc::new(
                crate::live::session_ws::registry::SessionChannelRegistry::new(),
            ),
        })
    }
}

impl AppState {
    /// Create a builder for `AppState`.
    pub fn builder(db: Database) -> AppStateBuilder {
        AppStateBuilder {
            db,
            indexing: None,
            registry: None,
            shutdown: None,
        }
    }

    /// Create a new application state with all defaults. Delegates to builder.
    pub fn new(db: Database) -> Arc<Self> {
        Self::builder(db).build()
    }

    /// Deprecated — use `AppState::builder(db).with_indexing(i).build()`.
    #[deprecated(note = "Use AppState::builder(db).with_indexing(i).build() instead")]
    #[allow(dead_code)]
    pub fn new_with_indexing(db: Database, indexing: Arc<IndexingState>) -> Arc<Self> {
        Self::builder(db).with_indexing(indexing).build()
    }

    /// Create with both an external `IndexingState` and a shared registry holder.
    /// Deprecated — use `AppState::builder(db).with_indexing(i).with_registry(r).build()`.
    #[deprecated(
        note = "Use AppState::builder(db).with_indexing(i).with_registry(r).build() instead"
    )]
    #[allow(dead_code)]
    pub fn new_with_indexing_and_registry(
        db: Database,
        indexing: Arc<IndexingState>,
        registry: RegistryHolder,
    ) -> Arc<Self> {
        Self::builder(db)
            .with_indexing(indexing)
            .with_registry(registry)
            .build()
    }

    /// Build a `MutationContext` borrowing shared state for one coordinator call.
    pub fn mutation_context(&self) -> MutationContext<'_> {
        MutationContext {
            sessions: &self.live_sessions,
            live_tx: &self.live_tx,
            live_manager: self.live_manager.as_ref(),
            db: &self.db,
            transcript_to_session: &self.transcript_to_session,
            hook_event_channels: &self.hook_event_channels,
        }
    }

    /// Get the server uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    /// Helper to create an AppState with an in-memory database for testing.
    async fn test_state() -> Arc<AppState> {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        AppState::new(db)
    }

    #[tokio::test]
    async fn test_app_state_new() {
        let state = test_state().await;
        assert!(state.uptime_secs() < 1);
    }

    #[tokio::test]
    async fn test_app_state_uptime() {
        let state = test_state().await;
        sleep(Duration::from_millis(100));
        let uptime = state.uptime_secs();
        assert!(uptime < 5);
    }

    #[tokio::test]
    async fn test_app_state_clone() {
        let state = test_state().await;
        let cloned = state.clone();
        assert_eq!(state.uptime_secs(), cloned.uptime_secs());
    }

    // ================================================================
    // Stage 0B: AppState builder pattern tests
    // ================================================================

    #[tokio::test]
    async fn test_builder_defaults_match_new() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let state = AppState::builder(db).build();
        assert!(state.live_manager.is_none());
        assert!(state.jwks.is_none());
        assert!(state.share.is_none());
        assert!(state.telemetry.is_none());
        assert!(state.uptime_secs() < 1);
    }

    #[tokio::test]
    async fn test_builder_with_indexing() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let indexing = Arc::new(IndexingState::new());
        let indexing_clone = indexing.clone();
        let state = AppState::builder(db).with_indexing(indexing).build();
        assert!(Arc::ptr_eq(&state.indexing, &indexing_clone));
    }

    #[tokio::test]
    async fn test_builder_with_registry() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let registry: RegistryHolder = Arc::new(RwLock::new(None));
        let registry_clone = registry.clone();
        let state = AppState::builder(db).with_registry(registry).build();
        assert!(Arc::ptr_eq(&state.registry, &registry_clone));
    }

    #[tokio::test]
    async fn test_builder_with_indexing_and_registry() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let indexing = Arc::new(IndexingState::new());
        let registry: RegistryHolder = Arc::new(RwLock::new(None));
        let indexing_clone = indexing.clone();
        let registry_clone = registry.clone();
        let state = AppState::builder(db)
            .with_indexing(indexing)
            .with_registry(registry)
            .build();
        assert!(Arc::ptr_eq(&state.indexing, &indexing_clone));
        assert!(Arc::ptr_eq(&state.registry, &registry_clone));
    }

    #[tokio::test]
    async fn test_builder_with_shutdown() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let state = AppState::builder(db).with_shutdown(shutdown_rx).build();
        assert!(!*state.shutdown.borrow());
        shutdown_tx.send(true).unwrap();
        assert!(*state.shutdown.borrow());
    }

    #[tokio::test]
    async fn test_builder_chaining_order_irrelevant() {
        let db1 = Database::new_in_memory().await.expect("in-memory DB");
        let db2 = Database::new_in_memory().await.expect("in-memory DB");
        let indexing = Arc::new(IndexingState::new());
        let registry: RegistryHolder = Arc::new(RwLock::new(None));
        let state_a = AppState::builder(db1)
            .with_indexing(indexing.clone())
            .with_registry(registry.clone())
            .build();
        let state_b = AppState::builder(db2)
            .with_registry(registry.clone())
            .with_indexing(indexing.clone())
            .build();
        assert!(Arc::ptr_eq(&state_a.indexing, &state_b.indexing));
        assert!(Arc::ptr_eq(&state_a.registry, &state_b.registry));
    }

    #[tokio::test]
    async fn test_mutation_context_from_builder_state() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let state = AppState::builder(db).build();
        let ctx = state.mutation_context();
        assert!(ctx.live_manager.is_none());
    }
}
