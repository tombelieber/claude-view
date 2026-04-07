// crates/server/src/sidecar/mod.rs
//! Node.js sidecar process manager for interactive control.
//!
//! The sidecar wraps the Claude Agent SDK (npm-only) and exposes a local
//! HTTP + WebSocket API on TCP port 3001. The frontend connects directly
//! to the sidecar via Vite proxy; the Rust server uses this manager for
//! lifecycle management (spawn, health check, model fetch).

mod error;
mod health;
mod lifecycle;
mod manager;
mod process;

#[cfg(test)]
mod tests;

// Re-export all public items to preserve the original `crate::sidecar::*` API.
pub use error::SidecarError;
pub use manager::SidecarManager;
