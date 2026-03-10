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

// ── Per-line invariant checks ──

use crate::accumulator::SessionAccumulator;
use crate::live_parser::{LineType, LiveLine};
use crate::pricing::default_pricing;
use std::collections::HashSet;

/// Check 7: Every assistant line with `usage` in raw JSON -> parsed tokens are non-zero.
pub fn check_token_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let has_raw_usage = raw
        .get("message")
        .and_then(|m| m.get("usage"))
        .and_then(|u| u.as_object())
        .is_some_and(|u| {
            u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) > 0
                || u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) > 0
        });

    if !has_raw_usage {
        return;
    }

    let parsed_has_tokens =
        parsed.input_tokens.unwrap_or(0) > 0 || parsed.output_tokens.unwrap_or(0) > 0;

    if parsed_has_tokens {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has usage (input/output > 0) but parsed tokens are all None/0",
        );
    }
}

/// Check 8: Every assistant line with `model` in raw JSON -> parsed model is Some.
pub fn check_model_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let raw_model = raw
        .get("message")
        .and_then(|m| m.get("model"))
        .and_then(|v| v.as_str());
    if raw_model.is_none() {
        return;
    }
    if parsed.model.is_some() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!("raw has model={:?} but parsed model is None", raw_model),
        );
    }
}

/// Check 9: Every assistant line with tool_use blocks -> parsed tool_names non-empty.
pub fn check_tool_name_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let has_tool_use = raw
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .is_some_and(|blocks| {
            blocks
                .iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        });
    if !has_tool_use {
        return;
    }
    if !parsed.tool_names.is_empty() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has tool_use blocks but parsed tool_names is empty",
        );
    }
}

/// Check 10: Every Read/Edit/Write tool_use with file_path -> parser tool_names includes it.
pub fn check_file_path_tool_presence(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let file_tools = ["Read", "Edit", "Write"];
    let has_file_path_tool = raw
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .is_some_and(|blocks| {
            blocks.iter().any(|b| {
                let is_tool_use = b.get("type").and_then(|t| t.as_str()) == Some("tool_use");
                if !is_tool_use {
                    return false;
                }
                let tool_name = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if !file_tools.contains(&tool_name) {
                    return false;
                }
                b.get("input")
                    .and_then(|i| i.get("file_path"))
                    .and_then(|fp| fp.as_str())
                    .is_some_and(|path| !path.is_empty())
            })
        });
    if !has_file_path_tool {
        return;
    }
    let has_file_tool_in_parsed = parsed
        .tool_names
        .iter()
        .any(|t| file_tools.contains(&t.as_str()));
    if has_file_tool_in_parsed {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has file-path tool (Read/Edit/Write with file_path) but parsed tool_names missing it",
        );
    }
}

/// Check 11: Every user/assistant line with text content -> content_preview non-empty.
///
/// Correctly handles system-injected noise tags (`<command-message>`, `<task-notification>`,
/// etc.) which `strip_noise_tags()` intentionally removes. If the raw text is entirely
/// noise tags, empty `content_preview` is correct parser behavior.
pub fn check_content_preview(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    use crate::live_parser::strip_noise_tags;

    let raw_type = raw.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if raw_type != "user" && raw_type != "assistant" {
        return;
    }

    // Collect raw text content, checking if ANY meaningful text survives noise-tag stripping.
    let has_meaningful_text = match raw.get("message").and_then(|m| m.get("content")) {
        Some(serde_json::Value::String(s)) => {
            if s.is_empty() {
                false
            } else {
                let (stripped, _) = strip_noise_tags(s);
                !stripped.is_empty()
            }
        }
        Some(serde_json::Value::Array(blocks)) => blocks.iter().any(|b| {
            b.get("type").and_then(|t| t.as_str()) == Some("text")
                && b.get("text").and_then(|t| t.as_str()).is_some_and(|text| {
                    if text.is_empty() {
                        return false;
                    }
                    let (stripped, _) = strip_noise_tags(text);
                    !stripped.is_empty()
                })
        }),
        _ => false,
    };
    if !has_meaningful_text {
        return;
    }
    if !parsed.content_preview.is_empty() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has text content (after noise-tag stripping) but parsed content_preview is empty",
        );
    }
}

