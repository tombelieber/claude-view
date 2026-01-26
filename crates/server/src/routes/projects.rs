// crates/server/src/routes/projects.rs
//! Projects listing endpoint.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use vibe_recall_core::ProjectInfo;

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/projects - List all Claude Code projects with their sessions.
///
/// Returns a list of all projects found in the Claude projects directory,
/// with session metadata for each project.
pub async fn list_projects(State(_state): State<Arc<AppState>>) -> ApiResult<Json<Vec<ProjectInfo>>> {
    // Use the core discovery function
    let projects = vibe_recall_core::get_projects().await?;
    Ok(Json(projects))
}

/// Create the projects routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/projects", get(list_projects))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vibe_recall_core::{SessionInfo, ToolCounts};

    #[test]
    fn test_project_info_serialization() {
        let project = ProjectInfo {
            name: "test-project".to_string(),
            display_name: "Test Project".to_string(),
            path: "/path/to/project".to_string(),
            sessions: vec![SessionInfo {
                id: "abc123".to_string(),
                project: "test-project".to_string(),
                project_path: "/path/to/project".to_string(),
                file_path: "/path/to/session.jsonl".to_string(),
                modified_at: 1706369000,
                size_bytes: 1024,
                preview: "Hello Claude".to_string(),
                last_message: "Here is the result".to_string(),
                files_touched: vec!["src/main.rs".to_string()],
                skills_used: vec!["commit".to_string()],
                tool_counts: ToolCounts {
                    edit: 5,
                    read: 10,
                    bash: 3,
                    write: 2,
                },
                message_count: 20,
                turn_count: 10,
            }],
            active_count: 1,
        };

        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"name\":\"test-project\""));
        assert!(json.contains("\"displayName\":\"Test Project\""));
        assert!(json.contains("\"activeCount\":1"));
    }
}
