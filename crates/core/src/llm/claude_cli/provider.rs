// crates/core/src/llm/claude_cli/provider.rs
//! `ClaudeCliProvider` struct and `LlmProvider` trait implementation.

use async_trait::async_trait;
use tokio::process::Command as TokioCommand;

use crate::llm::provider::LlmProvider;
use crate::llm::types::{
    ClassificationRequest, ClassificationResponse, CompletionRequest, CompletionResponse, LlmError,
};

use super::parsing::parse_classification_response;
use super::prompt::build_classification_prompt;

/// Return type for streaming completions: a text-chunk receiver paired with a task handle.
pub(crate) type StreamResult = (
    tokio::sync::mpsc::Receiver<String>,
    tokio::task::JoinHandle<Result<(), LlmError>>,
);

/// LLM provider that uses the Claude CLI binary.
///
/// Spawns `claude -p --output-format json` to classify sessions.
pub struct ClaudeCliProvider {
    model: String,
    pub(crate) timeout_secs: u64,
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
    pub fn stream_completion(&self, prompt: String) -> Result<StreamResult, LlmError> {
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

        let mut cmd = tokio::process::Command::new(crate::resolved_cli_path().unwrap_or("claude"));
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

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LlmError::SpawnFailed("failed to capture stdout".to_string()))?;

        let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);

        // Capture timeout before moving into the spawned task (self is not moved).
        let timeout_secs = self.timeout_secs;

        let handle = tokio::spawn(async move {
            use tokio::time::{sleep, Duration};

            let deadline = sleep(Duration::from_secs(timeout_secs));
            tokio::pin!(deadline);

            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            loop {
                tokio::select! {
                    _ = &mut deadline => {
                        tracing::error!(
                            timeout_secs,
                            "claude CLI stream_completion(): timed out, killing child"
                        );
                        let _ = child.kill().await;
                        return Err(LlmError::Timeout(timeout_secs));
                    }
                    line_result = lines.next_line() => {
                        match line_result {
                            Ok(Some(line)) => {
                                if tx.send(line).await.is_err() {
                                    // Receiver dropped — abort the child
                                    let _ = child.kill().await;
                                    return Ok(());
                                }
                            }
                            Ok(None) => break, // EOF
                            Err(_) => break,    // Read error, proceed to wait
                        }
                    }
                }
            }

            // Wait for the child to exit (still under the same timeout)
            tokio::select! {
                _ = &mut deadline => {
                    tracing::error!(
                        timeout_secs,
                        "claude CLI stream_completion(): timed out waiting for child exit"
                    );
                    let _ = child.kill().await;
                    return Err(LlmError::Timeout(timeout_secs));
                }
                wait_result = child.wait() => {
                    match wait_result {
                        Ok(status) if !status.success() => {
                            tracing::warn!(
                                exit_code = ?status.code(),
                                "claude CLI stream_completion(): non-zero exit"
                            );
                        }
                        Err(e) => {
                            return Err(LlmError::SpawnFailed(
                                format!("failed to wait for CLI: {e}")
                            ));
                        }
                        _ => {}
                    }
                }
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
        let known_vars = [
            "CLAUDECODE",
            "CLAUDE_CODE_SSE_PORT",
            "CLAUDE_CODE_ENTRYPOINT",
        ];
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
                tracing::error!(
                    elapsed_ms = t0.elapsed().as_millis() as u64,
                    "claude CLI: timed out"
                );
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
        tracing::info!(
            elapsed_ms,
            stdout_len = stdout.len(),
            "claude CLI: response received"
        );

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
        let known_vars = [
            "CLAUDECODE",
            "CLAUDE_CODE_SSE_PORT",
            "CLAUDE_CODE_ENTRYPOINT",
        ];
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
                tracing::error!(
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "claude CLI complete(): timed out"
                );
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

        // Model ID is the first key of the "modelUsage" object (no top-level "model" field).
        let model = parsed["modelUsage"]
            .as_object()
            .and_then(|m| m.keys().next().cloned());

        let input_tokens = parsed["usage"]["input_tokens"].as_u64();
        let output_tokens = parsed["usage"]["output_tokens"].as_u64();

        Ok(CompletionResponse {
            content,
            model,
            input_tokens,
            output_tokens,
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
