// crates/core/src/llm/claude_cli/mod.rs
//! Claude CLI provider — spawns `claude` process and parses JSON output.

mod parsing;
mod prompt;
mod provider;

#[cfg(test)]
mod tests;

pub use parsing::parse_classification_response;
pub use prompt::build_classification_prompt;
pub use provider::ClaudeCliProvider;
