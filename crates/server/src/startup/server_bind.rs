//! HTTP listener bind + post-bind wiring (hooks, port file, telemetry).
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Smart port binding behaviour
//! (auto-increment on conflict, kill-stale-then-retry) and sandbox-mode
//! contract (`CLAUDE_VIEW_SKIP_HOOKS=1` disables auto-increment) are
//! unchanged.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use tokio::net::TcpListener;

use crate::startup::install::{detect_install_source, ping_install_beacon};
use crate::startup::port::{get_port, try_reclaim_port};
use crate::telemetry::TelemetryClient;

/// Bind the HTTP listener using the smart port-resolution strategy and
/// return the bound listener together with the port actually used.
///
/// Strategy:
/// 1. Try the requested port (`CLAUDE_VIEW_PORT` / `PORT` / default).
/// 2. On `EADDRINUSE`, if the holder looks like a stale claude-view → kill
///    it and retry the same port.
/// 3. If the holder is another app → auto-increment (up to +10) unless
///    `CLAUDE_VIEW_SKIP_HOOKS=1` (sandbox mode, hooks are pre-configured
///    for a fixed port; failing fast is safer than binding a different
///    port that hooks wouldn't route to).
pub async fn bind_listener() -> Result<(TcpListener, u16)> {
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

    let skip_hooks = std::env::var("CLAUDE_VIEW_SKIP_HOOKS").as_deref() == Ok("1");
    let mut try_port = port;
    let max_port = if skip_hooks { port } else { port + 10 };
    loop {
        let addr = SocketAddr::from((bind_addr, try_port));
        match TcpListener::bind(addr).await {
            Ok(l) => return Ok((l, try_port)),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if !skip_hooks && try_reclaim_port(try_port) {
                    // Killed a stale claude-view — retry same port
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    if let Ok(l) = TcpListener::bind(addr).await {
                        return Ok((l, try_port));
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
}

/// Register Claude Code hooks with the ACTUAL bound port (may differ from
/// the requested port due to auto-increment on conflict) and write a port
/// file so CLI subcommands can discover the running server.
pub fn register_hooks_and_port_file(port: u16) {
    crate::register_hooks(port);

    let port_file = claude_view_core::paths::data_dir().join("port");
    if let Err(e) = std::fs::write(&port_file, port.to_string()) {
        tracing::warn!("Failed to write port file: {e}");
    }
}

/// Fire `server_started` PostHog event and the install-beacon ping.
/// Both are best-effort, fire-and-forget, non-blocking.
pub fn fire_startup_events(telemetry: Option<&TelemetryClient>) {
    let install_source = detect_install_source();
    if let Some(client) = telemetry {
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
    ping_install_beacon(install_source);
}
