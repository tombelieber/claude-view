//! Sliding-window SDLC phase classifier for coding sessions.
//!
//! 6 product stages: Thinking, Planning, Building, Testing, Reviewing, Shipping.
//! Architecture: 4-class XGBoost (confidence-gated) + post-ML thinking/planning
//! split + rule-based shipping detection.

pub mod classifier;
pub mod features;
pub mod matchers;

pub use classifier::{classify_window, PhaseClassifier};
pub use features::flatten_window;
pub use matchers::*;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

/// SDLC session phase (6 product stages + Working fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum SessionPhase {
    Thinking,
    Planning,
    Building,
    Testing,
    Reviewing,
    Shipping,
    Working,
}

impl SessionPhase {
    pub const ALL: [SessionPhase; 6] = [
        Self::Thinking,
        Self::Planning,
        Self::Building,
        Self::Testing,
        Self::Reviewing,
        Self::Shipping,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Thinking => "thinking",
            Self::Planning => "planning",
            Self::Building => "building",
            Self::Testing => "testing",
            Self::Reviewing => "reviewing",
            Self::Shipping => "shipping",
            Self::Working => "working",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Thinking => "Thinking",
            Self::Planning => "Planning",
            Self::Building => "Building",
            Self::Testing => "Testing",
            Self::Reviewing => "Reviewing",
            Self::Shipping => "Shipping",
            Self::Working => "Working",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Thinking => "💭",
            Self::Planning => "📋",
            Self::Building => "🔨",
            Self::Testing => "🧪",
            Self::Reviewing => "🔍",
            Self::Shipping => "🚀",
            Self::Working => "⚙️",
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

/// Compute dominant phase from a history of labels.
pub fn dominant_phase(labels: &[PhaseLabel]) -> Option<SessionPhase> {
    if labels.is_empty() {
        return None;
    }
    let mut counts = [0u32; 7]; // 6 phases + Working
    for label in labels {
        let idx = match label.phase {
            SessionPhase::Thinking => 0,
            SessionPhase::Planning => 1,
            SessionPhase::Building => 2,
            SessionPhase::Testing => 3,
            SessionPhase::Reviewing => 4,
            SessionPhase::Shipping => 5,
            SessionPhase::Working => 6,
        };
        counts[idx] += 1;
    }
    // Exclude Working from dominant calculation
    let (max_idx, &max_count) = counts[..6].iter().enumerate().max_by_key(|(_, &c)| c)?;
    if max_count == 0 {
        return None;
    }
    Some(SessionPhase::ALL[max_idx])
}

// ============================================================================
// Step Signals (input to classifier)
// ============================================================================

/// Signal vector for a single step, extracted from tool-use blocks.
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
    pub todo_count: u32,

    // File type counts
    pub config_files_edited: u32,
    pub test_files_edited: u32,
    pub doc_files_edited: u32,
    pub plan_files_edited: u32,
    pub script_files_edited: u32,
    pub ci_files_edited: u32,
    pub migration_files_edited: u32,

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

    // Enriched bash signals
    pub has_review_combo: bool,
    pub has_plan_execute_combo: bool,
    pub has_tdd_combo: bool,
    pub has_git_commit: bool,
    pub has_git_diff: bool,
    pub has_docker_cmd: bool,
    pub has_lint_cmd: bool,

    // Prompt keyword flags
    pub prompt_plan_kw: bool,
    pub prompt_impl_kw: bool,
    pub prompt_fix_kw: bool,
    pub prompt_review_kw: bool,
    pub prompt_test_kw: bool,
    pub prompt_release_kw: bool,
    pub prompt_config_kw: bool,
    pub prompt_explore_kw: bool,
}

#[cfg(test)]
mod tests;
