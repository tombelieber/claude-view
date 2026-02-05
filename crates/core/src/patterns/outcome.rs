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
    if let Some(i) = o02_abandoned_sessions(sessions, time_range_days) {
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

/// O02: Abandoned Sessions - sessions with no commits categorized by type.
fn o02_abandoned_sessions(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    if sessions.len() < 100 {
        return None;
    }

    let mut abandoned = 0u32;
    let mut quick_lookup = 0u32;
    let mut productive = 0u32;

    for s in sessions {
        if s.commit_count == 0 && s.duration_seconds > 300 {
            abandoned += 1;
        } else if s.commit_count == 0 && s.duration_seconds <= 300 {
            quick_lookup += 1;
        } else {
            productive += 1;
        }
    }

    let total = sessions.len() as f64;
    let abandoned_pct = (abandoned as f64 / total) * 100.0;
    let exploration_pct = (quick_lookup as f64 / total) * 100.0;

    let sample_size = sessions.len() as u32;
    let mut vars = HashMap::new();
    vars.insert("abandoned_pct".to_string(), format!("{:.0}", abandoned_pct));
    vars.insert("exploration_pct".to_string(), format!("{:.0}", exploration_pct));

    let mut comparison = HashMap::new();
    comparison.insert("abandoned".to_string(), abandoned as f64);
    comparison.insert("quick_lookup".to_string(), quick_lookup as f64);
    comparison.insert("productive".to_string(), productive as f64);

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
        let insight = o02_abandoned_sessions(&sessions, 30);
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
