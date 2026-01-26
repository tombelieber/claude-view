// crates/server/src/main.rs
//! Vibe-recall server binary.
//!
//! Starts an Axum HTTP server that serves the vibe-recall API.
//! Optionally serves static files for the frontend (SPA mode).

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Determine static file serving mode
    let static_dir = get_static_dir();

    // Create the app with or without static file serving
    let app = match static_dir {
        Some(ref dir) => {
            info!("Serving static files from: {:?}", dir);
            create_app_with_static(Some(dir.clone()))
        }
        None => {
            info!("API-only mode (no static dir)");
            create_app()
        }
    };

    // Bind to the address
    let port = get_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    // Log startup information
    info!("Starting vibe-recall server v{}", env!("CARGO_PKG_VERSION"));
    info!("Listening on http://{}", addr);
    info!("API endpoints:");
    info!("  GET /api/health     - Health check");
    info!("  GET /api/projects   - List all projects");
    info!("  GET /api/session/:project/:id - Get session details");

    if static_dir.is_some() {
        info!("Static files:");
        info!("  GET /              - Frontend SPA (index.html)");
        info!("  GET /*             - Static assets with SPA fallback");
    }

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
