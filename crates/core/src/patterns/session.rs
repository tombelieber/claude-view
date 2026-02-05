//! Session patterns (S01-S08): Duration, turn count, fatigue, file count correlations.

use std::collections::HashMap;

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;

use super::{mean, Bucket, best_bucket, relative_improvement, worst_bucket};

/// Calculate all session patterns from session data.
pub fn calculate_session_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = s01_optimal_duration(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = s02_turn_count_sweet_spot(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = s04_fatigue_signal(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = s08_file_count_correlation(sessions, time_range_days) {
        insights.push(i);
    }

    insights
}

/// S01: Optimal Duration - which duration bucket yields the best edits/minute.
fn s01_optimal_duration(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.duration_seconds > 0 && s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let bucket = match s.duration_seconds {
            0..=899 => "<15min",
            900..=2699 => "15-45min",
            2700..=5399 => "45-90min",
            _ => ">90min",
        };
        let velocity = s.files_edited_count as f64 / (s.duration_seconds as f64 / 60.0);
        buckets.entry(bucket).or_default().push(velocity);
    }

    let computed_buckets: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= 10)
        .map(|(label, vals)| {
            let avg = mean(&vals).unwrap_or(0.0);
            Bucket::new(label, vals.len() as u32, avg)
        })
        .collect();

    if computed_buckets.len() < 2 {
        return None;
    }

    // For velocity, higher is better (more edits per minute)
    let best = computed_buckets
        .iter()
        .max_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))?;
    let worst = computed_buckets
        .iter()
        .min_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))?;

    let improvement = if worst.value > 0.0 {
        ((best.value - worst.value) / worst.value) * 100.0
    } else {
        0.0
    };

    let sample_size: u32 = computed_buckets.iter().map(|b| b.count).sum();
    let mut vars = HashMap::new();
    vars.insert("optimal_duration".to_string(), best.label.clone());
    vars.insert("worst_duration".to_string(), worst.label.clone());
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    for b in &computed_buckets {
        comparison.insert(format!("velocity_{}", b.label), b.value);
    }

    generate_insight(
        "S01",
        "Session Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement / 100.0,
        Actionability::Moderate,
        comparison,
    )
}

/// S02: Turn Count Sweet Spot - optimal number of turns for lowest re-edit rate.
fn s02_turn_count_sweet_spot(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let bucket = match s.turn_count {
            0..=5 => "1-5",
            6..=8 => "6-8",
            9..=12 => "9-12",
            13..=15 => "13-15",
            _ => "16+",
        };
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        buckets.entry(bucket).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= 5)
        .map(|(label, vals)| {
            let avg = mean(&vals).unwrap_or(0.0);
            Bucket::new(label, vals.len() as u32, avg)
        })
        .collect();

    if computed.len() < 2 {
        return None;
    }

    let best = best_bucket(&computed)?;
    let worst = worst_bucket(&computed)?;
    let improvement = relative_improvement(best.value, worst.value) * 100.0;
    let sample_size: u32 = computed.iter().map(|b| b.count).sum();

    let mut vars = HashMap::new();
    vars.insert("optimal_turns".to_string(), best.label.clone());
    vars.insert("reedit_rate".to_string(), format!("{:.0}", best.value * 100.0));
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "S02",
        "Session Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement / 100.0,
        Actionability::Moderate,
        comparison,
    )
}

/// S04: Fatigue Signal - re-edit rate increases after a certain turn count.
fn s04_fatigue_signal(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.turn_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let threshold = 12;
    let early: Vec<f64> = editing_sessions
        .iter()
        .filter(|s| s.turn_count <= threshold)
        .filter_map(|s| s.reedit_rate())
        .collect();

    let late: Vec<f64> = editing_sessions
        .iter()
        .filter(|s| s.turn_count > threshold)
        .filter_map(|s| s.reedit_rate())
        .collect();

    if early.len() < 10 || late.len() < 10 {
        return None;
    }

    let early_avg = mean(&early)?;
    let late_avg = mean(&late)?;
    let improvement = if early_avg > 0.0 {
        ((late_avg - early_avg) / early_avg) * 100.0
    } else {
        0.0
    };

    if improvement <= 0.0 {
        return None; // No fatigue signal
    }

    let sample_size = (early.len() + late.len()) as u32;
    let mut vars = HashMap::new();
    vars.insert("threshold".to_string(), threshold.to_string());
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    comparison.insert("early_reedit_rate".to_string(), early_avg);
    comparison.insert("late_reedit_rate".to_string(), late_avg);

    generate_insight(
        "S04",
        "Session Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement / 100.0,
        Actionability::Moderate,
        comparison,
    )
}

