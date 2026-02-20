// crates/db/src/queries/dashboard.rs
// Dashboard statistics, project summaries, and paginated session queries.

use super::row_types::SessionRow;
use super::BranchCount;
use crate::{Database, DbResult};
use chrono::Utc;
use claude_view_core::{
    BranchFilter, DashboardStats, DayActivity, ProjectStat, ProjectSummary, SessionDurationStat,
    SessionInfo, SessionsPage, SkillStat, ToolCounts,
};

/// Parameters for filtered, paginated session queries.
/// All fields are optional — omitted fields apply no filter.
pub struct SessionFilterParams {
    pub q: Option<String>,
    pub branches: Option<Vec<String>>,
    pub models: Option<Vec<String>>,
    pub has_commits: Option<bool>,
    pub has_skills: Option<bool>,
    pub min_duration: Option<i64>,
    pub min_files: Option<i64>,
    pub min_tokens: Option<i64>,
    pub high_reedit: Option<bool>,
    pub time_after: Option<i64>,
    pub time_before: Option<i64>,
    pub sort: String, // "recent", "tokens", "prompts", "files_edited", "duration"
    pub limit: i64,   // default 30
    pub offset: i64,  // default 0
}

/// A single point in the activity histogram.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActivityPoint {
    pub date: String,
    pub count: i64,
}

impl Database {
    /// List lightweight project summaries (no sessions array).
    /// Returns ProjectSummary with counts only — sidebar payload.
    pub async fn list_project_summaries(&self) -> DbResult<Vec<ProjectSummary>> {
        let now = Utc::now().timestamp();
        let active_threshold = now - 300; // 5 minutes

        let rows: Vec<(String, String, String, i64, i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT
                project_id,
                COALESCE(project_display_name, project_id),
                COALESCE(project_path, ''),
                COUNT(*) as session_count,
                SUM(CASE WHEN last_message_at > ?1 THEN 1 ELSE 0 END) as active_count,
                MAX(CASE WHEN last_message_at > 0 THEN last_message_at ELSE NULL END) as last_activity_at
            FROM valid_sessions
            GROUP BY project_id
            ORDER BY last_activity_at DESC
            "#,
        )
        .bind(active_threshold)
        .fetch_all(self.pool())
        .await?;

        let summaries = rows
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
        let mut conditions = vec!["s.project_id = ?1".to_string()];
        conditions.push("s.last_message_at > 0".to_string());
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
                s.project_path, s.project_display_name,
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
                s.prompt_word_count, s.correction_count, s.same_file_edit_count
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
                s.project_path, s.project_display_name,
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
                s.prompt_word_count, s.correction_count, s.same_file_edit_count
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
            s.project_path, s.project_display_name,
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
            s.total_task_time_seconds, s.longest_task_seconds, s.longest_task_preview
        "#;

        // Helper closure: appends all WHERE clauses to a QueryBuilder.
        // Called twice — once for COUNT(*), once for SELECT.
        fn append_filters<'args>(
            qb: &mut sqlx::QueryBuilder<'args, sqlx::Sqlite>,
            params: &'args SessionFilterParams,
        ) {
            qb.push(" WHERE 1=1");

            // Text search
            if let Some(q) = &params.q {
                let pattern = format!("%{}%", q);
                qb.push(" AND (s.preview LIKE ");
                qb.push_bind(pattern.clone());
                qb.push(" OR s.last_message LIKE ");
                qb.push_bind(pattern.clone());
                qb.push(" OR s.project_display_name LIKE ");
                qb.push_bind(pattern);
                qb.push(")");
            }

            // Branch filter (IN list)
            if let Some(branches) = &params.branches {
                if !branches.is_empty() {
                    qb.push(" AND s.git_branch IN (");
                    let mut sep = qb.separated(", ");
                    for b in branches {
                        sep.push_bind(b.as_str());
                    }
                    sep.push_unseparated(")");
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
                qb.push(" AND (COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0)) >= ");
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
        }

        // --- COUNT query ---
        let mut count_qb = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM valid_sessions s");
        append_filters(&mut count_qb, params);

        let total: (i64,) = count_qb.build_query_as().fetch_one(self.pool()).await?;
        let total = total.0 as usize;

        // --- DATA query ---
        let mut data_qb =
            sqlx::QueryBuilder::new(format!("SELECT {} FROM valid_sessions s", select_cols));
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

    /// List distinct branches with session counts for a project.
    ///
    /// Returns branches sorted by session count DESC.
    /// Includes sessions with `git_branch = NULL` as a separate entry.
    pub async fn list_branches_for_project(&self, project_id: &str) -> DbResult<Vec<BranchCount>> {
        let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
            r#"
            SELECT NULLIF(git_branch, '') as branch, COUNT(*) as count
            FROM valid_sessions
            WHERE project_id = ?1
            GROUP BY NULLIF(git_branch, '')
            ORDER BY count DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(branch, count)| BranchCount { branch, count })
            .collect())
    }

