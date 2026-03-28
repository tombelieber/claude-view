//! Classification stabilizer: temperature-diversified voting + scope normalization.

use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashSet;
use std::sync::LazyLock;

use super::SessionPhase;

const RING_SIZE: usize = 3;
const TEMP_SCHEDULE: [f32; 3] = [0.2, 0.4, 0.6];

static STOP_WORDS: &[&str] = &["the", "a", "an", "for", "of", "and", "in", "to", "with"];

static GENERIC_STEMS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let stemmer = Stemmer::create(Algorithm::English);
    [
        "code",
        "changes",
        "work",
        "task",
        "implementation",
        "feature",
        "update",
        "fix",
        "general",
        "various",
        "project",
        "codebase",
    ]
    .iter()
    .map(|w| stemmer.stem(w).to_string())
    .collect()
});

pub struct ClassificationStabilizer {
    phase_ring: [Option<SessionPhase>; RING_SIZE],
    scope_norm_ring: [Option<Vec<String>>; RING_SIZE],
    scope_raw_ring: [Option<String>; RING_SIZE],
    ring_idx: usize,
    ring_count: usize,
    displayed_phase: Option<SessionPhase>,
    displayed_scope: Option<String>,
    pub shipping_locked: bool,
    non_shipping_count: u32,
    temp_idx: usize,
}

impl Default for ClassificationStabilizer {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassificationStabilizer {
    pub fn new() -> Self {
        Self {
            phase_ring: [None, None, None],
            scope_norm_ring: [None, None, None],
            scope_raw_ring: [None, None, None],
            ring_idx: 0,
            ring_count: 0,
            displayed_phase: None,
            displayed_scope: None,
            shipping_locked: false,
            non_shipping_count: 0,
            temp_idx: 0,
        }
    }

    pub fn update(&mut self, phase: SessionPhase, scope: Option<String>) {
        if self.shipping_locked {
            self.non_shipping_count += 1;
            if self.non_shipping_count >= 3 {
                self.shipping_locked = false;
                self.non_shipping_count = 0;
            }
        }

        let norm = scope.as_ref().map(|s| normalize_scope(s));

        self.phase_ring[self.ring_idx] = Some(phase);
        self.scope_norm_ring[self.ring_idx] = norm;
        self.scope_raw_ring[self.ring_idx] = scope;
        self.ring_idx = (self.ring_idx + 1) % RING_SIZE;
        if self.ring_count < RING_SIZE {
            self.ring_count += 1;
        }
        self.temp_idx = (self.temp_idx + 1) % RING_SIZE;

        if !self.shipping_locked {
            self.compute_display();
        }
    }

    fn compute_display(&mut self) {
        let majority = self.majority_phase();
        self.displayed_phase = majority;

        // Scope strategy: try stemmed 2/3 majority first; if that fails,
        // take the scope from the most recent result that agreed with the
        // majority phase. Free-text scope rarely achieves word-level consensus
        // across temperature-varied calls, so falling back to the latest
        // phase-agreeing scope is the practical path.
        if let Some(majority_idx) = self.majority_scope_idx() {
            let raw = self.scope_raw_ring[majority_idx].clone();
            if let Some(ref s) = raw {
                if !is_generic_scope(s) {
                    self.displayed_scope = raw;
                    return;
                }
            }
        }

        // Fallback: latest scope from a result that matched the majority phase
        if let Some(phase) = majority {
            // Walk ring backwards from most recent write
            for offset in 1..=RING_SIZE {
                let idx = (self.ring_idx + RING_SIZE - offset) % RING_SIZE;
                if self.phase_ring[idx] == Some(phase) {
                    if let Some(ref s) = self.scope_raw_ring[idx] {
                        if !is_generic_scope(s) {
                            self.displayed_scope = Some(s.clone());
                            return;
                        }
                    }
                }
            }
        }

        self.displayed_scope = None;
    }

    fn majority_phase(&self) -> Option<SessionPhase> {
        let filled: Vec<SessionPhase> = self.phase_ring.iter().filter_map(|p| *p).collect();
        if filled.len() < 2 {
            return None;
        }
        for &candidate in &filled {
            let count = filled.iter().filter(|&&p| p == candidate).count();
            if count >= 2 {
                return Some(candidate);
            }
        }
        None
    }

