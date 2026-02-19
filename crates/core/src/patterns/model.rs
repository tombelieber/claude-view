//! Model patterns (M01-M05): Model task fit, model by complexity.

use std::collections::HashMap;

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;

use super::{best_bucket, mean, relative_improvement, worst_bucket, Bucket};

/// Calculate all model patterns from session data.
pub fn calculate_model_patterns(
    sessions: &[SessionInfo],
    time_range_days: u32,
) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = m01_model_task_fit(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = m05_model_by_complexity(sessions, time_range_days) {
        insights.push(i);
    }

    insights
}

/// Extract model family from primary_model string.
fn model_family(model: &str) -> &str {
    let lower = model.to_lowercase();
    if lower.contains("opus") {
        "opus"
    } else if lower.contains("sonnet") {
        "sonnet"
    } else if lower.contains("haiku") {
        "haiku"
    } else if lower.contains("gpt-4") {
        "gpt-4"
    } else if lower.contains("gpt-3") {
        "gpt-3"
    } else {
        "other"
    }
}

/// M01: Model Task Fit - which model family has the lowest re-edit rate.
fn m01_model_task_fit(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.primary_model.is_some())
        .collect();

    if editing_sessions.len() < 30 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let family = model_family(s.primary_model.as_deref().unwrap_or("unknown"));
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        buckets.entry(family).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= super::MIN_MODEL_BUCKET)
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
    vars.insert("best_model".to_string(), best.label.clone());
    vars.insert("worst_model".to_string(), worst.label.clone());
    vars.insert(
        "improvement".to_string(),
        super::format_improvement(improvement),
    );

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "M01",
        "Model Patterns",
        &vars,
        sample_size,
        30,
        time_range_days,
        improvement / 100.0,
        Actionability::Moderate,
        comparison,
    )
}

/// M05: Model by Complexity - which model performs best on complex tasks.
fn m05_model_by_complexity(
    sessions: &[SessionInfo],
    time_range_days: u32,
) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.primary_model.is_some())
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    // Focus on "high complexity" sessions (many files)
    let complex: Vec<_> = editing_sessions
        .iter()
        .filter(|s| s.files_edited_count > 7)
        .collect();

    if complex.len() < 20 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &complex {
        let family = model_family(s.primary_model.as_deref().unwrap_or("unknown"));
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        buckets.entry(family).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= super::MIN_MODEL_BUCKET)
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
    let sample_size = complex.len() as u32;

    let mut vars = HashMap::new();
    vars.insert("best_model".to_string(), best.label.clone());
    vars.insert(
        "improvement".to_string(),
        super::format_improvement(improvement),
    );

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("complex_reedit_{}", b.label), b.value);
    }

    generate_insight(
        "M05",
        "Model Patterns",
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

    fn make_model_sessions(count: usize) -> Vec<SessionInfo> {
        (0..count)
            .map(|i| {
                let mut s = make_session_with_stats(
                    &format!("m{}", i),
                    600,
                    if i % 3 == 0 { 10 } else { 3 },
                    if i % 3 == 0 { 3 } else { 1 },
                    5,
                    1,
                );
                s.primary_model = Some(match i % 3 {
                    0 => "claude-opus-4-5-20251101".to_string(),
                    1 => "claude-sonnet-4-20250514".to_string(),
                    _ => "claude-haiku-4-20250514".to_string(),
                });
                s
            })
            .collect()
    }

    #[test]
    fn test_model_family_extraction() {
        assert_eq!(model_family("claude-opus-4-5-20251101"), "opus");
        assert_eq!(model_family("claude-sonnet-4-20250514"), "sonnet");
        assert_eq!(model_family("claude-haiku-4-20250514"), "haiku");
        assert_eq!(model_family("gpt-4o-2024-08-06"), "gpt-4");
        assert_eq!(model_family("unknown-model"), "other");
    }

    #[test]
    fn test_m01_insufficient_data() {
        let sessions = make_model_sessions(5);
        assert!(m01_model_task_fit(&sessions, 30).is_none());
    }

    #[test]
    fn test_m01_with_data() {
        let sessions = make_model_sessions(120);
        let insight = m01_model_task_fit(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "M01");
            assert_eq!(insight.category, "Model Patterns");
        }
    }

    #[test]
    fn test_all_model_patterns_empty() {
        let insights = calculate_model_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
