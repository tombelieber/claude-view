//! Workflow patterns (W01-W08): Planning, test-first, commits, branch discipline, read-before-write.

use std::collections::HashMap;

use crate::insights::generator::{generate_insight, GeneratedInsight};
use crate::insights::scoring::Actionability;
use crate::types::SessionInfo;

use super::{mean, Bucket, best_bucket, relative_improvement, worst_bucket};

/// Calculate all workflow patterns from session data.
pub fn calculate_workflow_patterns(sessions: &[SessionInfo], time_range_days: u32) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    if let Some(i) = w03_planning_to_execution(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = w04_test_first_correlation(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = w05_commit_frequency(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = w06_branch_discipline(sessions, time_range_days) {
        insights.push(i);
    }
    if let Some(i) = w07_read_before_write(sessions, time_range_days) {
        insights.push(i);
    }

    insights
}

/// W03: Planning to Execution - sessions with more reads before edits perform better.
fn w03_planning_to_execution(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let ratio = s.files_read_count as f64 / s.files_edited_count as f64;
        let bucket = if ratio > 2.0 {
            "heavy_planning"
        } else if ratio > 1.0 {
            "some_planning"
        } else {
            "execution_focused"
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
    let improvement = relative_improvement(best.value, worst.value) * 100.0;
    let sample_size: u32 = computed.iter().map(|b| b.count).sum();

    let mut vars = HashMap::new();
    vars.insert("improvement".to_string(), super::format_improvement(improvement));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "W03",
        "Workflow Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement / 100.0,
        Actionability::Immediate,
        comparison,
    )
}

/// W04: Test First Correlation - sessions with test file edits have fewer re-edits.
fn w04_test_first_correlation(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 30 {
        return None;
    }

    let mut with_tests: Vec<f64> = Vec::new();
    let mut without_tests: Vec<f64> = Vec::new();

    for s in &editing_sessions {
        let has_tests = s.files_edited.iter().any(|f| {
            let lower = f.to_lowercase();
            lower.contains("test") || lower.contains("spec") || lower.contains("_test.")
        });
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        if has_tests {
            with_tests.push(reedit_rate);
        } else {
            without_tests.push(reedit_rate);
        }
    }

    if with_tests.len() < 10 || without_tests.len() < 10 {
        return None;
    }

    let with_avg = mean(&with_tests)?;
    let without_avg = mean(&without_tests)?;
    let improvement = relative_improvement(with_avg, without_avg) * 100.0;
    let sample_size = (with_tests.len() + without_tests.len()) as u32;

    let mut vars = HashMap::new();
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    comparison.insert("with_tests_reedit".to_string(), with_avg);
    comparison.insert("without_tests_reedit".to_string(), without_avg);

    generate_insight(
        "W04",
        "Workflow Patterns",
        &vars,
        sample_size,
        30,
        time_range_days,
        improvement / 100.0,
        Actionability::Moderate,
        comparison,
    )
}

/// W05: Commit Frequency - frequent committers vs end-of-session committers.
fn w05_commit_frequency(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut frequent: Vec<f64> = Vec::new();
    let mut single: Vec<f64> = Vec::new();
    let mut none: Vec<f64> = Vec::new();

    for s in &editing_sessions {
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        match s.commit_count {
            0 => none.push(reedit_rate),
            1 => single.push(reedit_rate),
            _ => frequent.push(reedit_rate),
        }
    }

    // Need at least MIN_BUCKETS groups with data
    let groups: Vec<(&str, &[f64])> = [
        ("frequent", frequent.as_slice()),
        ("single", single.as_slice()),
        ("none", none.as_slice()),
    ]
    .into_iter()
    .filter(|(_, vals)| vals.len() >= super::MIN_BUCKET_SIZE)
    .collect();

    if groups.len() < super::MIN_BUCKETS {
        return None;
    }

    let computed: Vec<Bucket> = groups
        .iter()
        .map(|(label, vals)| {
            let avg = mean(vals).unwrap_or(0.0);
            Bucket::new(*label, vals.len() as u32, avg)
        })
        .collect();

    let best = best_bucket(&computed)?;
    let worst = worst_bucket(&computed)?;
    let improvement = relative_improvement(best.value, worst.value) * 100.0;
    let sample_size: u32 = computed.iter().map(|b| b.count).sum();

    let mut vars = HashMap::new();
    vars.insert("commit_style".to_string(), best.label.clone());
    vars.insert("improvement".to_string(), super::format_improvement(improvement));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "W05",
        "Workflow Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement / 100.0,
        Actionability::Moderate,
        comparison,
    )
}

/// W06: Branch Discipline - feature branch vs main branch re-edit rates.
fn w06_branch_discipline(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0 && s.git_branch.is_some())
        .collect();

    if editing_sessions.len() < 30 {
        return None;
    }

    let mut main_rates: Vec<f64> = Vec::new();
    let mut feature_rates: Vec<f64> = Vec::new();

    for s in &editing_sessions {
        let branch = s.git_branch.as_deref().unwrap_or("");
        let reedit_rate = s.reedited_files_count as f64 / s.files_edited_count as f64;
        if branch == "main" || branch == "master" {
            main_rates.push(reedit_rate);
        } else {
            feature_rates.push(reedit_rate);
        }
    }

    if main_rates.len() < super::MIN_BUCKET_SIZE || feature_rates.len() < super::MIN_BUCKET_SIZE {
        return None;
    }

    let main_avg = mean(&main_rates)?;
    let feature_avg = mean(&feature_rates)?;
    let improvement = relative_improvement(feature_avg, main_avg) * 100.0;
    let sample_size = (main_rates.len() + feature_rates.len()) as u32;

    let mut vars = HashMap::new();
    vars.insert("improvement".to_string(), format!("{:.0}", improvement));

    let mut comparison = HashMap::new();
    comparison.insert("main_reedit".to_string(), main_avg);
    comparison.insert("feature_reedit".to_string(), feature_avg);

    generate_insight(
        "W06",
        "Workflow Patterns",
        &vars,
        sample_size,
        30,
        time_range_days,
        improvement / 100.0,
        Actionability::Moderate,
        comparison,
    )
}

