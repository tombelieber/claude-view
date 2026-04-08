//! Helper functions for contribution metrics computation.

use std::sync::Arc;

use claude_view_db::{AggregatedContributions, SkillStats, TimeRange, UncommittedWork};

use crate::error::ApiResult;
use crate::state::AppState;

use super::types::ContributionWarning;

/// Get contributions for the previous period (for trend comparison).
pub(super) async fn get_previous_period_contributions(
    state: &Arc<AppState>,
    range: TimeRange,
    project_id: Option<&str>,
    branch: Option<&str>,
) -> ApiResult<AggregatedContributions> {
    // Calculate the previous period based on current range
    let (prev_from, prev_to) = match range {
        TimeRange::Today => {
            // Yesterday
            let yesterday = chrono::Local::now() - chrono::Duration::days(1);
            let date = yesterday.format("%Y-%m-%d").to_string();
            (date.clone(), date)
        }
        TimeRange::Week => {
            // Previous 7 days
            let now = chrono::Local::now();
            let from = (now - chrono::Duration::days(14))
                .format("%Y-%m-%d")
                .to_string();
            let to = (now - chrono::Duration::days(8))
                .format("%Y-%m-%d")
                .to_string();
            (from, to)
        }
        TimeRange::Month => {
            // Previous 30 days
            let now = chrono::Local::now();
            let from = (now - chrono::Duration::days(60))
                .format("%Y-%m-%d")
                .to_string();
            let to = (now - chrono::Duration::days(31))
                .format("%Y-%m-%d")
                .to_string();
            (from, to)
        }
        TimeRange::NinetyDays => {
            // Previous 90 days
            let now = chrono::Local::now();
            let from = (now - chrono::Duration::days(180))
                .format("%Y-%m-%d")
                .to_string();
            let to = (now - chrono::Duration::days(91))
                .format("%Y-%m-%d")
                .to_string();
            (from, to)
        }
        TimeRange::All | TimeRange::Custom => {
            // For all/custom, return empty previous period
            return Ok(AggregatedContributions::default());
        }
    };

    state
        .db
        .get_aggregated_contributions(
            TimeRange::Custom,
            Some(&prev_from),
            Some(&prev_to),
            project_id,
            branch,
        )
        .await
        .map_err(Into::into)
}

/// Generate skill insight comparing sessions with and without skills.
pub(super) fn generate_skill_insight(by_skill: &[SkillStats]) -> String {
    // Find "(no skill)" entry and compare with best skill
    let no_skill = by_skill.iter().find(|s| s.skill == "(no skill)");
    let best_skill = by_skill
        .iter()
        .filter(|s| s.skill != "(no skill)" && s.sessions >= 2)
        .min_by(|a, b| a.reedit_rate.total_cmp(&b.reedit_rate));

    match (no_skill, best_skill) {
        (Some(ns), Some(bs)) if ns.reedit_rate > 0.0 => {
            let improvement = ((ns.reedit_rate - bs.reedit_rate) / ns.reedit_rate) * 100.0;
            if improvement > 30.0 {
                format!(
                    "Sessions using {} skill have {:.0}% lower re-edit rate than sessions without skills - structured workflows produce better results",
                    bs.skill, improvement
                )
            } else if improvement > 10.0 {
                format!(
                    "{} skill provides {:.0}% improvement in output quality",
                    bs.skill, improvement
                )
            } else {
                "Similar quality with or without skills".to_string()
            }
        }
        (None, Some(bs)) => {
            format!(
                "{} is your most effective skill ({:.0}% re-edit rate)",
                bs.skill,
                bs.reedit_rate * 100.0
            )
        }
        _ => "Skill usage patterns not yet established".to_string(),
    }
}

/// Detect warnings based on data quality indicators.
pub(super) fn detect_warnings(
    agg: &AggregatedContributions,
    uncommitted: &[UncommittedWork],
) -> Vec<ContributionWarning> {
    let mut warnings = Vec::new();

    // GitSyncIncomplete: Sessions exist but no commits were correlated
    // This suggests git sync hasn't run or failed
    if agg.sessions_count > 0 && agg.commits_count == 0 && !uncommitted.is_empty() {
        warnings.push(ContributionWarning {
            code: "GitSyncIncomplete".to_string(),
            message: "Some commit data unavailable - run sync to update git history".to_string(),
        });
    }

    // CostUnavailable: No token data when we have sessions.
    // Cost is computed from token usage + pricing; without tokens it is unavailable.
    if agg.sessions_count > 0 && agg.tokens_used == 0 {
        warnings.push(ContributionWarning {
            code: "CostUnavailable".to_string(),
            message: "Cost metrics unavailable - token data missing from some sessions".to_string(),
        });
    }

    warnings
}

/// Generate uncommitted insight from uncommitted work data.
pub(super) fn generate_uncommitted_insight(
    uncommitted: &[UncommittedWork],
    total_lines: i64,
) -> String {
    if uncommitted.is_empty() {
        return "All AI work has been committed".to_string();
    }

    let total_uncommitted: i64 = uncommitted.iter().map(|u| u.lines_added).sum();
    let project_count = uncommitted.len();

    if total_lines > 0 {
        let pct = (total_uncommitted as f64 / total_lines as f64) * 100.0;
        if pct > 20.0 {
            format!(
                "You have {} uncommitted AI lines across {} projects - {:.0}% of recent output. Commit often to avoid losing work.",
                total_uncommitted, project_count, pct
            )
        } else {
            format!(
                "{} uncommitted lines across {} projects - small amount of work in progress",
                total_uncommitted, project_count
            )
        }
    } else {
        format!(
            "{} uncommitted lines across {} projects",
            total_uncommitted, project_count
        )
    }
}
