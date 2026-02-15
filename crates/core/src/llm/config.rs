// crates/core/src/llm/config.rs
//! LLM provider configuration types.

/// Configuration for an LLM provider instance.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: ProviderType,
    pub model: String,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub enabled: bool,
    pub timeout_secs: u64,
}

/// Supported LLM provider types.
#[derive(Debug, Clone)]
pub enum ProviderType {
    ClaudeCli,
    AnthropicApi,
    OpenAi,
    Ollama,
    Custom,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: ProviderType::ClaudeCli,
            model: "claude-haiku-4-5-20251001".into(),
            api_key: None,
            endpoint: None,
            enabled: true,
            timeout_secs: 10,
        }
    }
}
