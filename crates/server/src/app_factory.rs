// crates/server/src/app_factory.rs
//! Application factory — all `create_app*` constructors, CORS setup, and hook registration.

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::HeaderValue;
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::cache;
use crate::classify_state;
use crate::facet_ingest::{self, FacetIngestState};
use crate::git_sync_state::GitSyncState;
use crate::indexing_state::IndexingState;
use crate::jobs;
use crate::live;
use crate::local_llm;
use crate::routes;
use crate::sidecar;
use crate::state::{
    self, AppState, PromptIndexHolder, PromptStatsHolder, PromptTemplatesHolder, RegistryHolder,
    SearchIndexHolder, ShareConfig,
};
use crate::teams;
use crate::telemetry;
use crate::terminal_state;

use claude_view_db::Database;

/// Create a CORS layer that only allows localhost origins.
///
/// This prevents cross-origin attacks where a malicious website could exfiltrate
/// Claude Code session data via `fetch()` to `localhost:47892`.
fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(
            |origin: &HeaderValue, _req_parts: &axum::http::request::Parts| {
                if let Ok(origin) = origin.to_str() {
                    origin.starts_with("http://localhost:")
                        || origin.starts_with("http://127.0.0.1:")
                        || origin == "http://localhost"
                        || origin == "http://127.0.0.1"
                } else {
                    false
                }
            },
        ))
        .allow_methods(Any)
        .allow_headers(Any)
}

/// Create the Axum application with all routes and middleware (API-only mode).
///
/// This sets up:
/// - API routes (health, projects, sessions)
/// - CORS restricted to localhost origins
/// - Request tracing
pub fn create_app(db: Database) -> Router {
    create_app_with_static(db, None)
}

/// Test-only: creates an app with a custom telemetry config path.
///
/// Allows integration tests to redirect telemetry config reads/writes to a
/// temporary directory instead of `~/.claude-view/telemetry.json`.
pub fn create_app_with_telemetry_path(db: Database, telemetry_config_path: PathBuf) -> Router {
    use std::collections::HashMap;
    let state = Arc::new(state::AppState {
        start_time: std::time::Instant::now(),
        db,
        indexing: Arc::new(IndexingState::new()),
        git_sync: Arc::new(GitSyncState::new()),
        registry: Arc::new(std::sync::RwLock::new(None)),
        jobs: Arc::new(jobs::JobRunner::new()),
        classify: Arc::new(classify_state::ClassifyState::new()),
        facet_ingest: Arc::new(facet_ingest::FacetIngestState::new()),
        pricing: Arc::new(claude_view_core::pricing::load_pricing()),
        live_sessions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        recently_closed: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        live_tx: tokio::sync::broadcast::channel(256).0,
        rules_dir: dirs::home_dir()
            .expect("home dir exists")
            .join(".claude")
            .join("rules"),
        terminal_connections: Arc::new(terminal_state::TerminalConnectionManager::new()),
        live_manager: None,
        search_index: Arc::new(std::sync::RwLock::new(None)),
        shutdown: tokio::sync::watch::channel(false).1,
        hook_event_channels: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        sidecar: Arc::new(sidecar::SidecarManager::new()),
        terminal_manager: Arc::new(crate::routes::cli_sessions::terminal::TerminalManager::new()),
        jwks: None,
        share: None,
        auth_identity: tokio::sync::OnceCell::new(),
        oauth_usage_cache: cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        plugin_cli_cache: cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        teams: Arc::new(teams::TeamsStore::empty()),
        prompt_index: Arc::new(std::sync::RwLock::new(None)),
        prompt_stats: Arc::new(std::sync::RwLock::new(None)),
        prompt_templates: Arc::new(std::sync::RwLock::new(None)),
        available_ides: Vec::new(),
        monitor_tx: tokio::sync::broadcast::channel(64).0,
        monitor_subscribers: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        oracle_rx: live::process_oracle::stub(),
        plugin_op_queue: Arc::new(routes::plugin_ops::PluginOpQueue::new()),
        plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
        marketplace_refresh: Arc::new(routes::marketplace_refresh::MarketplaceRefreshTracker::new()),
        transcript_to_session: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        pending_statusline: tokio::sync::Mutex::new(live::buffer::PendingMutations::new(
            std::time::Duration::from_secs(120),
        )),
        coordinator: Arc::new(live::coordinator::SessionCoordinator::new()),
        telemetry: None,
        telemetry_config_path,
        debug_statusline_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(
                ".debug/statusline.jsonl",
            ))
        } else {
            None
        },
        debug_hooks_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(".debug/hooks.jsonl"))
        } else {
            None
        },
        debug_omlx_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(".debug/omlx.jsonl"))
        } else {
            None
        },
        local_llm: Arc::new(local_llm::LocalLlmService::new(
            Arc::new(local_llm::LocalLlmConfig::new_disabled()),
            Arc::new(local_llm::LlmStatus::new()),
        )),
        session_channels: Arc::new(
            crate::live::session_ws::registry::SessionChannelRegistry::new(),
        ),
        api_key_store: Arc::new(tokio::sync::RwLock::new(
            crate::auth::api_key::ApiKeyStore::default(),
        )),
        api_key_store_path: claude_view_core::paths::config_dir().join("api-keys.json"),
        webhook_config_path: claude_view_core::paths::config_dir().join("notifications.json"),
        webhook_secrets_path: claude_view_core::paths::config_dir().join("webhook-secrets.json"),
        app_config: claude_view_core::app_config::AppConfig::default(),
        cli_sessions: Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new()),
        interaction_data: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        tmux: Arc::new(crate::routes::cli_sessions::tmux::RealTmux),
    });
    routes::api_routes(state)
}

