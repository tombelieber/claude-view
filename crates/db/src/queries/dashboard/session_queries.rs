// crates/db/src/queries/dashboard/session_queries.rs
// Flat session listing and filtered/paginated session queries.

use super::super::row_types::SessionRow;
use super::types::SessionFilterParams;
use crate::{Database, DbResult};
use claude_view_core::SessionInfo;

impl Database {
    /// List all non-sidechain sessions across all projects.
    ///
    /// Flat query — no project grouping, no turns JOIN.
    /// Returns sessions sorted by `last_message_at` DESC.
    pub async fn list_all_sessions(&self) -> DbResult<Vec<SessionInfo>> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT
                s.id, s.project_id, s.preview, s.turn_count,
                s.last_message_at, s.file_path,
                s.project_path, s.git_root, s.project_display_name,
                s.size_bytes, s.last_message, s.files_touched, s.skills_used,
                s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
                s.message_count,
                COALESCE(s.summary_text, s.summary) AS summary,
                s.git_branch, s.is_sidechain, s.deep_indexed_at,
                s.total_input_tokens,
                s.total_output_tokens,
                s.cache_read_tokens AS total_cache_read_tokens,
                s.cache_creation_tokens AS total_cache_creation_tokens,
                s.api_call_count AS turn_count_api,
                s.primary_model,
                s.user_prompt_count, s.api_call_count, s.tool_call_count,
                s.files_read, s.files_edited,
                s.files_read_count, s.files_edited_count, s.reedited_files_count,
                s.duration_seconds, s.first_message_at, s.commit_count,
                s.thinking_block_count, s.turn_duration_avg_ms, s.turn_duration_max_ms,
                s.api_error_count, s.compaction_count, s.agent_spawn_count,
                s.bash_progress_count, s.hook_progress_count, s.mcp_progress_count,
                s.lines_added, s.lines_removed, s.loc_source,
                s.summary_text, s.parse_version,
                s.category_l1, s.category_l2, s.category_l3,
                s.category_confidence, s.category_source, s.classified_at,
                s.prompt_word_count, s.correction_count, s.same_file_edit_count,
                s.total_task_time_seconds, s.longest_task_seconds, s.longest_task_preview,
                s.total_cost_usd,
                s.slug,
                s.entrypoint
            FROM valid_sessions s
            ORDER BY s.last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        let sessions = rows
            .into_iter()
            .map(|r| {
                let pid = r.project_id.clone();
                r.into_session_info(&pid)
            })
            .collect();

        Ok(sessions)
    }

    /// Query sessions with server-side filtering, sorting, and pagination.
    ///
    /// Returns (sessions, total_matching_count).
    /// Uses sqlx::QueryBuilder for safe dynamic WHERE clauses.
    pub async fn query_sessions_filtered(
        &self,
        params: &SessionFilterParams,
    ) -> DbResult<(Vec<SessionInfo>, usize)> {
        // --- Shared WHERE clause builder ---
        // We build the WHERE fragment once for COUNT and once for SELECT.

        let select_cols = r#"
            s.id, s.project_id, s.preview, s.turn_count,
            s.last_message_at, s.file_path,
            s.project_path, s.git_root, s.project_display_name,
            s.size_bytes, s.last_message, s.files_touched, s.skills_used,
            s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
            s.message_count,
            COALESCE(s.summary_text, s.summary) AS summary,
            s.git_branch, s.is_sidechain, s.deep_indexed_at,
            s.total_input_tokens,
            s.total_output_tokens,
            s.cache_read_tokens AS total_cache_read_tokens,
            s.cache_creation_tokens AS total_cache_creation_tokens,
            s.api_call_count AS turn_count_api,
            s.primary_model,
            s.user_prompt_count, s.api_call_count, s.tool_call_count,
            s.files_read, s.files_edited,
            s.files_read_count, s.files_edited_count, s.reedited_files_count,
            s.duration_seconds, s.first_message_at, s.commit_count,
            s.thinking_block_count, s.turn_duration_avg_ms, s.turn_duration_max_ms,
            s.api_error_count, s.compaction_count, s.agent_spawn_count,
            s.bash_progress_count, s.hook_progress_count, s.mcp_progress_count,
            s.lines_added, s.lines_removed, s.loc_source,
            s.summary_text, s.parse_version,
            s.category_l1, s.category_l2, s.category_l3,
            s.category_confidence, s.category_source, s.classified_at,
            s.prompt_word_count, s.correction_count, s.same_file_edit_count,
            s.total_task_time_seconds, s.longest_task_seconds, s.longest_task_preview,
            s.total_cost_usd,
            s.slug,
            s.entrypoint
        "#;

        // Choose base table: show_archived queries `sessions` directly
        // (with is_sidechain filter in append_filters), default uses `valid_sessions` view.
        let base_from = if params.show_archived == Some(true) {
            "sessions s"
        } else {
            "valid_sessions s"
        };

        // --- COUNT query ---
        let mut count_qb = sqlx::QueryBuilder::new(format!("SELECT COUNT(*) FROM {base_from}"));
        append_filters(&mut count_qb, params);

        let total: (i64,) = count_qb.build_query_as().fetch_one(self.pool()).await?;
        let total = total.0 as usize;

        // --- DATA query ---
        let mut data_qb = sqlx::QueryBuilder::new(format!("SELECT {select_cols} FROM {base_from}"));
        append_filters(&mut data_qb, params);

        // ORDER BY (with s.last_message_at DESC tiebreaker for deterministic order)
        match params.sort.as_str() {
            "tokens" => data_qb.push(" ORDER BY (COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0)) DESC, s.last_message_at DESC"),
            "prompts" => data_qb.push(" ORDER BY s.user_prompt_count DESC, s.last_message_at DESC"),
            "files_edited" => data_qb.push(" ORDER BY s.files_edited_count DESC, s.last_message_at DESC"),
            "duration" => data_qb.push(" ORDER BY s.duration_seconds DESC, s.last_message_at DESC"),
            _ => data_qb.push(" ORDER BY s.last_message_at DESC"), // "recent"
        };

        // LIMIT + OFFSET
        data_qb.push(" LIMIT ");
        data_qb.push_bind(params.limit);
        data_qb.push(" OFFSET ");
        data_qb.push_bind(params.offset);

        let rows: Vec<SessionRow> = data_qb.build_query_as().fetch_all(self.pool()).await?;

        let sessions = rows
            .into_iter()
            .map(|r| {
                let pid = r.project_id.clone();
                r.into_session_info(&pid)
            })
            .collect();

        Ok((sessions, total))
    }
}

