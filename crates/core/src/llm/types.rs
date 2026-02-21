// crates/core/src/llm/types.rs
//! Request/response/error types for LLM integration.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Request to classify a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationRequest {
    pub session_id: String,
    pub first_prompt: String,
    pub files_touched: Vec<String>,
    pub skills_used: Vec<String>,
}

/// Classification response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResponse {
    pub category_l1: String,
    pub category_l2: String,
    pub category_l3: String,
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// Request for a general-purpose LLM completion.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub system_prompt: Option<String>,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub response_format: ResponseFormat,
}

/// Desired response format for a completion request.
#[derive(Debug, Clone)]
pub enum ResponseFormat {
    Text,
    Json,
}

/// Response from a general-purpose LLM completion.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub content: String,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub latency_ms: u64,
}

/// Errors that can occur during LLM operations.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("Failed to spawn LLM process: {0}")]
    SpawnFailed(String),

    #[error("CLI returned error: {0}")]
    CliError(String),

    #[error("Failed to parse response: {0}")]
    ParseFailed(String),

    #[error("Provider not available: {0}")]
    NotAvailable(String),

    #[error("Rate limited, retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("Invalid response format: {0}")]
    InvalidFormat(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_request_serialize() {
        let req = ClassificationRequest {
            session_id: "sess-123".to_string(),
            first_prompt: "Fix the bug in main.rs".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec!["/commit".to_string()],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("sess-123"));
        assert!(json.contains("Fix the bug"));
    }

    #[test]
    fn test_classification_response_deserialize() {
        let json = r#"{
            "category_l1": "code_work",
            "category_l2": "bug_fix",
            "category_l3": "error-fix",
            "confidence": 0.92
        }"#;
        let resp: ClassificationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.category_l1, "code_work");
        assert_eq!(resp.category_l2, "bug_fix");
        assert_eq!(resp.category_l3, "error-fix");
        assert!((resp.confidence - 0.92).abs() < f64::EPSILON);
        assert!(resp.reasoning.is_none());
    }

    #[test]
    fn test_classification_response_with_reasoning() {
        let json = r#"{
            "category_l1": "code_work",
            "category_l2": "feature",
            "category_l3": "new-component",
            "confidence": 0.85,
            "reasoning": "User is building a new React component"
        }"#;
        let resp: ClassificationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.reasoning, Some("User is building a new React component".to_string()));
    }

    #[test]
    fn test_completion_response_has_model_field() {
        let resp = CompletionResponse {
            content: "hello".to_string(),
            model: Some("claude-haiku-4-5-20251001".to_string()),
            input_tokens: Some(10),
            output_tokens: Some(321),
            latency_ms: 100,
        };
        assert_eq!(resp.model.as_deref(), Some("claude-haiku-4-5-20251001"));
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::Timeout(30);
        assert_eq!(err.to_string(), "Timeout after 30 seconds");

        let err = LlmError::SpawnFailed("command not found".to_string());
        assert_eq!(err.to_string(), "Failed to spawn LLM process: command not found");

        let err = LlmError::RateLimited { retry_after_secs: 60 };
        assert_eq!(err.to_string(), "Rate limited, retry after 60 seconds");
    }
}
