//! Codebase patterns (C01-C07): Language efficiency, project complexity, new vs existing files.

use std::collections::HashMap;

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;

use super::{mean, Bucket, best_bucket, relative_improvement, worst_bucket};

/// Calculate all codebase patterns from session data.
pub fn calculate_codebase_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = c03_project_complexity(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = c04_new_vs_existing(sessions, time_range_days) {
        insights.push(i);
    }

    insights
}

/// C03: Project Complexity - per-project re-edit rates.
fn c03_project_complexity(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 20 {
        return None;
    }

    let mut by_project: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        by_project.entry(&s.project).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = by_project
        .into_iter()
        .filter(|(_, vals)| vals.len() >= super::MIN_BUCKET_SIZE)
        .map(|(name, vals)| {
            let avg = mean(&vals).unwrap_or(0.0);
            Bucket::new(name, vals.len() as u32, avg)
        })
        .collect();

    if computed.len() < 2 {
        return None;
    }

    let worst = worst_bucket(&computed)?;
    let overall_rates: Vec<f64> = editing_sessions
        .iter()
        .map(|s| s.reedited_files_count as f64 / s.files_edited_count as f64)
        .collect();
    let overall_avg = mean(&overall_rates)?;

    if overall_avg <= 0.0 {
        return None; // No re-edits across sessions — pattern not applicable
    }
    let multiplier = worst.value / overall_avg;

    let sample_size: u32 = computed.iter().map(|b| b.count).sum();
    let mut vars = HashMap::new();
    vars.insert("project_name".to_string(), super::format_project_name(&worst.label));
    vars.insert("multiplier".to_string(), format!("{:.1}", multiplier));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }
    comparison.insert("overall_avg".to_string(), overall_avg);

    generate_insight(
        "C03",
        "Codebase Patterns",
        &vars,
        sample_size,
        20,
        time_range_days,
        (multiplier - 1.0).abs().min(1.0),
        Actionability::Informational,
        comparison,
    )
}

/// C04: New vs Existing Files - compare outcomes of creating new vs modifying existing.
fn c04_new_vs_existing(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        // Use tool_counts to distinguish: write = new files, edit = modify existing
        let bucket = if s.tool_counts.write > s.tool_counts.edit {
            "mostly_new"
        } else if s.tool_counts.write > 0 {
            "mixed"
        } else {
            "mostly_modify"
        };
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        buckets.entry(bucket).or_default().push(reedit_rate);
    }

    let computed: Vec<Bucket> = buckets
        .into_iter()
        .filter(|(_, vals)| vals.len() >= super::MIN_BUCKET_SIZE)
        .map(|(label, vals)| {
            let avg = mean(&vals).unwrap_or(0.0);
            Bucket::new(label, vals.len() as u32, avg)
        })
        .collect();

    if computed.len() < super::MIN_BUCKETS {
        return None;
    }

    let best = best_bucket(&computed)?;
    let worst = worst_bucket(&computed)?;
    let improvement = relative_improvement(best.value, worst.value);
    let sample_size: u32 = computed.iter().map(|b| b.count).sum();

    let mut vars = HashMap::new();
    vars.insert("improvement".to_string(), super::format_improvement(improvement * 100.0));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "C04",
        "Codebase Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement,
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
    use crate::types::ToolCounts;

    fn make_codebase_sessions(count: usize) -> Vec<SessionInfo> {
        (0..count)
            .map(|i| {
                let mut s = make_session_with_stats(
                    &format!("c{}", i),
                    600,
                    5,
                    if i % 3 == 0 { 3 } else { 1 },
                    5,
                    1,
                );
                s.project = match i % 3 {
                    0 => "project-alpha".to_string(),
                    1 => "project-beta".to_string(),
                    _ => "project-gamma".to_string(),
                };
                s.tool_counts = ToolCounts {
                    edit: if i % 4 == 0 { 0 } else { 5 },
                    read: 3,
                    bash: 1,
                    write: if i % 4 == 0 { 5 } else { 0 },
                };
                s
            })
            .collect()
    }

    #[test]
    fn test_c03_insufficient_data() {
        let sessions = make_codebase_sessions(5);
        assert!(c03_project_complexity(&sessions, 30).is_none());
    }

    #[test]
    fn test_c03_with_data() {
        let sessions = make_codebase_sessions(120);
        let insight = c03_project_complexity(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "C03");
        }
    }

    #[test]
    fn test_c04_with_data() {
        let sessions = make_codebase_sessions(200);
        let insight = c04_new_vs_existing(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "C04");
        }
    }

    #[test]
    fn test_all_codebase_patterns_empty() {
        let insights = calculate_codebase_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
