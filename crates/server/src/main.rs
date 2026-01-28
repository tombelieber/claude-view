// crates/server/src/main.rs
//! Vibe-recall server binary.
//!
//! Starts an Axum HTTP server that serves the vibe-recall API.
//! Optionally serves static files for the frontend (SPA mode).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use vibe_recall_db::indexer;
use vibe_recall_db::Database;
use vibe_recall_server::{create_app, create_app_with_static};

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

/// Format byte size into a human-readable string (e.g., "828 MB").
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (quiet — startup UX uses println)
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let startup_start = Instant::now();

    // Print banner
    eprintln!("\n\u{1f50d} vibe-recall v{}\n", env!("CARGO_PKG_VERSION"));

    // Step 1: Open the database
    let db = Database::open_default().await?;

    // Step 2: Scan for .jsonl files
    let base_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".claude/projects");

    let scan = indexer::scan_files(&base_dir).await;
    let session_count = scan.files.len();

    // Step 3: Diff against DB
    let diff = indexer::diff_against_db(&scan.files, &db).await?;
    let files_to_index: Vec<_> = diff
        .new_files
        .iter()
        .chain(diff.modified_files.iter())
        .cloned()
        .collect();
    let to_index_count = files_to_index.len();

    let all_valid_paths: Vec<String> = scan
        .files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();

    // Determine whether this is a first launch, incremental, or no-change scenario
    let is_first_launch = diff.unchanged_count == 0 && to_index_count > 0;
    let has_changes = to_index_count > 0;

    if is_first_launch {
        // First launch
        eprintln!(
            "  Scanning projects...          found {} projects, {} sessions ({})",
            scan.project_count,
            session_count,
            format_size(scan.total_size),
        );
    } else if has_changes {
        // Incremental
        let new_count = diff.new_files.len();
        let mod_count = diff.modified_files.len();
        let mut parts = Vec::new();
        if new_count > 0 {
            parts.push(format!("{} new", new_count));
        }
        if mod_count > 0 {
            parts.push(format!("{} modified", mod_count));
        }
        eprintln!(
            "  Checking for changes...       {} sessions",
            parts.join(", "),
        );
    } else {
        // No changes
        eprintln!(
            "  Checking for changes...       up to date ({} sessions)",
            diff.unchanged_count,
        );
    }

    // Step 4: Index changed files with progress bar
    if has_changes {
        let pb = ProgressBar::new(to_index_count as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  Indexing sessions {bar:20} {pos}/{len} ({elapsed})")
                .expect("valid progress bar template")
                .progress_chars("\u{2501}\u{2501}\u{2500}"),
        );

        indexer::index_files(&files_to_index, &all_valid_paths, &db, |indexed, _total| {
            pb.set_position(indexed as u64);
        })
        .await?;

        pb.finish();
    }

    // Step 5: Start the server
    let static_dir = get_static_dir();
    let app = match static_dir {
        Some(ref dir) => create_app_with_static(db, Some(dir.clone())),
        None => create_app(db),
    };

    let port = get_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let total_elapsed = startup_start.elapsed();
    eprintln!("\n  \u{2713} Ready in {:.1}s", total_elapsed.as_secs_f64());
    eprintln!("  \u{2192} http://localhost:{}\n", port);

    axum::serve(listener, app).await?;

    Ok(())
}
