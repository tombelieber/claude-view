// crates/core/src/phase/mod.rs
//! Sliding-window SDLC phase classifier for coding sessions.
//!
//! Classifies windows of assistant tool-use steps into one of 8 SDLC phases.
//! Uses a weighted heuristic with exponential decay (recent steps weighted higher).
//!
//! ## Algorithm
//!
//! Each step in the window contributes weighted votes to phase scores.
//! Weight = DECAY^(age), where age=0 is the newest step.
//! Signal types: skills (5x), agents (2x), bash patterns (2-4x), tool ratios (1-1.5x), keywords (1.5x).
//! Winner = highest normalized score above MIN_CONFIDENCE threshold.

pub mod matchers;
pub use matchers::*;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

/// SDLC session phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum SessionPhase {
    Planning,
    Exploring,
    Implementing,
    Debugging,
    Testing,
    Reviewing,
    Configuring,
    Releasing,
    Unknown,
}

impl SessionPhase {
    pub const ALL: [SessionPhase; 8] = [
        Self::Planning,
        Self::Exploring,
        Self::Implementing,
        Self::Debugging,
        Self::Testing,
        Self::Reviewing,
        Self::Configuring,
        Self::Releasing,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Exploring => "exploring",
            Self::Implementing => "implementing",
            Self::Debugging => "debugging",
            Self::Testing => "testing",
            Self::Reviewing => "reviewing",
            Self::Configuring => "configuring",
            Self::Releasing => "releasing",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "planning" => Some(Self::Planning),
            "exploring" => Some(Self::Exploring),
            "implementing" => Some(Self::Implementing),
            "debugging" => Some(Self::Debugging),
            "testing" => Some(Self::Testing),
            "reviewing" => Some(Self::Reviewing),
            "configuring" => Some(Self::Configuring),
            "releasing" => Some(Self::Releasing),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Planning => "Planning",
            Self::Exploring => "Exploring",
            Self::Implementing => "Implementing",
            Self::Debugging => "Debugging",
            Self::Testing => "Testing",
            Self::Reviewing => "Reviewing",
            Self::Configuring => "Configuring",
            Self::Releasing => "Releasing",
            Self::Unknown => "Unknown",
        }
    }

    fn index(&self) -> usize {
        match self {
            Self::Planning => 0,
            Self::Exploring => 1,
            Self::Implementing => 2,
            Self::Debugging => 3,
            Self::Testing => 4,
            Self::Reviewing => 5,
            Self::Configuring => 6,
            Self::Releasing => 7,
            Self::Unknown => 8,
        }
    }
}

impl std::fmt::Display for SessionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Classification result for a single window.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PhaseLabel {
    pub phase: SessionPhase,
    pub confidence: f64,
    pub secondary: Option<SessionPhase>,
    pub window_size: u32,
}

/// Full phase history for a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PhaseHistory {
    pub current: Option<PhaseLabel>,
    pub labels: Vec<PhaseLabel>,
    pub dominant: Option<SessionPhase>,
}

// ============================================================================
// Step Signals (input to classifier)
// ============================================================================

/// Signal vector for a single step, extracted from JSONL tool-use blocks.
#[derive(Debug, Clone, Default)]
pub struct StepSignals {
    pub is_user_prompt: bool,

    // Tool counts
    pub edit_count: u32,
    pub write_count: u32,
    pub read_count: u32,
    pub glob_count: u32,
    pub grep_count: u32,
    pub bash_count: u32,
    pub agent_count: u32,
    pub skill_count: u32,

    // Skill type flags
    pub has_plan_skill: bool,
    pub has_review_skill: bool,
    pub has_test_skill: bool,
    pub has_ship_skill: bool,
    pub has_debug_skill: bool,
    pub has_config_skill: bool,
    pub has_impl_skill: bool,
    pub has_explore_skill: bool,

    // Agent type flags
    pub has_plan_agent: bool,
    pub has_review_agent: bool,
    pub has_explore_agent: bool,

    // Bash command flags
    pub has_test_cmd: bool,
    pub has_build_cmd: bool,
    pub has_git_push: bool,
    pub has_publish_cmd: bool,
    pub has_deploy_cmd: bool,
    pub has_install_cmd: bool,

    // Prompt keyword flags
    pub prompt_plan_kw: bool,
    pub prompt_impl_kw: bool,
    pub prompt_fix_kw: bool,
    pub prompt_review_kw: bool,
    pub prompt_test_kw: bool,
    pub prompt_release_kw: bool,
    pub prompt_config_kw: bool,
    pub prompt_explore_kw: bool,

    // File type counts
    pub config_files_edited: u32,
    pub test_files_edited: u32,
    pub doc_files_edited: u32,
}

// ============================================================================
// Classifier
// ============================================================================

/// Tunable classifier parameters.
pub struct PhaseClassifierConfig {
    /// Exponential decay factor per step age (0.85 = 20% weight at age 10).
    pub decay_factor: f64,
    /// Minimum confidence to emit a phase (below this → Unknown).
    pub min_confidence: f64,
    /// Number of steps in the sliding window.
    pub window_size: usize,
}

