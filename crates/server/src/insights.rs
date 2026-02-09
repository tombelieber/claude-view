// crates/server/src/insights.rs
//! Insight generation for the contributions page.
//!
//! This module provides plain-English insights based on contribution metrics.
//! Insights help users understand their productivity patterns and identify
//! areas for improvement.
//!
//! ## Insight Categories
//!
//! - **Fluency**: How active the user has been compared to previous period
//! - **Output**: Productivity level and peak activity days
//! - **Effectiveness**: Quality metrics (commit rate, re-edit rate)
//! - **Model**: Which models perform best for different tasks
//! - **Learning Curve**: Improvement in prompting over time
//! - **Branch**: Human/AI contribution balance per branch
//! - **Skill**: Impact of using skills on output quality
//! - **Uncommitted**: Warnings about uncommitted work

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

/// A single insight with optional severity/type.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct Insight {
    /// The insight text (plain English).
    pub text: String,
    /// Insight type for styling (info, success, warning, tip).
    #[serde(default)]
    pub kind: InsightKind,
}

/// Insight severity/type for UI styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum InsightKind {
    /// Neutral informational insight
    #[default]
    Info,
    /// Positive/encouraging insight
    Success,
    /// Warning or area of concern
    Warning,
    /// Actionable suggestion
    Tip,
}

impl Insight {
    /// Create a new info insight.
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Info,
        }
    }

    /// Create a new success insight.
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Success,
        }
    }

    /// Create a new warning insight.
    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Warning,
        }
    }

    /// Create a new tip insight.
    pub fn tip(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Tip,
        }
    }
}

// ============================================================================
// Insight Generators
// ============================================================================

/// Generate fluency insight based on activity comparison.
///
/// Compares current period sessions to previous period.
pub fn fluency_insight(current: i64, previous: i64) -> Insight {
    if previous == 0 {
        if current > 0 {
            return Insight::info(format!("{} sessions this period", current));
        } else {
            return Insight::info("No activity this period");
        }
    }

    let delta_percent = ((current - previous) as f64 / previous as f64) * 100.0;

    if delta_percent > 10.0 {
        Insight::success(format!(
            "More active than last period (+{:.0}%)",
            delta_percent
        ))
    } else if delta_percent < -10.0 {
        Insight::info(format!(
            "Less active than last period ({:.0}%)",
            delta_percent
        ))
    } else {
        Insight::info("Consistent activity level")
    }
}

/// Generate output insight based on lines produced.
pub fn output_insight(lines: i64, peak_day: Option<(&str, i64)>) -> Insight {
    match (lines, peak_day) {
        (l, Some((day, peak_lines))) if l > 1000 => Insight::success(format!(
            "Highly productive - {} was peak ({} lines)",
            day, peak_lines
        )),
        (l, Some((day, _))) if l > 500 => {
            Insight::info(format!("Good output - {} was most active", day))
        }
        (l, _) if l > 100 => Insight::info("Light AI usage this period"),
        _ => Insight::info("Minimal AI contribution this period"),
    }
}

/// Generate effectiveness insight based on commit rate and re-edit rate.
pub fn effectiveness_insight(commit_rate: Option<f64>, reedit_rate: Option<f64>) -> Insight {
    match (commit_rate, reedit_rate) {
        (Some(cr), Some(rr)) if cr > 0.8 && rr < 0.2 => {
            Insight::success("Excellent - high commit rate, low re-edits")
        }
        (_, Some(rr)) if rr > 0.35 => {
            Insight::warning("High re-edit rate - try more specific prompts")
        }
        (Some(cr), _) if cr < 0.5 => {
            Insight::tip("Low commit rate - AI output may need more guidance")
        }
        (Some(_), Some(_)) => Insight::info("Good balance of quality and throughput"),
        (Some(cr), None) => {
            if cr > 0.7 {
                Insight::success(format!("{:.0}% of sessions resulted in commits", cr * 100.0))
            } else {
                Insight::info(format!("{:.0}% commit rate", cr * 100.0))
            }
        }
        (None, Some(rr)) => {
            if rr < 0.2 {
                Insight::success(format!("{:.0}% re-edit rate", rr * 100.0))
            } else {
                Insight::info(format!("{:.0}% re-edit rate", rr * 100.0))
            }
        }
        (None, None) => Insight::info("Not enough data for effectiveness metrics"),
    }
}

/// Model comparison insight.
#[derive(Debug, Clone)]
pub struct ModelStats {
    pub model: String,
    pub reedit_rate: Option<f64>,
    pub cost_per_line: Option<f64>,
}