/// Create the Axum application with optional static file serving.
///
/// Uses a default (idle) `IndexingState`. For server-first startup where the
/// caller owns the indexing handle, use [`create_app_with_indexing_and_static`].
///
/// # Arguments
///
/// * `db` - Database handle for session/project queries.
/// * `static_dir` - Optional path to static files directory.
pub fn create_app_with_static(db: Database, static_dir: Option<PathBuf>) -> Router {
    create_app_with_indexing_and_static(db, Arc::new(IndexingState::new()), static_dir)
}

/// Create app with an external `IndexingState` (API-only mode).
///
/// This is the primary entry point for server-first startup, where the caller
/// creates an `IndexingState`, passes it here, and also hands it to the
/// background indexing task.
pub fn create_app_with_indexing(db: Database, indexing: Arc<IndexingState>) -> Router {
    create_app_with_indexing_and_static(db, indexing, None)
}

/// Create app with an external `GitSyncState` (API-only mode, for testing).
///
/// Sets up an `AppState` with a default `IndexingState` but a caller-provided
/// `GitSyncState`, allowing tests to pre-configure sync progress/phase and
/// then assert on the SSE endpoint output.
pub fn create_app_with_git_sync(db: Database, git_sync: Arc<GitSyncState>) -> Router {
    let state = Arc::new(state::AppState {
        start_time: std::time::Instant::now(),
        db,
        indexing: Arc::new(IndexingState::new()),
        git_sync,
        registry: Arc::new(std::sync::RwLock::new(None)),
        jobs: Arc::new(jobs::JobRunner::new()),
        classify: Arc::new(classify_state::ClassifyState::new()),
        facet_ingest: Arc::new(facet_ingest::FacetIngestState::new()),
        pricing: Arc::new(claude_view_core::pricing::load_pricing()),
        live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        recently_closed: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        live_tx: tokio::sync::broadcast::channel(256).0,
        rules_dir: dirs::home_dir()
            .expect("home dir exists")
            .join(".claude")
            .join("rules"),
        terminal_connections: Arc::new(terminal_state::TerminalConnectionManager::new()),
        live_manager: None,
        search_index: Arc::new(std::sync::RwLock::new(None)),
        shutdown: tokio::sync::watch::channel(false).1,
        hook_event_channels: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        sidecar: Arc::new(sidecar::SidecarManager::new()),
        terminal_manager: Arc::new(crate::routes::cli_sessions::terminal::TerminalManager::new()),
        jwks: None,
        share: None,
        auth_identity: tokio::sync::OnceCell::new(),
        oauth_usage_cache: cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        plugin_cli_cache: cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        teams: Arc::new(teams::TeamsStore::empty()),
        prompt_index: Arc::new(std::sync::RwLock::new(None)),
        prompt_stats: Arc::new(std::sync::RwLock::new(None)),
        prompt_templates: Arc::new(std::sync::RwLock::new(None)),
        available_ides: Vec::new(),
        monitor_tx: tokio::sync::broadcast::channel(64).0,
        monitor_subscribers: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        oracle_rx: live::process_oracle::stub(),
        plugin_op_queue: Arc::new(routes::plugin_ops::PluginOpQueue::new()),
        plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
        marketplace_refresh: Arc::new(routes::marketplace_refresh::MarketplaceRefreshTracker::new()),
        transcript_to_session: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        pending_statusline: tokio::sync::Mutex::new(live::buffer::PendingMutations::new(
            std::time::Duration::from_secs(120),
        )),
        coordinator: Arc::new(live::coordinator::SessionCoordinator::new()),
        telemetry: None,
        telemetry_config_path: claude_view_core::telemetry_config::telemetry_config_path(),
        debug_statusline_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(
                ".debug/statusline.jsonl",
            ))
        } else {
            None
        },
        debug_hooks_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(".debug/hooks.jsonl"))
        } else {
            None
        },
        debug_omlx_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(".debug/omlx.jsonl"))
        } else {
            None
        },
        local_llm: Arc::new(local_llm::LocalLlmService::new(
            Arc::new(local_llm::LocalLlmConfig::new_disabled()),
            Arc::new(local_llm::LlmStatus::new()),
        )),
        session_channels: Arc::new(
            crate::live::session_ws::registry::SessionChannelRegistry::new(),
        ),
        api_key_store: Arc::new(tokio::sync::RwLock::new(
            crate::auth::api_key::ApiKeyStore::default(),
        )),
        api_key_store_path: claude_view_core::paths::config_dir().join("api-keys.json"),
        webhook_config_path: claude_view_core::paths::config_dir().join("notifications.json"),
        webhook_secrets_path: claude_view_core::paths::config_dir().join("webhook-secrets.json"),
        app_config: claude_view_core::app_config::AppConfig::default(),
        cli_sessions: Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new()),
        interaction_data: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        tmux: Arc::new(crate::routes::cli_sessions::tmux::RealTmux),
    });
    routes::api_routes(state)
}

