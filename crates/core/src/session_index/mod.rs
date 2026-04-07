//! Parser for Claude Code's `sessions-index.json` files.
//!
//! Each Claude Code project stores a `sessions-index.json` that lists all
//! sessions with metadata (summary, message count, timestamps, etc.).

mod classify;
mod discovery;
mod parse;
mod types;

pub use classify::classify_jsonl_file;
pub use discovery::{discover_orphan_sessions, read_all_session_indexes, resolve_cwd_for_project};
pub use parse::parse_session_index;
pub use types::{SessionClassification, SessionIndexEntry, SessionKind, StartType};
