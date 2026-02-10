//! Outcome patterns (O01-O05): Commit rate, abandoned sessions, session productivity.

use std::collections::HashMap;

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;


/// Calculate all outcome patterns from session data.
pub fn calculate_outcome_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = o01_commit_rate(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = o02_session_outcomes(sessions, time_range_days) {
        insights.push(i);
    }

    insights
}

/// O01: Commit Rate - what percentage of sessions result in commits.
fn o01_commit_rate(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    if sessions.len() < 50 {
        return None;
    }

    let with_commits = sessions.iter().filter(|s| s.commit_count > 0).count();
    let commit_pct = (with_commits as f64 / sessions.len() as f64) * 100.0;
    let sample_size = sessions.len() as u32;

    let mut vars = HashMap::new();
    vars.insert("commit_pct".to_string(), format!("{:.0}", commit_pct));

    let mut comparison = HashMap::new();
    comparison.insert("total_sessions".to_string(), sessions.len() as f64);
    comparison.insert("with_commits".to_string(), with_commits as f64);

    generate_insight(
        "O01",
        "Outcome Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        0.1, // Informational, low effect size
        Actionability::Informational,
        comparison,
    )
}

/// O02: Session Outcomes - categorize sessions by activity type.
fn o02_session_outcomes(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    if sessions.len() < 100 {
        return None;
    }

    let mut deep_work = 0u32;
    let mut quick_task = 0u32;
    let mut exploration = 0u32;
    let mut minimal = 0u32;

    for s in sessions {
        if s.files_edited_count > 0 && s.duration_seconds > 900 {
            deep_work += 1;
        } else if s.files_edited_count > 0 && s.duration_seconds <= 900 {
            quick_task += 1;
        } else if s.files_read_count > 0 || s.duration_seconds > 300 {
            exploration += 1;
        } else {
            minimal += 1;
        }
    }

    let total = sessions.len() as f64;
    let deep_work_pct = (deep_work as f64 / total) * 100.0;
    let quick_task_pct = (quick_task as f64 / total) * 100.0;
    let exploration_pct = (exploration as f64 / total) * 100.0;
    let minimal_pct = (minimal as f64 / total) * 100.0;

    let sample_size = sessions.len() as u32;
    let mut vars = HashMap::new();
    vars.insert("deep_work_pct".to_string(), format!("{:.0}", deep_work_pct));
    vars.insert("quick_task_pct".to_string(), format!("{:.0}", quick_task_pct));
    vars.insert("exploration_pct".to_string(), format!("{:.0}", exploration_pct));
    vars.insert("minimal_pct".to_string(), format!("{:.0}", minimal_pct));

    let mut comparison = HashMap::new();
    comparison.insert("deep_work".to_string(), deep_work as f64);
    comparison.insert("quick_task".to_string(), quick_task as f64);
    comparison.insert("exploration".to_string(), exploration as f64);
    comparison.insert("minimal".to_string(), minimal as f64);

    generate_insight(
        "O02",
        "Outcome Patterns",
        &vars,
        sample_size,
        100,
        time_range_days,
        0.1,
        Actionability::Informational,
        comparison,
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::test_helpers::*;

    fn make_outcome_sessions(count: usize) -> Vec<SessionInfo> {
        (0..count)
            .map(|i| {
                let commit = if i % 3 == 0 { 2 } else { 0 };
                let duration = if i % 5 == 0 { 120 } else { 900 };
                make_session_with_stats(
                    &format!("o{}", i),
                    duration,
                    3,
                    1,
                    5,
                    commit,
                )
            })
            .collect()
    }

    #[test]
    fn test_o01_insufficient_data() {
        let sessions = make_outcome_sessions(10);
        assert!(o01_commit_rate(&sessions, 30).is_none());
    }

    #[test]
    fn test_o01_with_data() {
        let sessions = make_outcome_sessions(100);
        let insight = o01_commit_rate(&sessions, 30);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "O01");
    }

    #[test]
    fn test_o02_with_data() {
        let sessions = make_outcome_sessions(200);
        let insight = o02_session_outcomes(&sessions, 30);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "O02");
        assert!(insight.body.contains('%'));
    }

    #[test]
    fn test_all_outcome_patterns_empty() {
        let insights = calculate_outcome_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