/// Create the full Axum application with external `IndexingState`, shared
/// registry holder, and optional static file serving.
///
/// This is the most flexible constructor — all other `create_app*` functions
/// delegate to this one. Starts the `LiveSessionManager` for Live Monitor.
#[allow(clippy::too_many_arguments)]
pub fn create_app_full(
    db: Database,
    indexing: Arc<IndexingState>,
    registry: RegistryHolder,
    search_index: SearchIndexHolder,
    shutdown: tokio::sync::watch::Receiver<bool>,
    static_dir: Option<PathBuf>,
    sidecar: Arc<sidecar::SidecarManager>,
    jwks: Option<Arc<tokio::sync::RwLock<auth::supabase::JwksCache>>>,
    share: Option<ShareConfig>,
    prompt_index: PromptIndexHolder,
    prompt_stats: PromptStatsHolder,
    prompt_templates: PromptTemplatesHolder,
    telemetry: Option<telemetry::TelemetryClient>,
    app_config: claude_view_core::app_config::AppConfig,
) -> (Router, Arc<local_llm::LocalLlmService>) {
    // Start live session monitoring (file watcher, process detector, cleanup).
    let pricing = Arc::new(claude_view_core::pricing::load_pricing());
    let claude_dir = dirs::home_dir().expect("home dir exists").join(".claude");
    let claude_view_dir = claude_view_core::paths::data_dir();
    let teams = Arc::new(teams::TeamsStore::load_with_backup(
        &claude_dir,
        &claude_view_dir,
    ));
    // Create local LLM service before oracle (both need the status handle).
    let llm_config = Arc::new(local_llm::LocalLlmConfig::load());
    let llm_status = Arc::new(local_llm::LlmStatus::new());
    let local_llm_service = Arc::new(local_llm::LocalLlmService::new(
        llm_config.clone(),
        llm_status.clone(),
    ));
    local_llm_service.start_lifecycle();

    // Start the unified process oracle BEFORE the manager (both share the same receiver).
    let oracle_rx = if app_config.features.system_monitor {
        live::process_oracle::start_oracle(sidecar.clone(), llm_status.clone())
    } else {
        tracing::info!("System monitor feature disabled by config");
        live::process_oracle::stub()
    };

    // Create hook event channels before the manager so both manager and AppState share one instance.
    let hook_event_channels: std::sync::Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<
                String,
                tokio::sync::broadcast::Sender<live::state::HookEvent>,
            >,
        >,
    > = std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

    let debug_llm_tx = if cfg!(debug_assertions) {
        Some(live::debug_log::DebugEventLog::new(".debug/omlx.jsonl").sender())
    } else {
        None
    };
    let llm_client = Arc::new(local_llm_service.client(debug_llm_tx));

    // Create CLI session store early so it can be shared with the live manager
    // (for ownership resolution) and AppState.
    let cli_sessions: Arc<crate::routes::cli_sessions::store::CliSessionStore> = {
        let tmux_impl = crate::routes::cli_sessions::tmux::RealTmux;
        let existing = crate::routes::cli_sessions::reconcile::reconcile_tmux_sessions(&tmux_impl);
        if existing.is_empty() {
            Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new())
        } else {
            Arc::new(crate::routes::cli_sessions::store::CliSessionStore::from_sessions(existing))
        }
    };

    // Create interaction data side-map early so it can be shared with the live
    // manager (for coordinator side effects) and AppState (for HTTP endpoints).
    let interaction_data: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, claude_view_types::InteractionBlock>>,
    > = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

    let (manager, live_sessions, recently_closed, transcript_to_session, live_tx, coordinator) =
        live::manager::LiveSessionManager::start(
            pricing.clone(),
            db.clone(),
            search_index.clone(),
            registry.clone(),
            Some(sidecar.clone()),
            teams.clone(),
            claude_dir,
            claude_view_dir,
            llm_status.clone(),
            llm_config.clone(),
            llm_client,
            oracle_rx.clone(),
            hook_event_channels.clone(),
            cli_sessions.clone(),
            interaction_data.clone(),
        );

    // Hook registration deferred — caller must invoke register_hooks()
    // AFTER binding the actual port (which may auto-increment on conflict).

    // Load API key store for webhook auth.
    let api_key_store_path = claude_view_core::paths::config_dir().join("api-keys.json");
    let api_key_store = Arc::new(tokio::sync::RwLock::new(crate::auth::api_key::load_store(
        &api_key_store_path,
    )));
    let webhook_config_path = claude_view_core::paths::config_dir().join("notifications.json");
    let webhook_secrets_path = claude_view_core::paths::config_dir().join("webhook-secrets.json");

    // Detect installed IDEs (runs `which` for each known command).
    let available_ides = routes::ide::detect_installed_ides();
    if available_ides.is_empty() {
        tracing::info!("No known IDEs detected on PATH");
    } else {
        let names: Vec<&str> = available_ides
            .iter()
            .map(|(info, _)| info.name.as_str())
            .collect();
        tracing::info!(ides = ?names, "Detected installed IDEs");
    }

    let state = Arc::new(state::AppState {
        start_time: std::time::Instant::now(),
        db,
        indexing,
        git_sync: Arc::new(GitSyncState::new()),
        registry,
        jobs: Arc::new(jobs::JobRunner::new()),
        classify: Arc::new(classify_state::ClassifyState::new()),
        facet_ingest: Arc::new(FacetIngestState::new()),
        pricing,
        live_sessions,
        recently_closed,
        live_tx,
        rules_dir: dirs::home_dir()
            .expect("home dir exists")
            .join(".claude")
            .join("rules"),
        terminal_connections: Arc::new(terminal_state::TerminalConnectionManager::new()),
        live_manager: Some(manager),
        search_index,
        shutdown,
        hook_event_channels,
        sidecar,
        terminal_manager: Arc::new(crate::routes::cli_sessions::terminal::TerminalManager::new()),
        jwks,
        share,
        auth_identity: tokio::sync::OnceCell::new(),
        oauth_usage_cache: cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        plugin_cli_cache: cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        teams: teams.clone(),
        prompt_index,
        prompt_stats,
        prompt_templates,
        available_ides,
        monitor_tx: tokio::sync::broadcast::channel(64).0,
        monitor_subscribers: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        oracle_rx: oracle_rx.clone(),
        plugin_op_queue: Arc::new(routes::plugin_ops::PluginOpQueue::new()),
        plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
        marketplace_refresh: Arc::new(routes::marketplace_refresh::MarketplaceRefreshTracker::new()),
        transcript_to_session,
        pending_statusline: tokio::sync::Mutex::new(live::buffer::PendingMutations::new(
            std::time::Duration::from_secs(120),
        )),
        coordinator,
        telemetry,
        telemetry_config_path: claude_view_core::telemetry_config::telemetry_config_path(),
        debug_statusline_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(
                ".debug/statusline.jsonl",
            ))
        } else {
            None
        },
        debug_hooks_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(".debug/hooks.jsonl"))
        } else {
            None
        },
        debug_omlx_log: if cfg!(debug_assertions) {
            Some(live::debug_log::DebugEventLog::new(".debug/omlx.jsonl"))
        } else {
            None
        },
        local_llm: local_llm_service.clone(),
        session_channels: Arc::new(
            crate::live::session_ws::registry::SessionChannelRegistry::new(),
        ),
        api_key_store,
        api_key_store_path,
        webhook_config_path,
        webhook_secrets_path,
        app_config,
        cli_sessions,
        interaction_data,
        tmux: Arc::new(crate::routes::cli_sessions::tmux::RealTmux),
    });

    // Spawn webhook notification engine.
    let _webhook_engine = crate::webhook_engine::spawn_engine(
        &state.live_tx,
        state.shutdown.clone(),
        state.webhook_config_path.clone(),
        state.webhook_secrets_path.clone(),
        None, // base_url: auto-detect later
    );

    // Spawn the plugin operation worker (processes queued installs/updates serially).
    {
        let queue = state.plugin_op_queue.clone();
        let notify = state.plugin_op_notify.clone();
        let worker_state = state.clone();
        routes::plugin_ops::spawn_op_worker(queue, notify, move |op| {
            let st = worker_state.clone();
            async move { routes::plugin_ops::execute_plugin_op(&st, op).await }
        });
    }

    // Spawn CLI session health check (marks dead tmux sessions as Exited every 30s).
    crate::routes::cli_sessions::health::spawn_health_check(
        state.cli_sessions.clone(),
        state.tmux.clone(),
        state.live_tx.clone(),
        state.shutdown.clone(),
    );

    // Seed official workflow YAMLs to ~/.claude-view/workflows/official/ (idempotent, fast)
    crate::routes::workflows::seed_official_workflows();

    let mut app = Router::new()
        .merge(routes::api_routes(state))
        .layer(CompressionLayer::new())
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http());

    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(&index)));
    }

    (app, local_llm_service)
}

