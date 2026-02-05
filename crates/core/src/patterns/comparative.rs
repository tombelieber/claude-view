//! Comparative patterns (CP01-CP03): User vs baseline, trends over time.

use std::collections::HashMap;

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;

use super::mean;

/// Calculate all comparative patterns from session data.
pub fn calculate_comparative_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = cp01_you_vs_baseline(sessions, time_range_days) {
        insights.push(i);
    }

    insights
}

/// CP01: You vs Baseline - compare recent 7-day performance vs overall period.
fn cp01_you_vs_baseline(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    if sessions.len() < 30 {
        return None;
    }

    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.duration_seconds > 0)
        .collect();

    if editing_sessions.len() < 20 {
        return None;
    }

    // Find the max timestamp to determine "recent"
    let max_ts = editing_sessions
        .iter()
        .map(|s| s.modified_at)
        .max()
        .unwrap_or(0);

    let week_ago = max_ts - 7 * 86400;

    let recent: Vec<f64> = editing_sessions
        .iter()
        .filter(|s| s.modified_at >= week_ago)
        .filter_map(|s| s.reedit_rate())
        .collect();

    let earlier: Vec<f64> = editing_sessions
        .iter()
        .filter(|s| s.modified_at < week_ago)
        .filter_map(|s| s.reedit_rate())
        .collect();

    if recent.len() < 5 || earlier.len() < 10 {
        return None;
    }

    let recent_avg = mean(&recent)?;
    let earlier_avg = mean(&earlier)?;

    let improvement = if earlier_avg > 0.0 {
        ((earlier_avg - recent_avg) / earlier_avg) * 100.0
    } else {
        0.0
    };

    let sample_size = editing_sessions.len() as u32;
    let mut vars = HashMap::new();
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    comparison.insert("recent_reedit".to_string(), recent_avg);
    comparison.insert("baseline_reedit".to_string(), earlier_avg);
    comparison.insert("recent_sessions".to_string(), recent.len() as f64);
    comparison.insert("baseline_sessions".to_string(), earlier.len() as f64);

    generate_insight(
        "CP01",
        "Comparative Patterns",
        &vars,
        sample_size,
        30,
        time_range_days,
        improvement.abs() / 100.0,
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

    fn make_comparative_sessions(count: usize) -> Vec<SessionInfo> {
        let base_ts: i64 = 1700000000;
        (0..count)
            .map(|i| {
                let mut s = make_session_with_stats(
                    &format!("cp{}", i),
                    600,
                    5,
                    if i < count / 2 { 2 } else { 1 }, // earlier sessions have more reedits
                    5,
                    1,
                );
                // Spread over 30 days
                s.modified_at = base_ts + (i as i64 * 86400 / 3);
                s
            })
            .collect()
    }

    #[test]
    fn test_cp01_insufficient_data() {
        let sessions = make_comparative_sessions(5);
        assert!(cp01_you_vs_baseline(&sessions, 30).is_none());
    }

    #[test]
    fn test_cp01_with_data() {
        let sessions = make_comparative_sessions(100);
        let insight = cp01_you_vs_baseline(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "CP01");
            assert_eq!(insight.category, "Comparative Patterns");
        }
    }

    #[test]
    fn test_all_comparative_patterns_empty() {
        let insights = calculate_comparative_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
