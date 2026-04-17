// crates/core/src/discovery/projects.rs
//! Project and session scanning from the Claude projects directory.

use super::metadata::extract_session_metadata;
use super::paths::claude_projects_dir;
use super::resolve::{resolve_project_path_with_cwd, ResolvedProject};
use crate::error::DiscoveryError;
use crate::types::{ProjectInfo, SessionInfo};
use std::path::Path;
use tokio::fs;
use tracing::debug;

/// Count sessions that are "active" (modified within the last 5 minutes).
/// This matches the Node.js behavior for the activeCount field.
pub fn count_active_sessions(sessions: &[SessionInfo]) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let five_minutes_ago = now - 5 * 60;

    sessions
        .iter()
        .filter(|s| s.modified_at > five_minutes_ago)
        .count()
}

/// Discover all Claude Code projects and their sessions.
///
/// # Returns
/// A list of `ProjectInfo` sorted by most recent session activity.
///
/// # Errors
/// Returns an error only for permission denied. Missing directories return empty vec.
pub async fn get_projects() -> Result<Vec<ProjectInfo>, DiscoveryError> {
    let projects_dir = claude_projects_dir()?;

    // If the directory doesn't exist, return empty list (not an error)
    if !projects_dir.exists() {
        debug!(
            "Claude projects directory does not exist: {:?}",
            projects_dir
        );
        return Ok(vec![]);
    }

    let mut entries = match fs::read_dir(&projects_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(DiscoveryError::PermissionDenied { path: projects_dir });
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => {
            return Err(DiscoveryError::Io {
                path: projects_dir,
                source: e,
            });
        }
    };

    let mut projects = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|e| DiscoveryError::Io {
        path: projects_dir.clone(),
        source: e,
    })? {
        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        let encoded_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Use cwd from JSONL as authoritative source (no naive decode)
        let cwd = crate::session_index::resolve_cwd_for_project(&path);
        let resolved = resolve_project_path_with_cwd(&encoded_name, cwd.as_deref());

        // Get sessions for this project
        let sessions = match get_project_sessions(&path, &encoded_name, &resolved).await {
            Ok(s) => s,
            Err(e) => {
                debug!("Error reading sessions for {:?}: {}", path, e);
                continue;
            }
        };

        // Skip projects with no sessions
        if sessions.is_empty() {
            continue;
        }

        let active_count = count_active_sessions(&sessions);

        projects.push(ProjectInfo {
            name: encoded_name,
            display_name: resolved.display_name,
            path: resolved.full_path,
            active_count,
            sessions,
        });
    }

    // Sort projects by most recent session
    projects.sort_by(|a, b| {
        let a_latest = a.sessions.iter().map(|s| s.modified_at).max().unwrap_or(0);
        let b_latest = b.sessions.iter().map(|s| s.modified_at).max().unwrap_or(0);
        b_latest.cmp(&a_latest)
    });

    Ok(projects)
}

