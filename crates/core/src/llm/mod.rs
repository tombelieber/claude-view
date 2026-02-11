// crates/core/src/llm/mod.rs
//! LLM integration module for session classification.
//!
//! Provides the `LlmProvider` trait and implementations for spawning
//! LLM processes (Claude CLI) or calling APIs to classify sessions.

pub mod claude_cli;
pub mod provider;
pub mod types;

pub use claude_cli::ClaudeCliProvider;
pub use provider::LlmProvider;
pub use types::{ClassificationRequest, ClassificationResponse, LlmError};
