//! CQRS Phase 1 parser crate — Stage A (`parse_jsonl`) + Stage B
//! (`extract_stats`) + blake3 content-hash staleness helpers.
//!
//! Pure functions; no I/O except the blake3 staleness helpers, which
//! read their input file directly. No database access; no async runtime
//! dependency. This crate is the single gateway between raw `~/.claude/`
//! JSONL and everything the read-side needs.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

pub mod doc;
pub mod extract;
pub mod parse;
pub mod staleness;
pub mod version;

pub use claude_view_core::session_stats::SessionStats;
pub use doc::SessionDoc;
pub use extract::extract_stats;
pub use parse::{parse_jsonl, ParseError};
pub use staleness::{blake3_head_tail, blake3_mid};
pub use version::{
    ParserVersion, RollupVersion, StatsVersion, PARSER_VERSION, ROLLUP_VERSION, STATS_VERSION,
};