/// Check 12: Every line with timestamp string -> parsed timestamp is Some.
pub fn check_timestamp_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let raw_ts = raw.get("timestamp").and_then(|v| v.as_str());
    if raw_ts.is_none() || raw_ts.is_some_and(|s| s.is_empty()) {
        return;
    }
    if parsed.timestamp.is_some() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "raw has timestamp={:?} but parsed timestamp is None",
                raw_ts
            ),
        );
    }
}

/// Check 13: Cache creation 5m + 1hr == total when both splits are present.
///
/// Handles early API data quirk where `cache_creation` split object exists with
/// both values at 0 while `cache_creation_input_tokens` total is non-zero.
/// In this case the split data is unreliable — skip the check for that line.
pub fn check_cache_token_split(
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let total = match parsed.cache_creation_tokens {
        Some(t) if t > 0 => t,
        _ => return,
    };
    let t5m = parsed.cache_creation_5m_tokens;
    let t1hr = parsed.cache_creation_1hr_tokens;
    if t5m.is_none() && t1hr.is_none() {
        return; // No split data (older API)
    }
    let sum = t5m.unwrap_or(0) + t1hr.unwrap_or(0);
    if sum == 0 && total > 0 {
        return; // Early API data: split object exists but values not populated
    }
    if sum == total {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "cache split mismatch: 5m({}) + 1hr({}) = {} != total({})",
                t5m.unwrap_or(0),
                t1hr.unwrap_or(0),
                sum,
                total
            ),
        );
    }
}

/// Check 16: Every line with tokens > 0 should have a model.
pub fn check_cost_requires_model(
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let has_tokens = parsed.input_tokens.unwrap_or(0) > 0 || parsed.output_tokens.unwrap_or(0) > 0;
    if !has_tokens {
        return;
    }
    if parsed.model.is_some() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "has tokens (in={}, out={}) but no model",
                parsed.input_tokens.unwrap_or(0),
                parsed.output_tokens.unwrap_or(0)
            ),
        );
    }
}

/// Check 18: Raw type field -> parser LineType must match.
pub fn check_role_classification(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let raw_type = raw.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let (expected_line_type, type_name) = match raw_type {
        "assistant" => (LineType::Assistant, "Assistant"),
        "user" => (LineType::User, "User"),
        "system" => (LineType::System, "System"),
        "progress" => (LineType::Progress, "Progress"),
        _ => return,
    };
    if parsed.line_type == expected_line_type {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "raw type={:?} but parser classified as {:?} (expected {:?})",
                raw_type, parsed.line_type, type_name
            ),
        );
    }
}

/// Run all per-line invariant checks on a single parsed line.
pub fn run_per_line_checks(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    signals: &mut PipelineSignals,
) {
    check_token_extraction(raw, parsed, file, line_num, &mut signals.token_extraction);
    check_model_extraction(raw, parsed, file, line_num, &mut signals.model_extraction);
    check_tool_name_extraction(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.tool_name_extraction,
    );
    check_file_path_tool_presence(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.file_path_tool_presence,
    );
    check_content_preview(raw, parsed, file, line_num, &mut signals.content_preview);
    check_timestamp_extraction(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.timestamp_extraction,
    );
    check_cache_token_split(parsed, file, line_num, &mut signals.cache_token_split);
    check_cost_requires_model(parsed, file, line_num, &mut signals.cost_requires_model);
    check_role_classification(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.role_classification,
    );
}

// ── Per-session invariant checks ──

