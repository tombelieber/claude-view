// crates/db/src/queries/dashboard/stats.rs
// Dashboard statistics, all-time metrics, and invocable top-10 queries.

use crate::{Database, DbResult};
use claude_view_core::{
    DashboardStats, DayActivity, ProjectStat, SessionDurationStat, SkillStat, ToolCounts,
};

impl Database {
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
              AND (?1 IS NULL OR s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1))
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
              AND (?3 IS NULL OR s.project_id = ?3 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
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
            WHERE (?1 IS NULL OR project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1)) AND (?2 IS NULL OR git_branch = ?2)
            "#,
        )
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;

        // Heatmap: all-time activity (sessions per day) — respects project/branch
        // filters but not time range (all-time view has no time bounds).
        let heatmap_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT date(COALESCE(first_message_at, last_message_at), 'unixepoch', 'localtime') as day, COUNT(*) as cnt
            FROM valid_sessions
            WHERE last_message_at > 0
              AND (?1 IS NULL OR project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1)) AND (?2 IS NULL OR git_branch = ?2)
            GROUP BY day
            ORDER BY day ASC
            "#,
        )
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
            WHERE (?1 IS NULL OR project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1)) AND (?2 IS NULL OR git_branch = ?2)
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

        // Top 5 sessions by longest task time
        let longest_rows: Vec<(String, String, String, String, i32)> = sqlx::query_as(
            r#"
            SELECT id, COALESCE(NULLIF(longest_task_preview, ''), preview), project_id, COALESCE(project_display_name, project_id), longest_task_seconds
            FROM valid_sessions
                        WHERE longest_task_seconds > 0
              AND (?1 IS NULL OR project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1)) AND (?2 IS NULL OR git_branch = ?2)
            ORDER BY longest_task_seconds DESC
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
              AND (?3 IS NULL OR project_id = ?3 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3)) AND (?4 IS NULL OR git_branch = ?4)
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(project)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;

        // Heatmap: respects the caller's time range (from..to)
        let heatmap_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT date(COALESCE(first_message_at, last_message_at), 'unixepoch', 'localtime') as day, COUNT(*) as cnt
            FROM valid_sessions
            WHERE last_message_at >= ?1 AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3)) AND (?4 IS NULL OR git_branch = ?4)
            GROUP BY day
            ORDER BY day ASC
            "#,
        )
        .bind(from)
        .bind(to)
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
              AND (?3 IS NULL OR project_id = ?3 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3)) AND (?4 IS NULL OR git_branch = ?4)
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

        // Top 5 sessions by longest task time (filtered)
        let longest_rows: Vec<(String, String, String, String, i32)> = sqlx::query_as(
            r#"
            SELECT id, COALESCE(NULLIF(longest_task_preview, ''), preview), project_id, COALESCE(project_display_name, project_id), longest_task_seconds
            FROM valid_sessions
            WHERE longest_task_seconds > 0 AND last_message_at >= ?1 AND last_message_at <= ?2
              AND (?3 IS NULL OR project_id = ?3 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3)) AND (?4 IS NULL OR git_branch = ?4)
            ORDER BY longest_task_seconds DESC
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
                     WHERE (?1 IS NULL OR project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1)) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COALESCE(SUM(COALESCE(total_input_tokens, 0) + COALESCE(total_output_tokens, 0)), 0)
                     FROM valid_sessions
                     WHERE (?1 IS NULL OR project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1)) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COALESCE(SUM(files_edited_count), 0) FROM valid_sessions
                     WHERE (?1 IS NULL OR project_id = ?1 OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?1) OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?1)) AND (?2 IS NULL OR git_branch = ?2)),
                  (SELECT COUNT(DISTINCT sc.commit_hash) FROM session_commits sc INNER JOIN valid_sessions s ON sc.session_id = s.id
                     WHERE (?1 IS NULL OR s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1) OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1)) AND (?2 IS NULL OR s.git_branch = ?2))
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
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM valid_sessions")
            .fetch_one(self.pool())
            .await?;
        Ok(count)
    }

    /// Get the total count of projects.
    pub async fn get_project_count(&self) -> DbResult<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(DISTINCT project_id) FROM valid_sessions")
                .fetch_one(self.pool())
                .await?;
        Ok(count)
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
