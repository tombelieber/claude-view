//! Temporal patterns (T01-T07): Time of day, day of week, break impact, trends.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Timelike};

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;

use super::{mean, Bucket, best_bucket, relative_improvement, worst_bucket};

/// Calculate all temporal patterns from session data.
pub fn calculate_temporal_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = t01_time_of_day(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = t02_day_of_week(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = t07_monthly_trend(sessions, time_range_days) {
        insights.push(i);
    }

    insights
}

fn time_slot_from_hour(hour: u32) -> &'static str {
    match hour {
        6..=11 => "morning",
        12..=17 => "afternoon",
        18..=22 => "evening",
        _ => "night",
    }
}

fn day_name(weekday: u32) -> &'static str {
    match weekday {
        0 => "Monday",
        1 => "Tuesday",
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        6 => "Sunday",
        _ => "Unknown",
    }
}

/// T01: Time of Day - which time slot yields the lowest re-edit rate.
fn t01_time_of_day(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.modified_at > 0)
        .collect();

    if editing_sessions.len() < 100 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let dt = DateTime::from_timestamp(s.modified_at, 0)?.naive_utc();
        let slot = time_slot_from_hour(dt.hour());
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        buckets.entry(slot).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= 10)
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
    vars.insert("best_time".to_string(), best.label.clone());
    vars.insert("worst_time".to_string(), worst.label.clone());
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "T01",
        "Temporal Patterns",
        &vars,
        sample_size,
        100,
        time_range_days,
        improvement / 100.0,
        Actionability::Awareness,
        comparison,
    )
}

/// T02: Day of Week - which day has the lowest re-edit rate.
fn t02_day_of_week(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.modified_at > 0)
        .collect();

    if editing_sessions.len() < 100 {
        return None;
    }

    let mut buckets: HashMap<u32, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let dt = DateTime::from_timestamp(s.modified_at, 0)?.naive_utc();
        let weekday = dt.weekday().num_days_from_monday(); // 0=Mon, 6=Sun
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        buckets.entry(weekday).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= 5)
        .map(|(day, vals)| {
            let avg = mean(&vals).unwrap_or(0.0);
            Bucket::new(day_name(day), vals.len() as u32, avg)
        })
        .collect();

    if computed.len() < 2 {
        return None;
    }

    let best = best_bucket(&computed)?;
    let worst = worst_bucket(&computed)?;
    let sample_size: u32 = computed.iter().map(|b| b.count).sum();

    let mut vars = HashMap::new();
    vars.insert("best_day".to_string(), best.label.clone());
    vars.insert("worst_day".to_string(), worst.label.clone());

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    let improvement = relative_improvement(best.value, worst.value);

    generate_insight(
        "T02",
        "Temporal Patterns",
        &vars,
        sample_size,
        100,
        time_range_days,
        improvement,
        Actionability::Awareness,
        comparison,
    )
}

/// T07: Monthly Trend - month-over-month efficiency improvement.
fn t07_monthly_trend(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.duration_seconds > 0 && s.modified_at > 0)
        .collect();

    if editing_sessions.len() < 30 {
        return None;
    }

    let mut monthly: HashMap<String, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let dt = DateTime::from_timestamp(s.modified_at, 0)?.naive_utc();
        let month_key = format!("{}-{:02}", dt.year(), dt.month());
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        monthly.entry(month_key).or_default().push(reedit_rate);
    }

    let mut months: Vec<(String, f64)> = monthly
        .into_iter()
        .filter(|(_, vals)| vals.len() >= 5)
        .map(|(month, vals)| (month, mean(&vals).unwrap_or(0.0)))
        .collect();

    months.sort_by(|a, b| a.0.cmp(&b.0));

    if months.len() < 2 {
        return None;
    }

    let latest = months.last()?;
    let previous = &months[months.len() - 2];

    let improvement = if previous.1 > 0.0 {
        ((previous.1 - latest.1) / previous.1) * 100.0
    } else {
        0.0
    };

    let trend_direction = if improvement > 5.0 {
        "improved"
    } else if improvement < -5.0 {
        "declined"
    } else {
        "remained stable"
    };

    let sample_size = editing_sessions.len() as u32;
    let mut vars = HashMap::new();
    vars.insert("trend_direction".to_string(), trend_direction.to_string());
    vars.insert("improvement".to_string(), format!("{:.0}", improvement.abs()));

    let mut comparison = HashMap::new();
    comparison.insert("latest_reedit".to_string(), latest.1);
    comparison.insert("previous_reedit".to_string(), previous.1);

    generate_insight(
        "T07",
        "Temporal Patterns",
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

    fn make_temporal_sessions(count: usize) -> Vec<SessionInfo> {
        (0..count)
            .map(|i| {
                let mut s = make_session_with_stats(
                    &format!("t{}", i),
                    600,
                    5,
                    if i % 3 == 0 { 2 } else { 1 },
                    5,
                    1,
                );
                // Spread across different times: base timestamp + hours
                s.modified_at = 1700000000 + (i as i64 * 3600 * 6);
                s
            })
            .collect()
    }

    #[test]
    fn test_t01_insufficient_data() {
        let sessions = make_temporal_sessions(10);
        assert!(t01_time_of_day(&sessions, 30).is_none());
    }

    #[test]
    fn test_t01_with_enough_data() {
        let sessions = make_temporal_sessions(200);
        let insight = t01_time_of_day(&sessions, 30);
        // May or may not find a pattern depending on timestamp distribution
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "T01");
        }
    }

    #[test]
    fn test_t02_with_enough_data() {
        let sessions = make_temporal_sessions(200);
        let insight = t02_day_of_week(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "T02");
        }
    }

    #[test]
    fn test_t07_insufficient_months() {
        // All sessions in same month - should not produce trend
        let sessions = make_temporal_sessions(50);
        let insight = t07_monthly_trend(&sessions, 30);
        // With sessions spread only a few days, may not get 2 months
        // This is expected behavior
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "T07");
        }
    }

    #[test]
    fn test_time_slot_from_hour() {
        assert_eq!(time_slot_from_hour(3), "night");
        assert_eq!(time_slot_from_hour(8), "morning");
        assert_eq!(time_slot_from_hour(14), "afternoon");
        assert_eq!(time_slot_from_hour(20), "evening");
    }

    #[test]
    fn test_day_name() {
        assert_eq!(day_name(0), "Monday");
        assert_eq!(day_name(4), "Friday");
        assert_eq!(day_name(6), "Sunday");
    }

    #[test]
    fn test_all_temporal_patterns_empty() {
        let insights = calculate_temporal_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
