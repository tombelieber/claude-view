//! `axum::serve(...)` with graceful shutdown for hook cleanup.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Signal handling (SIGINT +
//! SIGTERM), SSE shutdown broadcast, port-file removal, hook cleanup,
//! local-LLM/sidecar shutdown, and the 2-second grace window are all
//! unchanged from the pre-split runtime.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::local_llm::LocalLlmService;
use crate::SidecarManager;

/// Serve the axum router with graceful shutdown on SIGINT / SIGTERM.
///
/// On shutdown:
/// 1. Broadcast `true` on `shutdown_tx` so SSE streams break their `select!`
///    loops (without this, axum waits forever for open SSE connections).
/// 2. Remove the port file so CLI subcommands don't connect to a dead
///    server.
/// 3. Clean up Claude Code hooks from `~/.claude/settings.json` (unless
///    `CLAUDE_VIEW_SKIP_HOOKS=1`).
/// 4. Shut down the managed local-LLM process and the Node.js sidecar.
/// 5. Wait up to 2 s for SSE streams to close (or a second signal to skip).
pub async fn run(
    listener: TcpListener,
    app: Router,
    shutdown_tx: watch::Sender<bool>,
    port: u16,
    local_llm: Arc<LocalLlmService>,
    sidecar: Arc<SidecarManager>,
) -> Result<()> {
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

            // Remove port file so CLI subcommands don't connect to a dead server
            let _ = std::fs::remove_file(claude_view_core::paths::data_dir().join("port"));

            // Clean up hooks from ~/.claude/settings.json (skip in sandbox mode)
            if std::env::var("CLAUDE_VIEW_SKIP_HOOKS").as_deref() != Ok("1") {
                crate::live::hook_registrar::cleanup(shutdown_port);
                crate::live::statusline_injector::cleanup();
            }

            // Shut down managed oMLX process if we own it
            local_llm.shutdown_managed().await;

            // Shut down Node.js sidecar if running
            sidecar.shutdown();

            // Give SSE streams a moment to see the shutdown signal and break.
            // Second signal (Ctrl+C or another SIGTERM) skips the wait for
            // impatient users. `sigterm.recv()` is re-armable and cancel-safe —
            // safe to reuse in a second `select!`.
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
            }
        })
        .await?;
    Ok(())
}