/// W07: Read Before Write - sessions with more reads have better outcomes.
fn w07_read_before_write(sessions: &[SessionInfo], time_range_days: u32) -> Option<GeneratedInsight> {
    let editing_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.files_edited_count > 0)
        .collect();

    if editing_sessions.len() < 50 {
        return None;
    }

    let mut buckets: HashMap<&str, Vec<f64>> = HashMap::new();
    for s in &editing_sessions {
        let ratio = s.files_read_count as f64 / s.files_edited_count as f64;
        let bucket = if ratio < 1.0 {
            "low"
        } else if ratio < 3.0 {
            "moderate"
        } else {
            "high"
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
    let improvement = relative_improvement(best.value, worst.value) * 100.0;
    let sample_size: u32 = computed.iter().map(|b| b.count).sum();

    let mut vars = HashMap::new();
    vars.insert("read_level".to_string(), best.label.clone());
    vars.insert("improvement".to_string(), super::format_improvement(improvement));

    let mut comparison = HashMap::new();
    for b in &computed {
        comparison.insert(format!("reedit_{}", b.label), b.value);
    }

    generate_insight(
        "W07",
        "Workflow Patterns",
        &vars,
        sample_size,
        50,
        time_range_days,
        improvement / 100.0,
        Actionability::Immediate,
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

    fn make_workflow_sessions(count: usize) -> Vec<SessionInfo> {
        (0..count)
            .map(|i| {
                let mut s = make_session_with_stats(
                    &format!("w{}", i),
                    600,
                    5,
                    if i % 3 == 0 { 2 } else { 1 },
                    5,
                    if i % 2 == 0 { 2 } else { 0 },
                );
                s.files_read_count = if i % 4 == 0 { 20 } else { 2 };
                s.files_edited = if i % 5 == 0 {
                    vec!["src/test_foo.rs".to_string(), "src/main.rs".to_string()]
                } else {
                    vec!["src/main.rs".to_string()]
                };
                s.git_branch = Some(if i % 3 == 0 {
                    "main".to_string()
                } else {
                    format!("feature/test-{}", i)
                });
                s
            })
            .collect()
    }

    #[test]
    fn test_w03_insufficient_data() {
        let sessions = make_workflow_sessions(10);
        assert!(w03_planning_to_execution(&sessions, 30).is_none());
    }

    #[test]
    fn test_w03_with_data() {
        let sessions = make_workflow_sessions(200);
        let insight = w03_planning_to_execution(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "W03");
            assert_eq!(insight.category, "Workflow Patterns");
        }
    }

    #[test]
    fn test_w05_with_data() {
        let sessions = make_workflow_sessions(200);
        let insight = w05_commit_frequency(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "W05");
        }
    }

    #[test]
    fn test_w06_branch_discipline() {
        let sessions = make_workflow_sessions(200);
        let insight = w06_branch_discipline(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "W06");
        }
    }

    #[test]
    fn test_w07_read_before_write() {
        let sessions = make_workflow_sessions(200);
        let insight = w07_read_before_write(&sessions, 30);
        if let Some(insight) = insight {
            assert_eq!(insight.pattern_id, "W07");
        }
    }

    #[test]
    fn test_all_workflow_patterns_empty() {
        let insights = calculate_workflow_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
