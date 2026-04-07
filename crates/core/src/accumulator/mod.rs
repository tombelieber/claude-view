//! Unified session accumulator for both live (streaming) and history (batch) JSONL parsing.
//!
//! `SessionAccumulator` extracts the same rich session data regardless of whether
//! lines arrive one-at-a-time via live tailing or all-at-once via batch file reads.
//! This ensures live monitoring and historical session views produce identical data.

pub(crate) mod helpers;
mod process_line;
#[cfg(test)]
mod tests;
mod types;

pub use types::{RichSessionData, SessionAccumulator};
