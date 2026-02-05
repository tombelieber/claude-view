// crates/core/src/llm/claude_cli.rs
//! Claude CLI provider — spawns `claude` process and parses JSON output.

use async_trait::async_trait;
use tokio::process::Command as TokioCommand;

use super::provider::LlmProvider;
use super::types::{ClassificationRequest, ClassificationResponse, LlmError};

/// LLM provider that uses the Claude CLI binary.
///
/// Spawns `claude -p --output-format json` to classify sessions.
pub struct ClaudeCliProvider {
    model: String,
    timeout_secs: u64,
}

impl ClaudeCliProvider {
    /// Create a new provider with the given model name.
    ///
    /// Model names: "haiku", "sonnet", "opus"
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            timeout_secs: 30,
        }
    }

    /// Set the timeout in seconds for CLI invocations.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Spawn Claude CLI and parse JSON response.
    ///
    /// Command: `claude -p --output-format json --model {model} "{prompt}"`
    async fn spawn_and_parse(&self, prompt: &str) -> Result<serde_json::Value, LlmError> {
        use tokio::time::{timeout, Duration};

        let timeout_duration = Duration::from_secs(self.timeout_secs);

        let future = TokioCommand::new("claude")
            .args([
                "-p",
                "--output-format",
                "json",
                "--model",
                &self.model,
                prompt,
            ])
            .output();

        let output = timeout(timeout_duration, future)
            .await
            .map_err(|_| LlmError::Timeout(self.timeout_secs))?
            .map_err(|e| LlmError::SpawnFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::CliError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout).map_err(|e| LlmError::ParseFailed(e.to_string()))
    }
}

#[async_trait]
impl LlmProvider for ClaudeCliProvider {
    async fn classify(
        &self,
        request: ClassificationRequest,
    ) -> Result<ClassificationResponse, LlmError> {
        let prompt = build_classification_prompt(&request);
        let json = self.spawn_and_parse(&prompt).await?;
        parse_classification_response(json)
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        let output = TokioCommand::new("claude")
            .arg("--version")
            .output()
            .await
            .map_err(|e| LlmError::SpawnFailed(format!("claude not found: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(LlmError::NotAvailable("claude --version failed".into()))
        }
    }

    fn name(&self) -> &str {
        "claude-cli"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// Build the classification prompt for a session.
pub fn build_classification_prompt(request: &ClassificationRequest) -> String {
    format!(
        r#"Classify this Claude Code session. Respond with JSON only.

First prompt:
```
{}
```

Files touched: {}
Skills used: {}

Categories:
L1: code_work | support_work | thinking_work
L2 (if code_work): feature | bug_fix | refactor | testing
L2 (if support_work): docs | config | ops
L2 (if thinking_work): planning | explanation | architecture
L3 (by L2):
  - feature: new-component | new-endpoint | new-integration | enhancement
  - bug_fix: error-fix | logic-fix | regression-fix | crash-fix
  - refactor: rename | extract | restructure | cleanup
  - testing: unit-test | integration-test | e2e-test | test-fix
  - docs: readme | api-docs | inline-docs | changelog
  - config: env-config | build-config | ci-config | deps
  - ops: deploy | monitoring | migration | backup
  - planning: design-doc | spike | estimation | roadmap
  - explanation: how-it-works | debugging-help | code-review | comparison
  - architecture: system-design | data-model | api-design | infra

Respond:
{{"category_l1": "...", "category_l2": "...", "category_l3": "...", "confidence": 0.0-1.0}}"#,
        request.first_prompt,
        request.files_touched.join(", "),
        request.skills_used.join(", ")
    )
}

/// Parse LLM JSON response into a ClassificationResponse.
///
/// Handles both direct JSON objects and Claude CLI's `result` wrapper format.
pub fn parse_classification_response(
    json: serde_json::Value,
) -> Result<ClassificationResponse, LlmError> {
    // Claude CLI wraps output in { "result": "..." } — check for that
    let inner = if let Some(result_str) = json.get("result").and_then(|v| v.as_str()) {
        // The result field contains a JSON string — parse it
        serde_json::from_str(result_str)
            .map_err(|e| LlmError::ParseFailed(format!("inner JSON parse failed: {}", e)))?
    } else {
        json
    };

    serde_json::from_value(inner).map_err(|e| {
        LlmError::InvalidFormat(format!("response missing required fields: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(prompt.contains("bug_fix"));
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
            "category_l2": "bug_fix",
            "category_l3": "error-fix",
            "confidence": 0.95
        });

        let resp = parse_classification_response(json).unwrap();
        assert_eq!(resp.category_l1, "code_work");
        assert_eq!(resp.category_l2, "bug_fix");
        assert_eq!(resp.category_l3, "error-fix");
        assert!((resp.confidence - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_classification_response_claude_cli_wrapper() {
        // Claude CLI wraps output as: { "result": "{ json string }" }
        let json = serde_json::json!({
            "result": r#"{"category_l1": "thinking_work", "category_l2": "explanation", "category_l3": "how-it-works", "confidence": 0.88}"#
        });

        let resp = parse_classification_response(json).unwrap();
        assert_eq!(resp.category_l1, "thinking_work");
        assert_eq!(resp.category_l2, "explanation");
        assert_eq!(resp.category_l3, "how-it-works");
        assert!((resp.confidence - 0.88).abs() < f64::EPSILON);
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
}
