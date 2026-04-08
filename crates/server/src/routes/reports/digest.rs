// crates/server/src/routes/reports/digest.rs
//! Context digest builder for report generation.

use std::collections::HashMap;

use crate::error::ApiError;
use crate::state::AppState;
use claude_view_core::report::{BranchDigest, ContextDigest, ProjectDigest, SessionDigest};

/// Build a context digest from DB data for the given date range.
pub(super) async fn build_context_digest(
    state: &AppState,
    report_type: &str,
    date_start: &str,
    date_end: &str,
    start_ts: i64,
    end_ts: i64,
) -> Result<ContextDigest, ApiError> {
    // Query sessions in range via Database method
    let sessions = state
        .db
        .get_sessions_in_range(start_ts, end_ts)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to query sessions: {e}")))?;

    if sessions.is_empty() {
        return Ok(ContextDigest::default());
    }

    // Group by project -> branch
    let mut project_map: HashMap<String, HashMap<String, Vec<SessionDigest>>> = HashMap::new();
    let mut project_durations: HashMap<String, i64> = HashMap::new();
    let mut project_session_counts: HashMap<String, usize> = HashMap::new();

    for (_, project, preview, category, duration, branch) in &sessions {
        let branch_name = branch.as_deref().unwrap_or("(no branch)").to_string();

        project_map
            .entry(project.clone())
            .or_default()
            .entry(branch_name)
            .or_default()
            .push(SessionDigest {
                first_prompt: preview.clone(),
                category: category.clone(),
                duration_secs: *duration,
            });

        *project_durations.entry(project.clone()).or_default() += duration;
        *project_session_counts.entry(project.clone()).or_default() += 1;
    }

    // Query commit counts per project via Database method
    let commit_rows = state
        .db
        .get_commit_counts_in_range(start_ts, end_ts)
        .await
        .unwrap_or_default();
    let commit_counts: HashMap<String, i64> = commit_rows.into_iter().collect();

    // Query top tools and skills via Database methods
    let top_tools = state
        .db
        .get_top_tools_in_range(start_ts, end_ts, 5)
        .await
        .unwrap_or_default();
    let top_skills = state
        .db
        .get_top_skills_in_range(start_ts, end_ts, 5)
        .await
        .unwrap_or_default();

    // Query token totals
    let (total_input_tokens, total_output_tokens) = state
        .db
        .get_token_totals_in_range(start_ts, end_ts)
        .await
        .unwrap_or((0, 0));

    // Build project digests
    let mut projects: Vec<ProjectDigest> = project_map
        .into_iter()
        .map(|(name, branches)| {
            let branch_digests: Vec<BranchDigest> = branches
                .into_iter()
                .map(|(branch_name, sessions)| BranchDigest {
                    name: branch_name,
                    sessions,
                })
                .collect();

            ProjectDigest {
                session_count: *project_session_counts.get(&name).unwrap_or(&0),
                commit_count: *commit_counts.get(&name).unwrap_or(&0) as usize,
                total_duration_secs: *project_durations.get(&name).unwrap_or(&0),
                branches: branch_digests,
                name,
            }
        })
        .collect();

    // Sort projects by session count descending
    projects.sort_by(|a, b| b.session_count.cmp(&a.session_count));

    let total_sessions = sessions.len();
    let total_projects = projects.len();
    let date_range = if date_start == date_end {
        date_start.to_string()
    } else {
        format!("{date_start} to {date_end}")
    };

    Ok(ContextDigest {
        report_type: report_type.to_string(),
        date_range,
        projects,
        top_tools,
        top_skills,
        summary_line: format!("{total_sessions} sessions across {total_projects} projects"),
        total_input_tokens: total_input_tokens as u64,
        total_output_tokens: total_output_tokens as u64,
    })
}
