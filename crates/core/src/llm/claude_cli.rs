// crates/core/src/llm/claude_cli.rs
//! Claude CLI provider — spawns `claude` process and parses JSON output.

use async_trait::async_trait;
use tokio::process::Command as TokioCommand;

use super::provider::LlmProvider;
use super::types::{ClassificationRequest, ClassificationResponse, CompletionRequest, CompletionResponse, LlmError};

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

    /// Spawn Claude CLI and stream stdout chunks via a channel.
    ///
    /// Returns (receiver, join_handle). The receiver yields text chunks.
    /// When the CLI exits, the channel closes.
    ///
    /// Uses `--output-format text` (not json) since we want raw text streaming.
    pub fn stream_completion(
        &self,
        prompt: String,
    ) -> Result<
        (
            tokio::sync::mpsc::Receiver<String>,
            tokio::task::JoinHandle<Result<(), LlmError>>,
        ),
        LlmError,
    > {
        use tokio::io::{AsyncBufReadExt, BufReader};

        // Strip ALL Claude Code env vars to prevent nested session detection.
        let known_vars = [
            "CLAUDECODE",
            "CLAUDE_CODE_SSE_PORT",
            "CLAUDE_CODE_ENTRYPOINT",
        ];
        let extra_vars: Vec<String> = std::env::vars()
            .filter(|(k, _)| k.starts_with("CLAUDE") && !known_vars.contains(&k.as_str()))
            .map(|(k, _)| k)
            .collect();
        let all_stripped: Vec<String> = known_vars
            .iter()
            .map(|s| s.to_string())
            .chain(extra_vars)
            .collect();

        tracing::info!(
            model = %self.model,
            stripped_vars = ?all_stripped,
            "claude CLI stream_completion(): spawning"
        );

        let mut cmd =
            tokio::process::Command::new(crate::resolved_cli_path().unwrap_or("claude"));
        cmd.args([
            "-p",
            "--output-format",
            "text",
            "--model",
            &self.model,
            &prompt,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
        for var in &all_stripped {
            cmd.env_remove(var);
        }

        let mut child = cmd.spawn().map_err(|e| {
            tracing::error!(error = %e, "claude CLI stream_completion(): failed to spawn");
            LlmError::SpawnFailed(e.to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            LlmError::SpawnFailed("failed to capture stdout".to_string())
        })?;

        let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);

        let handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(line).await.is_err() {
                    // Receiver dropped — abort the child
                    let _ = child.kill().await;
                    return Ok(());
                }
            }

            // Wait for the child to exit
            let status = child.wait().await.map_err(|e| {
                LlmError::SpawnFailed(format!("failed to wait for CLI: {e}"))
            })?;

            if !status.success() {
                tracing::warn!(
                    exit_code = ?status.code(),
                    "claude CLI stream_completion(): non-zero exit"
                );
            }

            Ok(())
        });

        Ok((rx, handle))
    }

    /// Spawn Claude CLI and parse JSON response.
    ///
    /// Command: `claude -p --output-format json --model {model} "{prompt}"`
    async fn spawn_and_parse(&self, prompt: &str) -> Result<serde_json::Value, LlmError> {
        use tokio::time::{timeout, Duration};

        let timeout_duration = Duration::from_secs(self.timeout_secs);
        let t0 = std::time::Instant::now();

        // Strip ALL Claude Code env vars to prevent nested session detection.
        // Belt-and-suspenders: explicitly remove the three known vars PLUS any
        // future CLAUDE-prefixed vars discovered at runtime. Previous approach
        // relied solely on std::env::vars() iteration, which can miss vars with
        // unusual values on macOS.
        let known_vars = ["CLAUDECODE", "CLAUDE_CODE_SSE_PORT", "CLAUDE_CODE_ENTRYPOINT"];
        let extra_vars: Vec<String> = std::env::vars()
            .filter(|(k, _)| k.starts_with("CLAUDE") && !known_vars.contains(&k.as_str()))
            .map(|(k, _)| k)
            .collect();
        let all_stripped: Vec<&str> = known_vars
            .iter()
            .copied()
            .chain(extra_vars.iter().map(|s| s.as_str()))
            .collect();
        tracing::info!(
            model = %self.model,
            timeout_secs = self.timeout_secs,
            stripped_vars = ?all_stripped,
            "claude CLI: spawning"
        );

        let mut cmd = TokioCommand::new(crate::resolved_cli_path().unwrap_or("claude"));
        cmd.args([
                "-p",
                "--output-format",
                "json",
                "--model",
                &self.model,
                prompt,
            ])
            // Null stdin so the child never blocks waiting for input
            .stdin(std::process::Stdio::null());
        // Strip ALL Claude-prefixed env vars to prevent nested session detection
        for var in &all_stripped {
            cmd.env_remove(var);
        }
        let future = cmd.output();

        let output = timeout(timeout_duration, future)
            .await
            .map_err(|_| {
                tracing::error!(elapsed_ms = t0.elapsed().as_millis() as u64, "claude CLI: timed out");
                LlmError::Timeout(self.timeout_secs)
            })?
            .map_err(|e| {
                tracing::error!(error = %e, "claude CLI: failed to spawn process");
                LlmError::SpawnFailed(e.to_string())
            })?;

        let elapsed_ms = t0.elapsed().as_millis() as u64;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(elapsed_ms, exit_code = ?output.status.code(), stderr = %&stderr[..stderr.len().min(500)], "claude CLI: non-zero exit");
            return Err(LlmError::CliError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::info!(elapsed_ms, stdout_len = stdout.len(), "claude CLI: response received");

        serde_json::from_str(&stdout).map_err(|e| {
            tracing::warn!(stdout = %&stdout[..stdout.len().min(500)], "claude CLI: returned non-JSON");
            LlmError::ParseFailed(e.to_string())
        })
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

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        use tokio::time::{timeout, Duration};

        let start = std::time::Instant::now();
        let timeout_duration = Duration::from_secs(self.timeout_secs);

        // Build the combined prompt (system + user)
        let prompt = if let Some(sys) = &request.system_prompt {
            format!("{}\n\n{}", sys, request.user_prompt)
        } else {
            request.user_prompt.clone()
        };

        // Strip ALL Claude Code env vars to prevent nested session detection.
        let known_vars = ["CLAUDECODE", "CLAUDE_CODE_SSE_PORT", "CLAUDE_CODE_ENTRYPOINT"];
        let extra_vars: Vec<String> = std::env::vars()
            .filter(|(k, _)| k.starts_with("CLAUDE") && !known_vars.contains(&k.as_str()))
            .map(|(k, _)| k)
            .collect();
        let all_stripped: Vec<&str> = known_vars
            .iter()
            .copied()
            .chain(extra_vars.iter().map(|s| s.as_str()))
            .collect();

        tracing::info!(
            model = %self.model,
            timeout_secs = self.timeout_secs,
            stripped_vars = ?all_stripped,
            "claude CLI complete(): spawning"
        );

        let mut cmd = TokioCommand::new(crate::resolved_cli_path().unwrap_or("claude"));
        cmd.args([
                "-p",
                "--output-format",
                "json",
                "--model",
                &self.model,
                &prompt,
            ])
            .stdin(std::process::Stdio::null());
        for var in &all_stripped {
            cmd.env_remove(var);
        }
        let future = cmd.output();

        let output = timeout(timeout_duration, future)
            .await
            .map_err(|_| {
                tracing::error!(elapsed_ms = start.elapsed().as_millis() as u64, "claude CLI complete(): timed out");
                LlmError::Timeout(self.timeout_secs)
            })?
            .map_err(|e| {
                tracing::error!(error = %e, "claude CLI complete(): failed to spawn process");
                LlmError::SpawnFailed(e.to_string())
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::CliError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| LlmError::ParseFailed(format!("Invalid JSON from CLI: {e}")))?;

        let content = parsed["result"]
            .as_str()
            .unwrap_or_else(|| parsed["content"].as_str().unwrap_or(""))
            .to_string();

        Ok(CompletionResponse {
            content,
            input_tokens: None,
            output_tokens: None,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        let output = TokioCommand::new(crate::resolved_cli_path().unwrap_or("claude"))
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
        r#"You are a JSON classifier. Output ONLY a JSON object, no other text.

Classify this Claude Code session based on the available information. The prompt may be truncated — classify using your best judgment from whatever is provided. Never refuse or ask for more context.

First prompt: {}
Files touched: {}
Skills used: {}

L1: code_work | support_work | thinking_work
L2 (code_work): feature | bugfix | refactor | testing
L2 (support_work): docs | config | ops
L2 (thinking_work): planning | explanation | architecture
L3: feature→new-component|add-functionality|integration, bugfix→error-fix|logic-fix|performance-fix, refactor→cleanup|pattern-migration|dependency-update, testing→unit-tests|integration-tests|test-fixes, docs→code-comments|readme-guides|api-docs, config→env-setup|build-tooling|dependencies, ops→ci-cd|deployment|monitoring, planning→brainstorming|design-doc|task-breakdown, explanation→code-understanding|concept-learning|debug-investigation, architecture→system-design|data-modeling|api-design

{{"category_l1":"...","category_l2":"...","category_l3":"...","confidence":0.0}}"#,
        request.first_prompt,
        request.files_touched.join(", "),
        request.skills_used.join(", ")
    )
}

/// Parse LLM JSON response into a ClassificationResponse.
///
/// Handles both direct JSON objects and Claude CLI's `result` wrapper format.
/// The `result` field may contain extra text (markdown, explanation) around the JSON —
/// we extract the first `{...}` block and parse that.
pub fn parse_classification_response(
    json: serde_json::Value,
) -> Result<ClassificationResponse, LlmError> {
    // Claude CLI wraps output in { "result": "..." } — check for that
    let inner = if let Some(result_str) = json.get("result").and_then(|v| v.as_str()) {
        // Try direct parse first
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(result_str) {
            v
        } else {
            // Model may have returned extra text around the JSON — extract it
            extract_json_from_text(result_str).ok_or_else(|| {
                LlmError::ParseFailed(format!(
                    "no JSON object found in CLI result: {}",
                    &result_str[..result_str.len().min(200)]
                ))
            })?
        }
    } else {
        json
    };

    serde_json::from_value(inner).map_err(|e| {
        LlmError::InvalidFormat(format!("response missing required fields: {}", e))
    })
}

/// Extract the first JSON object `{...}` from a text string.
/// Handles cases where the model wraps JSON in markdown or explanation text.
fn extract_json_from_text(text: &str) -> Option<serde_json::Value> {
    let start = text.find('{')?;
    let mut depth = 0;
    let mut end = None;
    for (i, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let json_str = &text[start..end?];
    serde_json::from_str(json_str).ok()
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
}