    fn majority_scope_idx(&self) -> Option<usize> {
        let filled: Vec<(usize, &Vec<String>)> = self
            .scope_norm_ring
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|v| (i, v)))
            .collect();
        if filled.len() < 2 {
            return None;
        }
        for &(i, candidate) in &filled {
            let count = filled.iter().filter(|(_, v)| *v == candidate).count();
            if count >= 2 {
                return Some(i);
            }
        }
        None
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

    pub fn confidence(&self) -> f64 {
        let filled: Vec<SessionPhase> = self.phase_ring.iter().filter_map(|p| *p).collect();
        if filled.is_empty() {
            return 0.0;
        }
        let majority_count = filled
            .iter()
            .map(|&p| filled.iter().filter(|&&q| q == p).count())
            .max()
            .unwrap_or(0);
        match (filled.len(), majority_count) {
            (3, 3) => 1.0,
            (_, n) if n >= 2 => 0.8,
            (1, 1) => 0.5,
            _ => 0.0,
        }
    }

    pub fn next_temperature(&self) -> f32 {
        TEMP_SCHEDULE[self.temp_idx % RING_SIZE]
    }

    pub fn lock_shipping(&mut self) {
        self.shipping_locked = true;
        self.non_shipping_count = 0;
    }

    pub fn reset(&mut self) {
        self.phase_ring = [None, None, None];
        self.scope_norm_ring = [None, None, None];
        self.scope_raw_ring = [None, None, None];
        self.ring_idx = 0;
        self.ring_count = 0;
        self.displayed_phase = None;
        self.displayed_scope = None;
        self.shipping_locked = false;
        self.non_shipping_count = 0;
    }
}

pub fn normalize_scope(scope: &str) -> Vec<String> {
    let stemmer = Stemmer::create(Algorithm::English);
    let mut words: Vec<String> = scope
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !STOP_WORDS.contains(w))
        .map(|w| stemmer.stem(w).to_string())
        .collect();
    words.sort();
    words.truncate(4);
    words
}

pub fn is_generic_scope(scope: &str) -> bool {
    let stemmer = Stemmer::create(Algorithm::English);
    let significant: Vec<String> = scope
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !STOP_WORDS.contains(w))
        .map(|w| stemmer.stem(w).to_string())
        .collect();
    if significant.len() < 2 {
        return true;
    }
    significant.iter().all(|w| GENERIC_STEMS.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase::SessionPhase;

    #[test]
    fn phase_empty_no_emit() {
        let s = ClassificationStabilizer::new();
        assert!(!s.should_emit());
        assert!(s.displayed_phase().is_none());
    }

    #[test]
    fn phase_two_agree_emits() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, Some("auth system".into()));
        s.update(SessionPhase::Building, Some("auth system".into()));
        assert!(s.should_emit());
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Building));
    }

    #[test]
    fn phase_two_disagree_no_emit() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, Some("auth".into()));
        s.update(SessionPhase::Planning, Some("auth".into()));
        assert!(!s.should_emit());
    }

    #[test]
    fn phase_majority_2_of_3() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, None);
        s.update(SessionPhase::Planning, None);
        s.update(SessionPhase::Building, None);
        assert!(s.should_emit());
        assert_eq!(s.displayed_phase(), Some(SessionPhase::Building));
    }

    #[test]
    fn scope_inflection_match() {
        let a = normalize_scope("building new features");
        let b = normalize_scope("build the new feature");
        assert_eq!(a, b);
    }

    #[test]
    fn scope_generic_suppressed() {
        assert!(is_generic_scope("code changes"));
        assert!(is_generic_scope("fix update"));
        assert!(!is_generic_scope("XGBoost phase classifier"));
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
    fn temp_rotation() {
        let mut s = ClassificationStabilizer::new();
        assert_eq!(s.next_temperature(), 0.2);
        s.update(SessionPhase::Building, None);
        assert_eq!(s.next_temperature(), 0.4);
        s.update(SessionPhase::Building, None);
        assert_eq!(s.next_temperature(), 0.6);
        s.update(SessionPhase::Building, None);
        assert_eq!(s.next_temperature(), 0.2);
    }

    #[test]
    fn reset_clears_buffers() {
        let mut s = ClassificationStabilizer::new();
        s.update(SessionPhase::Building, Some("test".into()));
        s.update(SessionPhase::Building, Some("test".into()));
        assert!(s.should_emit());
        s.reset();
        assert!(!s.should_emit());
        assert!(s.displayed_phase().is_none());
    }
}
