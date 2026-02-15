// crates/core/src/llm/provider.rs
//! LlmProvider trait defining the interface for LLM integrations.

use async_trait::async_trait;
use super::types::{ClassificationRequest, ClassificationResponse, CompletionRequest, CompletionResponse, LlmError};

/// Trait for LLM providers that can classify sessions.
///
/// Implementations include:
/// - `ClaudeCliProvider` — spawns `claude` CLI process
/// - Future: direct API providers for Anthropic, OpenAI, etc.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Classify a session's first prompt into the category taxonomy.
    /// Used by: AI Fluency Score (Theme 4). DO NOT REMOVE.
    async fn classify(&self, request: ClassificationRequest) -> Result<ClassificationResponse, LlmError>;

    /// Run a general-purpose completion with system + user prompt.
    /// Used by: Intelligent Session States (pause classification), future features.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;

    /// Check if the provider is available (CLI installed, API key set, etc.)
    async fn health_check(&self) -> Result<(), LlmError>;

    /// Provider name for logging/display (e.g. "claude-cli", "anthropic-api").
    fn name(&self) -> &str;

    /// Model identifier (e.g. "haiku", "sonnet", "gpt-4o-mini").
    fn model(&self) -> &str;
}
