// crates/server/src/main.rs
//! Vibe-recall server binary.
//!
//! Starts an Axum HTTP server that serves the vibe-recall API.

use std::net::SocketAddr;

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use vibe_recall_server::create_app;

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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Create the app
    let app = create_app();

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

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
