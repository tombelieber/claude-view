//! Runtime bootstrap — assembles shared state, builds the Axum app, binds
//! the listener, and spawns all background tasks. Returns the handles
//! `serve::run` needs.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Ordering is preserved
//! exactly (indexer before TUI before facet ingest before backfill) and
//! all `tokio::spawn` calls stay on the `#[tokio::main]` runtime.

use std::sync::{Arc, RwLock};
use std::time::Instant;

use anyhow::Result;
use axum::Router;
use claude_view_core::app_config::AppConfig;
use claude_view_db::Database;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::local_llm::LocalLlmService;
use crate::startup::indexer::{spawn_indexer_task, spawn_search_rebuild_if_pending, IndexerDeps};
use crate::startup::{auth, data_dir, paths::get_static_dir, search, server_bind, tasks, tui};
use crate::{
    create_app_full, IndexingState, PromptIndexHolder, PromptStatsHolder, PromptTemplatesHolder,
    SidecarManager,
};

/// Handles `serve::run` needs from the bootstrap.
pub struct ServeHandles {
    pub listener: TcpListener,
    pub app: Router,
    pub shutdown_tx: watch::Sender<bool>,
    pub port: u16,
    pub local_llm: Arc<LocalLlmService>,
    pub sidecar: Arc<SidecarManager>,
}

/// Open the database, build all shared state, construct the Axum app, bind
/// the listener, and spawn every background task (indexer, TUI, facet
/// ingest, backfill). Callers pass the result straight to
/// [`crate::startup::serve::run`].
pub async fn bootstrap(app_config: AppConfig, startup_start: Instant) -> Result<ServeHandles> {
    let db = Database::open_default().await?;

    // Spawn the indexer_v2 shadow indexer (CQRS Phase 2). Fire-and-forget —
    // tokio tears it down at shutdown with every other spawned task.
    let _shadow_indexer_handle =
        claude_view_db::indexer_v2::spawn_shadow_indexer(Arc::new(db.clone()));

    data_dir::recover_stale_classification_jobs(&db).await;

    // Shared state: indexing status, registry holder, search index, shutdown
    // channel, prompt-history holders, auth + telemetry.
    let indexing = Arc::new(IndexingState::new());
    let registry_holder = Arc::new(RwLock::new(None));
    let (search_index_holder, pending_search_migration) = search::open_index(&app_config);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let prompt_index_holder: PromptIndexHolder = Arc::new(RwLock::new(None));
    let prompt_stats_holder: PromptStatsHolder = Arc::new(RwLock::new(None));
    let prompt_templates_holder: PromptTemplatesHolder = Arc::new(RwLock::new(None));

    let jwks = auth::load_jwks().await;
    let share = auth::load_share_config();
    auth::log_share_disabled_if_needed(&share);
    let telemetry = auth::init_telemetry();

    let sidecar = Arc::new(SidecarManager::new());
    let sidecar_for_shutdown = sidecar.clone();
    let (app, local_llm_for_shutdown) = create_app_full(
        db.clone(),
        indexing.clone(),
        registry_holder.clone(),
        search_index_holder.clone(),
        shutdown_rx,
        get_static_dir(),
        sidecar,
        jwks,
        share,
        prompt_index_holder.clone(),
        prompt_stats_holder.clone(),
        prompt_templates_holder.clone(),
        telemetry.clone(),
        app_config,
    );

    let (listener, port) = server_bind::bind_listener().await?;
    server_bind::register_hooks_and_port_file(port);
    server_bind::fire_startup_events(telemetry.as_ref());

    let claude_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".claude");
    let indexer_deps = IndexerDeps {
        db: db.clone(),
        claude_dir,
        indexing: indexing.clone(),
        registry_holder,
        search_holder: search_index_holder,
        prompt_index_holder,
        prompt_stats_holder,
        prompt_templates_holder,
        telemetry,
    };
    // ORDERING: indexer spawn BEFORE search rebuild — rebuild polls
    // `registry_holder`, which is only populated inside the indexer closure.
    spawn_search_rebuild_if_pending(pending_search_migration, &indexer_deps);
    spawn_indexer_task(indexer_deps);

    tui::spawn_tui_task(indexing, startup_start, port);
    tasks::spawn_facet_ingest(&db);
    tasks::spawn_git_root_backfill(&db);

    Ok(ServeHandles {
        listener,
        app,
        shutdown_tx,
        port,
        local_llm: local_llm_for_shutdown,
        sidecar: sidecar_for_shutdown,
    })
}
