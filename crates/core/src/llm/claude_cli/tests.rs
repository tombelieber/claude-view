// crates/core/src/llm/claude_cli/tests.rs
//! Unit tests for the Claude CLI provider module.

use crate::llm::types::{ClassificationRequest, LlmError};

use super::parsing::parse_classification_response;
use super::prompt::build_classification_prompt;
use super::provider::ClaudeCliProvider;

// Pull in the LlmProvider trait so we can call `.name()` / `.model()` on the provider.
use crate::llm::provider::LlmProvider;

#[test]
fn test_build_classification_prompt_contains_all_parts() {
    let request = ClassificationRequest {
        session_id: "sess-123".to_string(),
        first_prompt: "Fix the authentication bug in login.rs".to_string(),
        files_touched: vec!["src/login.rs".to_string(), "src/auth.rs".to_string()],
        skills_used: vec!["/commit".to_string(), "/review-pr".to_string()],
    };

    let prompt = build_classification_prompt(&request);

    // Should contain the first prompt
    assert!(prompt.contains("Fix the authentication bug in login.rs"));
    // Should contain files
    assert!(prompt.contains("src/login.rs, src/auth.rs"));
    // Should contain skills
    assert!(prompt.contains("/commit, /review-pr"));
    // Should contain category taxonomy
    assert!(prompt.contains("code_work"));
    assert!(prompt.contains("bugfix"));
    assert!(prompt.contains("error-fix"));
}

#[test]
fn test_build_classification_prompt_empty_lists() {
    let request = ClassificationRequest {
        session_id: "sess-456".to_string(),
        first_prompt: "What is Rust?".to_string(),
        files_touched: vec![],
        skills_used: vec![],
    };

    let prompt = build_classification_prompt(&request);
    assert!(prompt.contains("What is Rust?"));
    assert!(prompt.contains("Files touched: \n"));
}

#[test]
fn test_parse_classification_response_direct_json() {
    let json = serde_json::json!({
        "category_l1": "code_work",
        "category_l2": "bugfix",
        "category_l3": "error-fix",
        "confidence": 0.95
    });

    let resp = parse_classification_response(json).unwrap();
    assert_eq!(resp.category_l1, "code_work");
    assert_eq!(resp.category_l2, "bugfix");
    assert_eq!(resp.category_l3, "error-fix");
    assert!((resp.confidence - 0.95).abs() < f64::EPSILON);
}

#[test]
fn test_parse_classification_response_claude_cli_wrapper() {
    // Claude CLI wraps output as: { "result": "{ json string }" }
    let json = serde_json::json!({
        "result": r#"{"category_l1": "thinking_work", "category_l2": "explanation", "category_l3": "code-understanding", "confidence": 0.88}"#
    });

    let resp = parse_classification_response(json).unwrap();
    assert_eq!(resp.category_l1, "thinking_work");
    assert_eq!(resp.category_l2, "explanation");
    assert_eq!(resp.category_l3, "code-understanding");
    assert!((resp.confidence - 0.88).abs() < f64::EPSILON);
}

#[test]
fn test_parse_classification_response_wrapper_extracts_telemetry() {
    let json = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "result": r#"{"category_l1":"code_work","category_l2":"bugfix","category_l3":"error-fix","confidence":0.91}"#,
        "usage": {
            "input_tokens": 1200,
            "output_tokens": 340,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 45000
        },
        "modelUsage": {
            "claude-haiku-4-5-20251001": {
                "inputTokens": 1200,
                "outputTokens": 340
            }
        },
        "total_cost_usd": 0.006163
    });

    let resp = parse_classification_response(json).unwrap();
    assert_eq!(resp.category_l1, "code_work");
    assert_eq!(resp.total_cost_usd, Some(0.006163));
    assert_eq!(resp.model.as_deref(), Some("claude-haiku-4-5-20251001"));
    assert_eq!(resp.total_tokens_used(), Some(46540));
}

#[test]
fn test_parse_classification_response_missing_fields() {
    let json = serde_json::json!({
        "category_l1": "code_work"
    });

    let result = parse_classification_response(json);
    assert!(result.is_err());
}

#[test]
fn test_claude_cli_provider_creation() {
    let provider = ClaudeCliProvider::new("haiku").with_timeout(60);
    assert_eq!(provider.name(), "claude-cli");
    assert_eq!(provider.model(), "haiku");
    assert_eq!(provider.timeout_secs, 60);
}

#[test]
fn test_parse_cli_json_response_extracts_metadata() {
    // Actual Claude CLI --output-format json response shape (verified 2026-02-21)
    let cli_json = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "result": "Here is the report content...",
        "usage": {
            "input_tokens": 1200,
            "output_tokens": 340,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 45000
        },
        "modelUsage": {
            "claude-haiku-4-5-20251001": {
                "inputTokens": 1200,
                "outputTokens": 340
            }
        },
        "total_cost_usd": 0.006163
    });

    let content = cli_json["result"].as_str().unwrap_or("").to_string();
    assert_eq!(content, "Here is the report content...");

    let model = cli_json["modelUsage"]
        .as_object()
        .and_then(|m| m.keys().next().cloned());
    assert_eq!(model.as_deref(), Some("claude-haiku-4-5-20251001"));

    let input_tokens = cli_json["usage"]["input_tokens"].as_u64();
    let output_tokens = cli_json["usage"]["output_tokens"].as_u64();
    assert_eq!(input_tokens, Some(1200));
    assert_eq!(output_tokens, Some(340));
}

#[test]
fn test_parse_cli_json_missing_model_usage_returns_none() {
    let cli_json = serde_json::json!({
        "type": "result",
        "result": "content"
    });

    let model = cli_json["modelUsage"]
        .as_object()
        .and_then(|m| m.keys().next().cloned());
    assert!(model.is_none());

    let input_tokens = cli_json["usage"]["input_tokens"].as_u64();
    assert!(input_tokens.is_none());
}

#[test]
fn test_stream_completion_compiles() {
    // Verify the method exists and has the right signature.
    // We can't test actual CLI invocation in unit tests.
    let _provider = ClaudeCliProvider::new("haiku");
    // Just verify it compiles — calling it would require CLI to be installed
    let _: fn(
        &ClaudeCliProvider,
        String,
    ) -> Result<
        (
            tokio::sync::mpsc::Receiver<String>,
            tokio::task::JoinHandle<Result<(), LlmError>>,
        ),
        LlmError,
    > = ClaudeCliProvider::stream_completion;
}
