//! Multiplexed WebSocket for per-session real-time data.
//!
//! Replaces the two separate WebSocket connections (terminal WS + sidecar WS)
//! with a single multiplexed connection that carries typed frames.

pub mod frames;
pub mod handler;
pub mod registry;

#[cfg(test)]
mod tests;
