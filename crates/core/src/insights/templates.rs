//! Template-based insight text generation.
//!
//! Each pattern has a title, body template, and optional recommendation template.
//! Templates use `{variable}` placeholders that are substituted at render time.

use std::collections::HashMap;

/// A template for generating human-readable insight text.
pub struct InsightTemplate {
    pub pattern_id: &'static str,
    pub title: &'static str,
    pub body_template: &'static str,
    pub recommendation_template: Option<&'static str>,
}

/// All insight templates, indexed by pattern ID.
pub static TEMPLATES: &[InsightTemplate] = &[
    // ============================
    // Prompt Patterns (P01-P10)
    // ============================
    InsightTemplate {
        pattern_id: "P01",
        title: "Optimal Prompt Length",
        body_template: "{optimal_range} word prompts have {improvement}% better first-attempt success rate than {worst_range} word prompts.",
        recommendation_template: Some("Try keeping prompts between {min_words} and {max_words} words for best results."),
    },
    InsightTemplate {
        pattern_id: "P03",
        title: "File Path Specificity",
        body_template: "Prompts referencing specific file paths have {improvement}% fewer re-edits.",
        recommendation_template: Some("Include file paths in your prompts when asking for edits."),
    },
    InsightTemplate {
        pattern_id: "P04",
        title: "Context Before Prompting",
        body_template: "Sessions where files were read first have {improvement}% better outcomes.",
        recommendation_template: Some("Read relevant files before asking for changes."),
    },
    InsightTemplate {
        pattern_id: "P10",
        title: "Follow-up Diminishing Returns",
        body_template: "Sessions with more than {threshold} follow-ups show diminishing returns ({reedit_rate}% re-edit rate).",
        recommendation_template: Some("Consider starting a fresh session after {threshold} turns."),
    },
    // ============================
    // Session Patterns (S01-S08)
    // ============================
    InsightTemplate {
        pattern_id: "S01",
        title: "Session Duration Sweet Spot",
        body_template: "Your {optimal_duration} sessions have {improvement}% lower re-edit rate than {worst_duration} sessions.",
        recommendation_template: Some("Consider aiming for {optimal_duration} sessions when possible."),
    },
    InsightTemplate {
        pattern_id: "S02",
        title: "Turn Count Sweet Spot",
        body_template: "{optimal_turns} turns per session yields the lowest re-edit rate ({reedit_rate}%).",
        recommendation_template: Some("Aim for {optimal_turns} turns per session for best results."),
    },
    InsightTemplate {
        pattern_id: "S04",
        title: "Session Fatigue",
        body_template: "Re-edit rate increases by {improvement}% after turn {threshold}.",
        recommendation_template: Some("Consider taking a break or starting fresh after turn {threshold}."),
    },
    InsightTemplate {
        pattern_id: "S08",
        title: "File Count Correlation",
        body_template: "Sessions editing {threshold} files have the highest re-edit rate ({improvement}% above the best bucket).",
        recommendation_template: Some("Aim for fewer files per session to reduce re-edits."),
    },
    // ============================
    // Temporal Patterns (T01-T07)
    // ============================
    InsightTemplate {
        pattern_id: "T01",
        title: "Peak Productivity Hours",
        body_template: "You're {improvement}% more efficient during {best_time} compared to {worst_time}.",
        recommendation_template: Some("Schedule complex tasks for {best_time} when possible."),
    },
    InsightTemplate {
        pattern_id: "T02",
        title: "Day of Week Patterns",
        body_template: "{best_day} is your most productive day; {worst_day} has the highest re-edit rate.",
        recommendation_template: None,
    },
    InsightTemplate {
        pattern_id: "T05",
        title: "Break Impact",
        body_template: "Sessions after a {days}+ day break have {improvement}% higher re-edit rate.",
        recommendation_template: Some("After long breaks, start with a warm-up session on familiar code."),
    },
    InsightTemplate {
        pattern_id: "T07",
        title: "Monthly Trend",
        body_template: "Your efficiency has {trend_direction} by {improvement}% month-over-month.",
        recommendation_template: None,
    },
    // ============================
    // Workflow Patterns (W01-W08)
    // ============================
    InsightTemplate {
        pattern_id: "W03",
        title: "Planning Before Execution",
        body_template: "Sessions with heavy planning (reading files first) have {improvement}% lower re-edit rate.",
        recommendation_template: Some("Spend time reading relevant code before jumping into edits."),
    },
    InsightTemplate {
        pattern_id: "W04",
        title: "Test-First Correlation",
        body_template: "Sessions that include test files have {improvement}% fewer re-edits.",
        recommendation_template: Some("Include test file edits in your sessions for better outcomes."),
    },
    InsightTemplate {
        pattern_id: "W05",
        title: "Commit Frequency",
        body_template: "Sessions with {commit_style} commits have {improvement}% better outcomes.",
        recommendation_template: Some("Commit more frequently during sessions."),
    },
    InsightTemplate {
        pattern_id: "W06",
        title: "Branch Discipline",
        body_template: "Feature branch sessions have {improvement}% lower re-edit rate than main branch.",
        recommendation_template: Some("Use feature branches for AI-assisted development."),
    },
    InsightTemplate {
        pattern_id: "W07",
        title: "Read Before Write",
        body_template: "Sessions with {read_level} reads have {improvement}% better edit outcomes.",
        recommendation_template: Some("Read more files before making changes."),
    },
    // ============================
    // Model Patterns (M01-M05)
    // ============================
    InsightTemplate {
        pattern_id: "M01",
        title: "Model Task Fit",
        body_template: "{best_model} has {improvement}% lower re-edit rate than {worst_model}. Note: models may be used for different task complexities.",
        recommendation_template: Some("Consider using {best_model} for similar tasks."),
    },
    InsightTemplate {
        pattern_id: "M05",
        title: "Model by Complexity",
        body_template: "For high complexity tasks, {best_model} outperforms by {improvement}%.",
        recommendation_template: Some("Use {best_model} for complex multi-file tasks."),
    },
    // ============================
    // Codebase Patterns (C01-C07)
    // ============================
    InsightTemplate {
        pattern_id: "C03",
        title: "Project Complexity",
        body_template: "Project {project_name} has {multiplier}x the average re-edit rate.",
        recommendation_template: None,
    },
    InsightTemplate {
        pattern_id: "C04",
        title: "New vs Existing Files",
        body_template: "Creating new files has {improvement}% better outcomes than modifying existing ones.",
        recommendation_template: None,
    },
    // ============================
    // Outcome Patterns (O01-O05)
    // ============================
    InsightTemplate {
        pattern_id: "O01",
        title: "Commit Rate",
        body_template: "{commit_pct}% of sessions result in commits.",
        recommendation_template: None,
    },
    InsightTemplate {
        pattern_id: "O02",
        title: "Session Mix",
        body_template: "{deep_work_pct}% of sessions are deep work, {quick_task_pct}% are quick tasks, {exploration_pct}% exploration, and {minimal_pct}% brief interactions.",
        recommendation_template: None,
    },
    // ============================
    // Behavioral Patterns (B01-B07)
    // ============================
    InsightTemplate {
        pattern_id: "B01",
        title: "Retry Patterns",
        body_template: "On average, {avg_reedits} re-edits per session when re-editing occurs.",
        recommendation_template: Some("If you hit 3+ re-edits, try rephrasing your prompt entirely."),
    },
    // B03 removed: commit_count==0 is not a reliable abandonment signal
    // ============================
    // Comparative Patterns (CP01-CP03)
    // ============================
    InsightTemplate {
        pattern_id: "CP01",
        title: "You vs Baseline",
        body_template: "Your re-edit rate has {direction} by {improvement}% compared to your 30-day baseline.",
        recommendation_template: None,
    },
];

