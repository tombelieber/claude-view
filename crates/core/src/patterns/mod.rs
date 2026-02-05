//! Pattern detection engine for analyzing user behavior across sessions.
//!
//! Patterns are computed purely from `&[SessionInfo]` slices (no I/O).
//! Each pattern function examines sessions, computes bucket statistics,
//! and returns an optional `PatternResult` when sufficient data exists.

pub mod session;
pub mod temporal;
pub mod workflow;
pub mod model;
pub mod codebase;
pub mod outcome;
pub mod behavioral;
pub mod comparative;

use std::collections::HashMap;

use crate::insights::scoring::Actionability;
use crate::insights::generator::GeneratedInsight;
use crate::types::SessionInfo;

/// The result of a single pattern calculation.
#[derive(Debug, Clone)]
pub struct PatternResult {
    pub pattern_id: String,
    pub category: String,
    /// Arbitrary key-value data computed by the pattern.
    pub data: HashMap<String, serde_json::Value>,
    /// How many sessions contributed to this result.
    pub sample_size: u32,
    /// How actionable this pattern is.
    pub actionability: Actionability,
}

/// Error types for pattern calculations.
#[derive(Debug, thiserror::Error)]
pub enum PatternError {
    #[error("Insufficient data: need {required} samples, got {actual}")]
    InsufficientData { required: u32, actual: u32 },
    #[error("Invalid calculation: {0}")]
    InvalidCalculation(String),
}

/// A bucket with a label, count, and aggregate metric value.
#[derive(Debug, Clone)]
pub struct Bucket {
    pub label: String,
    pub count: u32,
    pub value: f64,
}

impl Bucket {
    pub fn new(label: impl Into<String>, count: u32, value: f64) -> Self {
        Self {
            label: label.into(),
            count,
            value,
        }
    }
}

/// Helper: compute the average of an iterator of f64 values.
pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Helper: compute relative improvement as `(baseline - better) / baseline`.
/// Returns a positive value when `better < baseline` (lower is better for re-edit rate).
pub fn relative_improvement(better: f64, baseline: f64) -> f64 {
    if baseline == 0.0 {
        return 0.0;
    }
    (baseline - better) / baseline
}

/// Helper: find the bucket with the lowest value (best for re-edit rate metrics).
pub fn best_bucket(buckets: &[Bucket]) -> Option<&Bucket> {
    buckets
        .iter()
        .filter(|b| b.count > 0)
        .min_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
}

/// Helper: find the bucket with the highest value (worst for re-edit rate metrics).
pub fn worst_bucket(buckets: &[Bucket]) -> Option<&Bucket> {
    buckets
        .iter()
        .filter(|b| b.count > 0)
        .max_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
}

/// Run all pattern calculations on a slice of sessions and return generated insights.
///
/// This is the main entry point for the pattern engine. It runs all pattern
/// categories and collects results that meet minimum sample requirements.
pub fn calculate_all_patterns(
    sessions: &[SessionInfo],
    time_range_days: u32,
) -> Vec<GeneratedInsight> {
    let mut insights = Vec::new();

    // Session patterns
    insights.extend(session::calculate_session_patterns(sessions, time_range_days));

    // Temporal patterns
    insights.extend(temporal::calculate_temporal_patterns(sessions, time_range_days));

    // Workflow patterns
    insights.extend(workflow::calculate_workflow_patterns(sessions, time_range_days));

    // Model patterns
    insights.extend(model::calculate_model_patterns(sessions, time_range_days));

    // Codebase patterns
    insights.extend(codebase::calculate_codebase_patterns(sessions, time_range_days));

    // Outcome patterns
    insights.extend(outcome::calculate_outcome_patterns(sessions, time_range_days));

    // Behavioral patterns
    insights.extend(behavioral::calculate_behavioral_patterns(sessions, time_range_days));

    // Comparative patterns
    insights.extend(comparative::calculate_comparative_patterns(sessions, time_range_days));

    insights
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::types::{SessionInfo, ToolCounts};

    /// Create a minimal test session with default values.
    pub fn make_session(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: "test-project".to_string(),
            project_path: "/test/project".to_string(),
            file_path: format!("/tmp/{}.jsonl", id),
            modified_at: 1700000000,
            size_bytes: 1024,
            preview: "Test".to_string(),
            last_message: "Test".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: Some(5000),
            total_output_tokens: Some(2500),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: Some(10),
            primary_model: Some("claude-sonnet-4".to_string()),
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 5,
            files_edited_count: 3,
            reedited_files_count: 1,
            duration_seconds: 600,
            commit_count: 1,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            summary_text: None,
            parse_version: 1,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
        }
    }

    /// Create a session with specific duration and reedit stats.
    pub fn make_session_with_stats(
        id: &str,
        duration_seconds: u32,
        files_edited: u32,
        reedited: u32,
        turn_count: usize,
        commit_count: u32,
    ) -> SessionInfo {
        let mut s = make_session(id);
        s.duration_seconds = duration_seconds;
        s.files_edited_count = files_edited;
        s.reedited_files_count = reedited;
        s.turn_count = turn_count;
        s.commit_count = commit_count;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean_normal() {
        assert_eq!(mean(&[1.0, 2.0, 3.0]), Some(2.0));
    }

    #[test]
    fn test_mean_empty() {
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn test_mean_single() {
        assert_eq!(mean(&[42.0]), Some(42.0));
    }

    #[test]
    fn test_relative_improvement() {
        // Lower is better (e.g., re-edit rate)
        // better = 0.2, baseline = 0.4 -> 50% improvement
        let improvement = relative_improvement(0.2, 0.4);
        assert!((improvement - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_relative_improvement_zero_baseline() {
        assert_eq!(relative_improvement(0.2, 0.0), 0.0);
    }

    #[test]
    fn test_best_and_worst_bucket() {
        let buckets = vec![
            Bucket::new("A", 10, 0.5),
            Bucket::new("B", 20, 0.2),
            Bucket::new("C", 15, 0.8),
        ];
        assert_eq!(best_bucket(&buckets).unwrap().label, "B");
        assert_eq!(worst_bucket(&buckets).unwrap().label, "C");
    }

    #[test]
    fn test_calculate_all_patterns_empty() {
        let insights = calculate_all_patterns(&[], 30);
        assert!(insights.is_empty());
    }
}