impl Default for PhaseClassifierConfig {
    fn default() -> Self {
        Self {
            decay_factor: 0.85,
            min_confidence: 0.25,
            window_size: 10,
        }
    }
}

/// Classify a window of steps into a phase.
pub fn classify_window(steps: &[StepSignals], config: &PhaseClassifierConfig) -> PhaseLabel {
    let n = steps.len();
    if n == 0 {
        return PhaseLabel {
            phase: SessionPhase::Unknown,
            confidence: 0.0,
            secondary: None,
            window_size: 0,
        };
    }

    // Accumulate weighted scores with exponential decay
    let mut scores = [0.0f64; 8]; // indexed by SessionPhase::ALL
    let mut total_weight = 0.0f64;

    for (i, step) in steps.iter().enumerate() {
        let age = (n - 1 - i) as f64;
        let w = config.decay_factor.powf(age);
        total_weight += w;

        // Skill signals (5.0 weight — skills are category-defining, must dominate)
        if step.has_ship_skill {
            scores[SessionPhase::Releasing.index()] += w * 5.0;
        }
        if step.has_review_skill {
            scores[SessionPhase::Reviewing.index()] += w * 5.0;
        }
        if step.has_debug_skill {
            scores[SessionPhase::Debugging.index()] += w * 5.0;
        }
        if step.has_test_skill {
            scores[SessionPhase::Testing.index()] += w * 4.0;
        }
        if step.has_plan_skill {
            scores[SessionPhase::Planning.index()] += w * 5.0;
        }
        if step.has_config_skill {
            scores[SessionPhase::Configuring.index()] += w * 4.0;
        }
        if step.has_impl_skill {
            scores[SessionPhase::Implementing.index()] += w * 4.0;
        }
        if step.has_explore_skill {
            scores[SessionPhase::Exploring.index()] += w * 3.0;
        }

        // Agent type signals (1.5–2.0)
        if step.has_plan_agent {
            scores[SessionPhase::Planning.index()] += w * 2.0;
        }
        if step.has_review_agent {
            scores[SessionPhase::Reviewing.index()] += w * 2.0;
        }
        if step.has_explore_agent {
            scores[SessionPhase::Exploring.index()] += w * 1.5;
        }

        // Bash command signals
        // TDD check: test_cmd in a heavy-edit step → impl, not testing
        if step.has_test_cmd {
            let edit_w = step.edit_count + step.write_count;
            if edit_w > 2 {
                // TDD pattern: more edits than tests → impl
                scores[SessionPhase::Implementing.index()] += w * 1.5;
                scores[SessionPhase::Testing.index()] += w * 0.5;
            } else {
                scores[SessionPhase::Testing.index()] += w * 2.0;
            }
        }
        if step.has_build_cmd {
            scores[SessionPhase::Implementing.index()] += w * 1.0;
        }
        // Release signals are definitive (highest non-skill weight)
        if step.has_git_push {
            scores[SessionPhase::Releasing.index()] += w * 3.5;
        }
        if step.has_publish_cmd {
            scores[SessionPhase::Releasing.index()] += w * 4.0;
        }
        if step.has_deploy_cmd {
            scores[SessionPhase::Releasing.index()] += w * 4.0;
        }
        if step.has_install_cmd {
            scores[SessionPhase::Configuring.index()] += w * 2.0;
        }

        // Tool ratio signals
        let edit_w = step.edit_count + step.write_count;
        let read_w = step.read_count + step.glob_count + step.grep_count;

        if edit_w > 0 {
            let cfg_pct = step.config_files_edited as f64 / edit_w.max(1) as f64;
            let tst_pct = step.test_files_edited as f64 / edit_w.max(1) as f64;

            let doc_pct = step.doc_files_edited as f64 / edit_w.max(1) as f64;

            if cfg_pct > 0.7 {
                scores[SessionPhase::Configuring.index()] += w * 1.5;
            } else if tst_pct > 0.5 {
                scores[SessionPhase::Testing.index()] += w * 1.5;
            } else if doc_pct > 0.5 {
                scores[SessionPhase::Configuring.index()] += w * 0.5;
            } else {
                scores[SessionPhase::Implementing.index()] += w * 1.2;
            }
        }

        if read_w > 0 && edit_w == 0 {
            scores[SessionPhase::Exploring.index()] += w * 1.5;
        }

        // Generic bash (no specific pattern matched) → weak impl signal
        if step.bash_count > 0
            && !step.has_test_cmd
            && !step.has_build_cmd
            && !step.has_git_push
            && !step.has_publish_cmd
            && !step.has_deploy_cmd
            && !step.has_install_cmd
        {
            scores[SessionPhase::Implementing.index()] += w * 0.5;
        }

        // User prompt keywords (1.5x)
        if step.is_user_prompt {
            let kw = w * 1.5;
            if step.prompt_plan_kw {
                scores[SessionPhase::Planning.index()] += kw;
            }
            if step.prompt_impl_kw {
                scores[SessionPhase::Implementing.index()] += kw;
            }
            if step.prompt_fix_kw {
                scores[SessionPhase::Debugging.index()] += kw;
            }
            if step.prompt_review_kw {
                scores[SessionPhase::Reviewing.index()] += kw;
            }
            if step.prompt_test_kw {
                scores[SessionPhase::Testing.index()] += kw;
            }
            if step.prompt_release_kw {
                scores[SessionPhase::Releasing.index()] += kw;
            }
            if step.prompt_config_kw {
                scores[SessionPhase::Configuring.index()] += kw;
            }
            if step.prompt_explore_kw {
                scores[SessionPhase::Exploring.index()] += kw;
            }
        }
    }

    // Skill dominance: skills are explicit user actions — they get a flat bonus
    // that doesn't depend on window position. This ensures a ship skill in any
    // step can overcome 9 steps of mundane edit activity.
    let skill_bonus = total_weight * 2.0; // bonus = 2x the full window weight
    for step in steps {
        if step.has_ship_skill {
            scores[SessionPhase::Releasing.index()] += skill_bonus;
        }
        if step.has_review_skill {
            scores[SessionPhase::Reviewing.index()] += skill_bonus;
        }
        if step.has_debug_skill {
            scores[SessionPhase::Debugging.index()] += skill_bonus;
        }
        if step.has_plan_skill {
            scores[SessionPhase::Planning.index()] += skill_bonus;
        }
        // Lower bonus for less definitive skills
        if step.has_test_skill {
            scores[SessionPhase::Testing.index()] += skill_bonus * 0.8;
        }
        if step.has_config_skill {
            scores[SessionPhase::Configuring.index()] += skill_bonus * 0.8;
        }
        if step.has_impl_skill {
            scores[SessionPhase::Implementing.index()] += skill_bonus * 0.8;
        }
        if step.has_explore_skill {
            scores[SessionPhase::Exploring.index()] += skill_bonus * 0.5;
        }
    }

    // Context adjustment: explore agents inside impl sessions → impl
    let impl_idx = SessionPhase::Implementing.index();
    let explore_idx = SessionPhase::Exploring.index();
    if scores[impl_idx] > 0.0 && scores[explore_idx] > 0.0 {
        let has_impl_skill = steps.iter().any(|s| s.has_impl_skill);
        if has_impl_skill {
            let transfer = scores[explore_idx] * 0.5;
            scores[impl_idx] += transfer;
            scores[explore_idx] -= transfer;
        }
    }

    // Normalize by total weight
    if total_weight > 0.0 {
        for score in &mut scores {
            *score /= total_weight;
        }
    }

    // Find winner and runner-up
    let mut sorted: Vec<(usize, f64)> = scores.iter().copied().enumerate().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let (winner_idx, winner_score) = sorted[0];
    let runner: Option<(usize, f64)> = if sorted.len() > 1 {
        Some(sorted[1])
    } else {
        None
    };
    let runner_score = runner.map_or(0.0, |(_, s)| s);

    if winner_score == 0.0 {
        return PhaseLabel {
            phase: SessionPhase::Unknown,
            confidence: 0.0,
            secondary: None,
            window_size: n as u32,
        };
    }

    // Confidence calculation
    let signal_types = scores.iter().filter(|&&s| s > 0.0).count();
    let density = (signal_types as f64 / 3.0).min(1.0);
    let agreement = (1.0 + 0.1 * (signal_types.saturating_sub(1) as f64)).min(1.3);
    let base_conf = (winner_score / (winner_score + runner_score).max(0.01)).min(1.0);
    let confidence = (base_conf * density * agreement).min(1.0);

    // Secondary phase if runner > 60% of winner
    let secondary = runner
        .filter(|(_, s)| *s > 0.6 * winner_score)
        .and_then(|(idx, _)| SessionPhase::ALL.get(idx).copied());

    let phase = if confidence < config.min_confidence {
        SessionPhase::Unknown
    } else {
        SessionPhase::ALL[winner_idx]
    };

    PhaseLabel {
        phase,
        confidence,
        secondary,
        window_size: n as u32,
    }
}

/// Compute dominant phase from a history of labels.
pub fn dominant_phase(labels: &[PhaseLabel]) -> Option<SessionPhase> {
    if labels.is_empty() {
        return None;
    }

    let mut counts = [0u32; 9];
    for label in labels {
        counts[label.phase.index()] += 1;
    }

    let (max_idx, _) = counts
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < 8) // exclude Unknown
        .max_by_key(|(_, &c)| c)?;

    if counts[max_idx] == 0 {
        None
    } else {
        Some(SessionPhase::ALL[max_idx])
    }
}

// Matchers are in the sibling module `phase::matchers`
// Tests are in sibling module `phase::tests` (via #[path] in mod.rs)

#[cfg(test)]
mod tests;

