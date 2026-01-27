// crates/server/src/routes/projects.rs
//! Projects listing endpoint.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use vibe_recall_core::ProjectInfo;

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/projects - List all Claude Code projects with their sessions.
///
/// Returns a list of all indexed projects from the database,
/// with session metadata for each project.
pub async fn list_projects(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<ProjectInfo>>> {
    let projects = state.db.list_projects().await?;
    Ok(Json(projects))
}

/// Create the projects routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/projects", get(list_projects))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode}};
    use tower::ServiceExt;
    use vibe_recall_core::{SessionInfo, ToolCounts};
    use vibe_recall_db::Database;

    /// Helper: create an in-memory database for tests.
    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    /// Helper: create a test session with sensible defaults.
    fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: project.to_string(),
            project_path: format!("/home/user/{}", project),
            file_path: format!(
                "/home/user/.claude/projects/{}/{}.jsonl",
                project, id
            ),
            modified_at,
            size_bytes: 2048,
            preview: format!("Preview for {}", id),
            last_message: format!("Last message for {}", id),
            files_touched: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
            skills_used: vec!["/commit".to_string()],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
        }
    }

    /// Helper: build a full app Router with the given database.
    fn build_app(db: Database) -> Router {
        crate::create_app(db)
    }

    /// Helper: make a GET request and return status + body string.
    async fn get(app: Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

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

        let json = serde_json::to_string_pretty(&project).unwrap();
        // Verify camelCase field names
        assert!(json.contains("\"name\": \"test-project\""));
        assert!(json.contains("\"displayName\": \"Test Project\""));
        assert!(json.contains("\"activeCount\": 1"));
        // Verify modifiedAt is serialized as ISO 8601 string (not number)
        println!("Serialized JSON:\n{}", json);
        assert!(
            json.contains("\"modifiedAt\": \"2024-"),
            "modifiedAt should be ISO string, got: {}",
            json
        );
        // Verify toolCounts structure
        assert!(json.contains("\"toolCounts\""));
        assert!(json.contains("\"edit\": 5"));
        assert!(json.contains("\"read\": 10"));
        assert!(json.contains("\"bash\": 3"));
        assert!(json.contains("\"write\": 2"));
    }

    #[tokio::test]
    async fn test_projects_endpoint_returns_from_db() {
        let db = test_db().await;

        // Insert known sessions into the in-memory DB
        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 2000);
        let s3 = make_session("sess-3", "project-b", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        let app = build_app(db);
        let (status, body) = get(app, "/api/projects").await;

        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let projects = json.as_array().expect("response should be an array");
        assert_eq!(projects.len(), 2, "Should have 2 projects");

        // Projects should be sorted by most recent activity (project-b first)
        assert_eq!(projects[0]["name"], "project-b");
        assert_eq!(projects[0]["displayName"], "Project B");
        assert_eq!(projects[0]["sessions"].as_array().unwrap().len(), 1);

        assert_eq!(projects[1]["name"], "project-a");
        assert_eq!(projects[1]["displayName"], "Project A");
        assert_eq!(projects[1]["sessions"].as_array().unwrap().len(), 2);

        // Verify camelCase field names in session objects
        let session = &projects[1]["sessions"][0];
        assert!(session.get("id").is_some());
        assert!(session.get("projectPath").is_some());
        assert!(session.get("filePath").is_some());
        assert!(session.get("modifiedAt").is_some());
        assert!(session.get("sizeBytes").is_some());
        assert!(session.get("lastMessage").is_some());
        assert!(session.get("filesTouched").is_some());
        assert!(session.get("skillsUsed").is_some());
        assert!(session.get("toolCounts").is_some());
        assert!(session.get("messageCount").is_some());
        assert!(session.get("turnCount").is_some());

        // Verify modifiedAt is an ISO string, not a number
        let modified_at = session["modifiedAt"].as_str()
            .expect("modifiedAt should be a string (ISO 8601)");
        assert!(
            modified_at.starts_with("19") || modified_at.starts_with("20"),
            "modifiedAt should be an ISO date string, got: {}",
            modified_at
        );

        // Verify toolCounts structure
        let tool_counts = &session["toolCounts"];
        assert_eq!(tool_counts["edit"], 5);
        assert_eq!(tool_counts["read"], 10);
        assert_eq!(tool_counts["bash"], 3);
        assert_eq!(tool_counts["write"], 2);
    }

    #[tokio::test]
    async fn test_projects_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = get(app, "/api/projects").await;

        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let projects = json.as_array().expect("response should be an array");
        assert_eq!(projects.len(), 0, "Empty DB should return empty array");
    }
}