/// Get all sessions for a project directory.
async fn get_project_sessions(
    project_path: &Path,
    encoded_name: &str,
    resolved: &ResolvedProject,
) -> Result<Vec<SessionInfo>, std::io::Error> {
    let mut entries = fs::read_dir(project_path).await?;
    let mut sessions = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Only process .jsonl files
        if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
            continue;
        }

        let metadata = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Extract session ID from filename
        let session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract metadata efficiently
        let extracted = extract_session_metadata(&path).await;

        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        sessions.push(SessionInfo {
            id: session_id,
            project: encoded_name.to_string(),
            project_path: resolved.full_path.clone(),
            display_name: resolved.display_name.clone(),
            git_root: None,
            file_path: path.to_string_lossy().to_string(),
            modified_at,
            size_bytes: metadata.len(),
            preview: extracted.preview,
            last_message: extracted.last_message,
            files_touched: extracted.files_touched,
            skills_used: extracted.skills_used,
            tool_counts: extracted.tool_counts,
            message_count: extracted.message_count,
            turn_count: extracted.turn_count,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            // Phase 3: Atomic unit metrics (initialized to defaults, populated by deep indexing)
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,

            parse_version: 0,
            // Phase C: LOC estimation
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            // Theme 4: Classification
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        });
    }

    // Sort sessions by modification time (most recent first)
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ============================================================================
    // get_projects Tests
    // ============================================================================

    #[tokio::test]
    async fn test_get_projects_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join(".claude").join("projects");
        tokio::fs::create_dir_all(&projects_dir).await.unwrap();

        // We can't easily test get_projects with a custom path since it uses claude_projects_dir()
        // Instead, test that it handles the real path gracefully
        let result = get_projects().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_projects_returns_vec() {
        let result = get_projects().await;
        assert!(result.is_ok());
        let _projects: Vec<ProjectInfo> = result.unwrap();
    }

    // ============================================================================
    // get_project_sessions Tests
    // ============================================================================

    #[tokio::test]
    async fn test_get_project_sessions_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved).await;
        assert!(sessions.is_ok());
        assert!(sessions.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_project_sessions_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        let session_path = temp_dir.path().join("session-123.jsonl");
        let content = r#"{"type":"user","message":{"content":"Test"}}"#;
        tokio::fs::write(&session_path, content).await.unwrap();

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved)
            .await
            .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-123");
        assert_eq!(sessions[0].preview, "Test");
    }

    #[tokio::test]
    async fn test_get_project_sessions_ignores_non_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        tokio::fs::write(
            temp_dir.path().join("session.jsonl"),
            r#"{"type":"user","message":{"content":"Test"}}"#,
        )
        .await
        .unwrap();
        tokio::fs::write(temp_dir.path().join("notes.txt"), "some notes")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.path().join("config.json"), "{}")
            .await
            .unwrap();

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved)
            .await
            .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session");
    }

    // ============================================================================
    // count_active_sessions Tests
    // ============================================================================

    #[test]
    fn test_count_active_sessions_within_5_minutes() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sessions = vec![
            create_test_session_with_time(now - 60), // 1 min ago (active)
            create_test_session_with_time(now - 240), // 4 min ago (active)
            create_test_session_with_time(now - 600), // 10 min ago (not active)
            create_test_session_with_time(now - 3600), // 1 hour ago (not active)
        ];

        let active = count_active_sessions(&sessions);
        assert_eq!(active, 2, "Should count 2 sessions within 5 minutes");
    }

    #[test]
    fn test_count_active_sessions_none_recent() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sessions = vec![
            create_test_session_with_time(now - 600),  // 10 min ago
            create_test_session_with_time(now - 1800), // 30 min ago
        ];

        let active = count_active_sessions(&sessions);
        assert_eq!(
            active, 0,
            "Should count 0 when no sessions within 5 minutes"
        );
    }

    #[test]
    fn test_count_active_sessions_boundary() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sessions = vec![
            create_test_session_with_time(now - 299), // Just under 5 min (active)
            create_test_session_with_time(now - 301), // Just over 5 min (not active)
        ];

        let active = count_active_sessions(&sessions);
        assert_eq!(
            active, 1,
            "Should count session at 4:59 as active, 5:01 as not"
        );
    }

    fn create_test_session_with_time(modified_at: i64) -> crate::types::SessionInfo {
        crate::types::SessionInfo {
            id: "test".to_string(),
            project: "test".to_string(),
            project_path: "/test".to_string(),
            display_name: "test".to_string(),
            git_root: None,
            file_path: "/test/session.jsonl".to_string(),
            modified_at,
            size_bytes: 100,
            preview: "Test".to_string(),
            last_message: "Test".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: crate::types::ToolCounts::default(),
            message_count: 1,
            turn_count: 1,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            // Phase 3: Atomic unit metrics
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,

            parse_version: 0,
            // Phase C: LOC estimation
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            // Theme 4: Classification
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        }
    }
}
