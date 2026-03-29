//! Classification stabilizer: EMA-based phase tracking with consecutive-confirm transitions.
//!
//! Replaces the previous 3-ring temperature-diversified voting approach.
//! - First call shows badge immediately (no waiting for consensus).
//! - Phase transitions require 2 consecutive results + EMA > threshold.
//! - Single fixed temperature (no rotation).
//! - Shipping lock heuristic preserved.

use std::collections::HashMap;

use super::SessionPhase;

/// EMA decay factor. Higher = more weight on recent call.
const EMA_DECAY: f32 = 0.4;
/// Minimum EMA score for the new phase to trigger a transition.
const TRANSITION_THRESHOLD: f32 = 0.6;
/// Fixed temperature for all classify calls (no rotation).
const FIXED_TEMPERATURE: f32 = 0.15;
/// Consecutive identical results required to transition displayed phase.
const CONSECUTIVE_REQUIRED: u8 = 2;

pub struct ClassificationStabilizer {
    /// EMA weight per phase. Each phase decays independently.
    phase_ema: HashMap<SessionPhase, f32>,
    /// Total classify results received.
    call_count: u32,
    /// The phase currently displayed in the badge.
    displayed_phase: Option<SessionPhase>,
    /// Raw scope from the most recent result matching displayed phase.
    displayed_scope: Option<String>,
    /// Last N phases seen (for consecutive detection).
    last_phases: Vec<SessionPhase>,
    /// Shipping lock: suppresses updates for 3 non-shipping results.
    pub shipping_locked: bool,
    non_shipping_count: u32,
}

impl Default for ClassificationStabilizer {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassificationStabilizer {
    pub fn new() -> Self {
        Self {
            phase_ema: HashMap::new(),
            call_count: 0,
            displayed_phase: None,
            displayed_scope: None,
            last_phases: Vec::new(),
            shipping_locked: false,
            non_shipping_count: 0,
        }
    }

    pub fn update(&mut self, phase: SessionPhase, scope: Option<String>) {
        // Shipping lock: count non-shipping results, unlock after 3
        if self.shipping_locked {
            self.non_shipping_count += 1;
            if self.non_shipping_count >= 3 {
                self.shipping_locked = false;
                self.non_shipping_count = 0;
            }
            return;
        }

        self.call_count += 1;

        // Update EMA: decay all, then boost incoming phase
        for score in self.phase_ema.values_mut() {
            *score *= 1.0 - EMA_DECAY;
        }
        *self.phase_ema.entry(phase).or_insert(0.0) += EMA_DECAY;

        // Track consecutive phases (keep last CONSECUTIVE_REQUIRED)
        self.last_phases.push(phase);
        if self.last_phases.len() > CONSECUTIVE_REQUIRED as usize {
            self.last_phases.remove(0);
        }

        // First call: show immediately
        if self.call_count == 1 {
            self.displayed_phase = Some(phase);
            self.displayed_scope = scope;
            return;
        }

        // Same phase as displayed: just update scope if present
        if Some(phase) == self.displayed_phase {
            if let Some(s) = scope {
                self.displayed_scope = Some(s);
            }
            return;
        }

        // Different phase: check transition conditions
        let consecutive = self
            .last_phases
            .iter()
            .all(|&p| p == phase)
            && self.last_phases.len() >= CONSECUTIVE_REQUIRED as usize;
        let ema_score = self.phase_ema.get(&phase).copied().unwrap_or(0.0);

        if consecutive && ema_score > TRANSITION_THRESHOLD {
            self.displayed_phase = Some(phase);
            self.displayed_scope = scope;
        }
    }

    pub fn should_emit(&self) -> bool {
        self.displayed_phase.is_some()
    }

    pub fn displayed_phase(&self) -> Option<SessionPhase> {
        self.displayed_phase
    }

    pub fn displayed_scope(&self) -> Option<String> {
        self.displayed_scope.clone()
    }

