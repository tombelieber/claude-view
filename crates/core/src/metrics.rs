// crates/core/src/metrics.rs
//! Derived metrics computed on read (not stored in the database).
//!
//! All functions return `Option<f64>` to handle division by zero gracefully.
//! Metrics are computed from atomic units stored in the session.

/// A2.1 Tokens Per Prompt
///
/// Formula: (total_input_tokens + total_output_tokens) / user_prompt_count
///
/// Returns `None` if `user_prompt_count` is 0 (division by zero).
pub fn tokens_per_prompt(total_input: u64, total_output: u64, user_prompt_count: u32) -> Option<f64> {
    if user_prompt_count == 0 {
        return None;
    }
    let total_tokens = total_input + total_output;
    Some(total_tokens as f64 / user_prompt_count as f64)
}

/// A2.2 Re-edit Rate
///
/// Formula: reedited_files_count / files_edited_count
///
/// Measures how often files are edited multiple times within a session.
/// Value range: 0.0 to 1.0 (or higher if a file is edited 3+ times).
///
/// Returns `None` if `files_edited_count` is 0 (division by zero).
pub fn reedit_rate(reedited_files_count: u32, files_edited_count: u32) -> Option<f64> {
    if files_edited_count == 0 {
        return None;
    }
    Some(reedited_files_count as f64 / files_edited_count as f64)
}

/// A2.3 Tool Density
///
/// Formula: tool_call_count / api_call_count
///
/// Measures how many tool calls are made per API request.
/// Higher values indicate more tool-heavy workflows.
///
/// Returns `None` if `api_call_count` is 0 (division by zero).
pub fn tool_density(tool_call_count: u32, api_call_count: u32) -> Option<f64> {
    if api_call_count == 0 {
        return None;
    }
    Some(tool_call_count as f64 / api_call_count as f64)
}

/// A2.4 Edit Velocity
///
/// Formula: files_edited_count / (duration_seconds / 60.0)
///
/// Measures files edited per minute. Higher values indicate faster editing pace.
///
/// Returns `None` if `duration_seconds` is 0 (division by zero).
pub fn edit_velocity(files_edited_count: u32, duration_seconds: u32) -> Option<f64> {
    if duration_seconds == 0 {
        return None;
    }
    let minutes = duration_seconds as f64 / 60.0;
    Some(files_edited_count as f64 / minutes)
}

/// A2.5 Read-to-Edit Ratio
///
/// Formula: files_read_count / files_edited_count
///
/// Measures how many files are read for every file edited.
/// Higher values indicate more exploratory/research-heavy sessions.
///
/// Returns `None` if `files_edited_count` is 0 (division by zero).
pub fn read_to_edit_ratio(files_read_count: u32, files_edited_count: u32) -> Option<f64> {
    if files_edited_count == 0 {
        return None;
    }
    Some(files_read_count as f64 / files_edited_count as f64)
}

