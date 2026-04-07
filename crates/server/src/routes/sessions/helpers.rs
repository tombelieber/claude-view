//! Shared helpers for session route handlers.

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Resolve a session's JSONL file path: DB first, then live session store fallback.
///
/// Live sessions (especially IDE-spawned ones) may not be indexed in the DB yet.
/// The live session store always has the file path for any actively-monitored session.
pub(crate) async fn resolve_session_file_path(
    state: &AppState,
    session_id: &str,
) -> ApiResult<std::path::PathBuf> {
    let file_path = match state.db.get_session_file_path(session_id).await? {
        Some(p) => p,
        None => {
            let map = state.live_sessions.read().await;
            map.get(session_id)
                .map(|s| s.jsonl.file_path.clone())
                .ok_or_else(|| ApiError::SessionNotFound(session_id.to_string()))?
        }
    };
    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id.to_string()));
    }
    Ok(path)
}
