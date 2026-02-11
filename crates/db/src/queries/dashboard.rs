// crates/db/src/queries/dashboard.rs
// Dashboard statistics, project summaries, and paginated session queries.

use crate::{Database, DbResult};
use chrono::Utc;
use vibe_recall_core::{
    BranchFilter, DashboardStats, DayActivity, ProjectStat, ProjectSummary,
    SessionDurationStat, SessionInfo, SessionsPage, SkillStat, ToolCounts,
};
use super::row_types::SessionRow;
use super::BranchCount;

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
            FROM sessions
            WHERE is_sidechain = 0
            GROUP BY project_id
            ORDER BY last_activity_at DESC
            "#,
        )
        .bind(active_threshold)
        .fetch_all(self.pool())
        .await?;

        let summaries = rows
            .into_iter()
            .map(|(name, display_name, path, session_count, active_count, last_activity_at)| {
                ProjectSummary {
                    name,
                    display_name,
                    path,
                    session_count: session_count as usize,
                    active_count: active_count as usize,
                    last_activity_at,
                }
            })
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
        let count_sql = format!(
            "SELECT COUNT(*) FROM sessions s WHERE {}",
            where_clause
        );
        let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql)
            .bind(project_id);
        if let BranchFilter::Named(name) = branch_filter {
            count_query = count_query.bind(*name);
        }
        let (total,) = count_query.fetch_one(self.pool()).await?;

        // Fetch paginated sessions with token LEFT JOIN
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
                tok.total_input_tokens,
                tok.total_output_tokens,
                tok.total_cache_read_tokens,
                tok.total_cache_creation_tokens,
                tok.turn_count_api,
                tok.primary_model,
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
            LEFT JOIN (
                SELECT session_id,
                       SUM(input_tokens) as total_input_tokens,
                       SUM(output_tokens) as total_output_tokens,
                       SUM(cache_read_tokens) as total_cache_read_tokens,
                       SUM(cache_creation_tokens) as total_cache_creation_tokens,
                       COUNT(*) as turn_count_api,
                       (SELECT model_id FROM turns t2
                        WHERE t2.session_id = t.session_id
                        GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 1
                       ) as primary_model
                FROM turns t
                GROUP BY session_id
            ) tok ON tok.session_id = s.id
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

    /// List distinct branches with session counts for a project.
    ///
    /// Returns branches sorted by session count DESC.
    /// Includes sessions with `git_branch = NULL` as a separate entry.
    pub async fn list_branches_for_project(
        &self,
        project_id: &str,
    ) -> DbResult<Vec<BranchCount>> {
        let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
            r#"
            SELECT NULLIF(git_branch, '') as branch, COUNT(*) as count
            FROM sessions
            WHERE project_id = ?1 AND is_sidechain = 0
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
    ) -> DbResult<(Vec<SkillStat>, Vec<SkillStat>, Vec<SkillStat>, Vec<SkillStat>)> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT inv.kind, inv.name, COUNT(*) as cnt
            FROM invocations i
            JOIN invocables inv ON i.invocable_id = inv.id
            INNER JOIN sessions s ON i.session_id = s.id
            WHERE inv.kind IN ('skill', 'command', 'mcp_tool', 'agent')
              AND s.is_sidechain = 0
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
    ) -> DbResult<(Vec<SkillStat>, Vec<SkillStat>, Vec<SkillStat>, Vec<SkillStat>)> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT inv.kind, inv.name, COUNT(*) as cnt
            FROM invocations i
            JOIN invocables inv ON i.invocable_id = inv.id
            INNER JOIN sessions s ON i.session_id = s.id
            WHERE inv.kind IN ('skill', 'command', 'mcp_tool', 'agent')
              AND s.is_sidechain = 0
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
    pub async fn get_dashboard_stats(&self, project: Option<&str>, branch: Option<&str>) -> DbResult<DashboardStats> {
        // Merged query: session count + project count + tool totals (replaces 3 queries)
        let (total_sessions, total_projects, edit, read, bash, write): (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COUNT(*),
              COUNT(DISTINCT project_id),
              COALESCE(SUM(tool_counts_edit), 0),
              COALESCE(SUM(tool_counts_read), 0),
              COALESCE(SUM(tool_counts_bash), 0),
              COALESCE(SUM(tool_counts_write), 0)
            FROM sessions
            WHERE is_sidechain = 0
              AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
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
            FROM sessions
            WHERE last_message_at >= ?1 AND is_sidechain = 0
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
            FROM sessions
            WHERE is_sidechain = 0
              AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
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
            SELECT id, preview, project_id, COALESCE(project_display_name, project_id), duration_seconds
            FROM sessions
            WHERE is_sidechain = 0 AND duration_seconds > 0
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
            .map(|(id, preview, project_name, project_display_name, duration_seconds)| {
                SessionDurationStat {
                    id,
                    preview,
                    project_name,
                    project_display_name,
                    duration_seconds: duration_seconds as u32,
                }
            })
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
        let (total_sessions, total_projects, edit, read, bash, write): (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COUNT(*),
              COUNT(DISTINCT project_id),
              COALESCE(SUM(tool_counts_edit), 0),
              COALESCE(SUM(tool_counts_read), 0),
              COALESCE(SUM(tool_counts_bash), 0),
              COALESCE(SUM(tool_counts_write), 0)
            FROM sessions
            WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2
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
            FROM sessions
            WHERE last_message_at >= ?1 AND is_sidechain = 0
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
        let (top_skills, top_commands, top_mcp_tools, top_agents) =
            self.all_top_invocables_by_kind_with_range(from, to, project, branch).await?;

        // Top 5 projects by session count (filtered)
        let project_rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT project_id, COALESCE(project_display_name, project_id), COUNT(*) as cnt
            FROM sessions
            WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2
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
            SELECT id, preview, project_id, COALESCE(project_display_name, project_id), duration_seconds
            FROM sessions
            WHERE is_sidechain = 0 AND duration_seconds > 0 AND last_message_at >= ?1 AND last_message_at <= ?2
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
            .map(|(id, preview, project_name, project_display_name, duration_seconds)| {
                SessionDurationStat {
                    id,
                    preview,
                    project_name,
                    project_display_name,
                    duration_seconds: duration_seconds as u32,
                }
            })
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
    pub async fn get_all_time_metrics(&self, project: Option<&str>, branch: Option<&str>) -> DbResult<(u64, u64, u64, u64)> {
        let (session_count, total_tokens, total_files_edited, commit_count): (i64, i64, i64, i64) =
            sqlx::query_as(
                r#"
                SELECT
                  (SELECT COUNT(*) FROM sessions
                     WHERE is_sidechain = 0
                     AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COALESCE(SUM(COALESCE(t.input_tokens, 0) + COALESCE(t.output_tokens, 0)), 0)
                     FROM turns t INNER JOIN sessions s ON t.session_id = s.id
                     WHERE s.is_sidechain = 0
                     AND (?1 IS NULL OR s.project_id = ?1) AND (?2 IS NULL OR s.git_branch = ?2)),
                  (SELECT COALESCE(SUM(files_edited_count), 0) FROM sessions
                     WHERE is_sidechain = 0
                     AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COUNT(*) FROM session_commits sc INNER JOIN sessions s ON sc.session_id = s.id
                     WHERE s.is_sidechain = 0
                     AND (?1 IS NULL OR s.project_id = ?1) AND (?2 IS NULL OR s.git_branch = ?2))
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
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0")
                .fetch_one(self.pool())
                .await?;
        Ok(count)
    }

    /// Get the total count of projects.
    pub async fn get_project_count(&self) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT project_id) FROM sessions WHERE is_sidechain = 0",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(count)
    }

    /// Get the total count of linked commits.
    pub async fn get_commit_count(&self) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_commits")
            .fetch_one(self.pool())
            .await?;
        Ok(count)
    }
}

/// Partition (kind, name, count) rows into per-kind top-10 vectors.
fn partition_invocables_by_kind(
    rows: Vec<(String, String, i64)>,
) -> DbResult<(Vec<SkillStat>, Vec<SkillStat>, Vec<SkillStat>, Vec<SkillStat>)> {
    let mut skills = Vec::new();
    let mut commands = Vec::new();
    let mut mcp_tools = Vec::new();
    let mut agents = Vec::new();

    for (kind, name, count) in rows {
        let stat = SkillStat { name, count: count as usize };
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