/// Helper: appends all WHERE clauses to a QueryBuilder.
/// Called twice — once for COUNT(*), once for SELECT.
fn append_filters<'args>(
    qb: &mut sqlx::QueryBuilder<'args, sqlx::Sqlite>,
    params: &'args SessionFilterParams,
) {
    qb.push(" WHERE 1=1");

    // When querying FROM sessions (show_archived mode), replicate
    // the sidechain filter that valid_sessions normally provides.
    if params.show_archived == Some(true) {
        qb.push(" AND s.is_sidechain = 0");
    }

    // Tantivy-resolved search: filter by pre-computed session IDs
    if let Some(ids) = &params.search_session_ids {
        if ids.is_empty() {
            // Tantivy returned no matches — short-circuit to zero results
            qb.push(" AND 1=0");
        } else {
            qb.push(" AND s.id IN (");
            let mut sep = qb.separated(", ");
            for id in ids {
                sep.push_bind(id.as_str());
            }
            sep.push_unseparated(")");
        }
    } else if let Some(q) = &params.q {
        // Fallback: SQLite LIKE if Tantivy is unavailable
        let pattern = format!("%{}%", q);
        qb.push(" AND (s.preview LIKE ");
        qb.push_bind(pattern.clone());
        qb.push(" OR s.last_message LIKE ");
        qb.push_bind(pattern.clone());
        qb.push(" OR s.project_display_name LIKE ");
        qb.push_bind(pattern);
        qb.push(")");
    }

    // Branch filter — handle NO_BRANCH sentinel ("~" → git_branch IS NULL)
    if let Some(branches) = &params.branches {
        if !branches.is_empty() {
            let has_no_branch = branches.iter().any(|b| b == "~");
            let named: Vec<&str> = branches
                .iter()
                .filter(|b| b.as_str() != "~")
                .map(|b| b.as_str())
                .collect();

            if has_no_branch && named.is_empty() {
                qb.push(" AND s.git_branch IS NULL");
            } else if has_no_branch {
                qb.push(" AND (s.git_branch IS NULL OR s.git_branch IN (");
                let mut sep = qb.separated(", ");
                for b in &named {
                    sep.push_bind(*b);
                }
                sep.push_unseparated("))");
            } else {
                qb.push(" AND s.git_branch IN (");
                let mut sep = qb.separated(", ");
                for b in branches {
                    sep.push_bind(b.as_str());
                }
                sep.push_unseparated(")");
            }
        }
    }

    // Model filter (IN list)
    if let Some(models) = &params.models {
        if !models.is_empty() {
            qb.push(" AND s.primary_model IN (");
            let mut sep = qb.separated(", ");
            for m in models {
                sep.push_bind(m.as_str());
            }
            sep.push_unseparated(")");
        }
    }

    // has_commits
    if let Some(has) = params.has_commits {
        if has {
            qb.push(" AND s.commit_count > 0");
        } else {
            qb.push(" AND s.commit_count = 0");
        }
    }

    // has_skills — skills_used is a JSON array string, '[]' means empty
    if let Some(has) = params.has_skills {
        if has {
            qb.push(" AND s.skills_used != '[]' AND s.skills_used != ''");
        } else {
            qb.push(" AND (s.skills_used = '[]' OR s.skills_used = '')");
        }
    }

    // min_duration
    if let Some(min) = params.min_duration {
        qb.push(" AND s.duration_seconds >= ");
        qb.push_bind(min);
    }

    // min_files
    if let Some(min) = params.min_files {
        qb.push(" AND s.files_edited_count >= ");
        qb.push_bind(min);
    }

    // min_tokens (input + output)
    if let Some(min) = params.min_tokens {
        qb.push(
            " AND (COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0)) >= ",
        );
        qb.push_bind(min);
    }

    // high_reedit (reedit rate > 0.2)
    if let Some(true) = params.high_reedit {
        qb.push(" AND s.files_edited_count > 0 AND CAST(s.reedited_files_count AS REAL) / s.files_edited_count > 0.2");
    }

    // time_after
    if let Some(after) = params.time_after {
        qb.push(" AND s.last_message_at >= ");
        qb.push_bind(after);
    }

    // time_before
    if let Some(before) = params.time_before {
        qb.push(" AND s.last_message_at <= ");
        qb.push_bind(before);
    }

    // Project filter (worktree-aware: match project_id, git_root, or project_path)
    if let Some(ref project) = params.project {
        qb.push(" AND (s.project_id = ");
        qb.push_bind(project.as_str());
        qb.push(" OR (s.git_root IS NOT NULL AND s.git_root != '' AND s.git_root = ");
        qb.push_bind(project.as_str());
        qb.push(") OR (s.project_path IS NOT NULL AND s.project_path != '' AND s.project_path = ");
        qb.push_bind(project.as_str());
        qb.push("))");
    }
}

