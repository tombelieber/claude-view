// crates/server/src/main.rs
//! Claude View server binary.
//!
//! Starts an Axum HTTP server **immediately**, then spawns background indexing.
//! Pass 1 (read sessions-index.json, <10ms) populates the "Ready" line,
//! Pass 2 (deep JSONL parsing) runs in parallel with a TUI progress spinner.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use claude_view_db::indexer_parallel::{build_index_hints, scan_and_index_all};
use claude_view_db::Database;
use claude_view_server::auth::supabase::fetch_decoding_key;
use claude_view_server::state::ShareConfig;
use claude_view_server::{
    create_app_full, init_metrics, record_sync, FacetIngestState, IndexingState, IndexingStatus,
    PromptIndexHolder, PromptStatsHolder, PromptTemplatesHolder, SearchIndexHolder,
};
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::FmtSubscriber;

/// Default port for the server.
const DEFAULT_PORT: u16 = 47892;

/// Get the server port from environment or use default.
fn get_port() -> u16 {
    std::env::var("CLAUDE_VIEW_PORT")
        .ok()
        .or_else(|| std::env::var("PORT").ok())
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Check if a process holding a port is a stale claude-view instance.
///
/// Returns true if the process name contains "claude-view" or "claude_view".
/// If we can't determine the process name, returns false (don't kill unknowns).
fn is_claude_view_process(pid: &str) -> bool {
    // macOS: `ps -p <pid> -o comm=` gives the binary name
    let output = std::process::Command::new("ps")
        .args(["-p", pid, "-o", "comm="])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let name = String::from_utf8_lossy(&o.stdout).to_lowercase();
            name.contains("claude-view") || name.contains("claude_view")
        }
        _ => false,
    }
}

/// Try to reclaim a port from a stale claude-view process.
///
/// Returns true if the port was freed (stale process killed).
/// Returns false if the port is held by a non-claude-view process.
fn try_reclaim_port(port: u16) -> bool {
    let output = std::process::Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output();

    let pids = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return false,
    };

    let my_pid = std::process::id().to_string();
    let mut killed_any = false;

    for pid in pids.split_whitespace() {
        if pid == my_pid {
            continue;
        }
        if is_claude_view_process(pid) {
            eprintln!("  killing stale claude-view (PID {pid}) on port {port}");
            let _ = std::process::Command::new("kill")
                .args(["-9", pid])
                .status();
            killed_any = true;
        } else {
            eprintln!("  port {port} held by another app (PID {pid}), skipping");
        }
    }
    killed_any
}

/// Detect how the server was installed/launched.
///
/// Returns one of: "plugin", "install_sh", "npx".
fn detect_install_source() -> &'static str {
    // Plugin sets CLAUDE_PLUGIN_ROOT when launching via hooks
    if std::env::var("CLAUDE_PLUGIN_ROOT").is_ok() {
        return "plugin";
    }
    // install.sh puts the binary under ~/.cache/claude-view/
    if let Ok(exe) = std::env::current_exe() {
        let exe_str = exe.to_string_lossy();
        if exe_str.contains(".cache/claude-view") {
            return "install_sh";
        }
    }
    "npx"
}

/// Fire-and-forget ping to CF Worker for unified install tracking.
fn ping_install_beacon(source: &str) {
    let url = format!(
        "https://get.claudeview.ai/ping?source={}&v={}",
        source,
        env!("CARGO_PKG_VERSION"),
    );
    tokio::spawn(async move {
        let _ = reqwest::Client::new()
            .get(&url)
            .timeout(Duration::from_secs(3))
            .send()
            .await;
    });
}

/// Get the static directory for serving frontend files.
///
/// Priority:
/// 1. STATIC_DIR environment variable (explicit override)
/// 2. Binary-relative ./dist (npx distribution: binary + dist/ are siblings)
/// 3. CWD-relative ./apps/web/dist (monorepo dev layout via cargo run)
/// 4. CWD-relative ./dist (legacy flat layout)
/// 5. None (API-only mode)
fn get_static_dir() -> Option<PathBuf> {
    // 1. Explicit override always wins
    if let Ok(dir) = std::env::var("STATIC_DIR") {
        let p = PathBuf::from(&dir);
        if p.exists() {
            return Some(p);
        }
        tracing::warn!(static_dir = %dir, "STATIC_DIR set but directory does not exist");
        return None;
    }

    // 2. Binary-relative: resolves symlinks (Homebrew), works regardless of CWD
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(canonical) = exe.canonicalize() {
            if let Some(exe_dir) = canonical.parent() {
                let bin_dist = exe_dir.join("dist");
                if bin_dist.exists() {
                    return Some(bin_dist);
                }
            }
        }
    }

    // 3. CWD-relative: monorepo layout (cargo run from repo root)
    let monorepo_dist = PathBuf::from("apps/web/dist");
    if monorepo_dist.exists() {
        return Some(monorepo_dist);
    }

    // 4. CWD-relative: flat layout fallback
    let dist = PathBuf::from("dist");
    dist.exists().then_some(dist)
}

