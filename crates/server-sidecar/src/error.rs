// crates/server/src/sidecar/error.rs
//! Sidecar error types.

/// Errors from sidecar operations.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("Failed to spawn sidecar: {0}")]
    SpawnFailed(std::io::Error),
    #[error("Sidecar health check timed out after 3s")]
    HealthCheckTimeout,
    #[error("Sidecar directory not found (set SIDECAR_DIR or place sidecar/ next to binary)")]
    SidecarDirNotFound,
    #[error("Node.js not found in PATH (required for interactive mode)")]
    NodeNotFound,
    #[error("Sidecar returned error: {0}")]
    RequestError(String),
    /// Too many sidecar spawns in a short window — something is repeatedly
    /// crashing it. Refuse to spawn again until user action or cooldown.
    /// See `CIRCUIT_BREAKER_THRESHOLD` and `CIRCUIT_BREAKER_WINDOW` in manager.rs.
    #[error("Sidecar circuit open: {0}")]
    CircuitOpen(String),
}