#[cfg(test)]
mod filtered_query_tests {
    use super::*;
    use crate::Database;
    use claude_view_core::{SessionInfo, ToolCounts};

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: project.to_string(),
            project_path: format!("/home/user/{}", project),
            display_name: project.to_string(),
            git_root: None,
            file_path: format!("/path/{}.jsonl", id),
            modified_at,
            size_bytes: 2048,
            preview: format!("Preview for {}", id),
            last_message: format!("Last message for {}", id),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: Some(10),
            primary_model: Some("claude-sonnet-4".to_string()),
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
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
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        }
    }

    fn default_params() -> SessionFilterParams {
        SessionFilterParams {
            q: None,
            search_session_ids: None,
            branches: None,
            models: None,
            has_commits: None,
            has_skills: None,
            min_duration: None,
            min_files: None,
            min_tokens: None,
            high_reedit: None,
            time_after: None,
            time_before: None,
            project: None,
            show_archived: None,
            sort: "recent".to_string(),
            limit: 30,
            offset: 0,
        }
    }

    #[tokio::test]
    async fn test_no_filters_returns_all() {
        let db = test_db().await;
        for i in 0..5 {
            let s = make_session(&format!("s-{i}"), "proj", 1700000000 + i);
            db.insert_session(&s, "proj", "Project").await.unwrap();
        }
        let (sessions, total) = db.query_sessions_filtered(&default_params()).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(sessions.len(), 5);
    }

    #[tokio::test]
    async fn test_pagination_limit_offset() {
        let db = test_db().await;
        for i in 0..10 {
            let s = make_session(&format!("s-{i}"), "proj", 1700000000 + i);
            db.insert_session(&s, "proj", "Project").await.unwrap();
        }
        let params = SessionFilterParams {
            limit: 3,
            offset: 2,
            ..default_params()
        };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 10);
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn test_text_search() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.preview = "Fix authentication bug".to_string();
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let mut s2 = make_session("s-2", "proj", 1700000001);
        s2.preview = "Add new feature".to_string();
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams {
            q: Some("auth".to_string()),
            ..default_params()
        };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_branch_filter() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.git_branch = Some("main".to_string());
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let mut s2 = make_session("s-2", "proj", 1700000001);
        s2.git_branch = Some("feature/auth".to_string());
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams {
            branches: Some(vec!["main".to_string()]),
            ..default_params()
        };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_has_commits_filter() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.commit_count = 3;
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let s2 = make_session("s-2", "proj", 1700000001);
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams {
            has_commits: Some(true),
            ..default_params()
        };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_time_range_filter() {
        let db = test_db().await;
        let s1 = make_session("s-1", "proj", 1700000000);
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let s2 = make_session("s-2", "proj", 1720000000);
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams {
            time_after: Some(1710000000),
            ..default_params()
        };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-2");
    }

    #[tokio::test]
    async fn test_sort_by_duration() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.duration_seconds = 100;
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let mut s2 = make_session("s-2", "proj", 1700000001);
        s2.duration_seconds = 5000;
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams {
            sort: "duration".to_string(),
            ..default_params()
        };
        let (sessions, _) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(sessions[0].id, "s-2"); // longest first
    }
}
