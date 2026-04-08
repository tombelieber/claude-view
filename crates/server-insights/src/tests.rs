// crates/server/src/insights/tests.rs
//! Tests for insight generation.

use super::*;

// ========================================================================
// Insight construction tests
// ========================================================================

#[test]
fn test_insight_info() {
    let insight = Insight::info("Test message");
    assert_eq!(insight.text, "Test message");
    assert_eq!(insight.kind, InsightKind::Info);
}

#[test]
fn test_insight_success() {
    let insight = Insight::success("Good job");
    assert_eq!(insight.kind, InsightKind::Success);
}

#[test]
fn test_insight_warning() {
    let insight = Insight::warning("Watch out");
    assert_eq!(insight.kind, InsightKind::Warning);
}

#[test]
fn test_insight_tip() {
    let insight = Insight::tip("Try this");
    assert_eq!(insight.kind, InsightKind::Tip);
}

// ========================================================================
// Fluency insight tests
// ========================================================================

#[test]
fn test_fluency_insight_more_active() {
    let insight = fluency_insight(120, 100);
    assert!(insight.text.contains("+20%"));
    assert_eq!(insight.kind, InsightKind::Success);
}

#[test]
fn test_fluency_insight_less_active() {
    let insight = fluency_insight(80, 100);
    assert!(insight.text.contains("-20%"));
    assert_eq!(insight.kind, InsightKind::Info);
}

#[test]
fn test_fluency_insight_consistent() {
    let insight = fluency_insight(105, 100);
    assert!(insight.text.contains("Consistent"));
}

#[test]
fn test_fluency_insight_no_previous() {
    let insight = fluency_insight(50, 0);
    assert!(insight.text.contains("50 sessions"));
}

#[test]
fn test_fluency_insight_no_activity() {
    let insight = fluency_insight(0, 0);
    assert!(insight.text.contains("No activity"));
}

// ========================================================================
// Output insight tests
// ========================================================================

#[test]
fn test_output_insight_highly_productive() {
    let insight = output_insight(1500, Some(("Monday", 600)));
    assert!(insight.text.contains("Highly productive"));
    assert!(insight.text.contains("Monday"));
    assert_eq!(insight.kind, InsightKind::Success);
}

#[test]
fn test_output_insight_good() {
    let insight = output_insight(700, Some(("Tuesday", 200)));
    assert!(insight.text.contains("Good output"));
}

#[test]
fn test_output_insight_light() {
    let insight = output_insight(200, None);
    assert!(insight.text.contains("Light AI usage"));
}

#[test]
fn test_output_insight_minimal() {
    let insight = output_insight(50, None);
    assert!(insight.text.contains("Minimal"));
}

// ========================================================================
// Effectiveness insight tests
// ========================================================================

#[test]
fn test_effectiveness_excellent() {
    let insight = effectiveness_insight(Some(0.85), Some(0.15));
    assert!(insight.text.contains("Excellent"));
    assert_eq!(insight.kind, InsightKind::Success);
}

#[test]
fn test_effectiveness_high_reedit() {
    let insight = effectiveness_insight(Some(0.7), Some(0.40));
    assert!(insight.text.contains("High re-edit rate"));
    assert_eq!(insight.kind, InsightKind::Warning);
}

#[test]
fn test_effectiveness_low_commit() {
    let insight = effectiveness_insight(Some(0.3), Some(0.2));
    assert!(insight.text.contains("Low commit rate"));
    assert_eq!(insight.kind, InsightKind::Tip);
}

#[test]
fn test_effectiveness_no_data() {
    let insight = effectiveness_insight(None, None);
    assert!(insight.text.contains("Not enough data"));
}

// ========================================================================
// Model insight tests
// ========================================================================

#[test]
fn test_model_insight_same_best() {
    let models = vec![
        ModelStats {
            model: "claude-sonnet".to_string(),
            reedit_rate: Some(0.1),
            cost_per_line: Some(0.001),
        },
        ModelStats {
            model: "claude-opus".to_string(),
            reedit_rate: Some(0.15),
            cost_per_line: Some(0.01),
        },
    ];
    let insight = model_insight(&models);
    assert!(insight.text.contains("claude-sonnet"));
    assert!(insight.text.contains("cheapest and most accurate"));
}

