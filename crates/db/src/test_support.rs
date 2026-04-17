// crates/db/src/test_support.rs
//
// Canonical test-seeding helper — thin wrapper around
// `execute_upsert_parsed_session`. Defaults supply empty JSON arrays, zero
// counts, and null cost so mechanical test-fixture construction is a
// one-for-one field-name port.

use crate::indexer_parallel::ParsedSession;
use crate::queries::sessions::execute_upsert_parsed_session;
use crate::{Database, DbResult};

/// Fluent builder for seeding test sessions via the production UPSERT path.
#[derive(Debug, Clone)]
pub struct SessionSeedBuilder {
    session: ParsedSession,
}

impl SessionSeedBuilder {
    /// Start a new builder. Only `id` is required; every other field has a
    /// Pass-1-equivalent default (empty string / `[]` / `None` / `0`).
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            session: ParsedSession {
                id: id.into(),
                project_id: String::new(),
                project_display_name: String::new(),
                project_path: String::new(),
                file_path: String::new(),
                preview: String::new(),
                summary: None,
                message_count: 0,
                last_message_at: 0,
                first_message_at: 0,
                git_branch: None,
                is_sidechain: false,
                size_bytes: 0,
                last_message: String::new(),
                turn_count: 0,
                tool_counts_edit: 0,
                tool_counts_read: 0,
                tool_counts_bash: 0,
                tool_counts_write: 0,
                files_touched: "[]".to_string(),
                skills_used: "[]".to_string(),
                user_prompt_count: 0,
                api_call_count: 0,
                tool_call_count: 0,
                files_read: "[]".to_string(),
                files_edited: "[]".to_string(),
                files_read_count: 0,
                files_edited_count: 0,
                reedited_files_count: 0,
                duration_seconds: 0,
                commit_count: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                thinking_block_count: 0,
                turn_duration_avg_ms: None,
                turn_duration_max_ms: None,
                turn_duration_total_ms: None,
                api_error_count: 0,
                api_retry_count: 0,
                compaction_count: 0,
                hook_blocked_count: 0,
                agent_spawn_count: 0,
                bash_progress_count: 0,
                hook_progress_count: 0,
                mcp_progress_count: 0,
                summary_text: None,
                parse_version: 0,
                file_size_at_index: 0,
                file_mtime_at_index: 0,
                lines_added: 0,
                lines_removed: 0,
                loc_source: 0,
                ai_lines_added: 0,
                ai_lines_removed: 0,
                work_type: None,
                primary_model: None,
                total_task_time_seconds: None,
                longest_task_seconds: None,
                longest_task_preview: None,
                total_cost_usd: None,
                slug: None,
                entrypoint: None,
            },
        }
    }

    // ── Pass-1 identity fields (matches `insert_session_from_index` arg list) ──

    pub fn project_id(mut self, s: impl Into<String>) -> Self {
        self.session.project_id = s.into();
        self
    }
    pub fn project_display_name(mut self, s: impl Into<String>) -> Self {
        self.session.project_display_name = s.into();
        self
    }
    pub fn project_path(mut self, s: impl Into<String>) -> Self {
        self.session.project_path = s.into();
        self
    }
    pub fn file_path(mut self, s: impl Into<String>) -> Self {
        self.session.file_path = s.into();
        self
    }
    pub fn preview(mut self, s: impl Into<String>) -> Self {
        self.session.preview = s.into();
        self
    }
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.session.summary = Some(s.into());
        self
    }
    pub fn message_count(mut self, n: i32) -> Self {
        self.session.message_count = n;
        self
    }
    /// Pass-1 `modified_at` mapped onto `last_message_at` (the `insert_session_from_index`
    /// SQL bound this arg to `last_message_at`).
    pub fn modified_at(mut self, ts: i64) -> Self {
        self.session.last_message_at = ts;
        self
    }
    pub fn last_message_at(mut self, ts: i64) -> Self {
        self.session.last_message_at = ts;
        self
    }
    pub fn first_message_at(mut self, ts: i64) -> Self {
        self.session.first_message_at = ts;
        self
    }
    pub fn git_branch(mut self, s: impl Into<String>) -> Self {
        self.session.git_branch = Some(s.into());
        self
    }
    pub fn is_sidechain(mut self, b: bool) -> Self {
        self.session.is_sidechain = b;
        self
    }
    pub fn size_bytes(mut self, n: i64) -> Self {
        self.session.size_bytes = n;
        self
    }

    // ── Pass-2 deep fields (most commonly set in tests) ──

    pub fn turn_count(mut self, n: i32) -> Self {
        self.session.turn_count = n;
        self
    }
    pub fn total_input_tokens(mut self, n: i64) -> Self {
        self.session.total_input_tokens = n;
        self
    }
    pub fn total_output_tokens(mut self, n: i64) -> Self {
        self.session.total_output_tokens = n;
        self
    }
    pub fn total_cost_usd(mut self, c: f64) -> Self {
        self.session.total_cost_usd = Some(c);
        self
    }
    pub fn primary_model(mut self, s: impl Into<String>) -> Self {
        self.session.primary_model = Some(s.into());
        self
    }
    pub fn ai_lines_added(mut self, n: i64) -> Self {
        self.session.ai_lines_added = n;
        self
    }
    pub fn ai_lines_removed(mut self, n: i64) -> Self {
        self.session.ai_lines_removed = n;
        self
    }

    // ── Escape hatch ──

    /// Mutate the underlying `ParsedSession` directly for fields without a
    /// dedicated setter. Use sparingly; prefer named setters when one exists.
    pub fn with_parsed<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut ParsedSession),
    {
        f(&mut self.session);
        self
    }

    /// Consume the builder and return the assembled `ParsedSession` without seeding.
    pub fn build(self) -> ParsedSession {
        self.session
    }

    /// Seed the session into the database via `execute_upsert_parsed_session`.
    pub async fn seed(self, db: &Database) -> DbResult<()> {
        execute_upsert_parsed_session(db.pool(), &self.session).await?;
        Ok(())
    }
}

