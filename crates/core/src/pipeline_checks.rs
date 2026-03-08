//! Phase 2 pipeline invariant checks — validates parser + accumulator
//! correctness against real JSONL data.

/// Max violations stored per check (bounds memory during large scans).
const MAX_STORED_VIOLATIONS: usize = 50;

/// A single invariant violation with location for debugging.
#[derive(Debug, Clone)]
pub struct Violation {
    pub file: String,
    pub line_number: usize,
    pub detail: String,
}

impl Violation {
    pub fn new(file: &str, line_number: usize, detail: &str) -> Self {
        Self {
            file: file.to_string(),
            line_number,
            detail: detail.to_string(),
        }
    }
}

/// Result of a single pipeline invariant check.
#[derive(Debug)]
pub struct PipelineCheckResult {
    pub name: String,
    pub passed: bool,
    pub lines_checked: usize,
    pub violation_count: usize,
    /// true = check was deferred (e.g. requires indexer), not a failure.
    pub skipped: bool,
    /// First N violations for display (capped to avoid flooding output).
    pub sample_violations: Vec<Violation>,
}

impl PipelineCheckResult {
    pub fn new(
        name: &str,
        lines_checked: usize,
        violation_count: usize,
        violations: Vec<Violation>,
    ) -> Self {
        Self {
            name: name.to_string(),
            passed: violation_count == 0,
            lines_checked,
            violation_count,
            skipped: false,
            sample_violations: violations.into_iter().take(5).collect(),
        }
    }

    pub fn new_skipped(name: &str, reason: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            lines_checked: 0,
            violation_count: 0,
            skipped: true,
            sample_violations: vec![Violation::new("N/A", 0, reason)],
        }
    }
}

/// Line reference: (start, end) byte offsets into a shared data buffer.
/// Avoids copying entire lines during collection.
#[derive(Debug, Clone, Copy)]
pub struct LineOffsets {
    pub start: usize,
    pub end: usize,
}

impl LineOffsets {
    pub fn slice<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        &data[self.start..self.end]
    }
}

/// Aggregated Phase 2 results across all files, ready for merge in parallel scan.
#[derive(Debug, Default)]
pub struct PipelineSignals {
    // Per-line check accumulators
    pub token_extraction: CheckAccum,
    pub model_extraction: CheckAccum,
    pub tool_name_extraction: CheckAccum,
    pub file_path_tool_presence: CheckAccum,
    pub content_preview: CheckAccum,
    pub timestamp_extraction: CheckAccum,
    pub cache_token_split: CheckAccum,
    pub cost_requires_model: CheckAccum,
    pub role_classification: CheckAccum,
    // Per-session check accumulators
    pub token_monotonicity: CheckAccum,
    pub count_list_parity: CheckAccum,
    pub token_round_trip: CheckAccum,
}

/// Accumulator for a single check across parallel scan threads.
// IMPORTANT: when adding a field, update merge() and into_result().
#[derive(Debug, Default)]
pub struct CheckAccum {
    pub lines_checked: usize,
    /// True total violations (not capped — accurate for display).
    pub violation_count: usize,
    /// Stored violation samples (capped at MAX_STORED_VIOLATIONS to bound memory).
    pub violations: Vec<Violation>,
    /// true = this check was deferred, don't report as failure.
    pub skipped: bool,
}

impl CheckAccum {
    pub fn record_pass(&mut self) {
        self.lines_checked += 1;
    }

    pub fn record_violation(&mut self, file: &str, line_number: usize, detail: &str) {
        self.lines_checked += 1;
        self.violation_count += 1;
        if self.violations.len() < MAX_STORED_VIOLATIONS {
            self.violations
                .push(Violation::new(file, line_number, detail));
        }
    }

    pub fn mark_skipped(&mut self) {
        self.skipped = true;
    }

