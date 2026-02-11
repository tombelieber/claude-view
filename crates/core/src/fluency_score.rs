// crates/core/src/fluency_score.rs
//! Pure math for computing the AI Fluency Score (0-100).
//!
//! The fluency score is a weighted composite of five sub-metrics,
//! each normalized to [0.0, 1.0]. Inputs are clamped so the final
//! score always lands in [0, 100].

use serde::Serialize;

/// Raw inputs for the fluency score calculation.
/// All values should be in [0.0, 1.0]; they are clamped internally.
#[derive(Debug, Clone)]
pub struct ScoreInput {
    /// Fraction of sessions that achieved their goal (0.0-1.0).
    pub achievement_rate: f64,
    /// Fraction of sessions with friction events (0.0-1.0).
    /// Inverted internally: lower friction = higher score contribution.
    pub friction_rate: f64,
    /// Cost efficiency metric (0.0-1.0). Placeholder for now.
    pub cost_efficiency: f64,
    /// Fraction of sessions with satisfied-or-above sentiment (0.0-1.0).
    pub satisfaction_trend: f64,
    /// Consistency metric (0.0-1.0). Placeholder for now.
    pub consistency: f64,
}

/// The computed fluency score plus the sub-metric breakdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FluencyScore {
    /// Composite score 0-100.
    pub score: i32,
    /// Achievement rate sub-metric (0.0-1.0).
    pub achievement_rate: f64,
    /// Friction rate sub-metric (0.0-1.0, lower = better).
    pub friction_rate: f64,
    /// Cost efficiency sub-metric (0.0-1.0).
    pub cost_efficiency: f64,
    /// Satisfaction trend sub-metric (0.0-1.0).
    pub satisfaction_trend: f64,
    /// Consistency sub-metric (0.0-1.0).
    pub consistency: f64,
    /// Number of sessions used to compute the score.
    pub sessions_analyzed: i64,
}

// Weight constants for each sub-metric.
const W_ACHIEVEMENT: f64 = 0.30;
const W_FRICTION: f64 = 0.25;
const W_COST: f64 = 0.20;
const W_SATISFACTION: f64 = 0.15;
const W_CONSISTENCY: f64 = 0.10;

/// Compute the fluency score from normalized inputs.
///
/// Each input is clamped to [0.0, 1.0]. Friction is inverted so that
/// lower friction yields a higher score. The result is an integer in [0, 100].
pub fn compute_fluency_score(input: &ScoreInput) -> i32 {
    let a = input.achievement_rate.clamp(0.0, 1.0);
    let f = 1.0 - input.friction_rate.clamp(0.0, 1.0); // invert: less friction = higher score
    let c = input.cost_efficiency.clamp(0.0, 1.0);
    let s = input.satisfaction_trend.clamp(0.0, 1.0);
    let k = input.consistency.clamp(0.0, 1.0);

    let raw =
        a * W_ACHIEVEMENT + f * W_FRICTION + c * W_COST + s * W_SATISFACTION + k * W_CONSISTENCY;

    (raw * 100.0).round().clamp(0.0, 100.0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_score_perfect() {
        let input = ScoreInput {
            achievement_rate: 1.0,
            friction_rate: 0.0, // no friction = best
            cost_efficiency: 1.0,
            satisfaction_trend: 1.0,
            consistency: 1.0,
        };
        assert_eq!(compute_fluency_score(&input), 100);
    }

    #[test]
    fn test_compute_score_terrible() {
        let input = ScoreInput {
            achievement_rate: 0.0,
            friction_rate: 1.0, // all friction = worst
            cost_efficiency: 0.0,
            satisfaction_trend: 0.0,
            consistency: 0.0,
        };
        assert_eq!(compute_fluency_score(&input), 0);
    }

    #[test]
    fn test_compute_score_mixed() {
        let input = ScoreInput {
            achievement_rate: 0.7,
            friction_rate: 0.3,
            cost_efficiency: 0.6,
            satisfaction_trend: 0.8,
            consistency: 0.5,
        };
        let score = compute_fluency_score(&input);
        assert!(score >= 50, "score {score} should be >= 50");
        assert!(score <= 80, "score {score} should be <= 80");
    }

    #[test]
    fn test_score_clamped() {
        // Inputs way out of range should still produce 0-100.
        let input = ScoreInput {
            achievement_rate: 5.0,
            friction_rate: -2.0,
            cost_efficiency: 10.0,
            satisfaction_trend: 999.0,
            consistency: -100.0,
        };
        let score = compute_fluency_score(&input);
        assert!(score >= 0, "score {score} should be >= 0");
        assert!(score <= 100, "score {score} should be <= 100");
    }

    #[test]
    fn test_score_weights_sum_to_one() {
        let sum = W_ACHIEVEMENT + W_FRICTION + W_COST + W_SATISFACTION + W_CONSISTENCY;
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "weights should sum to 1.0, got {sum}"
        );
    }

    #[test]
    fn test_fluency_score_serialization() {
        let score = FluencyScore {
            score: 72,
            achievement_rate: 0.7,
            friction_rate: 0.3,
            cost_efficiency: 0.6,
            satisfaction_trend: 0.8,
            consistency: 0.5,
            sessions_analyzed: 42,
        };
        let json = serde_json::to_string(&score).unwrap();
        assert!(json.contains("\"score\":72"));
        assert!(json.contains("\"sessionsAnalyzed\":42"));
        assert!(json.contains("\"achievementRate\":0.7"));
        assert!(json.contains("\"frictionRate\":0.3"));
    }
}