/// Register Claude Code hooks for the given port.
///
/// Must be called AFTER binding the actual port (which may differ from the
/// requested port due to auto-increment on conflict).
///
/// Skipped when `CLAUDE_VIEW_SKIP_HOOKS=1` — for enterprise/DACS sandboxes
/// where `~/.claude/settings.json` and `~/.cache/` are read-only.
/// In that case, hooks must be pre-configured by an install script.
pub fn register_hooks(port: u16) {
    if std::env::var("CLAUDE_VIEW_SKIP_HOOKS").as_deref() == Ok("1") {
        tracing::info!("CLAUDE_VIEW_SKIP_HOOKS=1 — skipping hook/statusline registration");
        return;
    }
    live::hook_registrar::register(port);
    live::statusline_injector::register(port);
}

/// Create the Axum application with an external `IndexingState` and optional
/// static file serving.
///
/// # Arguments
///
/// * `db` - Database handle for session/project queries.
/// * `indexing` - Shared indexing progress state.
/// * `static_dir` - Optional path to the directory containing static files
///   (e.g., React build output). If provided, the server will serve static
///   files and fall back to `index.html` for client-side routing (SPA mode).
pub fn create_app_with_indexing_and_static(
    db: Database,
    indexing: Arc<IndexingState>,
    static_dir: Option<PathBuf>,
) -> Router {
    let state = AppState::builder(db).with_indexing(indexing).build();

    let mut app = Router::new()
        .merge(routes::api_routes(state))
        .layer(CompressionLayer::new())
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http());

    // Serve static files with SPA fallback
    // Use .fallback() instead of .not_found_service() to return 200 for SPA routing
    // (not_found_service returns 404, which is incorrect for client-side routing)
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(&index)));
    }

    app
}
