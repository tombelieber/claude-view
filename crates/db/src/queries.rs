// crates/db/src/queries.rs
// Session CRUD operations for the vibe-recall SQLite database.

use crate::{Database, DbResult};
use chrono::Utc;
use std::collections::HashMap;
use vibe_recall_core::{ProjectInfo, SessionInfo, ToolCounts};

/// Indexer state entry returned from the database.
#[derive(Debug, Clone)]
pub struct IndexerEntry {
    pub file_path: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub indexed_at: i64,
}

impl Database {
    /// Upsert a session into the database.
    ///
    /// Uses `INSERT ... ON CONFLICT DO UPDATE` to preserve columns not listed in the upsert.
    /// `project_encoded` is the URL-encoded project name (stored as `project_id`).
    /// `project_display_name` is the human-readable project name.
    pub async fn insert_session(
        &self,
        session: &SessionInfo,
        project_encoded: &str,
        project_display_name: &str,
    ) -> DbResult<()> {
        let files_touched = serde_json::to_string(&session.files_touched)
            .unwrap_or_else(|_| "[]".to_string());
        let skills_used = serde_json::to_string(&session.skills_used)
            .unwrap_or_else(|_| "[]".to_string());
        let indexed_at = Utc::now().timestamp();
        let size_bytes = session.size_bytes as i64;
        let message_count = session.message_count as i32;
        let turn_count = session.turn_count as i32;
        let tool_edit = session.tool_counts.edit as i32;
        let tool_read = session.tool_counts.read as i32;
        let tool_bash = session.tool_counts.bash as i32;
        let tool_write = session.tool_counts.write as i32;

        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, project_id, preview, turn_count,
                last_message_at, file_path,
                indexed_at, project_path, project_display_name,
                size_bytes, last_message, files_touched, skills_used,
                tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
                message_count
            ) VALUES (
                ?1, ?2, ?3, ?4,
                ?5, ?6,
                ?7, ?8, ?9,
                ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17,
                ?18
            )
            ON CONFLICT(id) DO UPDATE SET
                project_id = excluded.project_id,
                preview = excluded.preview,
                turn_count = excluded.turn_count,
                last_message_at = excluded.last_message_at,
                file_path = excluded.file_path,
                indexed_at = excluded.indexed_at,
                project_path = excluded.project_path,
                project_display_name = excluded.project_display_name,
                size_bytes = excluded.size_bytes,
                last_message = excluded.last_message,
                files_touched = excluded.files_touched,
                skills_used = excluded.skills_used,
                tool_counts_edit = excluded.tool_counts_edit,
                tool_counts_read = excluded.tool_counts_read,
                tool_counts_bash = excluded.tool_counts_bash,
                tool_counts_write = excluded.tool_counts_write,
                message_count = excluded.message_count
            "#,
        )
        .bind(&session.id)
        .bind(project_encoded)
        .bind(&session.preview)
        .bind(turn_count)
        .bind(session.modified_at)
        .bind(&session.file_path)
        .bind(indexed_at)
        .bind(&session.project_path)
        .bind(project_display_name)
        .bind(size_bytes)
        .bind(&session.last_message)
        .bind(&files_touched)
        .bind(&skills_used)
        .bind(tool_edit)
        .bind(tool_read)
        .bind(tool_bash)
        .bind(tool_write)
        .bind(message_count)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// List all projects with their sessions, grouped by project_id.
    ///
    /// Sessions within each project are sorted by `last_message_at` DESC.
    /// `active_count` is calculated as sessions with `last_message_at` within
    /// the last 5 minutes (300 seconds).
    pub async fn list_projects(&self) -> DbResult<Vec<ProjectInfo>> {
        let now = Utc::now().timestamp();
        let active_threshold = now - 300;

        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT
                id, project_id, preview, turn_count,
                last_message_at, file_path,
                project_path, project_display_name,
                size_bytes, last_message, files_touched, skills_used,
                tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
                message_count
            FROM sessions
            ORDER BY last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        // Group rows by project_id
        let mut project_map: HashMap<String, Vec<SessionRow>> = HashMap::new();
        for row in rows {
            project_map
                .entry(row.project_id.clone())
                .or_default()
                .push(row);
        }

        let mut projects: Vec<ProjectInfo> = project_map
            .into_iter()
            .map(|(project_id, rows)| {
                let display_name = rows
                    .first()
                    .map(|r| r.project_display_name.clone())
                    .unwrap_or_default();
                let path = rows
                    .first()
                    .map(|r| r.project_path.clone())
                    .unwrap_or_default();

                let active_count = rows
                    .iter()
                    .filter(|r| r.last_message_at.unwrap_or(0) > active_threshold)
                    .count();

                let sessions: Vec<SessionInfo> = rows
                    .into_iter()
                    .map(|r| r.into_session_info(&project_id))
                    .collect();

                ProjectInfo {
                    name: project_id,
                    display_name,
                    path,
                    sessions,
                    active_count,
                }
            })
            .collect();

        // Sort projects by most recent session activity
        projects.sort_by(|a, b| {
            let a_latest = a.sessions.first().map(|s| s.modified_at).unwrap_or(0);
            let b_latest = b.sessions.first().map(|s| s.modified_at).unwrap_or(0);
            b_latest.cmp(&a_latest)
        });

        Ok(projects)
    }

    /// Check if a file needs re-indexing by retrieving its indexer state.
    pub async fn get_indexer_state(&self, file_path: &str) -> DbResult<Option<IndexerEntry>> {
        let row: Option<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT file_path, file_size, modified_at, indexed_at FROM indexer_state WHERE file_path = ?1",
        )
        .bind(file_path)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|(file_path, file_size, modified_at, indexed_at)| IndexerEntry {
            file_path,
            file_size,
            modified_at,
            indexed_at,
        }))
    }

    /// Batch-load all indexer states into a HashMap keyed by file_path.
    ///
    /// This avoids the N+1 query pattern when diffing many files against the DB.
    pub async fn get_all_indexer_states(&self) -> DbResult<HashMap<String, IndexerEntry>> {
        let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT file_path, file_size, modified_at, indexed_at FROM indexer_state",
        )
        .fetch_all(self.pool())
        .await?;

        let map = rows
            .into_iter()
            .map(|(file_path, file_size, modified_at, indexed_at)| {
                let entry = IndexerEntry {
                    file_path: file_path.clone(),
                    file_size,
                    modified_at,
                    indexed_at,
                };
                (file_path, entry)
            })
            .collect();

        Ok(map)
    }

    /// Mark a file as indexed with the given size and modification time.
    pub async fn update_indexer_state(
        &self,
        file_path: &str,
        size: i64,
        mtime: i64,
    ) -> DbResult<()> {
        let indexed_at = Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO indexer_state (file_path, file_size, modified_at, indexed_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(file_path)
        .bind(size)
        .bind(mtime)
        .bind(indexed_at)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Remove sessions whose file_path is NOT in the given list of valid paths.
    /// Also cleans up corresponding indexer_state entries.
    /// Both deletes run in a transaction for consistency.
    pub async fn remove_stale_sessions(&self, valid_paths: &[String]) -> DbResult<u64> {
        let mut tx = self.pool().begin().await?;

        if valid_paths.is_empty() {
            let result = sqlx::query("DELETE FROM sessions")
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM indexer_state")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(result.rows_affected());
        }

        // Build placeholders for the IN clause
        let placeholders: Vec<String> = (1..=valid_paths.len()).map(|i| format!("?{}", i)).collect();
        let in_clause = placeholders.join(", ");

        let delete_sessions_sql = format!(
            "DELETE FROM sessions WHERE file_path NOT IN ({})",
            in_clause
        );
        let delete_indexer_sql = format!(
            "DELETE FROM indexer_state WHERE file_path NOT IN ({})",
            in_clause
        );

        let mut query = sqlx::query(&delete_sessions_sql);
        for path in valid_paths {
            query = query.bind(path);
        }
        let result = query.execute(&mut *tx).await?;

        let mut query = sqlx::query(&delete_indexer_sql);
        for path in valid_paths {
            query = query.bind(path);
        }
        query.execute(&mut *tx).await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }
}

// Internal row type for reading sessions from SQLite.
#[derive(Debug)]
struct SessionRow {
    id: String,
    project_id: String,
    preview: String,
    turn_count: i32,
    last_message_at: Option<i64>,
    file_path: String,
    project_path: String,
    project_display_name: String,
    size_bytes: i64,
    last_message: String,
    files_touched: String,
    skills_used: String,
    tool_counts_edit: i32,
    tool_counts_read: i32,
    tool_counts_bash: i32,
    tool_counts_write: i32,
    message_count: i32,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for SessionRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            preview: row.try_get("preview")?,
            turn_count: row.try_get("turn_count")?,
            last_message_at: row.try_get("last_message_at")?,
            file_path: row.try_get("file_path")?,
            project_path: row.try_get("project_path")?,
            project_display_name: row.try_get("project_display_name")?,
            size_bytes: row.try_get("size_bytes")?,
            last_message: row.try_get("last_message")?,
            files_touched: row.try_get("files_touched")?,
            skills_used: row.try_get("skills_used")?,
            tool_counts_edit: row.try_get("tool_counts_edit")?,
            tool_counts_read: row.try_get("tool_counts_read")?,
            tool_counts_bash: row.try_get("tool_counts_bash")?,
            tool_counts_write: row.try_get("tool_counts_write")?,
            message_count: row.try_get("message_count")?,
        })
    }
}

