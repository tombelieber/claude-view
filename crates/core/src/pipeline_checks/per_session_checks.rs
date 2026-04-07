// ── Per-session invariant checks ──

use std::collections::HashSet;

use crate::accumulator::SessionAccumulator;
use crate::live_parser::LiveLine;
use crate::pricing::load_pricing;

use super::types::PipelineSignals;

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
    let pricing = load_pricing();
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
