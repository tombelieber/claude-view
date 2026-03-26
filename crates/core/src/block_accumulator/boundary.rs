// crates/core/src/block_accumulator/boundary.rs
//
// TurnBoundaryBlock accumulator + NoticeBlock detection.
// Assembles TurnBoundaryBlock from system.turn_duration, system.stop_hook_summary,
// and accumulated message.usage entries. Detects NoticeBlocks from compact_boundary,
// error/retry entries.

use std::collections::HashMap;

use crate::block_types::*;
use crate::pricing;

/// Accumulates data for a TurnBoundaryBlock across multiple JSONL entries.
pub struct TurnBoundaryAccumulator {
    total_usage: HashMap<String, u64>,
    model_usage: HashMap<String, serde_json::Value>,
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
            total_usage: HashMap::new(),
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

    /// Add usage from an assistant message's message.usage object.
    /// Usage fields: input_tokens, output_tokens, cache_read_input_tokens, etc.
    /// Accumulates into total_usage (summed) and model_usage (per-model).
    pub fn add_usage(&mut self, model: &str, usage: &serde_json::Value) {
        if let Some(obj) = usage.as_object() {
            for (key, val) in obj {
                if let Some(n) = val.as_u64() {
                    *self.total_usage.entry(key.clone()).or_insert(0) += n;
                }
            }
        }

        // Store per-model usage (last write wins per model, or merge)
        let model_entry = self
            .model_usage
            .entry(model.to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let (Some(existing), Some(new_obj)) = (model_entry.as_object_mut(), usage.as_object()) {
            for (key, val) in new_obj {
                if let Some(n) = val.as_u64() {
                    let prev = existing.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
                    existing.insert(key.clone(), serde_json::json!(prev + n));
                }
            }
        }
    }

    /// Get accumulated usage (total, per-model).
    pub fn get_usage(&self) -> (&HashMap<String, u64>, &HashMap<String, serde_json::Value>) {
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

        let total_cost_usd = self.calculate_cost();

        Some(TurnBoundaryBlock {
            id,
            success: !self.has_error && self.hook_summary.is_some(),
            total_cost_usd,
            num_turns: self.num_turns,
            duration_ms,
            duration_api_ms: self.duration_api_ms,
            usage: self.total_usage.clone(),
            model_usage: self.model_usage.clone(),
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
        if self.duration_ms.is_none() && self.total_usage.is_empty() {
            return None;
        }

        let total_cost_usd = self.calculate_cost();

        Some(TurnBoundaryBlock {
            id,
            success: false, // partial = not successful
            total_cost_usd,
            num_turns: self.num_turns,
            duration_ms,
            duration_api_ms: self.duration_api_ms,
            usage: self.total_usage.clone(),
            model_usage: self.model_usage.clone(),
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

    /// Reset for next turn.
    pub fn reset(&mut self) {
        self.total_usage.clear();
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

    /// Calculate cost from accumulated model_usage using default pricing.
    fn calculate_cost(&self) -> f64 {
        let pricing_table = pricing::load_pricing();

        let mut total_cost = 0.0;
        for (model, usage_val) in &self.model_usage {
            let input = usage_val
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let output = usage_val
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let cache_read = usage_val
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let cache_creation = usage_val
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let tokens = pricing::TokenUsage {
                input_tokens: input,
                output_tokens: output,
                cache_read_tokens: cache_read,
                cache_creation_tokens: cache_creation,
                cache_creation_5m_tokens: 0,
                cache_creation_1hr_tokens: 0,
                total_tokens: input + output + cache_read + cache_creation,
            };
            let breakdown = pricing::calculate_cost(&tokens, Some(model.as_str()), &pricing_table);
            total_cost += breakdown.total_usd;
        }
        total_cost
    }
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

    #[test]
    fn accumulate_usage_single_model() {
        let mut acc = TurnBoundaryAccumulator::new();
        let usage = serde_json::json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_input_tokens": 20
        });
        acc.add_usage("claude-sonnet-4-6", &usage);
        let (total, model) = acc.get_usage();
        assert_eq!(*total.get("input_tokens").unwrap(), 100u64);
        assert_eq!(*total.get("output_tokens").unwrap(), 50u64);
        assert!(model.contains_key("claude-sonnet-4-6"));
    }

    #[test]
    fn accumulate_usage_multiple_models() {
        let mut acc = TurnBoundaryAccumulator::new();
        acc.add_usage(
            "claude-sonnet-4-6",
            &serde_json::json!({"input_tokens": 100, "output_tokens": 50}),
        );
        acc.add_usage(
            "claude-haiku-4-5-20251001",
            &serde_json::json!({"input_tokens": 200, "output_tokens": 100}),
        );
        let (total, model) = acc.get_usage();
        assert_eq!(*total.get("input_tokens").unwrap(), 300u64);
        assert_eq!(*total.get("output_tokens").unwrap(), 150u64);
        assert_eq!(model.len(), 2);
    }

    #[test]
    fn build_boundary_from_parts() {
        let mut acc = TurnBoundaryAccumulator::new();
        acc.add_usage(
            "claude-sonnet-4-6",
            &serde_json::json!({"input_tokens": 1000, "output_tokens": 500}),
        );
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
        acc.add_usage(
            "claude-sonnet-4-6",
            &serde_json::json!({"input_tokens": 500}),
        );
        let block = acc.build_partial("tb-2".into());
        assert!(block.is_some());
        let block = block.unwrap();
        assert!(!block.success); // partial = not successful
        assert_eq!(block.duration_ms, 30000);
        assert_eq!(block.stop_reason, None); // no hook summary = no stop reason
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