    pub fn into_result(self, name: &str) -> PipelineCheckResult {
        if self.skipped {
            PipelineCheckResult::new_skipped(name, "Deferred — requires indexer (Phase 3)")
        } else {
            PipelineCheckResult {
                name: name.to_string(),
                passed: self.violation_count == 0,
                lines_checked: self.lines_checked,
                violation_count: self.violation_count,
                skipped: false,
                sample_violations: self.violations.into_iter().take(5).collect(),
            }
        }
    }

    pub fn merge(&mut self, other: CheckAccum) {
        self.lines_checked += other.lines_checked;
        self.violation_count += other.violation_count;
        self.skipped = self.skipped || other.skipped;
        let remaining = MAX_STORED_VIOLATIONS.saturating_sub(self.violations.len());
        self.violations
            .extend(other.violations.into_iter().take(remaining));
    }
}

impl PipelineSignals {
    pub fn merge(&mut self, other: PipelineSignals) {
        self.token_extraction.merge(other.token_extraction);
        self.model_extraction.merge(other.model_extraction);
        self.tool_name_extraction.merge(other.tool_name_extraction);
        self.file_path_tool_presence
            .merge(other.file_path_tool_presence);
        self.content_preview.merge(other.content_preview);
        self.timestamp_extraction.merge(other.timestamp_extraction);
        self.cache_token_split.merge(other.cache_token_split);
        self.cost_requires_model.merge(other.cost_requires_model);
        self.role_classification.merge(other.role_classification);
        self.token_monotonicity.merge(other.token_monotonicity);
        self.count_list_parity.merge(other.count_list_parity);
        self.token_round_trip.merge(other.token_round_trip);
    }

    pub fn into_results(self) -> Vec<PipelineCheckResult> {
        vec![
            self.token_extraction.into_result("Token extraction"),
            self.model_extraction.into_result("Model extraction"),
            self.tool_name_extraction
                .into_result("Tool name extraction"),
            self.file_path_tool_presence
                .into_result("File path tool presence"),
            self.content_preview.into_result("Content preview"),
            self.timestamp_extraction
                .into_result("Timestamp extraction"),
            self.cache_token_split.into_result("Cache token split"),
            self.cost_requires_model.into_result("Cost requires model"),
            self.role_classification.into_result("Role classification"),
            self.token_monotonicity.into_result("Token monotonicity"),
            self.count_list_parity.into_result("Count-list parity"),
            self.token_round_trip.into_result("Token round-trip"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_violation_tracks_location() {
        let v = Violation::new("test.jsonl", 42, "token mismatch: expected 100, got 0");
        assert_eq!(v.file, "test.jsonl");
        assert_eq!(v.line_number, 42);
        assert!(v.detail.contains("token mismatch"));
    }

    #[test]
    fn test_check_result_pass_when_no_violations() {
        let r = PipelineCheckResult::new("Token extraction", 1000, 0, vec![]);
        assert!(r.passed);
        assert_eq!(r.lines_checked, 1000);
        assert!(!r.skipped);
    }

    #[test]
    fn test_check_result_fail_when_violations_exist() {
        let violations = vec![Violation::new("a.jsonl", 1, "bad")];
        let r = PipelineCheckResult::new("Token extraction", 1000, 1, violations);
        assert!(!r.passed);
        assert_eq!(r.violation_count, 1);
    }

    #[test]
    fn test_check_accum_true_violation_count_beyond_storage_cap() {
        let mut accum = CheckAccum::default();
        for i in 0..60 {
            accum.record_violation("test.jsonl", i, "violation");
        }
        assert_eq!(accum.violation_count, 60);
        assert_eq!(accum.violations.len(), MAX_STORED_VIOLATIONS);
    }

    #[test]
    fn test_check_result_skipped() {
        let r = PipelineCheckResult::new_skipped("Count-list parity", "Requires indexer");
        assert!(r.skipped);
        assert!(!r.passed);
        assert_eq!(r.lines_checked, 0);
    }

    #[test]
    fn test_check_accum_skip_propagates_through_merge() {
        let mut a = CheckAccum::default();
        let mut b = CheckAccum::default();
        b.mark_skipped();
        a.merge(b);
        assert!(a.skipped);
    }
}
