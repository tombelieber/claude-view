//! Per-line JSONL processing logic.
//!
//! Handles token accumulation, cost calculation, sub-agent tracking,
//! tool integration tracking, progress items, and phase classification
//! for each parsed JSONL line.

mod channel_a;
mod phase;
mod process_line;
mod progress;
mod sub_agents;
mod tokens;