/// Format a `std::time::Duration` with smart unit selection:
/// - < 1ms → microseconds (e.g. "342µs")
/// - 1ms..999ms → milliseconds (e.g. "170ms")
/// - >= 1s → seconds with 2 decimal places (e.g. "1.23s")
pub fn format_duration(d: std::time::Duration) -> String {
    let micros = d.as_micros();
    if micros < 1_000 {
        format!("{}µs", micros)
    } else if d.as_millis() < 1_000 {
        format!("{}ms", d.as_millis())
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

/// Helper to round a metric to 2 decimal places for display.
pub fn round_for_display(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // A2.1 Tokens Per Prompt
    // ========================================================================

    #[test]
    fn test_tokens_per_prompt_normal() {
        // 1000 input + 500 output = 1500 tokens, 10 prompts = 150 tokens/prompt
        let result = tokens_per_prompt(1000, 500, 10);
        assert_eq!(result, Some(150.0));
    }

    #[test]
    fn test_tokens_per_prompt_large_values() {
        // 1M input + 500K output = 1.5M tokens, 100 prompts
        let result = tokens_per_prompt(1_000_000, 500_000, 100);
        assert_eq!(result, Some(15_000.0));
    }

    #[test]
    fn test_tokens_per_prompt_zero_prompts() {
        // Division by zero case
        let result = tokens_per_prompt(1000, 500, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_tokens_per_prompt_zero_tokens() {
        // Zero tokens is valid
        let result = tokens_per_prompt(0, 0, 10);
        assert_eq!(result, Some(0.0));
    }

    #[test]
    fn test_tokens_per_prompt_fractional_result() {
        // 100 tokens / 3 prompts = 33.333...
        let result = tokens_per_prompt(100, 0, 3);
        assert!(result.is_some());
        let value = result.unwrap();
        assert!((value - 33.333333).abs() < 0.001);
    }

    // ========================================================================
    // A2.2 Re-edit Rate
    // ========================================================================

    #[test]
    fn test_reedit_rate_normal() {
        // 5 files edited, 2 re-edited = 0.4
        let result = reedit_rate(2, 5);
        assert_eq!(result, Some(0.4));
    }

    #[test]
    fn test_reedit_rate_no_reedits() {
        // 5 files edited, 0 re-edited = 0.0
        let result = reedit_rate(0, 5);
        assert_eq!(result, Some(0.0));
    }

    #[test]
    fn test_reedit_rate_all_reedits() {
        // 5 files edited, all 5 re-edited = 1.0
        let result = reedit_rate(5, 5);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn test_reedit_rate_zero_files_edited() {
        // Division by zero case
        let result = reedit_rate(2, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_reedit_rate_high_reedits() {
        // Edge case: more reedits than files (if counting multiple re-edits)
        // 10 reedits on 5 files = 2.0
        let result = reedit_rate(10, 5);
        assert_eq!(result, Some(2.0));
    }

    // ========================================================================
    // A2.3 Tool Density
    // ========================================================================

    #[test]
    fn test_tool_density_normal() {
        // 50 tool calls / 10 api calls = 5.0
        let result = tool_density(50, 10);
        assert_eq!(result, Some(5.0));
    }

    #[test]
    fn test_tool_density_no_tools() {
        // 0 tool calls / 10 api calls = 0.0
        let result = tool_density(0, 10);
        assert_eq!(result, Some(0.0));
    }

    #[test]
    fn test_tool_density_zero_api_calls() {
        // Division by zero case
        let result = tool_density(50, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_tool_density_fractional() {
        // 7 tool calls / 3 api calls = 2.333...
        let result = tool_density(7, 3);
        assert!(result.is_some());
        let value = result.unwrap();
        assert!((value - 2.333333).abs() < 0.001);
    }

    // ========================================================================
    // A2.4 Edit Velocity
    // ========================================================================

    #[test]
    fn test_edit_velocity_normal() {
        // 10 files / 600 seconds = 10 files / 10 minutes = 1.0 files/min
        let result = edit_velocity(10, 600);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn test_edit_velocity_fast_edits() {
        // 6 files / 60 seconds = 6.0 files/min
        let result = edit_velocity(6, 60);
        assert_eq!(result, Some(6.0));
    }

    #[test]
    fn test_edit_velocity_slow_session() {
        // 5 files / 3600 seconds = 5 files / 60 minutes = 0.0833...
        let result = edit_velocity(5, 3600);
        assert!(result.is_some());
        let value = result.unwrap();
        assert!((value - 0.0833333).abs() < 0.001);
    }

    #[test]
    fn test_edit_velocity_zero_duration() {
        // Division by zero case
        let result = edit_velocity(10, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_edit_velocity_no_edits() {
        // 0 files / 600 seconds = 0.0
        let result = edit_velocity(0, 600);
        assert_eq!(result, Some(0.0));
    }

    // ========================================================================
    // A2.5 Read-to-Edit Ratio
    // ========================================================================

    #[test]
    fn test_read_to_edit_ratio_normal() {
        // 20 files read / 5 files edited = 4.0
        let result = read_to_edit_ratio(20, 5);
        assert_eq!(result, Some(4.0));
    }

    #[test]
    fn test_read_to_edit_ratio_equal() {
        // 10 files read / 10 files edited = 1.0
        let result = read_to_edit_ratio(10, 10);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn test_read_to_edit_ratio_more_edits() {
        // 5 files read / 10 files edited = 0.5
        let result = read_to_edit_ratio(5, 10);
        assert_eq!(result, Some(0.5));
    }

    #[test]
    fn test_read_to_edit_ratio_no_reads() {
        // 0 files read / 5 files edited = 0.0
        let result = read_to_edit_ratio(0, 5);
        assert_eq!(result, Some(0.0));
    }

    #[test]
    fn test_read_to_edit_ratio_zero_edits() {
        // Division by zero case
        let result = read_to_edit_ratio(20, 0);
        assert_eq!(result, None);
    }

    // ========================================================================
    // Display rounding helper
    // ========================================================================

    #[test]
    fn test_round_for_display() {
        assert_eq!(round_for_display(1.234567), 1.23);
        assert_eq!(round_for_display(1.235), 1.24); // rounds up
        assert_eq!(round_for_display(1.0), 1.0);
        assert_eq!(round_for_display(0.005), 0.01); // rounds up
        assert_eq!(round_for_display(0.004), 0.0); // rounds down
        assert_eq!(round_for_display(100.999), 101.0);
    }

    // ========================================================================
    // format_duration (smart unit selection)
    // ========================================================================

    #[test]
    fn test_format_duration_microseconds() {
        use std::time::Duration;
        assert_eq!(format_duration(Duration::from_micros(0)), "0µs");
        assert_eq!(format_duration(Duration::from_micros(1)), "1µs");
        assert_eq!(format_duration(Duration::from_micros(342)), "342µs");
        assert_eq!(format_duration(Duration::from_micros(999)), "999µs");
    }

    #[test]
    fn test_format_duration_milliseconds() {
        use std::time::Duration;
        assert_eq!(format_duration(Duration::from_millis(1)), "1ms");
        assert_eq!(format_duration(Duration::from_millis(170)), "170ms");
        assert_eq!(format_duration(Duration::from_millis(999)), "999ms");
    }

    #[test]
    fn test_format_duration_seconds() {
        use std::time::Duration;
        assert_eq!(format_duration(Duration::from_millis(1000)), "1.00s");
        assert_eq!(format_duration(Duration::from_millis(1230)), "1.23s");
        assert_eq!(format_duration(Duration::from_millis(2500)), "2.50s");
        assert_eq!(format_duration(Duration::from_secs(60)), "60.00s");
    }

    #[test]
    fn test_format_duration_boundary() {
        use std::time::Duration;
        // Exactly 1ms boundary: 1000µs → should show as ms
        assert_eq!(format_duration(Duration::from_micros(1000)), "1ms");
        // Exactly 1s boundary: 1000ms → should show as seconds
        assert_eq!(format_duration(Duration::from_millis(1000)), "1.00s");
    }

    // ========================================================================
    // Integration: All metrics on zero inputs
    // ========================================================================

    #[test]
    fn test_all_metrics_division_by_zero() {
        // All should return None when divisor is 0
        assert_eq!(tokens_per_prompt(100, 50, 0), None);
        assert_eq!(reedit_rate(5, 0), None);
        assert_eq!(tool_density(50, 0), None);
        assert_eq!(edit_velocity(10, 0), None);
        assert_eq!(read_to_edit_ratio(20, 0), None);
    }

    // ========================================================================
    // Precision tests (full precision for calculation)
    // ========================================================================

    #[test]
    fn test_full_precision_maintained() {
        // Verify that calculations maintain full f64 precision
        let result = tokens_per_prompt(1, 0, 3);
        assert!(result.is_some());
        let value = result.unwrap();
        // Should be exactly 1/3, not rounded
        assert!((value - 0.3333333333333333).abs() < 1e-15);
    }

    #[test]
    fn test_display_vs_calculation_precision() {
        // Calculation preserves full precision
        let calculated = tokens_per_prompt(1, 0, 3).unwrap();
        assert!((calculated - 0.3333333333333333).abs() < 1e-15);

        // Display rounds to 2 decimal places
        let displayed = round_for_display(calculated);
        assert_eq!(displayed, 0.33);
    }
}
