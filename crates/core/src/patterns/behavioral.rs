//! Behavioral patterns (B01-B07): Retry patterns, abandonment triggers, correction patterns.

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
    if let Some(i) = b03_abandonment_triggers(sessions, time_range_days) {
        insights.push(i);
    }

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

/// B03: Abandonment Triggers - what re-edit count precedes abandonment.
fn b03_abandonment_triggers(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut buckets: HashMap<&str, (u32, u32)> = HashMap::new(); // (total, abandoned)
    for s in &editing_sessions {
        let bucket = match s.reedited_files_count {
            0 => "0",
            1..=2 => "1-2",
            3..=5 => "3-5",
            _ => "6+",
        };
        let entry = buckets.entry(bucket).or_default();
        entry.0 += 1;
        if s.commit_count == 0 {
            entry.1 += 1;
        }
    }

    let computed: Vec<(String, u32, f64)> = buckets
        .into_iter()
        .filter(|(_, (total, _))| *total >= 5)
        .map(|(label, (total, abandoned))| {
            let rate = abandoned as f64 / total as f64;
            (label.to_string(), total, rate)
        })
        .collect();

    if computed.len() < 2 {
        return None;
    }

    // Find the bucket with the highest abandonment rate
    let worst = computed
        .iter()
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))?;

    let sample_size: u32 = computed.iter().map(|(_, count, _)| count).sum();
    let mut vars = HashMap::new();
    vars.insert("threshold".to_string(), worst.0.clone());
    vars.insert("abandon_rate".to_string(), format!("{:.0}", worst.2 * 100.0));

    let mut comparison = HashMap::new();
    for (label, count, rate) in &computed {
        comparison.insert(format!("abandon_rate_{}", label), *rate);
        comparison.insert(format!("count_{}", label), *count as f64);
    }

    generate_insight(
        "B03",
        "Behavioral Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        worst.2.min(1.0),
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
    fn test_b03_with_data() {
        let sessions = make_behavioral_sessions(100);
        let insight = b03_abandonment_triggers(&sessions, 30);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "B03");
    }

    #[test]
    fn test_all_behavioral_patterns_empty() {
        let insights = calculate_behavioral_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
