// crates/server/src/insights/generators.rs
//! Insight generator functions for each metric category.

use super::types::Insight;

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
                Insight::success(format!(
                    "{:.0}% of sessions resulted in commits",
                    cr * 100.0
                ))
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
                .unwrap_or(f64::MAX)
                .total_cmp(&b.reedit_rate.unwrap_or(f64::MAX))
        });

    // Find cheapest model
    let cheapest = models
        .iter()
        .filter(|m| m.cost_per_line.is_some())
        .min_by(|a, b| {
            a.cost_per_line
                .unwrap_or(f64::MAX)
                .total_cmp(&b.cost_per_line.unwrap_or(f64::MAX))
        });

    match (best_by_reedit, cheapest) {
        (Some(best), Some(cheap)) if best.model == cheap.model => {
            Insight::success(format!("{} is both cheapest and most accurate", best.model))
        }
        (Some(best), Some(cheap)) => Insight::info(format!(
            "{} has lowest re-edits; {} is most cost-effective",
            best.model, cheap.model
        )),
        (Some(best), None) => Insight::info(format!("{} has lowest re-edit rate", best.model)),
        (None, Some(cheap)) => Insight::info(format!("{} is most cost-effective", cheap.model)),
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