/// Convenience wrapper for the most common shape: seed a session with just
/// id + project_id (+ optionally the common Pass-1 fields). Matches the
/// intent of `insert_session_from_index` for tests that only need a session
/// row to exist.
pub async fn seed_session_via_upsert(
    db: &Database,
    id: impl Into<String>,
    project_id: impl Into<String>,
) -> DbResult<()> {
    SessionSeedBuilder::new(id)
        .project_id(project_id)
        .seed(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_session_via_upsert_inserts_row() {
        let db = Database::new_in_memory().await.unwrap();
        seed_session_via_upsert(&db, "sess-1", "proj-a")
            .await
            .unwrap();

        let (id, project_id): (String, String) =
            sqlx::query_as("SELECT id, project_id FROM sessions WHERE id = 'sess-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(id, "sess-1");
        assert_eq!(project_id, "proj-a");
    }

    #[tokio::test]
    async fn builder_roundtrip_preserves_fluent_fields() {
        let db = Database::new_in_memory().await.unwrap();
        SessionSeedBuilder::new("sess-2")
            .project_id("proj-b")
            .project_display_name("Project B")
            .project_path("/tmp/b")
            .file_path("/tmp/b/sess-2.jsonl")
            .preview("hello")
            .summary("test summary")
            .message_count(42)
            .modified_at(1_700_000_000)
            .git_branch("main")
            .is_sidechain(false)
            .size_bytes(12_345)
            .turn_count(7)
            .total_input_tokens(1000)
            .total_output_tokens(500)
            .total_cost_usd(0.0123)
            .primary_model("claude-sonnet-4")
            .seed(&db)
            .await
            .unwrap();

        // sqlx FromRow maxes out at 16-tuples; split into two queries.
        let identity: (
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            i32,
        ) = sqlx::query_as(
            r#"SELECT id, project_id, project_display_name, project_path,
                       file_path, preview, summary, message_count
                FROM sessions WHERE id = 'sess-2'"#,
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(identity.0, "sess-2");
        assert_eq!(identity.1, "proj-b");
        assert_eq!(identity.2, "Project B");
        assert_eq!(identity.3, "/tmp/b");
        assert_eq!(identity.4, "/tmp/b/sess-2.jsonl");
        assert_eq!(identity.5, "hello");
        assert_eq!(identity.6.as_deref(), Some("test summary"));
        assert_eq!(identity.7, 42);

        let deep: (
            i64,
            Option<String>,
            bool,
            i64,
            i32,
            i64,
            i64,
            Option<f64>,
            Option<String>,
        ) = sqlx::query_as(
            r#"SELECT last_message_at, git_branch, is_sidechain, size_bytes,
                           turn_count, total_input_tokens, total_output_tokens,
                           total_cost_usd, primary_model
                    FROM sessions WHERE id = 'sess-2'"#,
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(deep.0, 1_700_000_000);
        assert_eq!(deep.1.as_deref(), Some("main"));
        assert!(!deep.2);
        assert_eq!(deep.3, 12_345);
        assert_eq!(deep.4, 7);
        assert_eq!(deep.5, 1000);
        assert_eq!(deep.6, 500);
        assert!((deep.7.unwrap() - 0.0123).abs() < 1e-9);
        assert_eq!(deep.8.as_deref(), Some("claude-sonnet-4"));
    }

    #[tokio::test]
    async fn default_builder_matches_pass_1_shape() {
        // Defaults should mirror `insert_session_from_index`: empty JSON arrays
        // for list fields, zero counts, null cost — never NULL for list columns.
        let db = Database::new_in_memory().await.unwrap();
        SessionSeedBuilder::new("sess-3").seed(&db).await.unwrap();

        let (files_touched, skills_used, files_read, files_edited, cost): (
            String,
            String,
            String,
            String,
            Option<f64>,
        ) = sqlx::query_as(
            r#"SELECT files_touched, skills_used, files_read, files_edited, total_cost_usd
                FROM sessions WHERE id = 'sess-3'"#,
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(files_touched, "[]");
        assert_eq!(skills_used, "[]");
        assert_eq!(files_read, "[]");
        assert_eq!(files_edited, "[]");
        assert_eq!(cost, None);
    }

    #[tokio::test]
    async fn escape_hatch_allows_exhaustive_field_access() {
        let db = Database::new_in_memory().await.unwrap();
        SessionSeedBuilder::new("sess-4")
            .project_id("proj-c")
            .with_parsed(|s| {
                s.compaction_count = 3;
                s.hook_blocked_count = 2;
                s.longest_task_seconds = Some(120);
            })
            .seed(&db)
            .await
            .unwrap();

        let (compactions, hook_blocked, longest): (i32, i32, Option<i64>) = sqlx::query_as(
            "SELECT compaction_count, hook_blocked_count, longest_task_seconds \
             FROM sessions WHERE id = 'sess-4'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(compactions, 3);
        assert_eq!(hook_blocked, 2);
        assert_eq!(longest, Some(120));
    }
}
