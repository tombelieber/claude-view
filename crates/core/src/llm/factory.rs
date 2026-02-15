// crates/core/src/llm/factory.rs
//! Provider factory — creates an LlmProvider from configuration.

use std::sync::Arc;
use super::config::{LlmConfig, ProviderType};
use super::provider::LlmProvider;
use super::types::LlmError;
use super::claude_cli::ClaudeCliProvider;

/// Create an LLM provider based on the given configuration.
///
/// Currently only `ClaudeCli` is implemented. Other provider types will return
/// an error until their respective Phase 2 implementations are added.
pub fn create_provider(config: &LlmConfig) -> Result<Arc<dyn LlmProvider>, LlmError> {
    match config.provider {
        ProviderType::ClaudeCli => Ok(Arc::new(ClaudeCliProvider::new(&config.model))),
        _ => Err(LlmError::NotAvailable(format!(
            "Provider {:?} not yet implemented. Only ClaudeCli is available in MVP.",
            config.provider
        ))),
    }
}
