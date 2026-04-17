//! CQRS Phase 1 parser crate — Stage A (parse_jsonl) + Stage B (extract_stats)
//! + blake3 content-hash staleness helpers.
//!
//! PR 1.1 scaffold. PR 1.2 adds `parse`, `extract`, `staleness`, `doc`, `version`
//! modules with full implementations. See
//! `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

/// Parser protocol version. Bump when JSONL event shape or extraction semantics change.
pub const PARSER_VERSION: u32 = 0;