/// Run git sync with structured logging. Used by both initial and periodic sync.
async fn run_git_sync_logged(db: &Database, label: &str) {
    let start = Instant::now();
    tracing::info!(sync_type = label, "Starting git sync");

    match claude_view_db::git_correlation::run_git_sync(db, |_| {}).await {
        Ok(r) => {
            let duration = start.elapsed();
            if r.repos_scanned > 0 || r.links_created > 0 {
                tracing::info!(
                    sync_type = label,
                    repos_scanned = r.repos_scanned,
                    commits_found = r.commits_found,
                    links_created = r.links_created,
                    duration_secs = duration.as_secs_f64(),
                    "Git sync complete"
                );
            } else {
                tracing::debug!(
                    sync_type = label,
                    duration_secs = duration.as_secs_f64(),
                    "Git sync: no changes"
                );
            }
            if !r.errors.is_empty() {
                tracing::warn!(
                    sync_type = label,
                    error_count = r.errors.len(),
                    errors = ?r.errors,
                    "Git sync had errors"
                );
            }
            // Record sync metrics
            record_sync("git", duration, Some(r.commits_found as u64));
        }
        Err(e) => {
            let duration = start.elapsed();
            tracing::warn!(
                sync_type = label,
                error = %e,
                duration_secs = duration.as_secs_f64(),
                "Git sync failed (non-fatal)"
            );
            // Still record metrics for failed syncs
            record_sync("git", duration, None);
        }
    }
}

/// Generate contribution snapshots for historical days.
/// Initial run refreshes 365 days; periodic runs refresh 2 days (today + yesterday).
async fn run_snapshot_generation(db: &Database, label: &str) {
    let days_back = if label == "initial" { 365 } else { 2 };
    match db.generate_missing_snapshots(days_back).await {
        Ok(count) => {
            if count > 0 {
                tracing::info!("{} snapshot refresh: {} snapshots updated", label, count);
            } else {
                tracing::debug!("{} snapshot refresh: no active dates in range", label);
            }
        }
        Err(e) => {
            tracing::warn!("{} snapshot generation failed (non-fatal): {}", label, e);
        }
    }
}