    /// Fetch top 10 invocables for all 4 kinds in a single query (no time range).
    /// Returns (skills, commands, mcp_tools, agents) — each Vec has at most 10 entries.
    async fn all_top_invocables_by_kind(
        &self,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<(
        Vec<SkillStat>,
        Vec<SkillStat>,
        Vec<SkillStat>,
        Vec<SkillStat>,
    )> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT inv.kind, inv.name, COUNT(*) as cnt
            FROM invocations i
            JOIN invocables inv ON i.invocable_id = inv.id
            INNER JOIN valid_sessions s ON i.session_id = s.id
            WHERE inv.kind IN ('skill', 'command', 'mcp_tool', 'agent')
              AND (?1 IS NULL OR s.project_id = ?1)
              AND (?2 IS NULL OR s.git_branch = ?2)
            GROUP BY inv.kind, inv.name
            ORDER BY inv.kind, cnt DESC
            "#,
        )
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        partition_invocables_by_kind(rows)
    }

    /// Fetch top 10 invocables for all 4 kinds in a single query (with time range).
    /// Returns (skills, commands, mcp_tools, agents) — each Vec has at most 10 entries.
    async fn all_top_invocables_by_kind_with_range(
        &self,
        from: i64,
        to: i64,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<(
        Vec<SkillStat>,
        Vec<SkillStat>,
        Vec<SkillStat>,
        Vec<SkillStat>,
    )> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT inv.kind, inv.name, COUNT(*) as cnt
            FROM invocations i
            JOIN invocables inv ON i.invocable_id = inv.id
            INNER JOIN valid_sessions s ON i.session_id = s.id
            WHERE inv.kind IN ('skill', 'command', 'mcp_tool', 'agent')
              AND s.last_message_at >= ?1 AND s.last_message_at <= ?2
              AND (?3 IS NULL OR s.project_id = ?3)
              AND (?4 IS NULL OR s.git_branch = ?4)
            GROUP BY inv.kind, inv.name
            ORDER BY inv.kind, cnt DESC
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        partition_invocables_by_kind(rows)
    }

    /// Get pre-computed dashboard statistics.
    ///
    /// Returns heatmap (90 days), top 10 invocables per kind, top 5 projects, tool totals.
    /// Optimized: counts+tools merged (3→1), invocables merged (4→1) = 5 queries total.
    pub async fn get_dashboard_stats(
        &self,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<DashboardStats> {
        // Merged query: session count + project count + tool totals (replaces 3 queries)
        let (total_sessions, total_projects, edit, read, bash, write): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = sqlx::query_as(
            r#"
            SELECT
              COUNT(*),
              COUNT(DISTINCT project_id),
              COALESCE(SUM(tool_counts_edit), 0),
              COALESCE(SUM(tool_counts_read), 0),
              COALESCE(SUM(tool_counts_bash), 0),
              COALESCE(SUM(tool_counts_write), 0)
            FROM valid_sessions
            WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
            "#,
        )
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;

        // Heatmap: 90-day activity (sessions per day)
        let now = Utc::now().timestamp();
        let ninety_days_ago = now - (90 * 86400);
        let heatmap_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT date(last_message_at, 'unixepoch', 'localtime') as day, COUNT(*) as cnt
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND (?2 IS NULL OR project_id = ?2) AND (?3 IS NULL OR git_branch = ?3)
            GROUP BY day
            ORDER BY day ASC
            "#,
        )
        .bind(ninety_days_ago)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let heatmap: Vec<DayActivity> = heatmap_rows
            .into_iter()
            .map(|(date, count)| DayActivity {
                date,
                count: count as usize,
            })
            .collect();

        // Merged invocables query: all 4 kinds in one scan (replaces 4 queries)
        let (top_skills, top_commands, top_mcp_tools, top_agents) =
            self.all_top_invocables_by_kind(project, branch).await?;

        // Top 5 projects by session count
        let project_rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT project_id, COALESCE(project_display_name, project_id), COUNT(*) as cnt
            FROM valid_sessions
            WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
            GROUP BY project_id
            ORDER BY cnt DESC
            LIMIT 5
            "#,
        )
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let top_projects: Vec<ProjectStat> = project_rows
            .into_iter()
            .map(|(name, display_name, session_count)| ProjectStat {
                name,
                display_name,
                session_count: session_count as usize,
            })
            .collect();

        // Top 5 longest sessions by duration
        let longest_rows: Vec<(String, String, String, String, i32)> = sqlx::query_as(
            r#"
            SELECT id, preview, project_id, COALESCE(project_display_name, project_id), longest_task_seconds
            FROM valid_sessions
                        WHERE longest_task_seconds > 0
              AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
            ORDER BY duration_seconds DESC
            LIMIT 5
            "#,
        )
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let longest_sessions: Vec<SessionDurationStat> = longest_rows
            .into_iter()
            .map(
                |(id, preview, project_name, project_display_name, duration_seconds)| {
                    SessionDurationStat {
                        id,
                        preview,
                        project_name,
                        project_display_name,
                        duration_seconds: duration_seconds as u32,
                    }
                },
            )
            .collect();

        Ok(DashboardStats {
            total_sessions: total_sessions as usize,
            total_projects: total_projects as usize,
            heatmap,
            top_skills,
            top_commands,
            top_mcp_tools,
            top_agents,
            top_projects,
            tool_totals: ToolCounts {
                edit: edit as usize,
                read: read as usize,
                bash: bash as usize,
                write: write as usize,
            },
            longest_sessions,
        })
    }

    /// Get dashboard statistics filtered by a time range.
    ///
    /// Stats are filtered to sessions with `last_message_at` within [from, to].
    /// Heatmap always shows the last 90 days regardless of the filter.
    /// Optimized: counts+tools merged (3→1), invocables merged (4→1) = 5 queries total.
    pub async fn get_dashboard_stats_with_range(
        &self,
        from: Option<i64>,
        to: Option<i64>,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<DashboardStats> {
        let from = from.unwrap_or(1);
        let to = to.unwrap_or(i64::MAX);

        // Merged query: session count + project count + tool totals (replaces 3 queries)
        let (total_sessions, total_projects, edit, read, bash, write): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = sqlx::query_as(
            r#"
            SELECT
              COUNT(*),
              COUNT(DISTINCT project_id),
              COALESCE(SUM(tool_counts_edit), 0),
              COALESCE(SUM(tool_counts_read), 0),
              COALESCE(SUM(tool_counts_bash), 0),
              COALESCE(SUM(tool_counts_write), 0)
            FROM valid_sessions
            WHERE last_message_at >= ?1 AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3) AND (?4 IS NULL OR git_branch = ?4)
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;

        // Heatmap: always 90 days (not affected by time range filter)
        let now = Utc::now().timestamp();
        let ninety_days_ago = now - (90 * 86400);
        let heatmap_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT date(last_message_at, 'unixepoch', 'localtime') as day, COUNT(*) as cnt
            FROM valid_sessions
            WHERE last_message_at >= ?1
              AND (?2 IS NULL OR project_id = ?2) AND (?3 IS NULL OR git_branch = ?3)
            GROUP BY day
            ORDER BY day ASC
            "#,
        )
        .bind(ninety_days_ago)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let heatmap: Vec<DayActivity> = heatmap_rows
            .into_iter()
            .map(|(date, count)| DayActivity {
                date,
                count: count as usize,
            })
            .collect();

        // Merged invocables query with time range: all 4 kinds in one scan (replaces 4 queries)
        let (top_skills, top_commands, top_mcp_tools, top_agents) = self
            .all_top_invocables_by_kind_with_range(from, to, project, branch)
            .await?;

        // Top 5 projects by session count (filtered)
        let project_rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT project_id, COALESCE(project_display_name, project_id), COUNT(*) as cnt
            FROM valid_sessions
            WHERE last_message_at >= ?1 AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3) AND (?4 IS NULL OR git_branch = ?4)
            GROUP BY project_id
            ORDER BY cnt DESC
            LIMIT 5
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let top_projects: Vec<ProjectStat> = project_rows
            .into_iter()
            .map(|(name, display_name, session_count)| ProjectStat {
                name,
                display_name,
                session_count: session_count as usize,
            })
            .collect();

        // Top 5 longest sessions by duration (filtered)
        let longest_rows: Vec<(String, String, String, String, i32)> = sqlx::query_as(
            r#"
            SELECT id, preview, project_id, COALESCE(project_display_name, project_id), longest_task_seconds
            FROM valid_sessions
            WHERE longest_task_seconds > 0 AND last_message_at >= ?1 AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3) AND (?4 IS NULL OR git_branch = ?4)
            ORDER BY duration_seconds DESC
            LIMIT 5
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_all(self.pool())
        .await?;

        let longest_sessions: Vec<SessionDurationStat> = longest_rows
            .into_iter()
            .map(
                |(id, preview, project_name, project_display_name, duration_seconds)| {
                    SessionDurationStat {
                        id,
                        preview,
                        project_name,
                        project_display_name,
                        duration_seconds: duration_seconds as u32,
                    }
                },
            )
            .collect();

        Ok(DashboardStats {
            total_sessions: total_sessions as usize,
            total_projects: total_projects as usize,
            heatmap,
            top_skills,
            top_commands,
            top_mcp_tools,
            top_agents,
            top_projects,
            tool_totals: ToolCounts {
                edit: edit as usize,
                read: read as usize,
                bash: bash as usize,
                write: write as usize,
            },
            longest_sessions,
        })
    }

    /// Get all-time aggregate metrics for the dashboard.
    ///
    /// Returns (session_count, total_tokens, total_files_edited, commit_count).
    /// Optimized: 4 queries → 1 via scalar subqueries in a single round-trip.
    pub async fn get_all_time_metrics(
        &self,
        project: Option<&str>,
        branch: Option<&str>,
    ) -> DbResult<(u64, u64, u64, u64)> {
        let (session_count, total_tokens, total_files_edited, commit_count): (i64, i64, i64, i64) =
            sqlx::query_as(
                r#"
                SELECT
                  (SELECT COUNT(*) FROM valid_sessions
                     WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COALESCE(SUM(COALESCE(total_input_tokens, 0) + COALESCE(total_output_tokens, 0)), 0)
                     FROM valid_sessions
                     WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COALESCE(SUM(files_edited_count), 0) FROM valid_sessions
                     WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COUNT(DISTINCT sc.commit_hash) FROM session_commits sc INNER JOIN valid_sessions s ON sc.session_id = s.id
                     WHERE (?1 IS NULL OR s.project_id = ?1) AND (?2 IS NULL OR s.git_branch = ?2))
                "#,
            )
            .bind(project)
            .bind(branch)
            .fetch_one(self.pool())
            .await?;

        Ok((
            session_count as u64,
            total_tokens as u64,
            total_files_edited as u64,
            commit_count as u64,
        ))
    }

    /// Get the total count of sessions (excluding sidechains).
    pub async fn get_session_count(&self) -> DbResult<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM valid_sessions")
                .fetch_one(self.pool())
                .await?;
        Ok(count)
    }

    /// Get the total count of projects.
    pub async fn get_project_count(&self) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT project_id) FROM valid_sessions",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(count)
    }

    /// Activity histogram for sparkline chart.
    /// Auto-buckets by day/week/month based on data span.
    /// Returns (Vec<ActivityPoint>, bucket_name).
    pub async fn session_activity_histogram(&self) -> DbResult<(Vec<ActivityPoint>, String)> {
        // 1. Determine span
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(MIN(last_message_at), 0), COALESCE(MAX(last_message_at), 0) \
             FROM valid_sessions",
        )
        .fetch_one(self.pool())
        .await?;

        let span_days = (row.1 - row.0) / 86400;
        let (group_expr, bucket) = if span_days > 365 {
            ("strftime('%Y-%m', last_message_at, 'unixepoch')", "month")
        } else if span_days > 60 {
            ("strftime('%Y-W%W', last_message_at, 'unixepoch')", "week")
        } else {
            ("DATE(last_message_at, 'unixepoch')", "day")
        };

        // 2. Run grouped count
        let sql = format!(
            "SELECT {group_expr} AS date, COUNT(*) AS count \
             FROM valid_sessions \
             GROUP BY date ORDER BY date"
        );

        let raw_rows: Vec<(String, i64)> = sqlx::query_as(&sql).fetch_all(self.pool()).await?;

        let rows: Vec<ActivityPoint> = raw_rows
            .into_iter()
            .map(|(date, count)| ActivityPoint { date, count })
            .collect();

        Ok((rows, bucket.to_string()))
    }
}

/// Partition (kind, name, count) rows into per-kind top-10 vectors.
fn partition_invocables_by_kind(
    rows: Vec<(String, String, i64)>,
) -> DbResult<(
    Vec<SkillStat>,
    Vec<SkillStat>,
    Vec<SkillStat>,
    Vec<SkillStat>,
)> {
    let mut skills = Vec::new();
    let mut commands = Vec::new();
    let mut mcp_tools = Vec::new();
    let mut agents = Vec::new();

    for (kind, name, count) in rows {
        let stat = SkillStat {
            name,
            count: count as usize,
        };
        let target = match kind.as_str() {
            "skill" => &mut skills,
            "command" => &mut commands,
            "mcp_tool" => &mut mcp_tools,
            "agent" => &mut agents,
            _ => continue,
        };
        if target.len() < 10 {
            target.push(stat);
        }
    }

    Ok((skills, commands, mcp_tools, agents))
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
        }
    }

    fn default_params() -> SessionFilterParams {
        SessionFilterParams {
            q: None,
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