/// Generate model comparison insight.
pub fn model_insight(models: &[ModelStats]) -> Insight {
    if models.is_empty() {
        return Insight::info("No model usage data available");
    }

    // Find best model by re-edit rate (lower is better)
    let best_by_reedit = models
        .iter()
        .filter(|m| m.reedit_rate.is_some())
        .min_by(|a, b| {
            a.reedit_rate
                .partial_cmp(&b.reedit_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    // Find cheapest model
    let cheapest = models
        .iter()
        .filter(|m| m.cost_per_line.is_some())
        .min_by(|a, b| {
            a.cost_per_line
                .partial_cmp(&b.cost_per_line)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    match (best_by_reedit, cheapest) {
        (Some(best), Some(cheap)) if best.model == cheap.model => {
            Insight::success(format!(
                "{} is both cheapest and most accurate",
                best.model
            ))
        }
        (Some(best), Some(cheap)) => Insight::info(format!(
            "{} has lowest re-edits; {} is most cost-effective",
            best.model, cheap.model
        )),
        (Some(best), None) => {
            Insight::info(format!("{} has lowest re-edit rate", best.model))
        }
        (None, Some(cheap)) => {
            Insight::info(format!("{} is most cost-effective", cheap.model))
        }
        (None, None) => Insight::info("Multiple models used this period"),
    }
}

/// Generate learning curve insight.
///
/// Compares re-edit rate at start vs current.
pub fn learning_curve_insight(start_reedit_rate: f64, current_reedit_rate: f64) -> Insight {
    if start_reedit_rate == 0.0 {
        return Insight::info("Not enough historical data for learning curve");
    }

    let improvement = ((start_reedit_rate - current_reedit_rate) / start_reedit_rate) * 100.0;

    if improvement > 30.0 {
        Insight::success(format!(
            "Re-edit rate dropped {:.0}% - your prompting has improved significantly",
            improvement
        ))
    } else if improvement > 10.0 {
        Insight::success("Steady improvement in prompt accuracy")
    } else if improvement < -10.0 {
        Insight::warning("Re-edit rate increasing - consider reviewing prompt patterns")
    } else {
        Insight::info("Consistent prompting quality")
    }
}

/// Generate branch insight based on AI share and commit rate.
pub fn branch_insight(ai_share: Option<f64>, commit_rate: Option<f64>) -> Insight {
    match (ai_share, commit_rate) {
        (Some(ai), Some(cr)) if ai > 0.7 && cr > 0.8 => {
            Insight::info("High AI share + high commit rate - AI doing heavy lifting here")
        }
        (Some(ai), _) if ai < 0.3 => {
            Insight::info("Lower AI share - likely more manual investigation/debugging")
        }
        _ => Insight::info("Balanced human/AI contribution"),
    }
}

/// Generate skill usage insight.
pub fn skill_insight(with_skill_reedit: f64, without_skill_reedit: f64) -> Insight {
    if without_skill_reedit == 0.0 {
        return Insight::info("Skill usage detected");
    }

    let improvement = ((without_skill_reedit - with_skill_reedit) / without_skill_reedit) * 100.0;

    if improvement > 30.0 {
        Insight::success(format!(
            "Sessions with skills have {:.0}% lower re-edit rate - structured workflows help",
            improvement
        ))
    } else if improvement > 10.0 {
        Insight::info("Skills provide modest improvement to output quality")
    } else {
        Insight::info("Similar quality with or without skills")
    }
}

/// Generate uncommitted work insight.
pub fn uncommitted_insight(
    uncommitted_lines: i64,
    total_lines: i64,
    hours_since_activity: f64,
) -> Insight {
    if uncommitted_lines == 0 {
        return Insight::info("No uncommitted work");
    }

    if hours_since_activity > 24.0 {
        let days = (hours_since_activity / 24.0).floor() as i64;
        return Insight::warning(format!(
            "{} lines uncommitted for {}+ days - consider committing",
            uncommitted_lines, days
        ));
    }

    if total_lines > 0 {
        let pct = (uncommitted_lines as f64 / total_lines as f64) * 100.0;
        if pct > 20.0 {
            return Insight::warning(format!(
                "{:.0}% of recent work uncommitted - commit often to avoid losing work",
                pct
            ));
        }
    }

    Insight::info("Small amount of uncommitted work")
}

/// Generate efficiency insight based on cost metrics.
pub fn efficiency_insight(cost_per_line: Option<f64>, cost_trend: &[f64]) -> Insight {
    if let Some(cpl) = cost_per_line {
        // Check if cost is improving (decreasing)
        if cost_trend.len() >= 2 {
            let recent = cost_trend.last().unwrap_or(&0.0);
            let first = cost_trend.first().unwrap_or(&0.0);

            if recent < first && *first > 0.0 {
                let improvement = ((first - recent) / first) * 100.0;
                return Insight::success(format!(
                    "Cost efficiency improving ({:.0}% better than start)",
                    improvement
                ));
            }
        }

        // Generic cost insight
        if cpl < 0.001 {
            Insight::success("Very cost-efficient AI usage")
        } else if cpl < 0.01 {
            Insight::info("Good cost efficiency")
        } else {
            Insight::info(format!("${:.4} per line of AI output", cpl))
        }
    } else {
        Insight::info("Cost data unavailable")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
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
}
