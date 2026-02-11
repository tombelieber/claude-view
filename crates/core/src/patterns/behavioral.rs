//! Behavioral patterns (B01-B07): Retry patterns, correction patterns.

use std::collections::HashMap;

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;

use super::mean;

/// Calculate all behavioral patterns from session data.
pub fn calculate_behavioral_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = b01_retry_patterns(sessions, time_range_days) {
        insights.push(i);
    }

    // B03 (Abandonment Triggers) removed: used commit_count==0 as "abandonment"
    // signal, but ~90% of sessions have 0 commits, making the metric meaningless.

    insights
}

/// B01: Retry Patterns - average re-edits when re-editing occurs.
fn b01_retry_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let with_reedits: Vec<f64> = editing_sessions
        .iter()
        .filter(|s| s.reedited_files_count > 0)
        .map(|s| s.reedited_files_count as f64)
        .collect();

    if with_reedits.is_empty() {
        return None;
    }

    let avg_reedits = mean(&with_reedits)?;
    let sessions_with = with_reedits.len();
    let total = editing_sessions.len();
    let pct_with_reedits = (sessions_with as f64 / total as f64) * 100.0;

    let sample_size = total as u32;
    let mut vars = HashMap::new();
    vars.insert("avg_reedits".to_string(), format!("{:.1}", avg_reedits));

    let mut comparison = HashMap::new();
    comparison.insert("avg_reedits".to_string(), avg_reedits);
    comparison.insert("sessions_with_reedits".to_string(), sessions_with as f64);
    comparison.insert("pct_with_reedits".to_string(), pct_with_reedits);

    generate_insight(
        "B01",
        "Behavioral Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        0.15, // Moderate informational value
        Actionability::Moderate,
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

    fn make_behavioral_sessions(count: usize) -> Vec<SessionInfo> {
        (0..count)
            .map(|i| {
                let reedited = match i % 5 {
                    0 => 0,
                    1 => 1,
                    2 => 3,
                    3 => 6,
                    _ => 2,
                };
                let commits = if reedited > 4 { 0 } else { 1 };
                make_session_with_stats(
                    &format!("b{}", i),
                    600,
                    5,
                    reedited,
                    5,
                    commits,
                )
            })
            .collect()
    }

    #[test]
    fn test_b01_with_data() {
        let sessions = make_behavioral_sessions(100);
        let insight = b01_retry_patterns(&sessions, 30);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "B01");
    }

    #[test]
    fn test_all_behavioral_patterns_empty() {
        let insights = calculate_behavioral_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
