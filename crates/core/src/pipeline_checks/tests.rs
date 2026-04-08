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
    use super::types::MAX_STORED_VIOLATIONS;
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
