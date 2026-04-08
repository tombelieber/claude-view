//! Session source types: where a Claude Code session was launched from.
//!
//! Originally defined in `process.rs`; canonical home is now here so the
//! state crate is self-contained.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a Claude Code process was launched from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../packages/shared/src/types/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    /// Interactive shell (zsh, bash, fish, etc.)
    Terminal,
    /// IDE extension (VS Code, Cursor, IntelliJ, etc.)
    Ide,
    /// claude-view Agent SDK sidecar
    AgentSdk,
}

/// Metadata about the source environment of a Claude process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../packages/shared/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SessionSourceInfo {
    /// Category: terminal, ide, or agent_sdk.
    pub category: SessionSource,
    /// Human-readable label for the source (e.g. "VS Code", "IntelliJ", "Cursor").
    /// None for terminal sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}
