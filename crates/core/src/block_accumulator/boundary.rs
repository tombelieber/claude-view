// crates/core/src/block_accumulator/boundary.rs
//
// TurnBoundaryBlock accumulator + NoticeBlock detection.
// Assembles TurnBoundaryBlock from system.turn_duration, system.stop_hook_summary,
// and accumulated message.usage entries. Detects NoticeBlocks from compact_boundary,
// error/retry entries.

use std::collections::HashMap;

use crate::block_types::*;
use crate::pricing::{self, TokenUsage};

/// Accumulates data for a TurnBoundaryBlock across multiple JSONL entries.
pub struct TurnBoundaryAccumulator {
    /// Summed tokens across all models for this turn.
    total_usage: TokenUsage,
    /// Per-model accumulated tokens. Typed so cache_creation 5m/1h breakdown
    /// flows all the way through to `pricing::calculate_cost()`.
    model_usage: HashMap<String, TokenUsage>,
    duration_ms: Option<u64>,
    stop_reason: Option<String>,
    hook_summary: Option<serde_json::Value>,
    has_error: bool,
    error: Option<TurnError>,
    num_turns: u32,
    // Hook detail fields (GAP 3)
    hook_infos: Vec<serde_json::Value>,
    hook_errors: Vec<String>,
    hook_count: Option<u32>,
    prevented_continuation: Option<bool>,
    // Fields extracted from hook_summary (GAP 4)
    result: Option<String>,
    fast_mode_state: Option<String>,
    duration_api_ms: Option<u64>,
}

impl Default for TurnBoundaryAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnBoundaryAccumulator {
    pub fn new() -> Self {
        Self {
            total_usage: TokenUsage::default(),
            model_usage: HashMap::new(),
            duration_ms: None,
            stop_reason: None,
            hook_summary: None,
            has_error: false,
            error: None,
            num_turns: 0,
            hook_infos: Vec::new(),
            hook_errors: Vec::new(),
            hook_count: None,
            prevented_continuation: None,
            result: None,
            fast_mode_state: None,
            duration_api_ms: None,
        }
    }

    /// Add typed token usage for a model. Accumulates into both the total-usage
    /// summary (across models) and per-model usage. Typed `TokenUsage` means
    /// cache_creation 5m/1h breakdown is preserved end-to-end (fix for the
    /// 1hr-caching cost drift bug).
    pub fn add_usage(&mut self, model: &str, tokens: &TokenUsage) {
        add_tokens(&mut self.total_usage, tokens);
        let entry = self.model_usage.entry(model.to_string()).or_default();
        add_tokens(entry, tokens);
    }

    /// Get accumulated usage (total, per-model).
    pub fn get_usage(&self) -> (&TokenUsage, &HashMap<String, TokenUsage>) {
        (&self.total_usage, &self.model_usage)
    }

    /// Set turn duration from system.turn_duration entry.
    pub fn set_duration(&mut self, duration_ms: u64) {
        self.duration_ms = Some(duration_ms);
    }

