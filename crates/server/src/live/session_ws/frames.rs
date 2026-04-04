//! Typed frame protocol for the multiplexed session WebSocket.
//!
//! Each frame has a `type` discriminator and carries its payload as-is.
//! The server does NOT parse SDK event semantics — it routes typed payloads.

use serde::{Deserialize, Serialize};

// ── Client → Server ─────────────────────────────────────────────────

/// Handshake sent by the client immediately after WS open.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientHandshake {
    /// Which frame types the client wants to receive.
    pub modes: Vec<FrameMode>,
    /// Per-mode scrollback configuration.
    #[serde(default)]
    pub scrollback: ScrollbackConfig,
}

/// Frame modes the client can subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameMode {
    /// Parsed conversation blocks from JSONL.
    Block,
    /// Raw terminal lines from JSONL.
    TerminalRaw,
    /// Sidecar SDK events (relayed from Node.js sidecar WS).
    Sdk,
    /// Merged canonical session state updates.
    SessionState,
}

/// Per-mode scrollback line counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollbackConfig {
    #[serde(default = "default_block_scrollback")]
    pub block: u32,
    #[serde(default = "default_raw_scrollback")]
    pub raw: u32,
}

impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self {
            block: default_block_scrollback(),
            raw: default_raw_scrollback(),
        }
    }
}

fn default_block_scrollback() -> u32 {
    50
}
fn default_raw_scrollback() -> u32 {
    1000
}

/// Messages the client can send after handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Ping for application-level keepalive.
    Ping,
    /// Forward a message to the sidecar SDK WS.
    SdkSend { payload: serde_json::Value },
}

// ── Server → Client ─────────────────────────────────────────────────

/// Frames sent from server to client over the multiplexed WS.
///
/// Design choice: **thin framing.** Each variant tags the source and carries
/// the payload as-is. The server does NOT need to understand SDK event
/// semantics — it routes typed payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum SessionFrame {
    /// Handshake acknowledgement with session metadata.
    HandshakeAck {
        session_id: String,
        modes: Vec<FrameMode>,
    },

    /// A conversation block (parsed from JSONL). Same shape as terminal WS block mode.
    BlockDelta {
        #[serde(flatten)]
        block: serde_json::Value,
    },

    /// End-of-scrollback marker for blocks.
    BlockBufferEnd,

    /// A raw terminal line (from JSONL). Same shape as terminal WS raw mode.
    TerminalRaw { line: String },

    /// End-of-scrollback marker for raw lines.
    TerminalBufferEnd,

    /// Sidecar SDK event (relayed from Node.js sidecar). Payload is opaque.
    SdkEvent { payload: serde_json::Value },

    /// SDK connection status change.
    SdkStatus { connected: bool },

    /// Canonical session state update (merged from SDK + JSONL).
    SessionStateUpdate {
        #[serde(flatten)]
        state: serde_json::Value,
    },

    /// Application-level pong.
    Pong,

    /// Error frame.
    Error { message: String, code: String },
}
