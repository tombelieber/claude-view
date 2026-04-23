// crates/server/src/main.rs
//! Claude View server binary.
//!
//! Starts an Axum HTTP server **immediately**, then spawns background indexing.
//! Pass 1 (read sessions-index.json, <10ms) populates the "Ready" line,
//! Pass 2 (deep JSONL parsing) runs in parallel with a TUI progress spinner.
//!
//! Orchestration only — each startup concern lives in a dedicated module
//! under `claude_view_server::startup::*`. Extracted in CQRS Phase 7.f.

use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use claude_view_server::cli;
use claude_view_server::init_metrics;
use claude_view_server::startup::{
    bootstrap, cli_dispatch, data_dir, observability, platform, serve,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments FIRST — clap handles --version/--help and exits
    // before any I/O, tracing, or TLS setup. This keeps `--version` output
    // clean (no tracing init lines on stderr that break CI version checks).
    let cli_parsed = cli::Cli::parse();

    observability::install_tls_provider();
    let _obs_handle = observability::init_tracing()?;
    let app_config = observability::load_app_config();

    // `cleanup` is handled before async/DB work so it's a fast sync operation.
    cli_dispatch::handle_cleanup_if_requested(&cli_parsed);

    // Other query subcommands (monitor, live, stats) hit the running server.
    if let Some(cmd) = cli_parsed.command {
        return cli::run(cmd).await;
    }

    // No subcommand → start the server.
    let startup_start = Instant::now();
    platform::ensure_supported();
    init_metrics();
    eprintln!("\n\u{1f50d} claude-view v{}\n", env!("CARGO_PKG_VERSION"));
    data_dir::validate_and_cleanup_legacy();

    let handles = bootstrap::bootstrap(app_config, startup_start).await?;
    serve::run(
        handles.listener,
        handles.app,
        handles.shutdown_tx,
        handles.port,
        handles.local_llm,
        handles.sidecar,
    )
    .await?;

    // Hard exit: axum's graceful shutdown waits for all connections to close.
    // If any SSE stream missed the shutdown signal, the process would hang.
    std::process::exit(0);
}
