//! Impact scoring algorithm for pattern results.
//!
//! Each pattern is scored 0.0-1.0 based on three factors:
//! - Effect size (40% weight): How much improvement the pattern suggests
//! - Sample confidence (30% weight): Statistical confidence from observation count
//! - Actionability (30% weight): How easily the user can act on the insight

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How easily the user can act on an insight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actionability {
    /// User can change immediately (e.g., prompt length)
    Immediate,
    /// User can change with some effort (e.g., skill usage)
    Moderate,
    /// Awareness-only, hard to change (e.g., time of day)
    Awareness,
    /// Informational, no clear action (e.g., historical trend)
    Informational,
}

impl Actionability {
    /// Convert to a 0.0-1.0 score.
    pub fn score(self) -> f64 {
        match self {
            Actionability::Immediate => 1.0,
            Actionability::Moderate => 0.7,
            Actionability::Awareness => 0.4,
            Actionability::Informational => 0.2,
        }
    }
}

/// Scored pattern result with individual component scores.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PatternScore {
    /// Effect size score (0.0-1.0).
    pub effect_size: f64,
    /// Sample confidence score (0.0-1.0).
    pub sample_confidence: f64,
    /// Actionability score (0.0-1.0).
    pub actionability: f64,
    /// Weighted combination of all three scores (0.0-1.0).
    pub combined: f64,
}

impl PatternScore {
    /// Calculate a combined score from the three components.
    pub fn calculate(effect: f64, sample: f64, action: f64) -> Self {
        let combined = effect * 0.4 + sample * 0.3 + action * 0.3;
        Self {
            effect_size: effect,
            sample_confidence: sample,
            actionability: action,
            combined,
        }
    }

    /// Returns the impact tier based on the combined score.
    pub fn tier(&self) -> &'static str {
        if self.combined >= 0.7 {
            "high"
        } else if self.combined >= 0.4 {
            "medium"
        } else {
            "observation"
        }
    }
}

/// Calculate the effect size score from the relative difference between
/// a "better" value and a baseline.
///
/// Uses a Cohen's d-like interpretation:
/// - < 10%  -> small effect (maps to 0.0-0.2)
/// - 10-25% -> medium effect (maps to 0.2-0.5)
/// - > 25%  -> large effect (maps to 0.5-0.8+)
pub fn calculate_effect_size(relative_diff: f64) -> f64 {
    let d = relative_diff.abs();
    let raw = if d < 0.10 {
        d * 2.0 // 0.0 - 0.2
    } else if d < 0.25 {
        0.2 + (d - 0.10) * 2.0 // 0.2 - 0.5
    } else if d < 0.50 {
        0.5 + (d - 0.25) * 1.2 // 0.5 - 0.8
    } else {
        0.8 + (d - 0.50).min(0.2) // 0.8 - 1.0
    };
    raw.clamp(0.0, 1.0)
}

/// Calculate sample confidence from observation count vs minimum threshold.
///
/// Uses logarithmic scaling:
/// - threshold -> ~0.5
/// - 2x threshold -> ~0.75
/// - 5x threshold -> ~0.9
/// - 10x+ threshold -> ~1.0
pub fn calculate_sample_confidence(n: u32, threshold: u32) -> f64 {
    if n < threshold || threshold == 0 {
        return 0.0;
    }
    let ratio = n as f64 / threshold as f64;
    let ln_ratio = ratio.ln().max(0.0);
    (1.0 - (1.0 / (1.0 + ln_ratio))).clamp(0.0, 1.0)
}