/// Format a byte count as a human-readable string (e.g. "23.4 GB", "512 MB").
fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_000_000_000;
    const MB: u64 = 1_000_000;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing — respects RUST_LOG env var, defaults to WARN.
    // RUST_LOG=debug in dev:server script enables info/debug logs for classify, etc.
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Handle `claude-view cleanup` subcommand early, before any async/DB work
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("cleanup") {
        eprintln!("\n\u{1f9f9} claude-view cleanup\n");
        let mut actions = Vec::new();

        // 1. Remove hooks from ~/.claude/settings.json (also removes .tmp)
        actions.extend(claude_view_server::live::hook_registrar::cleanup(0));
        claude_view_server::live::statusline_injector::cleanup();

        // 2. Remove cache directory (DB + Tantivy index)
        actions.extend(claude_view_core::paths::remove_cache_data());

        // 3. Remove lock files from /tmp
        actions.extend(claude_view_core::paths::remove_lock_files());

        if actions.is_empty() {
            eprintln!("  Nothing to clean up.");
        } else {
            for action in &actions {
                eprintln!("  \u{2713} {action}");
            }
        }
        eprintln!();
        std::process::exit(0);
    }

    let startup_start = Instant::now();

    // Platform gate: macOS is first-class. Linux is supported but less tested.
    // WHY this gate remains after cross-platform hardening:
    //   - macOS is the primary dev/test platform; Linux has less CI coverage
    //   - Some features (process tree, keychain) have platform-specific codepaths
    //   - Docker/WSL/CI users can bypass via env var
    // Set CLAUDE_VIEW_SKIP_PLATFORM_CHECK=1 to run on Linux (Docker, WSL, native).
    if std::env::consts::OS != "macos"
        && std::env::var("CLAUDE_VIEW_SKIP_PLATFORM_CHECK").as_deref() != Ok("1")
    {
        eprintln!("\n\u{26a0}\u{fe0f}  claude-view is optimized for macOS. Linux support is available but less tested.");
        eprintln!("   Set CLAUDE_VIEW_SKIP_PLATFORM_CHECK=1 to run on Linux.");
        eprintln!("   Report issues: https://github.com/tombelieber/claude-view/issues\n");
        std::process::exit(1);
    }

    // Initialize Prometheus metrics
    init_metrics();

    // Print banner
    eprintln!("\n\u{1f50d} claude-view v{}\n", env!("CARGO_PKG_VERSION"));

    // Step 0: Validate data directory is writable before proceeding
    let data_dir = claude_view_core::paths::data_dir();
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        eprintln!(
            "ERROR: Cannot create data directory: {}\n\
             Path: {}\n\
             Set CLAUDE_VIEW_DATA_DIR to a writable directory.",
            e,
            data_dir.display()
        );
        std::process::exit(1);
    }
    let probe = data_dir.join(".write-test");
    if std::fs::write(&probe, b"ok").is_err() {
        eprintln!(
            "ERROR: Data directory is not writable: {}\n\
             Set CLAUDE_VIEW_DATA_DIR to a writable directory.",
            data_dir.display()
        );
        std::process::exit(1);
    }
    let _ = std::fs::remove_file(&probe);
    tracing::info!("Data directory: {}", data_dir.display());

    // Step 1: Open database
    let db = Database::open_default().await?;

    // Recover any classification jobs left in "running" state from previous crash
    match db.recover_stale_classification_jobs().await {
        Ok(count) if count > 0 => {
            tracing::info!("Recovered {} stale classification jobs", count);
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("Failed to recover stale classification jobs: {}", e);
        }
    }

    // Step 2: Create shared indexing state and registry holder
    let indexing = Arc::new(IndexingState::new());
    let registry_holder = Arc::new(RwLock::new(None));

    // Step 3: Open the Tantivy full-text search index (fast — reads existing files).
    // Wrapped in SearchIndexHolder so clear_cache can swap it at runtime.
    let search_index_holder: SearchIndexHolder = {
        let index_dir = claude_view_core::paths::search_index_dir()
            .expect("search_index_dir() always returns Some after data_dir() refactor");

        match claude_view_search::SearchIndex::open(&index_dir) {
            Ok(idx) => {
                tracing::info!("Search index opened at {}", index_dir.display());
                Arc::new(RwLock::new(Some(Arc::new(idx))))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to open search index: {}. Search will be unavailable.",
                    e
                );
                Arc::new(RwLock::new(None))
            }
        }
    };

    // Step 3b: Create shutdown channel for SSE stream termination on Ctrl+C
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Step 4: Load Supabase JWKS for JWT validation (sharing feature)
    let jwks = if let Some(supabase_url) = std::env::var("SUPABASE_URL")
        .ok()
        .or_else(|| option_env!("SUPABASE_URL").map(str::to_string))
    {
        match fetch_decoding_key(&supabase_url).await {
            Ok(cache) => {
                tracing::info!("Supabase JWKS loaded");
                Some(Arc::new(tokio::sync::RwLock::new(cache)))
            }
            Err(e) => {
                tracing::warn!("Failed to load Supabase JWKS: {e}. Auth will be disabled.");
                None
            }
        }
    } else {
        tracing::info!("SUPABASE_URL not set — auth disabled (dev mode)");
        None
    };

    let share = match (
        std::env::var("SHARE_WORKER_URL")
            .ok()
            .or_else(|| option_env!("SHARE_WORKER_URL").map(str::to_string)),
        std::env::var("SHARE_VIEWER_URL")
            .ok()
            .or_else(|| option_env!("SHARE_VIEWER_URL").map(str::to_string)),
    ) {
        (Some(worker_url), Some(viewer_url)) => Some(ShareConfig {
            worker_url,
            viewer_url,
            http_client: reqwest::Client::new(),
        }),
        _ => {
            tracing::info!("SHARE_WORKER_URL/SHARE_VIEWER_URL not set — sharing disabled");
            None
        }
    };

    // Step 4b: Create prompt history holders
    let prompt_index_holder: PromptIndexHolder = Arc::new(RwLock::new(None));
    let prompt_stats_holder: PromptStatsHolder = Arc::new(RwLock::new(None));
    let prompt_templates_holder: PromptTemplatesHolder = Arc::new(RwLock::new(None));

    // Step 4c: Initialize PostHog telemetry client (compile-time key only)
    let telemetry = match option_env!("POSTHOG_API_KEY") {
        Some(key) if !key.is_empty() => {
            let config_path = claude_view_core::telemetry_config::telemetry_config_path();
            let _ = claude_view_core::telemetry_config::create_telemetry_config_if_missing(
                &config_path,
            );
            let config = claude_view_core::telemetry_config::read_telemetry_config(&config_path);
            let status = claude_view_core::telemetry_config::resolve_telemetry_status(
                Some(key),
                &config_path,
            );
            let client =
                claude_view_server::telemetry::TelemetryClient::new(key, &config.anonymous_id);
            if status == claude_view_core::telemetry_config::TelemetryStatus::Enabled {
                client.set_enabled(true);
            }
            Some(client)
        }
        _ => None,
    };
    // Clone for the background indexing task and server_started event.
    // All clones share the same Arc<AtomicBool> enabled flag, so set_enabled
    // on any copy is immediately visible to all others.
    let telemetry_for_indexer = telemetry.clone();
    let telemetry_for_server_started = telemetry.clone();

    // Step 4d: Build the Axum app with indexing state, registry holder, and search index
    let static_dir = get_static_dir();
    let sidecar = Arc::new(claude_view_server::SidecarManager::new());
    let sidecar_for_shutdown = sidecar.clone();
    let (app, local_llm_for_shutdown) = create_app_full(
        db.clone(),
        indexing.clone(),
        registry_holder.clone(),
        search_index_holder.clone(),
        shutdown_rx,
        static_dir,
        sidecar,
        jwks,
        share,
        prompt_index_holder.clone(),
        prompt_stats_holder.clone(),
        prompt_templates_holder.clone(),
        telemetry,
    );

    // Step 5: Bind and start the HTTP server IMMEDIATELY (before any indexing)
    let port = get_port();
    let bind_addr: std::net::IpAddr = std::env::var("CLAUDE_VIEW_BIND_ADDR")
        .ok()
        .and_then(|s| {
            s.parse()
                .map_err(|e| {
                    tracing::warn!(
                        "Invalid CLAUDE_VIEW_BIND_ADDR '{s}': {e}, falling back to localhost"
                    );
                })
                .ok()
        })
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    // Bind with smart port resolution:
    // 1. Try the requested port
    // 2. If EADDRINUSE and it's a stale claude-view → kill it, retry same port
    // 3. If EADDRINUSE and it's another app → auto-increment port (up to +10)
    //
    // When CLAUDE_VIEW_SKIP_HOOKS=1 (sandbox mode), auto-increment is DISABLED
    // because hooks are pre-configured for a fixed port. Silently starting on a
    // different port would break hooks. Fail fast instead.
    let skip_hooks = std::env::var("CLAUDE_VIEW_SKIP_HOOKS").as_deref() == Ok("1");
    let (listener, port) = {
        let mut try_port = port;
        let max_port = if skip_hooks { port } else { port + 10 };
        loop {
            let addr = SocketAddr::from((bind_addr, try_port));
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => break (l, try_port),
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                    if !skip_hooks && try_reclaim_port(try_port) {
                        // Killed a stale claude-view — retry same port
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        if let Ok(l) = tokio::net::TcpListener::bind(addr).await {
                            break (l, try_port);
                        }
                        // still in use, fall through to increment
                    }
                    // Port held by another app — try next (or fail in sandbox)
                    try_port += 1;
                    if try_port > max_port {
                        if skip_hooks {
                            anyhow::bail!(
                                "Port {port} in use. In sandbox mode, the port must match \
                                 pre-configured hooks. Free the port or change CLAUDE_VIEW_PORT."
                            );
                        }
                        anyhow::bail!(
                            "Ports {port}–{max_port} all in use. Set CLAUDE_VIEW_PORT to override."
                        );
                    }
                    eprintln!(
                        "Port {} in use by another app, trying {}…",
                        try_port - 1,
                        try_port
                    );
                }
                Err(e) => return Err(e.into()),
            }
        }
    };

    // Register Claude Code hooks with the ACTUAL bound port (may differ
    // from requested port due to auto-increment on conflict).
    claude_view_server::register_hooks(port);

    // Fire server_started telemetry event (fire-and-forget, non-blocking)
    let install_source = detect_install_source();
    if let Some(ref client) = telemetry_for_server_started {
        client.track(
            "server_started",
            serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "platform": std::env::consts::OS,
                "install_source": install_source,
            }),
        );
    }

    // Ping CF Worker for unified install tracking (fire-and-forget).
    // All install paths (plugin, npx, install.sh) converge to one dashboard.
    ping_install_beacon(&install_source);

    // Step 6: Resolve the claude dir for indexing
    let claude_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".claude");

    // Step 7: Spawn background indexing task (with registry build in parallel)
    let idx_state = indexing.clone();
    let idx_db = db.clone();
    let idx_registry = registry_holder.clone();
    let idx_search = search_index_holder.clone();
    let idx_prompt_index = prompt_index_holder.clone();
    let idx_prompt_stats = prompt_stats_holder.clone();
    let idx_prompt_templates = prompt_templates_holder.clone();
    let idx_telemetry = telemetry_for_indexer;
    tokio::spawn(async move {
        idx_state.set_status(IndexingStatus::ReadingIndexes);
        let index_start = Instant::now();

        // 1. Build hints from sessions-index.json (no DB writes, sync function)
        let hints = build_index_hints(&claude_dir);
        let hint_count = hints.len();
        idx_state.set_sessions_found(hint_count);
        // Count unique projects from hints for the "ready" SSE event
        let unique_projects: std::collections::HashSet<&str> = hints
            .values()
            .filter_map(|h| h.project_display_name.as_deref())
            .collect();
        idx_state.set_projects_found(unique_projects.len());

        // 2. Build registry
        let registry = claude_view_core::build_registry(&claude_dir).await;

        // 2b. Seed invocables into DB so invocations can reference them (FK constraint)
        let invocable_tuples: Vec<(String, Option<String>, String, String, String)> = registry
            .all_invocables()
            .map(|info| {
                (
                    info.id.clone(),
                    info.plugin_name.clone(),
                    info.name.clone(),
                    info.kind.to_string(),
                    info.description.clone(),
                )
            })
            .collect();
        if !invocable_tuples.is_empty() {
            if let Err(e) = idx_db.batch_upsert_invocables(&invocable_tuples).await {
                tracing::warn!(error = %e, "Failed to seed invocables");
            }
        }

        // 2c. Auto-reindex: compare registry fingerprint with stored hash
        let new_hash = registry.fingerprint();
        match idx_db.get_registry_hash().await {
            Ok(Some(stored)) if stored == new_hash => {
                tracing::debug!("Registry unchanged (hash={new_hash}), skipping full re-index");
            }
            Ok(stored) => {
                let reason = if stored.is_none() {
                    "first run"
                } else {
                    "registry changed"
                };
                tracing::info!(
                    "Registry hash mismatch ({reason}), marking all sessions for re-index"
                );
                match idx_db.mark_all_sessions_for_reindex().await {
                    Ok(n) => tracing::info!("Marked {n} sessions for re-index"),
                    Err(e) => tracing::warn!("Failed to mark sessions for re-index: {e}"),
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read registry hash: {e}, skipping auto-reindex check");
            }
        }

        // Store registry in shared holder for API routes and keep an Arc for indexing
        let registry_arc = Arc::new(registry);
        *idx_registry.write().unwrap() = Some((*registry_arc).clone());

        // Extract search index Arc from holder (clone Arc, don't hold lock during scan)
        let search_for_scan = idx_search.read().unwrap().clone();

        // 3. Single-pass scan: parse + upsert for each changed file
        idx_state.set_status(IndexingStatus::DeepIndexing);
        let state_for_progress = idx_state.clone();
        let state_for_total = idx_state.clone();
        let state_for_finalize = idx_state.clone();
        match scan_and_index_all(
            &claude_dir,
            &idx_db,
            &hints,
            search_for_scan,
            Some(registry_arc.clone()),
            move |_session_id| {
                state_for_progress.increment_indexed();
            },
            move |file_count| {
                state_for_total.set_total(file_count);
                // Filesystem .jsonl count is source of truth for session count.
                // sessions-index.json hints only cover ~24% of project dirs.
                state_for_total.set_sessions_found(file_count);
            },
            move || {
                state_for_finalize.set_status(IndexingStatus::Finalizing);
            },
        )
        .await
        {
            Ok((indexed, skipped)) => {
                tracing::info!(
                    indexed,
                    skipped,
                    elapsed_ms = index_start.elapsed().as_millis() as u64,
                    "Startup scan complete"
                );

                // Signal Done immediately — search index is ready.
                // Post-scan cleanup below is housekeeping, not indexing.
                idx_state.set_status(IndexingStatus::Done);

                // Persist index metadata so Settings > Data Status shows real values.
                let duration_ms = index_start.elapsed().as_millis() as i64;
                let sessions = idx_db.get_session_count().await.unwrap_or(0);
                let projects = idx_db.get_project_count().await.unwrap_or(0);
                if let Err(e) = idx_db
                    .update_index_metadata_on_success(duration_ms, sessions, projects)
                    .await
                {
                    tracing::warn!(error = %e, "Failed to persist index metadata");
                }

                // Fire telemetry events for first_index_completed and sessions_milestone
                if let Some(ref client) = idx_telemetry {
                    let telemetry_config_path =
                        claude_view_core::telemetry_config::telemetry_config_path();
                    let mut config = claude_view_core::telemetry_config::read_telemetry_config(
                        &telemetry_config_path,
                    );

                    // first_index_completed — fires exactly once per install
                    if !config.first_index_completed {
                        config.first_index_completed = true;
                        let _ = claude_view_core::telemetry_config::write_telemetry_config(
                            &telemetry_config_path,
                            &config,
                        );
                        client.track(
                            "first_index_completed",
                            serde_json::json!({
                                "session_count": sessions,
                                "version": env!("CARGO_PKG_VERSION"),
                            }),
                        );
                    }

                    // sessions_milestone — fires each time a new milestone is crossed
                    if let Some(milestone) = claude_view_core::telemetry_config::check_milestone(
                        sessions as u64,
                        config.last_milestone.unwrap_or(0),
                    ) {
                        client.track(
                            "sessions_milestone",
                            serde_json::json!({
                                "milestone": milestone,
                                "session_count": sessions,
                            }),
                        );
                        config.last_milestone = Some(milestone);
                        let _ = claude_view_core::telemetry_config::write_telemetry_config(
                            &telemetry_config_path,
                            &config,
                        );
                    }
                }

                // 4. Post-scan cleanup
                // Prune DB rows for JSONL files that no longer exist on disk.
                match claude_view_db::indexer_parallel::prune_stale_sessions(&idx_db).await {
                    Ok(n) if n > 0 => tracing::info!("Pruned {} stale sessions from DB", n),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Failed to prune stale sessions: {}", e),
                }
                // Persist registry fingerprint so next startup can detect changes.
                if let Err(e) = idx_db.set_registry_hash(&new_hash).await {
                    tracing::warn!("Failed to persist registry hash: {e}");
                }

                // 5. Post-index tasks
                run_git_sync_logged(&idx_db, "initial").await;
                run_snapshot_generation(&idx_db, "initial").await;

                // 6. Prompt History Indexing
                let ph_start = std::time::Instant::now();
                let history_path = dirs::home_dir()
                    .expect("home dir")
                    .join(".claude")
                    .join("history.jsonl");

                if history_path.exists() {
                    match claude_view_core::prompt_history::parse_history(&history_path).await {
                        Ok(entries) => {
                            tracing::info!(count = entries.len(), "parsed prompt history");

                            // Compute stats
                            let stats = claude_view_core::prompt_history::compute_stats(&entries);
                            *idx_prompt_stats.write().unwrap() = Some(stats);

                            // Compute templates
                            let prompt_strs: Vec<&str> =
                                entries.iter().map(|e| e.display.as_str()).collect();
                            let templates = claude_view_core::prompt_templates::detect_templates(
                                &prompt_strs,
                                3,
                            );
                            *idx_prompt_templates.write().unwrap() = Some(templates);

                            // Build Tantivy index
                            let index_path = claude_view_core::paths::prompt_index_dir();
                            match claude_view_search::prompt_index::PromptSearchIndex::open(
                                &index_path,
                            ) {
                                Ok(index) => {
                                    let documents: Vec<
                                        claude_view_search::prompt_index::PromptDocument,
                                    > = entries
                                        .iter()
                                        .enumerate()
                                        .map(|(i, e)| {
                                            claude_view_search::prompt_index::PromptDocument {
                                                prompt_id: format!(
                                                    "{}-{}",
                                                    e.timestamp_ms, i
                                                ),
                                                display: e.display.clone(),
                                                paste_text: e.paste_text(),
                                                project: e
                                                    .project_display_name()
                                                    .to_string(),
                                                session_id: e.session_id.clone(),
                                                branch: String::new(),
                                                model: String::new(),
                                                git_root: e.project.clone(),
                                                intent:
                                                    claude_view_core::prompt_history::classify_intent(
                                                        &e.display,
                                                    )
                                                    .to_string(),
                                                complexity:
                                                    claude_view_core::prompt_history::complexity_bucket(
                                                        &e.display,
                                                    )
                                                    .to_string(),
                                                timestamp: e.timestamp_secs(),
                                                has_paste: e
                                                    .pasted_contents
                                                    .as_ref()
                                                    .is_some_and(|p| !p.is_empty()),
                                            }
                                        })
                                        .collect();

                                    if let Err(e) = index.index_prompts(&documents) {
                                        tracing::error!(
                                            error = %e,
                                            "failed to index prompts"
                                        );
                                    } else if let Err(e) = index.commit() {
                                        tracing::error!(
                                            error = %e,
                                            "failed to commit prompt index"
                                        );
                                    } else {
                                        index.mark_schema_synced();
                                        *idx_prompt_index.write().unwrap() = Some(Arc::new(index));
                                        tracing::info!(
                                            count = documents.len(),
                                            elapsed_ms = ph_start.elapsed().as_millis() as u64,
                                            "prompt history indexed"
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        error = %e,
                                        "failed to open prompt index"
                                    )
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "failed to parse prompt history"
                            )
                        }
                    }
                } else {
                    tracing::info!("~/.claude/history.jsonl not found, skipping prompt indexing");
                }

                // 7. Periodic sync loop: re-scan changed sessions, git-sync, snapshots.
                // No more two-pass polling — the watcher handles incremental updates.
                // This loop handles periodic git-sync and snapshot generation only,
                // plus a lightweight re-scan for any files the watcher might have missed.
                loop {
                    let interval_secs = idx_db.get_git_sync_interval().await.unwrap_or(120);
                    let sync_interval = Duration::from_secs(interval_secs);
                    tokio::time::sleep(sync_interval).await;

                    // Lightweight re-scan: picks up any files the watcher missed (skips unchanged)
                    let hints = build_index_hints(&claude_dir);
                    let rescan_start = Instant::now();
                    let search_rescan = idx_search.read().unwrap().clone();
                    match scan_and_index_all(
                        &claude_dir,
                        &idx_db,
                        &hints,
                        search_rescan,
                        Some(registry_arc.clone()),
                        |_| {},
                        |_| {},
                        || {},
                    )
                    .await
                    {
                        Ok((indexed, _)) => {
                            if indexed > 0 {
                                tracing::info!(indexed, "Periodic re-scan indexed new sessions");
                                record_sync(
                                    "periodic-rescan",
                                    rescan_start.elapsed(),
                                    Some(indexed as u64),
                                );
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "Periodic re-scan failed (non-fatal)"),
                    }

                    run_git_sync_logged(&idx_db, "periodic").await;
                    run_snapshot_generation(&idx_db, "periodic").await;
                }
            }
            Err(e) => {
                idx_state.set_error(e);
            }
        }
    });

    // Step 8: Spawn TUI progress task (runs concurrently with the server)
    let tui_state = indexing.clone();
    tokio::spawn(async move {
        // Wait for Pass 1 to complete (status transitions out of Idle/ReadingIndexes)
        loop {
            let status = tui_state.status();
            if status != IndexingStatus::Idle && status != IndexingStatus::ReadingIndexes {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Wait briefly for session data when hints report 0 sessions.
        // on_total_known fires during scan_and_index_all with the filesystem count —
        // typically within a few hundred ms of entering DeepIndexing.
        if tui_state.sessions_found() == 0 {
            for _ in 0..20 {
                let status = tui_state.status();
                if status == IndexingStatus::Done
                    || status == IndexingStatus::Error
                    || tui_state.sessions_found() > 0
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // Print the "Ready" line with Pass 1 results.
        // Use max(sessions_found, total) — same defense-in-depth as the SSE ready event.
        let elapsed = startup_start.elapsed();
        let projects = tui_state.projects_found();
        let sessions = std::cmp::max(tui_state.sessions_found(), tui_state.total());
        eprintln!(
            "  \u{2713} Ready in {} \u{2014} {} projects, {} sessions",
            claude_view_core::format_duration(elapsed),
            projects,
            sessions,
        );
        // In dev mode, open the Vite dev server; otherwise open the server directly.
        // VITE_PORT env var or RUST_LOG presence signals dev mode.
        let browse_url = std::env::var("VITE_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .map(|vite_port| format!("http://localhost:{}", vite_port))
            .unwrap_or_else(|| format!("http://localhost:{}", port));
        eprintln!("  \u{2192} {}\n", browse_url);

        // Auto-open browser on first startup only (not cargo-watch restarts).
        // We detect restarts via a lock file that persists across restarts.
        let lock_dir = claude_view_core::paths::lock_dir().unwrap_or_else(std::env::temp_dir);
        let _ = std::fs::create_dir_all(&lock_dir);
        let lock_path = lock_dir.join(format!("claude-view-{}.lock", port));
        let should_open = if lock_path.exists() {
            // Lock exists — check if it's stale (older than 5 seconds means fresh start, not a restart)
            lock_path
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(5))
                .unwrap_or(true)
        } else {
            true
        };
        // Touch the lock file
        let _ = std::fs::write(&lock_path, b"");

        // Only open browser if not suppressed (hook starts set CLAUDE_VIEW_NO_OPEN=1)
        if should_open && std::env::var("CLAUDE_VIEW_NO_OPEN").unwrap_or_default() != "1" {
            if let Err(e) = open::that(&browse_url) {
                tracing::debug!("Could not open browser: {e}");
            }
        }

        // Show deep indexing spinner if Pass 2 is running
        if tui_state.status() == IndexingStatus::DeepIndexing {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner} Deep indexing {msg}")
                    .expect("valid spinner template"),
            );
            pb.enable_steady_tick(Duration::from_millis(100));

            let deep_start = Instant::now();
            loop {
                let status = tui_state.status();
                if status == IndexingStatus::Done || status == IndexingStatus::Error {
                    break;
                }
                let indexed = tui_state.indexed();
                let total = tui_state.total();
                let bp = tui_state.bytes_processed();
                let bt = tui_state.bytes_total();
                if total > 0 {
                    let elapsed_secs = deep_start.elapsed().as_secs_f64();
                    let throughput = if elapsed_secs > 0.1 {
                        format!("  ({}/s)", format_bytes((bp as f64 / elapsed_secs) as u64))
                    } else {
                        String::new()
                    };
                    if bt > 0 {
                        pb.set_message(format!(
                            "{} / {}{}  {}/{} sessions...",
                            format_bytes(bp),
                            format_bytes(bt),
                            throughput,
                            indexed,
                            total,
                        ));
                    } else {
                        pb.set_message(format!("{}/{} sessions...", indexed, total));
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            pb.finish_and_clear();

            if tui_state.status() == IndexingStatus::Done {
                let deep_elapsed = deep_start.elapsed();
                let total = tui_state.sessions_found();
                let bp = tui_state.bytes_processed();
                eprintln!(
                    "  \u{2713} Deep index complete \u{2014} {} sessions, {} processed ({})\n",
                    total,
                    format_bytes(bp),
                    claude_view_core::format_duration(deep_elapsed),
                );
            } else if let Some(err) = tui_state.error() {
                eprintln!("  \u{2717} Indexing error: {}\n", err);
            }
        } else if tui_state.status() == IndexingStatus::Error {
            if let Some(err) = tui_state.error() {
                eprintln!("  \u{2717} Indexing error: {}\n", err);
            }
        }
    });

    // Step 9: Spawn facet ingest background tasks (startup + periodic)
    //
    // Uses a dedicated FacetIngestState for background tasks. The AppState has
    // its own FacetIngestState for user-triggered ingest via the API/SSE endpoint.
    // Background ingest is fire-and-forget with tracing output only.
    {
        let db = db.clone();
        let ingest_state = Arc::new(FacetIngestState::new());

        // Initial ingest (delayed 3s to let indexing finish first)
        let db_init = db.clone();
        let state_init = ingest_state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            if state_init.is_running() {
                return;
            }
            match claude_view_server::facet_ingest::run_facet_ingest(&db_init, &state_init, None)
                .await
            {
                Ok(n) => {
                    if n > 0 {
                        tracing::info!(
                            "Facet ingest: imported {n} new facets from /insights cache"
                        );
                    }
                }
                Err(e) => tracing::warn!("Facet ingest skipped: {e}"),
            }
        });

        // Periodic re-ingest (every 12 hours)
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(12 * 3600));
            interval.tick().await; // skip immediate tick (startup already handled above)
            loop {
                interval.tick().await;
                if ingest_state.is_running() {
                    tracing::debug!("Periodic facet ingest skipped: already running");
                    continue;
                }
                tracing::info!("Periodic facet re-ingest starting");
                match claude_view_server::facet_ingest::run_facet_ingest(&db, &ingest_state, None)
                    .await
                {
                    Ok(n) => {
                        if n > 0 {
                            tracing::info!("Periodic facet ingest: imported {n} new facets");
                        }
                    }
                    Err(e) => tracing::warn!("Periodic facet ingest failed: {e}"),
                }
            }
        });
    }

    // Step 10: Backfill git_root for sessions indexed before this field existed.
    {
        let db = Arc::new(db.clone());
        tokio::spawn(async move {
            claude_view_server::backfill::backfill_git_roots(db).await;
        });
    }

    // Step 11: Serve forever (with graceful shutdown for hook cleanup)
    let shutdown_port = port;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            // Listen for both SIGINT (Ctrl+C) and SIGTERM (kill, Docker, systemd).
            // Without SIGTERM handling, `kill <pid>` bypasses all cleanup.
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("register SIGTERM handler");

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            eprintln!("\n  Shutting down...");

            // Signal all SSE streams to terminate (breaks their select! loops).
            // This is the key step — without it, axum waits forever for open
            // SSE connections to close, and the process never exits.
            let _ = shutdown_tx.send(true);

            // Clean up hooks from ~/.claude/settings.json (skip in sandbox mode)
            if std::env::var("CLAUDE_VIEW_SKIP_HOOKS").as_deref() != Ok("1") {
                claude_view_server::live::hook_registrar::cleanup(shutdown_port);
                claude_view_server::live::statusline_injector::cleanup();
            }

            // Shut down managed oMLX process if we own it
            local_llm_for_shutdown.shutdown_managed().await;

            // Shut down Node.js sidecar if running
            sidecar_for_shutdown.shutdown();

            // Give SSE streams a moment to see the shutdown signal and break.
            // Second signal (Ctrl+C or another SIGTERM) skips the wait for impatient users.
            // sigterm.recv() is re-armable and cancel-safe — safe to reuse in a second select!
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
            }
        })
        .await?;

    // Hard exit: axum's graceful shutdown waits for all connections to close.
    // If any SSE stream missed the shutdown signal, the process would hang.
    // Force exit to guarantee the process terminates.
    std::process::exit(0);
}
