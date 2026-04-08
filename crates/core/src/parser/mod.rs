// crates/core/src/parser/mod.rs
//! Async JSONL parser for Claude Code session files.
//!
//! This module provides battle-tested parsing of Claude Code's JSONL session format,
//! handling malformed lines gracefully, aggregating tool calls, and cleaning command tags.
//!
//! # Submodules
//! - `content` — content extraction from message blocks (text, tools, thinking)
//! - `helpers` — shared field-attachment utilities
//! - `session` — core session parsing and pagination
//! - `tags` — command tag stripping from user messages

mod content;
mod helpers;
mod session;
mod tags;
#[cfg(test)]
mod tests;

// Re-export the public API (preserves `claude_view_core::parser::*` and `claude_view_core::*`)
pub use session::{
    parse_session, parse_session_paginated, parse_session_paginated_with_raw,
    parse_session_with_raw,
};