/// Calculate a full pattern score from individual components.
pub fn calculate_pattern_score(
    relative_improvement: f64,
    sample_size: u32,
    min_sample_size: u32,
    actionability: Actionability,
) -> PatternScore {
    let effect = calculate_effect_size(relative_improvement);
    let sample = calculate_sample_confidence(sample_size, min_sample_size);
    let action = actionability.score();
    PatternScore::calculate(effect, sample, action)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actionability_scores() {
        assert_eq!(Actionability::Immediate.score(), 1.0);
        assert_eq!(Actionability::Moderate.score(), 0.7);
        assert_eq!(Actionability::Awareness.score(), 0.4);
        assert_eq!(Actionability::Informational.score(), 0.2);
    }

    #[test]
    fn test_effect_size_small() {
        let score = calculate_effect_size(0.05);
        assert!((score - 0.1).abs() < 0.01, "5% diff should be ~0.1, got {}", score);
    }

    #[test]
    fn test_effect_size_medium() {
        let score = calculate_effect_size(0.15);
        assert!(score > 0.2 && score < 0.5, "15% diff should be medium, got {}", score);
    }

    #[test]
    fn test_effect_size_large() {
        let score = calculate_effect_size(0.35);
        assert!(score > 0.5 && score < 0.9, "35% diff should be large, got {}", score);
    }

    #[test]
    fn test_effect_size_very_large() {
        let score = calculate_effect_size(0.80);
        assert!(score >= 0.8, "80% diff should be very large, got {}", score);
    }

    #[test]
    fn test_effect_size_zero() {
        assert_eq!(calculate_effect_size(0.0), 0.0);
    }

    #[test]
    fn test_effect_size_clamped() {
        let score = calculate_effect_size(2.0);
        assert!(score <= 1.0, "Should be clamped to 1.0, got {}", score);
    }

    #[test]
    fn test_sample_confidence_below_threshold() {
        assert_eq!(calculate_sample_confidence(10, 50), 0.0);
    }

    #[test]
    fn test_sample_confidence_at_threshold() {
        let score = calculate_sample_confidence(50, 50);
        // At exactly threshold, ln(1) = 0, so 1 - 1/(1+0) = 0
        assert!((score - 0.0).abs() < 0.01, "At threshold should be ~0, got {}", score);
    }

    #[test]
    fn test_sample_confidence_double_threshold() {
        let score = calculate_sample_confidence(100, 50);
        // ln(2) ~= 0.693, 1 - 1/(1+0.693) = 1 - 0.59 = 0.41
        assert!(score > 0.3 && score < 0.6, "2x threshold should be medium, got {}", score);
    }

    #[test]
    fn test_sample_confidence_large() {
        let score = calculate_sample_confidence(500, 50);
        assert!(score > 0.6, "10x threshold should be high, got {}", score);
    }

    #[test]
    fn test_sample_confidence_zero_threshold() {
        assert_eq!(calculate_sample_confidence(50, 0), 0.0);
    }

    #[test]
    fn test_pattern_score_combined() {
        let score = PatternScore::calculate(0.8, 0.6, 1.0);
        // 0.8*0.4 + 0.6*0.3 + 1.0*0.3 = 0.32 + 0.18 + 0.30 = 0.80
        assert!((score.combined - 0.80).abs() < 0.01);
    }

    #[test]
    fn test_pattern_score_tier_high() {
        let score = PatternScore::calculate(1.0, 1.0, 1.0);
        assert_eq!(score.tier(), "high");
    }

    #[test]
    fn test_pattern_score_tier_medium() {
        let score = PatternScore::calculate(0.5, 0.5, 0.5);
        assert_eq!(score.tier(), "medium");
    }

    #[test]
    fn test_pattern_score_tier_observation() {
        let score = PatternScore::calculate(0.1, 0.1, 0.1);
        assert_eq!(score.tier(), "observation");
    }

    #[test]
    fn test_calculate_pattern_score_full() {
        let score = calculate_pattern_score(0.25, 100, 30, Actionability::Immediate);
        assert!(score.combined > 0.0);
        assert!(score.effect_size > 0.0);
        assert!(score.sample_confidence > 0.0);
        assert_eq!(score.actionability, 1.0);
    }

    #[test]
    fn test_calculate_pattern_score_insufficient() {
        let score = calculate_pattern_score(0.25, 10, 30, Actionability::Immediate);
        // sample_confidence should be 0 because below threshold
        assert_eq!(score.sample_confidence, 0.0);
    }
}
