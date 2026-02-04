// crates/server/src/main.rs
//! Vibe-recall server binary.
//!
//! Starts an Axum HTTP server **immediately**, then spawns background indexing.
//! Pass 1 (read sessions-index.json, <10ms) populates the "Ready" line,
//! Pass 2 (deep JSONL parsing) runs in parallel with a TUI progress spinner.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use vibe_recall_db::indexer_parallel::run_background_index;
use vibe_recall_db::Database;
use vibe_recall_server::{create_app_full, init_metrics, record_sync, IndexingState, IndexingStatus};

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

/// Get the static directory for serving frontend files.
///
/// Priority:
/// 1. STATIC_DIR environment variable (explicit override)
/// 2. ./dist directory (if it exists)
/// 3. None (API-only mode)
fn get_static_dir() -> Option<PathBuf> {
    std::env::var("STATIC_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let dist = PathBuf::from("dist");
            dist.exists().then_some(dist)
        })
}

/// Run git sync with structured logging. Used by both initial and periodic sync.
async fn run_git_sync_logged(db: &Database, label: &str) {
    let start = Instant::now();
    tracing::info!(sync_type = label, "Starting git sync");

    match vibe_recall_db::git_correlation::run_git_sync(db).await {
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
/// Initial run backfills 365 days; periodic runs only need 2 days (today + yesterday).
async fn run_snapshot_generation(db: &Database, label: &str) {
    let days_back = if label == "initial" { 365 } else { 2 };
    match db.generate_missing_snapshots(days_back).await {
        Ok(count) => {
            if count > 0 {
                tracing::info!("{} snapshot generation: {} snapshots created", label, count);
            } else {
                tracing::debug!("{} snapshot generation: all snapshots up to date", label);
            }
        }
        Err(e) => {
            tracing::warn!("{} snapshot generation failed (non-fatal): {}", label, e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (quiet — startup UX uses eprintln)
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let startup_start = Instant::now();

    // Initialize Prometheus metrics
    init_metrics();

    // Print banner
    eprintln!("\n\u{1f50d} vibe-recall v{}\n", env!("CARGO_PKG_VERSION"));

    // Step 1: Open database
    let db = Database::open_default().await?;

    // Step 2: Create shared indexing state and registry holder
    let indexing = Arc::new(IndexingState::new());
    let registry_holder = Arc::new(RwLock::new(None));

    // Step 3: Build the Axum app with indexing state and registry holder
    let static_dir = get_static_dir();
    let app = create_app_full(
        db.clone(),
        indexing.clone(),
        registry_holder.clone(),
        static_dir,
    );

    // Step 4: Bind and start the HTTP server IMMEDIATELY (before any indexing)
    let port = get_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Step 5: Resolve the claude dir for indexing
    let claude_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".claude");

    // Step 6: Spawn background indexing task (with registry build in parallel)
    let idx_state = indexing.clone();
    let idx_db = db.clone();
    let idx_registry = registry_holder.clone();
    tokio::spawn(async move {
        idx_state.set_status(IndexingStatus::ReadingIndexes);

        let state_for_pass1 = idx_state.clone();
        let state_for_progress = idx_state.clone();
        let state_for_done = idx_state.clone();

        let result = run_background_index(
            &claude_dir,
            &idx_db,
            Some(idx_registry),
            // on_pass1_done: Pass 1 finished — store project/session counts, transition to DeepIndexing
            move |projects, sessions| {
                state_for_pass1.set_projects_found(projects);
                state_for_pass1.set_sessions_found(sessions);
                state_for_pass1.set_status(IndexingStatus::DeepIndexing);
            },
            // on_file_done: each deep-indexed file reports progress
            move |indexed, total| {
                state_for_progress.set_total(total);
                state_for_progress.set_indexed(indexed);
            },
            // on_complete: all done
            move |_total_indexed| {
                state_for_done.set_status(IndexingStatus::Done);
            },
        )
        .await;

        match result {
            Ok(_) => {
                // Auto git-sync: correlate commits with sessions after indexing completes.
                run_git_sync_logged(&idx_db, "initial").await;

                // Build contribution snapshots for all historical days.
                run_snapshot_generation(&idx_db, "initial").await;

                // Periodic git-sync: re-scan to pick up new commits.
                // Interval is user-configurable via Settings UI (stored in DB).
                // At 10x scale (~100 repos, ~5000 sessions), each run takes ~4-6s.
                // Re-read interval from DB each cycle so changes take effect without restart.
                loop {
                    let interval_secs = idx_db.get_git_sync_interval().await.unwrap_or(60);
                    let sync_interval = Duration::from_secs(interval_secs);
                    tokio::time::sleep(sync_interval).await;
                    run_git_sync_logged(&idx_db, "periodic").await;
                    run_snapshot_generation(&idx_db, "periodic").await;
                }
            }
            Err(e) => {
                idx_state.set_error(e);
            }
        }
    });

    // Step 7: Spawn TUI progress task (runs concurrently with the server)
    let tui_state = indexing.clone();
    tokio::spawn(async move {
        // Wait for Pass 1 to complete (status transitions out of Idle/ReadingIndexes)
        loop {
            let status = tui_state.status();
            if status != IndexingStatus::Idle && status != IndexingStatus::ReadingIndexes {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Print the "Ready" line with Pass 1 results
        let elapsed = startup_start.elapsed();
        let projects = tui_state.projects_found();
        let sessions = tui_state.sessions_found();
        eprintln!(
            "  \u{2713} Ready in {} \u{2014} {} projects, {} sessions",
            vibe_recall_core::format_duration(elapsed),
            projects,
            sessions,
        );
        eprintln!("  \u{2192} http://localhost:{}\n", port);

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
                if total > 0 {
                    pb.set_message(format!("{}/{} sessions...", indexed, total));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            pb.finish_and_clear();

            if tui_state.status() == IndexingStatus::Done {
                let deep_elapsed = deep_start.elapsed();
                let total = tui_state.sessions_found();
                eprintln!(
                    "  \u{2713} Deep index complete \u{2014} {} sessions ({})\n",
                    total,
                    vibe_recall_core::format_duration(deep_elapsed),
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

    // Step 8: Serve forever
    axum::serve(listener, app).await?;

    Ok(())
}
