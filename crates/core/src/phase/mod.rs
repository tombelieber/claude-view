//! SDLC phase + scope classifier via Qwen3.5-4B on oMLX.

pub mod scheduler;
pub mod stabilizer;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
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
            Self::Thinking => "\u{1F4AD}",
            Self::Planning => "\u{1F4CB}",
            Self::Building => "\u{1F528}",
            Self::Testing => "\u{1F9EA}",
            Self::Reviewing => "\u{1F50D}",
            Self::Shipping => "\u{1F680}",
            Self::Working => "\u{2699}\u{FE0F}",
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
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct PhaseLabel {
    pub phase: SessionPhase,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// How fresh the phase classification is — drives badge visual state.
///
/// `Fresh` → solid badge (just classified).
/// `Pending` → subtle shimmer (~400ms, classify in-flight).
/// `Settled` → dimmed badge (NeedsYou session, phase frozen).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "lowercase")]
pub enum PhaseFreshness {
    /// Classified recently — solid badge at full opacity.
    #[default]
    Fresh,
    /// Classify call in-flight — brief shimmer animation.
    Pending,
    /// Session idle (NeedsYou), phase frozen — dimmed badge at 60% opacity.
    Settled,
}

/// User-configurable aggressiveness for local LLM classification.
/// Stored in `~/.claude-view/local-llm.json` as `classify_mode`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClassifyMode {
    /// Budget multiplier 0.5x — faster updates, more GPU.
    Realtime,
    /// Budget multiplier 1.0x — auto-tuned default.
    #[default]
    Balanced,
    /// Budget multiplier 2.0x — slower updates, less GPU.
    Efficient,
}

impl ClassifyMode {
    pub fn budget_multiplier(self) -> f32 {
        match self {
            Self::Realtime => 0.5,
            Self::Balanced => 1.0,
            Self::Efficient => 2.0,
        }
    }
}

/// Full phase history for a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct PhaseHistory {
    pub current: Option<PhaseLabel>,
    pub labels: Vec<PhaseLabel>,
    pub dominant: Option<SessionPhase>,
    /// Badge animation hint: `fresh` = solid, `pending` = breathing.
    pub freshness: PhaseFreshness,
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

/// Check if a bash command represents a shipping/deploy action.
pub fn is_shipping_cmd(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    c.contains("npm publish")
        || c.contains("cargo publish")
        || c.contains("fly deploy")
        || c.contains("vercel --prod")
        || c.contains("netlify deploy")
        || c.contains("wrangler deploy")
        || c.contains("gh release create")
        || c.starts_with("docker push")
        || c.starts_with("podman push")
}

/// Maximum number of phase labels to retain per session.
pub const MAX_PHASE_LABELS: usize = 100;

#[cfg(test)]
mod tests;
