// crates/db/src/queries/dashboard/project_listing.rs
// Project summaries, branch listing, and display-name disambiguation.

use super::super::BranchCount;
use crate::{Database, DbResult};
use chrono::Utc;
use claude_view_core::{BranchFilter, ProjectSummary, SessionInfo, SessionsPage};

use super::super::row_types::SessionRow;

impl Database {
    /// List lightweight project summaries (no sessions array).
    /// Returns ProjectSummary with counts only — sidebar payload.
    pub async fn list_project_summaries(&self) -> DbResult<Vec<ProjectSummary>> {
        let now = Utc::now().timestamp();
        let active_threshold = now - 300; // 5 minutes

        // Group by real decoded path, falling through: git_root → project_path → project_id.
        // This prevents duplicates when some sessions have git_root resolved and
        // others only have the encoded project_id (both decode to the same path).
        let rows: Vec<(String, String, String, i64, i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT
                COALESCE(NULLIF(git_root, ''), NULLIF(project_path, ''), project_id) as effective_id,
                COALESCE(
                    CASE WHEN git_root IS NOT NULL AND git_root != ''
                         THEN REPLACE(REPLACE(git_root, RTRIM(git_root, REPLACE(git_root, '/', '')), ''), '/', '')
                         ELSE NULL END,
                    project_display_name,
                    project_id
                ) as display_name,
                COALESCE(NULLIF(git_root, ''), NULLIF(project_path, ''), '') as effective_path,
                COUNT(*) as session_count,
                SUM(CASE WHEN last_message_at > ?1 THEN 1 ELSE 0 END) as active_count,
                MAX(last_message_at) as last_activity_at
            FROM valid_sessions
            GROUP BY effective_id
            ORDER BY last_activity_at DESC
            "#,
        )
        .bind(active_threshold)
        .fetch_all(self.pool())
        .await?;

        let mut summaries: Vec<ProjectSummary> = rows
            .into_iter()
            .map(
                |(name, display_name, path, session_count, active_count, last_activity_at)| {
                    ProjectSummary {
                        name,
                        display_name,
                        path,
                        session_count: session_count as usize,
                        active_count: active_count as usize,
                        last_activity_at,
                    }
                },
            )
            .collect();

        disambiguate_display_names(&mut summaries);

        Ok(summaries)
    }

    /// List paginated sessions for a specific project.
    ///
    /// Supports sorting (recent, oldest, messages), branch filtering,
    /// and sidechain inclusion.
    pub async fn list_sessions_for_project(
        &self,
        project_id: &str,
        limit: i64,
        offset: i64,
        sort: &str,
        branch_filter: &BranchFilter<'_>,
        include_sidechains: bool,
    ) -> DbResult<SessionsPage> {
        // Build WHERE clause dynamically.
        // Bind indices: ?1 = project_id, ?2 = limit, ?3 = offset.
        // ?4 is used only for BranchFilter::Named (a concrete branch name).
        // Match by project_id, git_root, or project_path so sidebar clicks find
        // sessions regardless of which key was used as effective_id.
        let mut conditions = vec!["(s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root != '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path != '' AND s.project_path = ?1))".to_string()];
        // Always exclude archived sessions from user-facing lists
        conditions.push("s.archived_at IS NULL".to_string());
        if !include_sidechains {
            conditions.push("s.is_sidechain = 0".to_string());
        }
        match branch_filter {
            BranchFilter::All => { /* no condition */ }
            BranchFilter::NoBranch => {
                conditions.push("s.git_branch IS NULL".to_string());
            }
            BranchFilter::Named(_) => {
                conditions.push("s.git_branch = ?4".to_string());
            }
        }

        let where_clause = conditions.join(" AND ");

        let order_clause = match sort {
            "oldest" => "s.last_message_at ASC",
            "messages" => "s.message_count DESC",
            _ => "s.last_message_at DESC", // "recent" is default
        };

        // Count total matching sessions
        let count_sql = format!("SELECT COUNT(*) FROM sessions s WHERE {}", where_clause);
        let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql).bind(project_id);
        if let BranchFilter::Named(name) = branch_filter {
            count_query = count_query.bind(*name);
        }
        let (total,) = count_query.fetch_one(self.pool()).await?;

        // All token/model data is denormalized on the sessions table.
        // No LEFT JOIN on turns needed.
        let select_sql = format!(
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
            FROM sessions s
            WHERE {}
            ORDER BY {}
            LIMIT ?2 OFFSET ?3
            "#,
            where_clause, order_clause
        );

        let mut query = sqlx::query_as::<_, SessionRow>(&select_sql)
            .bind(project_id)
            .bind(limit)
            .bind(offset);
        if let BranchFilter::Named(name) = branch_filter {
            query = query.bind(*name);
        }

        let rows: Vec<SessionRow> = query.fetch_all(self.pool()).await?;

        let sessions: Vec<SessionInfo> = rows
            .into_iter()
            .map(|r| {
                let pid = project_id.to_string();
                r.into_session_info(&pid)
            })
            .collect();