impl SessionRow {
    fn into_session_info(self, project_encoded: &str) -> SessionInfo {
        let files_touched: Vec<String> =
            serde_json::from_str(&self.files_touched).unwrap_or_default();
        let skills_used: Vec<String> =
            serde_json::from_str(&self.skills_used).unwrap_or_default();

        SessionInfo {
            id: self.id,
            project: project_encoded.to_string(),
            project_path: self.project_path,
            file_path: self.file_path,
            modified_at: self.last_message_at.unwrap_or(0),
            size_bytes: self.size_bytes as u64,
            preview: self.preview,
            last_message: self.last_message,
            files_touched,
            skills_used,
            tool_counts: ToolCounts {
                edit: self.tool_counts_edit as usize,
                read: self.tool_counts_read as usize,
                bash: self.tool_counts_bash as usize,
                write: self.tool_counts_write as usize,
            },
            message_count: self.message_count as usize,
            turn_count: self.turn_count as usize,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test SessionInfo with sensible defaults.
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

    #[tokio::test]
    async fn test_insert_and_list_projects() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert 3 sessions across 2 projects
        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 2000);
        let s3 = make_session("sess-3", "project-b", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 2, "Should have 2 projects");

        // Projects should be sorted by most recent activity (project-b first)
        assert_eq!(projects[0].name, "project-b");
        assert_eq!(projects[0].sessions.len(), 1);
        assert_eq!(projects[0].display_name, "Project B");

        assert_eq!(projects[1].name, "project-a");
        assert_eq!(projects[1].sessions.len(), 2);
        assert_eq!(projects[1].display_name, "Project A");

        // Within project-a, sessions should be sorted by last_message_at DESC
        assert_eq!(projects[1].sessions[0].id, "sess-2");
        assert_eq!(projects[1].sessions[1].id, "sess-1");

        // Verify JSON fields deserialized correctly
        assert_eq!(
            projects[1].sessions[0].files_touched,
            vec!["src/main.rs", "Cargo.toml"]
        );
        assert_eq!(projects[1].sessions[0].skills_used, vec!["/commit"]);
        assert_eq!(projects[1].sessions[0].tool_counts.edit, 5);
    }

