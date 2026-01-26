// crates/server/src/routes/sessions.rs
//! Session retrieval endpoint.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use vibe_recall_core::ParsedSession;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// GET /api/session/:project_dir/:session_id - Get a parsed session by ID.
///
/// Returns the full parsed session with all messages and metadata.
/// The project_dir is the URL-encoded project directory name (can contain slashes).
/// The session_id is the UUID of the session.
pub async fn get_session(
    State(_state): State<Arc<AppState>>,
    Path((project_dir, session_id)): Path<(String, String)>,
) -> ApiResult<Json<ParsedSession>> {
    // Decode the project directory (URL-encoded, may contain slashes)
    let project_dir_decoded = urlencoding::decode(&project_dir)
        .map_err(|_| ApiError::ProjectNotFound(project_dir.clone()))?
        .into_owned();

    // Get the Claude projects directory and construct the session path
    let projects_dir = vibe_recall_core::claude_projects_dir()?;
    let session_path = projects_dir
        .join(&project_dir_decoded)
        .join(&session_id)
        .with_extension("jsonl");

    // Check if the session file exists
    if !session_path.exists() {
        return Err(ApiError::SessionNotFound(format!(
            "{}/{}",
            project_dir_decoded, session_id
        )));
    }

    // Parse and return the session
    let session = vibe_recall_core::parse_session(&session_path).await?;
    Ok(Json(session))
}

/// Create the sessions routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/session/{project_dir}/{session_id}", get(get_session))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vibe_recall_core::{Message, SessionMetadata};
    use std::path::PathBuf;

    #[test]
    fn test_parsed_session_serialization() {
        let session = ParsedSession {
            messages: vec![
                Message::user("Hello Claude!"),
                Message::assistant("Hello! How can I help?"),
            ],
            metadata: SessionMetadata {
                total_messages: 2,
                tool_call_count: 0,
            },
        };

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"totalMessages\":2"));
    }

    #[test]
    fn test_session_path_construction() {
        // Verify the path construction logic
        let project_dir = "Users-TBGor-dev-myproject";
        let session_id = "abc123-def456";

        let base = PathBuf::from("/Users/TBGor/.claude/projects");
        let session_path = base
            .join(project_dir)
            .join(session_id)
            .with_extension("jsonl");

        assert_eq!(
            session_path.to_string_lossy(),
            "/Users/TBGor/.claude/projects/Users-TBGor-dev-myproject/abc123-def456.jsonl"
        );
    }
}
