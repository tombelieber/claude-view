//! Incremental JSONL tail parser for live session monitoring.
//!
//! Reads only the new bytes appended since the last poll, using byte offsets
//! for efficiency. SIMD-accelerated pre-filtering via `memchr` avoids
//! deserialising lines that lack interesting keys.

pub(crate) mod content;
mod finders;
pub(crate) mod parse_line;
pub(crate) mod sub_agents;
mod tail_io;
#[cfg(test)]
mod tests;
mod types;
mod usage;

// Re-export all public items so external consumers see the same API.
pub(crate) use content::strip_noise_tags;
pub use finders::TailFinders;
pub use parse_line::parse_single_line;
pub use tail_io::parse_tail;
pub use types::{
    progress_message_content_fallback_count, reset_progress_message_content_fallback_count,
    HookProgressData, LineType, LiveLine, SubAgentNotification, SubAgentProgress, SubAgentResult,
    SubAgentSpawn, TailState,
};
