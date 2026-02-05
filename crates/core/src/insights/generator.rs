//! Insight text generator: converts raw pattern results into human-readable insights.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::scoring::{calculate_pattern_score, Actionability};
use super::templates::{get_template, render_template};

/// A generated insight with human-readable text and scoring.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct GeneratedInsight {
    pub pattern_id: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub recommendation: Option<String>,
    pub impact_score: f64,
    pub impact_tier: String,
    pub evidence: InsightEvidence,
}

/// Evidence backing an insight.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InsightEvidence {
    pub sample_size: u32,
    pub time_range_days: u32,
    #[ts(type = "Record<string, number>")]
    pub comparison_values: HashMap<String, f64>,
}

/// Build a GeneratedInsight from raw pattern data.
///
/// Returns `None` if the template cannot be found or required variables are missing.
pub fn generate_insight(
    pattern_id: &str,
    category: &str,
    vars: &HashMap<String, String>,
    sample_size: u32,
    min_sample_size: u32,
    time_range_days: u32,
    relative_improvement: f64,
    actionability: Actionability,
    comparison_values: HashMap<String, f64>,
) -> Option<GeneratedInsight> {
    let template = get_template(pattern_id)?;

    let body = render_template(template.body_template, vars);
    let recommendation = template
        .recommendation_template
        .map(|t| render_template(t, vars));

    let score = calculate_pattern_score(
        relative_improvement,
        sample_size,
        min_sample_size,
        actionability,
    );

    Some(GeneratedInsight {
        pattern_id: pattern_id.to_string(),
        category: category.to_string(),
        title: template.title.to_string(),
        body,
        recommendation,
        impact_score: score.combined,
        impact_tier: score.tier().to_string(),
        evidence: InsightEvidence {
            sample_size,
            time_range_days,
            comparison_values,
        },
    })
}

/// Sort insights by impact score descending.
pub fn sort_by_impact(insights: &mut [GeneratedInsight]) {
    insights.sort_by(|a, b| {
        b.impact_score
            .partial_cmp(&a.impact_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Group insights by impact tier.
pub fn group_by_tier(insights: &[GeneratedInsight]) -> (Vec<&GeneratedInsight>, Vec<&GeneratedInsight>, Vec<&GeneratedInsight>) {
    let high: Vec<_> = insights.iter().filter(|i| i.impact_tier == "high").collect();
    let medium: Vec<_> = insights.iter().filter(|i| i.impact_tier == "medium").collect();
    let observations: Vec<_> = insights.iter().filter(|i| i.impact_tier == "observation").collect();
    (high, medium, observations)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_insight_basic() {
        let mut vars = HashMap::new();
        vars.insert("optimal_duration".to_string(), "15-45min".to_string());
        vars.insert("worst_duration".to_string(), ">90min".to_string());
        vars.insert("improvement".to_string(), "45".to_string());

        let insight = generate_insight(
            "S01",
            "Session Patterns",
            &vars,
            100,
            50,
            30,
            0.45,
            Actionability::Moderate,
            HashMap::new(),
        );

        assert!(insight.is_some());
        let insight = insight.unwrap();
        assert_eq!(insight.pattern_id, "S01");
        assert_eq!(insight.category, "Session Patterns");
        assert_eq!(insight.title, "Session Duration Sweet Spot");
        assert!(insight.body.contains("15-45min"));
        assert!(insight.body.contains(">90min"));
        assert!(insight.recommendation.is_some());
        assert!(insight.impact_score > 0.0);
    }

    #[test]
    fn test_generate_insight_missing_template() {
        let vars = HashMap::new();
        let insight = generate_insight(
            "NONEXISTENT",
            "Test",
            &vars,
            100,
            50,
            30,
            0.5,
            Actionability::Immediate,
            HashMap::new(),
        );
        assert!(insight.is_none());
    }

    #[test]
    fn test_sort_by_impact() {
        let mut insights = vec![
            GeneratedInsight {
                pattern_id: "A".into(),
                category: "Test".into(),
                title: "Low".into(),
                body: "".into(),
                recommendation: None,
                impact_score: 0.2,
                impact_tier: "observation".into(),
                evidence: InsightEvidence {
                    sample_size: 10,
                    time_range_days: 30,
                    comparison_values: HashMap::new(),
                },
            },
            GeneratedInsight {
                pattern_id: "B".into(),
                category: "Test".into(),
                title: "High".into(),
                body: "".into(),
                recommendation: None,
                impact_score: 0.8,
                impact_tier: "high".into(),
                evidence: InsightEvidence {
                    sample_size: 100,
                    time_range_days: 30,
                    comparison_values: HashMap::new(),
                },
            },
        ];

        sort_by_impact(&mut insights);
        assert_eq!(insights[0].pattern_id, "B");
        assert_eq!(insights[1].pattern_id, "A");
    }

    #[test]
    fn test_group_by_tier() {
        let insights = vec![
            GeneratedInsight {
                pattern_id: "H".into(),
                category: "Test".into(),
                title: "".into(),
                body: "".into(),
                recommendation: None,
                impact_score: 0.8,
                impact_tier: "high".into(),
                evidence: InsightEvidence {
                    sample_size: 100,
                    time_range_days: 30,
                    comparison_values: HashMap::new(),
                },
            },
            GeneratedInsight {
                pattern_id: "M".into(),
                category: "Test".into(),
                title: "".into(),
                body: "".into(),
                recommendation: None,
                impact_score: 0.5,
                impact_tier: "medium".into(),
                evidence: InsightEvidence {
                    sample_size: 50,
                    time_range_days: 30,
                    comparison_values: HashMap::new(),
                },
            },
            GeneratedInsight {
                pattern_id: "O".into(),
                category: "Test".into(),
                title: "".into(),
                body: "".into(),
                recommendation: None,
                impact_score: 0.2,
                impact_tier: "observation".into(),
                evidence: InsightEvidence {
                    sample_size: 20,
                    time_range_days: 30,
                    comparison_values: HashMap::new(),
                },
            },
        ];

        let (high, medium, obs) = group_by_tier(&insights);
        assert_eq!(high.len(), 1);
        assert_eq!(medium.len(), 1);
        assert_eq!(obs.len(), 1);
    }
}
