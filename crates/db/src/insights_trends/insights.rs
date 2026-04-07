//! Insight generation helpers (pure computation, no I/O).

use super::types::{CategoryDataPoint, HeatmapCell, MetricDataPoint};

/// Calculate trend statistics from data points.
pub fn calculate_trend_stats(data: &[MetricDataPoint], metric: &str) -> (f64, f64, String) {
    if data.is_empty() {
        return (0.0, 0.0, "stable".to_string());
    }

    let average = data.iter().map(|d| d.value).sum::<f64>() / data.len() as f64;

    if data.len() < 2 {
        return (average, 0.0, "stable".to_string());
    }

    let first = data.first().map(|d| d.value).unwrap_or(0.0);
    let last = data.last().map(|d| d.value).unwrap_or(0.0);

    let trend = if first == 0.0 {
        0.0
    } else {
        ((last - first) / first) * 100.0
    };

    // For reedit_rate and cost_per_line, lower is better
    let is_lower_better = metric == "reedit_rate" || metric == "cost_per_line";
    let direction = if trend.abs() < 5.0 {
        "stable"
    } else if (trend < 0.0) == is_lower_better {
        "improving"
    } else {
        "worsening"
    };

    (average, trend, direction.to_string())
}

/// Generate a human-readable insight for the selected metric.
pub fn generate_metric_insight(metric: &str, trend: f64, range: &str) -> String {
    let range_text = match range {
        "3mo" => "3 months",
        "6mo" => "6 months",
        "1yr" => "1 year",
        "all" => "all time",
        _ => "the selected period",
    };

    match metric {
        "reedit_rate" if trend < -20.0 => {
            format!(
                "Your re-edit rate dropped {:.0}% over {} -- you're writing significantly better prompts that produce correct code first try",
                trend.abs(),
                range_text
            )
        }
        "reedit_rate" if trend > 20.0 => {
            format!(
                "Your re-edit rate increased {:.0}% over {} -- consider being more specific in your prompts",
                trend, range_text
            )
        }
        "sessions" if trend > 50.0 => {
            format!(
                "Your session count grew {:.0}% over {} -- you're using AI assistance more frequently",
                trend, range_text
            )
        }
        "prompts" if trend < -20.0 => {
            format!(
                "Your prompts per session dropped {:.0}% over {} -- you're getting results faster",
                trend.abs(),
                range_text
            )
        }
        _ => format!(
            "Your {} changed by {:.0}% over {}",
            metric.replace('_', " "),
            trend,
            range_text
        ),
    }
}

/// Generate a human-readable insight for category evolution.
pub fn generate_category_insight(data: &[CategoryDataPoint]) -> String {
    if data.len() < 2 {
        return "Not enough data to determine category trends".to_string();
    }

    let first = &data[0];
    let last = &data[data.len() - 1];

    let thinking_change = ((last.thinking_work - first.thinking_work) * 100.0).round() as i32;

    if thinking_change > 5 {
        format!(
            "Thinking Work increased from {:.0}% to {:.0}% -- you're doing more planning before coding (correlates with lower re-edit rate)",
            first.thinking_work * 100.0,
            last.thinking_work * 100.0
        )
    } else if thinking_change < -5 {
        format!(
            "Thinking Work decreased from {:.0}% to {:.0}% -- consider more upfront planning to reduce re-edits",
            first.thinking_work * 100.0,
            last.thinking_work * 100.0
        )
    } else {
        format!(
            "Work distribution is stable: {:.0}% Code, {:.0}% Support, {:.0}% Thinking",
            last.code_work * 100.0,
            last.support_work * 100.0,
            last.thinking_work * 100.0
        )
    }
}

/// Generate a human-readable insight for the activity heatmap.
pub fn generate_heatmap_insight(data: &[HeatmapCell]) -> String {
    if data.is_empty() {
        return "Not enough activity data to determine patterns".to_string();
    }

    let min_sessions: i64 = 5;
    let best_slots: Vec<&HeatmapCell> =
        data.iter().filter(|c| c.sessions >= min_sessions).collect();

    if best_slots.is_empty() {
        return "Build more history to see your peak productivity times".to_string();
    }

    let best = best_slots
        .iter()
        .min_by(|a, b| a.avg_reedit_rate.total_cmp(&b.avg_reedit_rate))
        .expect("best_slots guaranteed non-empty by is_empty check above");

    let worst = best_slots
        .iter()
        .max_by(|a, b| a.avg_reedit_rate.total_cmp(&b.avg_reedit_rate))
        .expect("best_slots guaranteed non-empty by is_empty check above");

    let days = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let efficiency_diff = if worst.avg_reedit_rate > 0.0 {
        ((worst.avg_reedit_rate - best.avg_reedit_rate) / worst.avg_reedit_rate * 100.0).round()
            as i32
    } else {
        0
    };

    if efficiency_diff > 20 {
        format!(
            "{} {}:00 is your sweet spot -- {:.0}% better efficiency than {} sessions",
            days[best.day_of_week as usize],
            best.hour_of_day,
            efficiency_diff,
            if worst.hour_of_day >= 18 {
                "evening"
            } else {
                "other"
            }
        )
    } else {
        format!(
            "Your productivity is consistent across the week (+/-{:.0}% variation)",
            efficiency_diff
        )
    }
}
