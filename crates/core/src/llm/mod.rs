// crates/core/src/llm/mod.rs
//! LLM integration module for session classification and general-purpose completions.
//!
//! Provides the `LlmProvider` trait and implementations for spawning
//! LLM processes (Claude CLI) or calling APIs to classify sessions
//! and run general-purpose completions.

pub mod claude_cli;
pub mod config;
pub mod factory;
pub mod provider;
pub mod types;

pub use claude_cli::ClaudeCliProvider;
pub use config::{LlmConfig, ProviderType};
pub use provider::LlmProvider;
pub use types::{ClassificationRequest, ClassificationResponse, CompletionRequest, CompletionResponse, LlmError, ResponseFormat};