    #[tokio::test]
    async fn test_upsert_session() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = make_session("sess-1", "project-a", 1000);
        db.insert_session(&s1, "project-a", "Project A").await.unwrap();

        // Update same session with new data
        let s1_updated = SessionInfo {
            preview: "Updated preview".to_string(),
            modified_at: 5000,
            message_count: 50,
            ..s1
        };
        db.insert_session(&s1_updated, "project-a", "Project A")
            .await
            .unwrap();

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1, "Should still have 1 project");
        assert_eq!(projects[0].sessions.len(), 1, "Should still have 1 session (upsert, not duplicate)");
        assert_eq!(projects[0].sessions[0].preview, "Updated preview");
        assert_eq!(projects[0].sessions[0].modified_at, 5000);
        assert_eq!(projects[0].sessions[0].message_count, 50);
    }

    #[tokio::test]
    async fn test_indexer_state_roundtrip() {
        let db = Database::new_in_memory().await.unwrap();

        let path = "/home/user/.claude/projects/test/session.jsonl";

        // Initially no state
        let state = db.get_indexer_state(path).await.unwrap();
        assert!(state.is_none(), "Should have no state initially");

        // Set state
        db.update_indexer_state(path, 4096, 1234567890).await.unwrap();

        // Read back
        let state = db.get_indexer_state(path).await.unwrap();
        assert!(state.is_some(), "Should have state after update");
        let entry = state.unwrap();
        assert_eq!(entry.file_path, path);
        assert_eq!(entry.file_size, 4096);
        assert_eq!(entry.modified_at, 1234567890);
        assert!(entry.indexed_at > 0, "indexed_at should be set");

        // Update state (upsert)
        db.update_indexer_state(path, 8192, 1234567999).await.unwrap();
        let entry = db.get_indexer_state(path).await.unwrap().unwrap();
        assert_eq!(entry.file_size, 8192);
        assert_eq!(entry.modified_at, 1234567999);
    }

    #[tokio::test]
    async fn test_remove_stale_sessions() {
        let db = Database::new_in_memory().await.unwrap();

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 2000);
        let s3 = make_session("sess-3", "project-b", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        // Also add indexer state for the sessions
        db.update_indexer_state(&s1.file_path, 2048, 1000).await.unwrap();
        db.update_indexer_state(&s2.file_path, 2048, 2000).await.unwrap();
        db.update_indexer_state(&s3.file_path, 2048, 3000).await.unwrap();

        // Keep only sess-1's file path; sess-2 and sess-3 are stale
        let valid = vec![s1.file_path.clone()];
        let removed = db.remove_stale_sessions(&valid).await.unwrap();
        assert_eq!(removed, 2, "Should have removed 2 stale sessions");

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1, "Should have 1 project left");
        assert_eq!(projects[0].sessions.len(), 1);
        assert_eq!(projects[0].sessions[0].id, "sess-1");

        // Indexer state should also be cleaned up
        assert!(db.get_indexer_state(&s2.file_path).await.unwrap().is_none());
        assert!(db.get_indexer_state(&s3.file_path).await.unwrap().is_none());
        // The valid file should still have its indexer state
        assert!(db.get_indexer_state(&s1.file_path).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_active_count_calculation() {
        let db = Database::new_in_memory().await.unwrap();
        let now = Utc::now().timestamp();

        // Session within the 5-minute window (active)
        let s_active = SessionInfo {
            modified_at: now - 60, // 1 minute ago
            ..make_session("active-sess", "project-a", now - 60)
        };

        // Session outside the 5-minute window (inactive)
        let s_old = SessionInfo {
            modified_at: now - 600, // 10 minutes ago
            ..make_session("old-sess", "project-a", now - 600)
        };

        db.insert_session(&s_active, "project-a", "Project A").await.unwrap();
        db.insert_session(&s_old, "project-a", "Project A").await.unwrap();

        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].active_count, 1, "Only 1 session should be active (within 5 min)");
        assert_eq!(projects[0].sessions.len(), 2, "Both sessions should be listed");
    }

    #[tokio::test]
    async fn test_get_all_indexer_states() {
        let db = Database::new_in_memory().await.unwrap();

        // Initially empty
        let states = db.get_all_indexer_states().await.unwrap();
        assert!(states.is_empty(), "Should be empty initially");

        // Insert some indexer state entries
        let path_a = "/home/user/.claude/projects/test/a.jsonl";
        let path_b = "/home/user/.claude/projects/test/b.jsonl";
        let path_c = "/home/user/.claude/projects/test/c.jsonl";

        db.update_indexer_state(path_a, 1000, 100).await.unwrap();
        db.update_indexer_state(path_b, 2000, 200).await.unwrap();
        db.update_indexer_state(path_c, 3000, 300).await.unwrap();

        // Fetch all states
        let states = db.get_all_indexer_states().await.unwrap();
        assert_eq!(states.len(), 3, "Should have 3 entries");

        // Verify each entry is keyed correctly and has correct values
        let a = states.get(path_a).expect("Should contain path_a");
        assert_eq!(a.file_size, 1000);
        assert_eq!(a.modified_at, 100);

        let b = states.get(path_b).expect("Should contain path_b");
        assert_eq!(b.file_size, 2000);
        assert_eq!(b.modified_at, 200);

        let c = states.get(path_c).expect("Should contain path_c");
        assert_eq!(c.file_size, 3000);
        assert_eq!(c.modified_at, 300);

        // All entries should have indexed_at set
        assert!(a.indexed_at > 0);
        assert!(b.indexed_at > 0);
        assert!(c.indexed_at > 0);
    }

    #[tokio::test]
    async fn test_list_projects_returns_camelcase_json() {
        let db = Database::new_in_memory().await.unwrap();
        let now = Utc::now().timestamp();

        let s1 = make_session("sess-1", "project-a", now);
        db.insert_session(&s1, "project-a", "Project A").await.unwrap();

        let projects = db.list_projects().await.unwrap();
        let json = serde_json::to_string(&projects).unwrap();

        // Verify camelCase keys in ProjectInfo
        assert!(json.contains("\"displayName\""), "Should use camelCase: displayName");
        assert!(json.contains("\"activeCount\""), "Should use camelCase: activeCount");

        // Verify camelCase keys in SessionInfo
        assert!(json.contains("\"projectPath\""), "Should use camelCase: projectPath");
        assert!(json.contains("\"filePath\""), "Should use camelCase: filePath");
        assert!(json.contains("\"modifiedAt\""), "Should use camelCase: modifiedAt");
        assert!(json.contains("\"sizeBytes\""), "Should use camelCase: sizeBytes");
        assert!(json.contains("\"lastMessage\""), "Should use camelCase: lastMessage");
        assert!(json.contains("\"filesTouched\""), "Should use camelCase: filesTouched");
        assert!(json.contains("\"skillsUsed\""), "Should use camelCase: skillsUsed");
        assert!(json.contains("\"toolCounts\""), "Should use camelCase: toolCounts");
        assert!(json.contains("\"messageCount\""), "Should use camelCase: messageCount");
        assert!(json.contains("\"turnCount\""), "Should use camelCase: turnCount");

        // modifiedAt should be an ISO string, not a number
        assert!(
            json.contains("\"modifiedAt\":\"20"),
            "modifiedAt should be ISO string: {}",
            json
        );
    }
}