    /// Set hook summary from system.stop_hook_summary entry.
    /// Extracts stopReason from the summary.
    pub fn set_hook_summary(&mut self, summary: &serde_json::Value) {
        self.stop_reason = summary
            .get("stopReason")
            .and_then(|v| v.as_str())
            .map(String::from);

        // GAP 3: Extract hook detail fields
        if let Some(arr) = summary.get("hookInfos").and_then(|v| v.as_array()) {
            self.hook_infos = arr.clone();
        }
        if let Some(arr) = summary.get("hookErrors").and_then(|v| v.as_array()) {
            self.hook_errors = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        self.hook_count = summary
            .get("hookCount")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        self.prevented_continuation = summary
            .get("preventedContinuation")
            .and_then(|v| v.as_bool());

        // GAP 4: Extract result, fastModeState, durationApiMs
        self.result = summary
            .get("result")
            .and_then(|v| v.as_str())
            .map(String::from);
        self.fast_mode_state = summary
            .get("fastModeState")
            .and_then(|v| v.as_str())
            .map(String::from);
        self.duration_api_ms = summary.get("durationApiMs").and_then(|v| v.as_u64());

        self.hook_summary = Some(summary.clone());
    }

    pub fn set_error(&mut self, error: TurnError) {
        self.has_error = true;
        self.error = Some(error);
    }

    pub fn increment_turns(&mut self) {
        self.num_turns += 1;
    }

    pub fn has_duration(&self) -> bool {
        self.duration_ms.is_some()
    }

    /// Build a complete TurnBoundaryBlock (when we have both duration + hook summary).
    pub fn build(&self, id: String) -> Option<TurnBoundaryBlock> {
        let duration_ms = self.duration_ms?;
        let total_cost_usd = self.compute_total_cost();

        Some(TurnBoundaryBlock {
            id,
            success: !self.has_error && self.hook_summary.is_some(),
            total_cost_usd,
            num_turns: self.num_turns,
            duration_ms,
            duration_api_ms: self.duration_api_ms,
            usage: serialize_total_usage(&self.total_usage),
            model_usage: serialize_model_usage(&self.model_usage),
            permission_denials: Vec::new(),
            result: self.result.clone(),
            structured_output: None,
            stop_reason: self.stop_reason.clone(),
            fast_mode_state: self.fast_mode_state.clone(),
            error: self.error.clone(),
            hook_infos: self.hook_infos.clone(),
            hook_errors: self.hook_errors.clone(),
            hook_count: self.hook_count,
            prevented_continuation: self.prevented_continuation,
        })
    }

    /// Build a partial TurnBoundaryBlock (interrupted session -- no hook summary).
    pub fn build_partial(&self, id: String) -> Option<TurnBoundaryBlock> {
        let duration_ms = self.duration_ms.unwrap_or(0);

        // Must have at least some data to build
        if self.duration_ms.is_none() && is_empty_usage(&self.total_usage) {
            return None;
        }

        let total_cost_usd = self.compute_total_cost();

        Some(TurnBoundaryBlock {
            id,
            success: false, // partial = not successful
            total_cost_usd,
            num_turns: self.num_turns,
            duration_ms,
            duration_api_ms: self.duration_api_ms,
            usage: serialize_total_usage(&self.total_usage),
            model_usage: serialize_model_usage(&self.model_usage),
            permission_denials: Vec::new(),
            result: self.result.clone(),
            structured_output: None,
            stop_reason: self.stop_reason.clone(),
            fast_mode_state: self.fast_mode_state.clone(),
            error: self.error.clone(),
            hook_infos: self.hook_infos.clone(),
            hook_errors: self.hook_errors.clone(),
            hook_count: self.hook_count,
            prevented_continuation: self.prevented_continuation,
        })
    }

    /// Compute the total cost by running `pricing::calculate_cost()` per-model
    /// with the typed `TokenUsage` that carries the 5m/1h breakdown.
    ///
    /// This replaces the previous private `calculate_cost()` method which hardcoded
    /// 5m/1h tokens to 0, producing a ~37.5% undercharge whenever Claude Code used
    /// 1-hour caching (its default).
    fn compute_total_cost(&self) -> f64 {
        let pricing_table = pricing::load_pricing();
        self.model_usage
            .iter()
            .map(|(model, tokens)| {
                pricing::calculate_cost(tokens, Some(model.as_str()), &pricing_table).total_usd
            })
            .sum()
    }

    /// Reset for next turn.
    pub fn reset(&mut self) {
        self.total_usage = TokenUsage::default();
        self.model_usage.clear();
        self.duration_ms = None;
        self.stop_reason = None;
        self.hook_summary = None;
        self.has_error = false;
        self.error = None;
        self.num_turns = 0;
        self.hook_infos.clear();
        self.hook_errors.clear();
        self.hook_count = None;
        self.prevented_continuation = None;
        self.result = None;
        self.fast_mode_state = None;
        self.duration_api_ms = None;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Add `src` tokens into `dst`, field-wise. `total_tokens` is recomputed as the
/// sum of the four primary fields (input, output, cache_read, cache_creation).
fn add_tokens(dst: &mut TokenUsage, src: &TokenUsage) {
    dst.input_tokens += src.input_tokens;
    dst.output_tokens += src.output_tokens;
    dst.cache_read_tokens += src.cache_read_tokens;
    dst.cache_creation_tokens += src.cache_creation_tokens;
    dst.cache_creation_5m_tokens += src.cache_creation_5m_tokens;
    dst.cache_creation_1hr_tokens += src.cache_creation_1hr_tokens;
    dst.total_tokens =
        dst.input_tokens + dst.output_tokens + dst.cache_read_tokens + dst.cache_creation_tokens;
}

fn is_empty_usage(tokens: &TokenUsage) -> bool {
    tokens.input_tokens == 0
        && tokens.output_tokens == 0
        && tokens.cache_read_tokens == 0
        && tokens.cache_creation_tokens == 0
        && tokens.cache_creation_5m_tokens == 0
        && tokens.cache_creation_1hr_tokens == 0
}

/// Serialise `total_usage` to the existing wire shape: `HashMap<String, u64>`
/// with snake_case JSONL keys. Keeps backwards-compatible with the frontend's
/// `TurnBoundaryBlock.usage: Record<string, number>` consumer. 5m/1h fields are
/// NOT included here because they were never in the existing wire contract for
/// the `usage` field (they live per-model inside `model_usage`).
fn serialize_total_usage(tokens: &TokenUsage) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    if tokens.input_tokens > 0 {
        out.insert("input_tokens".to_string(), tokens.input_tokens);
    }
    if tokens.output_tokens > 0 {
        out.insert("output_tokens".to_string(), tokens.output_tokens);
    }
    if tokens.cache_read_tokens > 0 {
        out.insert(
            "cache_read_input_tokens".to_string(),
            tokens.cache_read_tokens,
        );
    }
    if tokens.cache_creation_tokens > 0 {
        out.insert(
            "cache_creation_input_tokens".to_string(),
            tokens.cache_creation_tokens,
        );
    }
    out
}

/// Serialise `model_usage` to `HashMap<String, serde_json::Value>` so the
/// existing wire contract (`TurnBoundaryBlock.model_usage: Record<string, any>`)
/// is preserved. Each value is the `TokenUsage` struct serialised as camelCase
/// JSON, which is additive to the existing `ModelUsageInfo` shape on the TS
/// side — new 5m/1h fields appear but existing consumers ignore them.
fn serialize_model_usage(
    model_usage: &HashMap<String, TokenUsage>,
) -> HashMap<String, serde_json::Value> {
    model_usage
        .iter()
        .map(|(model, tokens)| {
            (
                model.clone(),
                serde_json::to_value(tokens).expect("TokenUsage always serializable"),
            )
        })
        .collect()
}

/// Detect a NoticeBlock from a system entry subtype.
pub fn detect_notice_from_system(subtype: &str, entry: &serde_json::Value) -> Option<NoticeBlock> {
    match subtype {
        "compact_boundary" | "microcompact_boundary" => Some(NoticeBlock {
            id: format!("notice-{}", uuid::Uuid::new_v4()),
            variant: NoticeVariant::ContextCompacted,
            data: entry.clone(),
            retry_in_ms: None,
            retry_attempt: None,
            max_retries: None,
        }),
        _ => None,
    }
}

/// Detect a NoticeBlock from an assistant entry with error/retry fields.
pub fn detect_notice_from_assistant_error(entry: &serde_json::Value) -> Option<NoticeBlock> {
    if entry.get("error").is_some() || entry.get("retryInMs").is_some() {
        let variant = if entry.get("retryInMs").is_some() {
            NoticeVariant::RateLimit
        } else {
            NoticeVariant::Error
        };

        let retry_in_ms = entry.get("retryInMs").and_then(|v| v.as_u64());
        let retry_attempt = entry
            .get("retryAttempt")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let max_retries = entry
            .get("maxRetries")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);

        Some(NoticeBlock {
            id: format!("notice-{}", uuid::Uuid::new_v4()),
            variant,
            data: entry.clone(),
            retry_in_ms,
            retry_attempt,
            max_retries,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(input: u64, output: u64, cache_read: u64, cache_creation: u64) -> TokenUsage {
        TokenUsage {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_creation_tokens: cache_creation,
            cache_creation_5m_tokens: 0,
            cache_creation_1hr_tokens: 0,
            total_tokens: input + output + cache_read + cache_creation,
        }
    }

    #[test]
    fn accumulate_usage_single_model() {
        let mut acc = TurnBoundaryAccumulator::new();
        acc.add_usage("claude-sonnet-4-6", &tokens(100, 50, 20, 0));
        let (total, model) = acc.get_usage();
        assert_eq!(total.input_tokens, 100);
        assert_eq!(total.output_tokens, 50);
        assert_eq!(total.cache_read_tokens, 20);
        assert!(model.contains_key("claude-sonnet-4-6"));
    }

    #[test]
    fn accumulate_usage_multiple_models() {
        let mut acc = TurnBoundaryAccumulator::new();
        acc.add_usage("claude-sonnet-4-6", &tokens(100, 50, 0, 0));
        acc.add_usage("claude-haiku-4-5-20251001", &tokens(200, 100, 0, 0));
        let (total, model) = acc.get_usage();
        assert_eq!(total.input_tokens, 300);
        assert_eq!(total.output_tokens, 150);
        assert_eq!(model.len(), 2);
    }

    #[test]
    fn build_boundary_from_parts() {
        let mut acc = TurnBoundaryAccumulator::new();
        acc.add_usage("claude-sonnet-4-6", &tokens(1000, 500, 0, 0));
        acc.set_duration(45566);
        acc.set_hook_summary(&serde_json::json!({
            "stopReason": "end_turn",
            "hookInfos": [],
            "hookErrors": [],
            "hookCount": 0
        }));
        let block = acc.build("tb-1".into());
        assert!(block.is_some());
        let block = block.unwrap();
        assert!(block.success);
        assert_eq!(block.duration_ms, 45566);
        assert_eq!(block.stop_reason, Some("end_turn".to_string()));
    }

    #[test]
    fn build_partial_boundary_no_hook_summary() {
        let mut acc = TurnBoundaryAccumulator::new();
        acc.set_duration(30000);
        acc.add_usage("claude-sonnet-4-6", &tokens(500, 0, 0, 0));
        let block = acc.build_partial("tb-2".into());
        assert!(block.is_some());
        let block = block.unwrap();
        assert!(!block.success); // partial = not successful
        assert_eq!(block.duration_ms, 30000);
        assert_eq!(block.stop_reason, None); // no hook summary = no stop reason
    }

    // ── Regression tests for 1hr caching cost bug ───────────────────────

    #[test]
    fn turn_boundary_applies_1hr_rate_not_5m_rate() {
        // Regression test: pre-fix, 1hr cache_creation tokens were charged at
        // the 5m rate ($6.25/MTok on opus-4-6) instead of the 1hr rate
        // ($10/MTok), producing a 37.5% undercharge. This test locks in the
        // correct 1hr pricing.
        let mut acc = TurnBoundaryAccumulator::new();
        let t = TokenUsage {
            cache_creation_tokens: 100_000,
            cache_creation_1hr_tokens: 100_000,
            total_tokens: 100_000,
            ..Default::default()
        };
        acc.add_usage("claude-opus-4-6", &t);
        acc.set_duration(1000);
        acc.set_hook_summary(&serde_json::json!({
            "stopReason": "end_turn", "hookInfos": [], "hookErrors": [], "hookCount": 0
        }));

        let block = acc.build("tb-1".into()).unwrap();

        // Opus 4.6: 100k at 1hr rate ($10/MTok) = $1.00
        assert!(
            (block.total_cost_usd - 1.0).abs() < 0.001,
            "expected $1.00 (1hr rate), got ${}",
            block.total_cost_usd
        );
        // Pre-fix this would have been $0.625 (5m rate). Guard against regression.
        assert!(
            block.total_cost_usd > 0.8,
            "1hr rate must be applied, got ${}",
            block.total_cost_usd
        );
    }

    #[test]
    fn turn_boundary_applies_mixed_5m_and_1hr_rates() {
        let mut acc = TurnBoundaryAccumulator::new();
        let t = TokenUsage {
            cache_creation_tokens: 200_000,
            cache_creation_5m_tokens: 100_000,
            cache_creation_1hr_tokens: 100_000,
            total_tokens: 200_000,
            ..Default::default()
        };
        acc.add_usage("claude-opus-4-6", &t);
        acc.set_duration(1000);
        acc.set_hook_summary(&serde_json::json!({
            "stopReason": "end_turn", "hookInfos": [], "hookErrors": [], "hookCount": 0
        }));

        let block = acc.build("tb-1".into()).unwrap();
        // 100k at 5m rate ($6.25/MTok) = $0.625
        // 100k at 1hr rate ($10/MTok) = $1.00
        // total = $1.625
        assert!(
            (block.total_cost_usd - 1.625).abs() < 0.001,
            "expected $1.625 (mixed rates), got ${}",
            block.total_cost_usd
        );
    }

    #[test]
    fn wire_format_model_usage_is_camelcase_serde_json() {
        // Verifies the C3 wire-format guarantee: model_usage serialises as
        // camelCase JSON with additive 5m/1h fields.
        let mut acc = TurnBoundaryAccumulator::new();
        let t = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 1000,
            cache_creation_1hr_tokens: 1000,
            total_tokens: 1150,
            ..Default::default()
        };
        acc.add_usage("claude-opus-4-6", &t);
        acc.set_duration(1000);
        acc.set_hook_summary(&serde_json::json!({
            "stopReason": "end_turn", "hookInfos": [], "hookErrors": [], "hookCount": 0
        }));

        let block = acc.build("tb-1".into()).unwrap();
        let model_entry = block.model_usage.get("claude-opus-4-6").unwrap();
        // TokenUsage serialises as camelCase via #[serde(rename_all = "camelCase")]
        assert_eq!(model_entry["inputTokens"].as_u64(), Some(100));
        assert_eq!(model_entry["outputTokens"].as_u64(), Some(50));
        assert_eq!(model_entry["cacheCreationTokens"].as_u64(), Some(1000));
        assert_eq!(model_entry["cacheCreation1hrTokens"].as_u64(), Some(1000));
        assert_eq!(model_entry["cacheCreation5mTokens"].as_u64(), Some(0));
    }

    #[test]
    fn wire_format_total_usage_is_snake_case_for_backcompat() {
        // The flat `usage` field keeps snake_case JSONL keys that existing
        // frontend consumers read (input_tokens, output_tokens, ...).
        let mut acc = TurnBoundaryAccumulator::new();
        acc.add_usage("claude-opus-4-6", &tokens(100, 50, 20, 0));
        acc.set_duration(1000);
        acc.set_hook_summary(&serde_json::json!({
            "stopReason": "end_turn", "hookInfos": [], "hookErrors": [], "hookCount": 0
        }));
        let block = acc.build("tb-1".into()).unwrap();
        assert_eq!(*block.usage.get("input_tokens").unwrap(), 100);
        assert_eq!(*block.usage.get("output_tokens").unwrap(), 50);
        assert_eq!(*block.usage.get("cache_read_input_tokens").unwrap(), 20);
    }

    #[test]
    fn detect_compact_boundary_notice() {
        let entry = serde_json::json!({
            "type": "system",
            "content": "Context compacted",
            "compactMetadata": {"trigger": "auto", "preTokens": 50000}
        });
        let notice = detect_notice_from_system("compact_boundary", &entry);
        assert!(notice.is_some());
        let notice = notice.unwrap();
        assert_eq!(notice.variant, NoticeVariant::ContextCompacted);
    }

    #[test]
    fn detect_microcompact_boundary_notice() {
        let entry = serde_json::json!({
            "type": "system",
            "microcompactMetadata": {"trigger": "tool_output", "preTokens": 10000}
        });
        let notice = detect_notice_from_system("microcompact_boundary", &entry);
        assert!(notice.is_some());
        let notice = notice.unwrap();
        assert_eq!(notice.variant, NoticeVariant::ContextCompacted);
    }

    #[test]
    fn detect_error_retry_notice() {
        let entry = serde_json::json!({
            "type": "assistant",
            "error": {"message": "rate limited"},
            "isApiErrorMessage": true,
            "retryInMs": 5000,
            "retryAttempt": 1,
            "maxRetries": 3
        });
        let notice = detect_notice_from_assistant_error(&entry);
        assert!(notice.is_some());
        let notice = notice.unwrap();
        assert_eq!(notice.variant, NoticeVariant::RateLimit);
    }
}