/// Look up a template by pattern ID.
pub fn get_template(pattern_id: &str) -> Option<&'static InsightTemplate> {
    TEMPLATES.iter().find(|t| t.pattern_id == pattern_id)
}

/// Render a template string by substituting `{key}` placeholders with values.
///
/// Handles format specifiers like `{key:.0}`, `{key:.1}`, `{key:.2}`.
pub fn render_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        // Try plain replacement first
        let plain = format!("{{{}}}", key);
        if result.contains(&plain) {
            result = result.replace(&plain, value);
        }
        // Try format specifiers
        for precision in 0..=2 {
            let pattern = format!("{{{key}:.{precision}}}");
            if result.contains(&pattern) {
                let formatted = if let Ok(f) = value.parse::<f64>() {
                    match precision {
                        0 => format!("{:.0}", f),
                        1 => format!("{:.1}", f),
                        _ => format!("{:.2}", f),
                    }
                } else {
                    value.clone()
                };
                result = result.replace(&pattern, &formatted);
            }
        }
    }
    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_template_simple() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "world".to_string());
        assert_eq!(render_template("Hello, {name}!", &vars), "Hello, world!");
    }

    #[test]
    fn test_render_template_multiple_vars() {
        let mut vars = HashMap::new();
        vars.insert("best_time".to_string(), "morning".to_string());
        vars.insert("worst_time".to_string(), "evening".to_string());
        vars.insert("improvement".to_string(), "25".to_string());
        let result = render_template(
            "You're {improvement}% more efficient during {best_time} compared to {worst_time}.",
            &vars,
        );
        assert_eq!(result, "You're 25% more efficient during morning compared to evening.");
    }

    #[test]
    fn test_render_template_format_specifier() {
        let mut vars = HashMap::new();
        vars.insert("value".to_string(), "3.14159".to_string());
        assert_eq!(render_template("{value:.0}", &vars), "3");
        assert_eq!(render_template("{value:.1}", &vars), "3.1");
        assert_eq!(render_template("{value:.2}", &vars), "3.14");
    }

    #[test]
    fn test_render_template_missing_var() {
        let vars = HashMap::new();
        assert_eq!(render_template("Hello, {name}!", &vars), "Hello, {name}!");
    }

    #[test]
    fn test_get_template_exists() {
        assert!(get_template("P01").is_some());
        assert!(get_template("S01").is_some());
        assert!(get_template("T01").is_some());
    }

    #[test]
    fn test_get_template_missing() {
        assert!(get_template("NONEXISTENT").is_none());
    }

    #[test]
    fn test_all_templates_have_body() {
        for t in TEMPLATES {
            assert!(!t.body_template.is_empty(), "Template {} has empty body", t.pattern_id);
            assert!(!t.title.is_empty(), "Template {} has empty title", t.pattern_id);
        }
    }
}