/// S08: File Count Correlation - more files = higher re-edit rate?
fn s08_file_count_correlation(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let bucket = match s.files_edited_count {
            1..=3 => "1-3",
            4..=7 => "4-7",
            8..=10 => "8-10",
            _ => "11+",
        };
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        buckets.entry(bucket).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= 5)
        .map(|(label, vals)| {
            let avg = mean(&vals).unwrap_or(0.0);
            Bucket::new(label, vals.len() as u32, avg)
        })
        .collect();

    if computed.len() < 2 {
        return None;
    }

    let best = best_bucket(&computed)?;
    let worst = worst_bucket(&computed)?;
    let improvement = relative_improvement(best.value, worst.value) * 100.0;
    let sample_size: u32 = computed.iter().map(|b| b.count).sum();

    let mut vars = HashMap::new();
    vars.insert("threshold".to_string(), worst.label.clone());
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "S08",
        "Session Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement / 100.0,
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

    fn make_diverse_sessions(count: usize) -> Vec<SessionInfo> {
        (0..count)
            .map(|i| {
                let duration = match i % 4 {
                    0 => 600,    // 10 min
                    1 => 1800,   // 30 min
                    2 => 3600,   // 60 min
                    _ => 7200,   // 120 min
                };
                // 30-min sessions are most productive
                let files_edited = if duration == 1800 { 10 } else { 3 };
                let reedited = if duration == 1800 { 1 } else { 2 };
                let turns = match i % 4 {
                    0 => 3,
                    1 => 7,
                    2 => 11,
                    _ => 20,
                };
                make_session_with_stats(
                    &format!("s{}", i),
                    duration,
                    files_edited,
                    reedited,
                    turns,
                    if i % 3 == 0 { 1 } else { 0 },
                )
            })
            .collect()
    }

    #[test]
    fn test_s01_insufficient_data() {
        let sessions = make_diverse_sessions(10);
        assert!(s01_optimal_duration(&sessions, 30).is_none());
    }

    #[test]
    fn test_s01_with_enough_data() {
        let sessions = make_diverse_sessions(200);
        let insight = s01_optimal_duration(&sessions, 30);
        assert!(insight.is_some(), "Should find optimal duration pattern with 200 sessions");
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "S01");
        assert!(insight.impact_score > 0.0);
    }

    #[test]
    fn test_s02_insufficient_data() {
        let sessions = make_diverse_sessions(10);
        assert!(s02_turn_count_sweet_spot(&sessions, 30).is_none());
    }

    #[test]
    fn test_s02_with_enough_data() {
        let sessions = make_diverse_sessions(200);
        let insight = s02_turn_count_sweet_spot(&sessions, 30);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "S02");
    }

    #[test]
    fn test_s04_fatigue_with_data() {
        // Create sessions where late sessions have higher re-edit
        let mut sessions = Vec::new();
        for i in 0..60 {
            let turns = if i < 30 { 5 } else { 15 };
            let reedited = if turns > 12 { 3 } else { 1 };
            sessions.push(make_session_with_stats(
                &format!("s{}", i),
                600,
                5,
                reedited,
                turns,
                1,
            ));
        }
        let insight = s04_fatigue_signal(&sessions, 30);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "S04");
    }

    #[test]
    fn test_s08_file_count_correlation() {
        let sessions = make_diverse_sessions(200);
        let insight = s08_file_count_correlation(&sessions, 30);
        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "S08");
    }

    #[test]
    fn test_all_session_patterns() {
        let sessions = make_diverse_sessions(200);
        let insights = calculate_session_patterns(&sessions, 30);
        // Should produce at least some insights
        assert!(!insights.is_empty(), "Should generate at least one session pattern");
    }
}