        Ok(SessionsPage {
            sessions,
            total: total as usize,
        })
    }

    /// List distinct branches with session counts for a project identity.
    ///
    /// Returns branches sorted by session count DESC.
    /// Includes sessions with `git_branch = NULL` as a separate entry.
    ///
    /// `project_identity` may be either:
    /// - `project_id` (legacy per-worktree identity), or
    /// - `git_root` (effective sidebar identity for merged worktrees).
    pub async fn list_branches_for_project(
        &self,
        project_identity: &str,
    ) -> DbResult<Vec<BranchCount>> {
        let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
            r#"
            SELECT NULLIF(git_branch, '') as branch, COUNT(*) as count
            FROM valid_sessions
            WHERE (
                project_id = ?1
                OR (git_root IS NOT NULL AND git_root != '' AND git_root = ?1)
                OR (project_path IS NOT NULL AND project_path != '' AND project_path = ?1)
            )
            GROUP BY NULLIF(git_branch, '')
            ORDER BY count DESC
            "#,
        )
        .bind(project_identity)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(branch, count)| BranchCount { branch, count })
            .collect())
    }
}

/// Extract "parent/leaf" from a path for disambiguation.
/// e.g. "/Users/dev/@acme/fashion-ai/pod-ai" → Some("fashion-ai/pod-ai")
///      "/Users/dev/@acme/pod-ai"             → Some("@acme/pod-ai")
///      "/"                                        → None
fn parent_qualified_name(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    let last_sep = trimmed.rfind('/')?;
    if last_sep == 0 && trimmed.len() == 1 {
        // path was "/"
        return None;
    }
    let leaf = &trimmed[last_sep + 1..];
    if leaf.is_empty() {
        return None;
    }
    let parent_slice = &trimmed[..last_sep];
    let parent_name = match parent_slice.rfind('/') {
        Some(pos) => &parent_slice[pos + 1..],
        None => parent_slice,
    };
    Some(format!("{}/{}", parent_name, leaf))
}

/// When multiple projects share the same `display_name`, qualify them with
/// their parent directory so the sidebar shows e.g. "fashion-ai/pod-ai" vs
/// "@acme/pod-ai" instead of two identical "pod-ai" entries.
fn disambiguate_display_names(summaries: &mut [ProjectSummary]) {
    use std::collections::HashMap;

    // Count occurrences of each display_name.
    let mut counts: HashMap<String, usize> = HashMap::new();
    for s in summaries.iter() {
        *counts.entry(s.display_name.clone()).or_default() += 1;
    }

    // For duplicates, replace with parent-qualified name.
    for s in summaries.iter_mut() {
        if counts.get(&s.display_name).copied().unwrap_or(0) > 1 {
            if let Some(qualified) = parent_qualified_name(&s.path) {
                s.display_name = qualified;
            }
        }
    }
}

#[cfg(test)]
mod disambiguate_tests {
    use super::*;
    use claude_view_core::ProjectSummary;

    fn make_summary(display_name: &str, path: &str) -> ProjectSummary {
        ProjectSummary {
            name: path.to_string(),
            display_name: display_name.to_string(),
            path: path.to_string(),
            session_count: 1,
            active_count: 0,
            last_activity_at: None,
        }
    }

    // --- parent_qualified_name ---

    #[test]
    fn parent_qualified_scoped_deep_path() {
        // Two parent segments visible: "fashion-ai/pod-ai"
        assert_eq!(
            parent_qualified_name("/Users/dev/@acme/fashion-ai/pod-ai"),
            Some("fashion-ai/pod-ai".to_string()),
        );
    }

    #[test]
    fn parent_qualified_scoped_namespace() {
        // Parent is a scoped namespace: "@acme/pod-ai"
        assert_eq!(
            parent_qualified_name("/Users/dev/@acme/pod-ai"),
            Some("@acme/pod-ai".to_string()),
        );
    }

    #[test]
    fn parent_qualified_root_returns_none() {
        // Root path has no parent directory
        assert_eq!(parent_qualified_name("/"), None);
    }

    #[test]
    fn parent_qualified_single_segment() {
        // Only one segment after root — parent is empty string from split
        assert_eq!(
            parent_qualified_name("/pod-ai"),
            Some("/pod-ai".to_string()),
        );
    }

    // --- disambiguate_display_names ---

    #[test]
    fn disambiguate_duplicate_names() {
        let mut summaries = vec![
            make_summary("pod-ai", "/Users/dev/@acme/fashion-ai/pod-ai"),
            make_summary("pod-ai", "/Users/dev/@acme/pod-ai"),
        ];
        disambiguate_display_names(&mut summaries);

        assert_eq!(summaries[0].display_name, "fashion-ai/pod-ai");
        assert_eq!(summaries[1].display_name, "@acme/pod-ai");
    }

    #[test]
    fn disambiguate_unique_names_unchanged() {
        let mut summaries = vec![
            make_summary("frontend", "/Users/dev/frontend"),
            make_summary("backend", "/Users/dev/backend"),
        ];
        disambiguate_display_names(&mut summaries);

        assert_eq!(summaries[0].display_name, "frontend");
        assert_eq!(summaries[1].display_name, "backend");
    }

    #[test]
    fn disambiguate_mixed_dup_and_unique() {
        let mut summaries = vec![
            make_summary("api", "/Users/dev/alpha/api"),
            make_summary("api", "/Users/dev/beta/api"),
            make_summary("web", "/Users/dev/web"),
        ];
        disambiguate_display_names(&mut summaries);

        // Duplicates get qualified
        assert_eq!(summaries[0].display_name, "alpha/api");
        assert_eq!(summaries[1].display_name, "beta/api");
        // Unique stays unchanged
        assert_eq!(summaries[2].display_name, "web");
    }
}
