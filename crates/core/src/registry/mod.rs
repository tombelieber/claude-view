// crates/core/src/registry/mod.rs
//
// Parse ~/.claude/plugins/installed_plugins.json, scan plugin directories,
// and build lookup maps for all invocables (skills, commands, agents, MCP tools, built-in tools).

mod build;
mod parse;
mod scanner;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_integration;
mod types;

// Re-export public API (matches the original registry.rs surface)
pub use build::build_registry;
pub use types::{InvocableInfo, InvocableKind, Registry, BUILTIN_TOOLS};