#[test]
fn test_model_insight_different_best() {
    let models = vec![
        ModelStats {
            model: "claude-opus".to_string(),
            reedit_rate: Some(0.05),
            cost_per_line: Some(0.02),
        },
        ModelStats {
            model: "claude-haiku".to_string(),
            reedit_rate: Some(0.2),
            cost_per_line: Some(0.001),
        },
    ];
    let insight = model_insight(&models);
    assert!(insight.text.contains("opus"));
    assert!(insight.text.contains("haiku"));
}

#[test]
fn test_model_insight_empty() {
    let insight = model_insight(&[]);
    assert!(insight.text.contains("No model usage"));
}

// ========================================================================
// Learning curve insight tests
// ========================================================================

#[test]
fn test_learning_curve_significant_improvement() {
    let insight = learning_curve_insight(0.4, 0.2);
    assert!(insight.text.contains("50%"));
    assert!(insight.text.contains("improved significantly"));
    assert_eq!(insight.kind, InsightKind::Success);
}

#[test]
fn test_learning_curve_steady_improvement() {
    let insight = learning_curve_insight(0.3, 0.25);
    assert!(insight.text.contains("Steady improvement"));
}

#[test]
fn test_learning_curve_worsening() {
    let insight = learning_curve_insight(0.2, 0.3);
    assert!(insight.text.contains("increasing"));
    assert_eq!(insight.kind, InsightKind::Warning);
}

#[test]
fn test_learning_curve_no_data() {
    let insight = learning_curve_insight(0.0, 0.2);
    assert!(insight.text.contains("Not enough"));
}

// ========================================================================
// Branch insight tests
// ========================================================================

#[test]
fn test_branch_insight_high_ai() {
    let insight = branch_insight(Some(0.8), Some(0.9));
    assert!(insight.text.contains("High AI share"));
}

#[test]
fn test_branch_insight_low_ai() {
    let insight = branch_insight(Some(0.2), Some(0.5));
    assert!(insight.text.contains("Lower AI share"));
}

#[test]
fn test_branch_insight_balanced() {
    let insight = branch_insight(Some(0.5), Some(0.6));
    assert!(insight.text.contains("Balanced"));
}

// ========================================================================
// Skill insight tests
// ========================================================================

#[test]
fn test_skill_insight_significant_improvement() {
    let insight = skill_insight(0.1, 0.2);
    assert!(insight.text.contains("50%"));
    assert!(insight.text.contains("structured workflows"));
    assert_eq!(insight.kind, InsightKind::Success);
}

#[test]
fn test_skill_insight_modest() {
    let insight = skill_insight(0.18, 0.2);
    assert!(insight.text.contains("modest improvement"));
}

// ========================================================================
// Uncommitted insight tests
// ========================================================================

#[test]
fn test_uncommitted_insight_old() {
    let insight = uncommitted_insight(500, 1000, 72.0);
    assert!(insight.text.contains("3+ days"));
    assert_eq!(insight.kind, InsightKind::Warning);
}

#[test]
fn test_uncommitted_insight_high_percent() {
    let insight = uncommitted_insight(300, 1000, 12.0);
    assert!(insight.text.contains("30%"));
    assert_eq!(insight.kind, InsightKind::Warning);
}

#[test]
fn test_uncommitted_insight_small() {
    let insight = uncommitted_insight(50, 1000, 2.0);
    assert!(insight.text.contains("Small amount"));
}

#[test]
fn test_uncommitted_insight_none() {
    let insight = uncommitted_insight(0, 1000, 0.0);
    assert!(insight.text.contains("No uncommitted"));
}

// ========================================================================
// Efficiency insight tests
// ========================================================================

#[test]
fn test_efficiency_insight_improving() {
    let insight = efficiency_insight(Some(0.002), &[0.005, 0.004, 0.003, 0.002]);
    assert!(insight.text.contains("improving"));
    assert_eq!(insight.kind, InsightKind::Success);
}

#[test]
fn test_efficiency_insight_very_efficient() {
    let insight = efficiency_insight(Some(0.0005), &[]);
    assert!(insight.text.contains("Very cost-efficient"));
}

#[test]
fn test_efficiency_insight_good() {
    let insight = efficiency_insight(Some(0.005), &[]);
    assert!(insight.text.contains("Good cost efficiency"));
}

#[test]
fn test_efficiency_insight_unavailable() {
    let insight = efficiency_insight(None, &[]);
    assert!(insight.text.contains("unavailable"));
}