    /// Returns the highest EMA score as confidence.
    pub fn confidence(&self) -> f64 {
        self.phase_ema
            .values()
            .cloned()
            .fold(0.0_f32, f32::max) as f64
    }

    /// Fixed temperature — no rotation.
    pub fn next_temperature(&self) -> f32 {
        FIXED_TEMPERATURE
    }

    pub fn lock_shipping(&mut self) {
        self.shipping_locked = true;
        self.non_shipping_count = 0;
    }

    pub fn reset(&mut self) {
        self.phase_ema.clear();
        self.call_count = 0;
        self.displayed_phase = None;
        self.displayed_scope = None;
        self.last_phases.clear();
        self.shipping_locked = false;
        self.non_shipping_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase::SessionPhase;

    #[test]
    fn first_call_shows_badge() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, Some("auth system".into()));
        assert!(s.should_emit());
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Building));
        assert_eq!(s.displayed_scope().as_deref(), Some("auth system"));
    }

    #[test]
    fn same_phase_updates_scope() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, Some("auth".into()));
        s.update(SessionPhase::Building, Some("auth refactor".into()));
        assert_eq!(s.displayed_scope().as_deref(), Some("auth refactor"));
    }

    #[test]
    fn transition_needs_two_consecutive() {
        let mut s = ClassificationStabilizer::new();
        // Establish building
        s.update(SessionPhase::Building, None);
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Building));

        // One testing call — not enough
        s.update(SessionPhase::Testing, None);
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Building));

        // Second consecutive testing — should transition
        s.update(SessionPhase::Testing, None);
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Testing));
    }

    #[test]
    fn noise_rejected() {
        let mut s = ClassificationStabilizer::new();
        // Establish building with several calls
        for _ in 0..5 {
            s.update(SessionPhase::Building, None);
        }

        // One noisy testing call
        s.update(SessionPhase::Testing, None);
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Building));

        // Back to building — no transition happened
        s.update(SessionPhase::Building, None);
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Building));
    }

    #[test]
    fn ema_decay_enables_transition() {
        let mut s = ClassificationStabilizer::new();
        // 5x building → EMA heavily weighted to building
        for _ in 0..5 {
            s.update(SessionPhase::Building, None);
        }

        // 5x testing → EMA should shift enough
        for _ in 0..5 {
            s.update(SessionPhase::Testing, None);
        }
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Testing));
    }

    #[test]
    fn shipping_lock_overrides_llm() {
        let mut s = ClassificationStabilizer::new();
        s.lock_shipping();
        s.update(SessionPhase::Building, Some("test".into()));
        assert!(!s.should_emit());
    }

    #[test]
    fn shipping_lock_clears_after_3() {
        let mut s = ClassificationStabilizer::new();
        s.lock_shipping();
        s.update(SessionPhase::Building, None);
        s.update(SessionPhase::Building, None);
        s.update(SessionPhase::Building, None);
        assert!(!s.shipping_locked);
    }

    #[test]
    fn fixed_temperature() {
        let mut s = ClassificationStabilizer::new();
        assert_eq!(s.next_temperature(), 0.15);
        s.update(SessionPhase::Building, None);
        assert_eq!(s.next_temperature(), 0.15);
        s.update(SessionPhase::Testing, None);
        assert_eq!(s.next_temperature(), 0.15);
    }

    #[test]
    fn confidence_reflects_ema() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, None);
        assert!(s.confidence() > 0.3);

        // More calls to same phase → higher confidence
        s.update(SessionPhase::Building, None);
        s.update(SessionPhase::Building, None);
        assert!(s.confidence() > 0.5);
    }

    #[test]
    fn reset_clears_everything() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, Some("test".into()));
        assert!(s.should_emit());
        s.reset();
        assert!(!s.should_emit());
        assert!(s.displayed_phase().is_none());
        assert_eq!(s.call_count, 0);
    }
}
