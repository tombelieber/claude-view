// crates/core/src/invocation/mod.rs
//
// Classify tool_use calls from JSONL lines against a Registry to determine
// which invocable was called (skill, command, agent, MCP tool, or built-in).

mod agents;
mod classify;
mod mcp_parser;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public items to preserve the module's public API.
pub use agents::BUILTIN_AGENT_NAMES;
pub use classify::classify_tool_use;
pub use types::{ClassifyResult, RawToolUse};