/// Run per-session invariant checks using pre-parsed line pairs.
///
/// Accepts `&[(&[u8], LiveLine)]` — raw bytes (for JSON field checks) + already-parsed
/// LiveLine (for accumulator feeding). This avoids double-parsing.
///
/// CRITICAL: The dedup logic MUST mirror the accumulator exactly:
/// - message_id comes from `message.id` (nested), NOT root `uuid`
/// - request_id comes from root `requestId`
/// - dedup key is only inserted when `has_measurement_data` is true
///
/// See accumulator.rs:111-128 and live_parser.rs:570-579.
pub fn run_per_session_checks(
    parsed_lines: &[(&[u8], LiveLine)],
    file: &str,
    signals: &mut PipelineSignals,
) {
    let pricing = default_pricing();
    let mut accumulator = SessionAccumulator::new();
    let mut prev_input_total: u64 = 0;
    let mut prev_output_total: u64 = 0;

    // Track raw usage sums for round-trip check
    let mut raw_input_sum: u64 = 0;
    let mut raw_output_sum: u64 = 0;
    let mut seen_api_calls: HashSet<String> = HashSet::new();

    for (i, (raw_bytes, parsed)) in parsed_lines.iter().enumerate() {
        let line_num = i + 1;

        // Feed pre-parsed LiveLine into accumulator (no re-parsing)
        accumulator.process_line(parsed, 0, &pricing);

        // Check 14: Token monotonicity — totals should only increase
        let cur_input = accumulator.tokens.input_tokens;
        let cur_output = accumulator.tokens.output_tokens;
        if cur_input < prev_input_total || cur_output < prev_output_total {
            signals.token_monotonicity.record_violation(
                file,
                line_num,
                &format!(
                    "tokens decreased: input {}→{}, output {}→{}",
                    prev_input_total, cur_input, prev_output_total, cur_output
                ),
            );
        } else {
            signals.token_monotonicity.record_pass();
        }
        prev_input_total = cur_input;
        prev_output_total = cur_output;

        // Sum raw usage for round-trip check (must mirror accumulator dedup exactly)
        if let Ok(raw_value) = serde_json::from_slice::<serde_json::Value>(raw_bytes) {
            let raw_type = raw_value.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if raw_type == "assistant" {
                // message_id from nested message.id (matches live_parser.rs:572-575)
                let msg_id = raw_value
                    .get("message")
                    .and_then(|m| m.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // request_id from root (matches live_parser.rs:576-579)
                let req_id = raw_value
                    .get("requestId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // has_measurement_data guard (matches accumulator.rs:111-116)
                let has_measurement_data = parsed.input_tokens.is_some()
                    || parsed.output_tokens.is_some()
                    || parsed.cache_read_tokens.is_some()
                    || parsed.cache_creation_tokens.is_some()
                    || parsed.cache_creation_5m_tokens.is_some()
                    || parsed.cache_creation_1hr_tokens.is_some();

                // Apply same dedup logic as accumulator (lines 118-128)
                let should_count = match (!msg_id.is_empty(), !req_id.is_empty()) {
                    (true, true) => {
                        if has_measurement_data {
                            let key = format!("{}:{}", msg_id, req_id);
                            seen_api_calls.insert(key) // true if newly inserted
                        } else {
                            false // no measurement data — don't insert or count
                        }
                    }
                    _ => true, // no IDs — count it (legacy fallback, line 127)
                };

                if should_count {
                    if let Some(usage) = raw_value.get("message").and_then(|m| m.get("usage")) {
                        raw_input_sum += usage
                            .get("input_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        raw_output_sum += usage
                            .get("output_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                    }
                }
            }
        }
    }

    // Check 17: Token accumulation round-trip
    let acc_input = accumulator.tokens.input_tokens;
    let acc_output = accumulator.tokens.output_tokens;
    if acc_input == raw_input_sum && acc_output == raw_output_sum {
        signals.token_round_trip.record_pass();
    } else {
        signals.token_round_trip.record_violation(
            file,
            0,
            &format!(
                "round-trip mismatch: raw sum input={}/output={} vs accumulator input={}/output={}",
                raw_input_sum, raw_output_sum, acc_input, acc_output
            ),
        );
    }

    // Check 15: Count-list parity — DEFERRED (requires indexer from claude-view-db).
    signals.count_list_parity.mark_skipped();
}

// ── Phase 3: field coverage checks ──

use crate::evidence_audit::FieldExtractionPaths;
use crate::field_inventory::FieldInventory;

/// Check 19: Field coverage — every path seen in real data is either
/// extracted by the parser or intentionally ignored by the baseline.
pub fn check_field_coverage(
    inventory: &FieldInventory,
    baseline: &FieldExtractionPaths,
) -> PipelineCheckResult {
    let known: HashSet<&str> = baseline
        .extracted
        .iter()
        .chain(&baseline.intentionally_ignored)
        .map(|s| s.as_str())
        .collect();

    let mut accum = CheckAccum::default();

    for (path, count) in &inventory.paths {
        if known.contains(path.as_str()) {
            accum.record_pass();
        } else {
            accum.record_violation(
                "corpus-wide",
                0,
                &format!("unhandled field path: \"{}\" ({} occurrences)", path, count),
            );
        }
    }

    accum.into_result("Field coverage")
}

/// Check 20: Phantom field detection — every path the baseline claims is
/// extracted must actually appear in the scanned data.
pub fn check_phantom_fields(
    inventory: &FieldInventory,
    baseline: &FieldExtractionPaths,
) -> PipelineCheckResult {
    let mut accum = CheckAccum::default();

    for path in &baseline.extracted {
        match inventory.paths.get(path.as_str()) {
            Some(count) if *count > 0 => {
                accum.record_pass();
            }
            _ => {
                accum.record_violation(
                    "corpus-wide",
                    0,
                    &format!(
                        "PHANTOM: parser claims to extract \"{}\" but 0 occurrences across all scanned data ({} distinct paths seen)",
                        path,
                        inventory.paths.len()
                    ),
                );
            }
        }
    }

    accum.into_result("Phantom field detection")
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

    // ── Per-line check tests ──

    use crate::live_parser::{parse_single_line, TailFinders};

    #[test]
    fn test_check_token_extraction_passes_when_usage_extracted() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi"}],"model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let parsed = parse_single_line(raw, &finders);
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_token_extraction(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.violations.len(), 0);
        assert_eq!(accum.lines_checked, 1);
    }

    #[test]
    fn test_check_role_classification_assistant() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi"}]}}"#;
        let parsed = parse_single_line(raw, &finders);
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_role_classification(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.violations.len(), 0);
    }

    #[test]
    fn test_check_cache_split_sums_correctly() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":"hi","model":"claude-opus-4-6","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200,"cache_creation_input_tokens":57339,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":57339}}}}"#;
        let parsed = parse_single_line(raw, &finders);
        let mut accum = CheckAccum::default();
        check_cache_token_split(&parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.violations.len(), 0);
    }

    #[test]
    fn test_check_model_extraction_passes() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":"hi","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let parsed = parse_single_line(raw, &finders);
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_model_extraction(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.violations.len(), 0);
    }

    #[test]
    fn test_check_timestamp_extraction_passes() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"user","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"hello"}}"#;
        let parsed = parse_single_line(raw, &finders);
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_timestamp_extraction(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.violations.len(), 0);
    }

    #[test]
    fn test_check_file_path_tool_presence_read() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01ABC","name":"Read","input":{"file_path":"/Users/test/file.rs"}}]}}"#;
        let parsed = parse_single_line(raw, &finders);
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_file_path_tool_presence(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.violations.len(), 0);
    }

    #[test]
    fn test_check_file_path_tool_presence_non_file_tool_skipped() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01ABC","name":"Bash","input":{"command":"ls"}}]}}"#;
        let parsed = parse_single_line(raw, &finders);
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_file_path_tool_presence(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.lines_checked, 0); // Non-file tool not counted
    }

    #[test]
    fn test_check_file_path_tool_presence_violation() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01ABC","name":"Read","input":{"file_path":"/path/file.rs"}}]}}"#;
        let mut parsed = parse_single_line(raw, &finders);
        parsed.tool_names.clear(); // Simulate parser failure
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_file_path_tool_presence(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(accum.violations.len(), 1);
    }

    // ── Per-session check tests ──

    #[test]
    fn test_session_token_round_trip() {
        let finders = TailFinders::new();
        let lines_raw = vec![
            br#"{"type":"assistant","message":{"id":"msg-1","role":"assistant","content":"hi","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":50}},"requestId":"req-1"}"#.to_vec(),
            br#"{"type":"assistant","message":{"id":"msg-2","role":"assistant","content":"bye","model":"claude-sonnet-4-20250514","usage":{"input_tokens":200,"output_tokens":75}},"requestId":"req-2"}"#.to_vec(),
        ];
        let parsed_lines: Vec<_> = lines_raw
            .iter()
            .map(|raw| (raw.as_slice(), parse_single_line(raw, &finders)))
            .collect();
        let mut signals = PipelineSignals::default();
        run_per_session_checks(&parsed_lines, "test.jsonl", &mut signals);
        assert_eq!(signals.token_round_trip.violations.len(), 0);
    }

    #[test]
    fn test_session_token_monotonicity() {
        let finders = TailFinders::new();
        let lines_raw = vec![
            br#"{"type":"assistant","message":{"id":"msg-1","role":"assistant","content":"hi","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":50}},"requestId":"req-1"}"#.to_vec(),
            br#"{"type":"assistant","message":{"id":"msg-2","role":"assistant","content":"more","model":"claude-sonnet-4-20250514","usage":{"input_tokens":200,"output_tokens":75}},"requestId":"req-2"}"#.to_vec(),
        ];
        let parsed_lines: Vec<_> = lines_raw
            .iter()
            .map(|raw| (raw.as_slice(), parse_single_line(raw, &finders)))
            .collect();
        let mut signals = PipelineSignals::default();
        run_per_session_checks(&parsed_lines, "test.jsonl", &mut signals);
        assert_eq!(signals.token_monotonicity.violations.len(), 0);
    }

    #[test]
    fn test_session_dedup_duplicate_blocks_not_double_counted() {
        let finders = TailFinders::new();
        let lines_raw = vec![
            br#"{"type":"assistant","message":{"id":"msg-1","role":"assistant","content":"block1","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":50}},"requestId":"req-1"}"#.to_vec(),
            br#"{"type":"assistant","message":{"id":"msg-1","role":"assistant","content":"block2","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":50}},"requestId":"req-1"}"#.to_vec(),
        ];
        let parsed_lines: Vec<_> = lines_raw
            .iter()
            .map(|raw| (raw.as_slice(), parse_single_line(raw, &finders)))
            .collect();
        let mut signals = PipelineSignals::default();
        run_per_session_checks(&parsed_lines, "test.jsonl", &mut signals);
        assert_eq!(
            signals.token_round_trip.violations.len(),
            0,
            "duplicate blocks should be deduped — raw sum should match accumulator"
        );
    }

    #[test]
    fn test_session_no_measurement_data_not_deduped() {
        let finders = TailFinders::new();
        let lines_raw = vec![
            br#"{"type":"assistant","message":{"id":"msg-1","role":"assistant","content":"block1"},"requestId":"req-1"}"#.to_vec(),
            br#"{"type":"assistant","message":{"id":"msg-1","role":"assistant","content":"block2","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":50}},"requestId":"req-1"}"#.to_vec(),
        ];
        let parsed_lines: Vec<_> = lines_raw
            .iter()
            .map(|raw| (raw.as_slice(), parse_single_line(raw, &finders)))
            .collect();
        let mut signals = PipelineSignals::default();
        run_per_session_checks(&parsed_lines, "test.jsonl", &mut signals);
        assert_eq!(signals.token_round_trip.violations.len(), 0);
    }

    #[test]
    fn test_check_content_preview_noise_tags_only_not_violation() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"user","message":{"role":"user","content":"<command-message>superpowers:dispatching-parallel-agents</command-message>\n<command-args>task1</command-args>"}}"#;
        let parsed = parse_single_line(raw, &finders);
        let raw_value: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut accum = CheckAccum::default();
        check_content_preview(&raw_value, &parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(
            accum.violations.len(),
            0,
            "noise-tag-only content should not be a violation"
        );
        assert_eq!(
            accum.lines_checked, 0,
            "noise-tag-only lines should be skipped entirely"
        );
    }

    #[test]
    fn test_check_cache_split_early_api_zero_split_not_violation() {
        let finders = TailFinders::new();
        // Early API data: total=730 but split has both values at 0
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":"hi","model":"claude-opus-4-6","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":730,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":0}}}}"#;
        let parsed = parse_single_line(raw, &finders);
        let mut accum = CheckAccum::default();
        check_cache_token_split(&parsed, "test.jsonl", 1, &mut accum);
        assert_eq!(
            accum.violations.len(),
            0,
            "early API zero-split should not be a violation"
        );
        assert_eq!(
            accum.lines_checked, 0,
            "early API zero-split should be skipped"
        );
    }
}
