//! Request and response types for the CLI sessions API.

use serde::{Deserialize, Serialize};

/// A tmux-managed CLI session running Claude Code.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct CliSession {
    /// Tmux session name, e.g. "cv-abc123".
    pub id: String,
    /// Unix timestamp in milliseconds when the session was created.
    pub created_at: u64,
    /// Current session status.
    pub status: CliSessionStatus,
    /// Working directory for the CLI session.
    pub project_dir: Option<String>,
    /// CLI args passed at creation.
    pub args: Vec<String>,
}

/// Status of a CLI session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CliSessionStatus {
    Running,
    Exited,
}

/// Request body for POST /api/cli-sessions.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    /// Optional working directory for the CLI session.
    #[serde(default)]
    pub project_dir: Option<String>,
    /// Additional CLI args to pass to `claude`.
    #[serde(default)]
    pub args: Vec<String>,
}

/// Response for POST /api/cli-sessions.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct CreateResponse {
    pub session: CliSession,
}

/// Response for GET /api/cli-sessions.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct ListResponse {
    pub sessions: Vec<CliSession>,
}
