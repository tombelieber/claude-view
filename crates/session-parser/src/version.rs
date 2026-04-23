//! Typed protocol versions for the parse / extract / rollup pipeline.
//!
//! Each stage bumps its own version independently — e.g. a new JSONL event
//! shape bumps `ParserVersion`; a new stats field bumps `StatsVersion`;
//! a new rollup metric bumps `RollupVersion`. Newtypes prevent callers
//! from accidentally passing a `StatsVersion` where a `ParserVersion`
//! was expected.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParserVersion(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatsVersion(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RollupVersion(pub u32);

/// Current parser protocol version. Bump when the JSONL event shape or
/// per-line parsing semantics change.
pub const PARSER_VERSION: ParserVersion = ParserVersion(1);

/// Current stats-extraction version. Bump when `extract_stats` changes
/// the set of fields it emits or how any field is computed.
///
/// - v2 (CQRS Phase 6.2): `SessionStats::invocation_counts` populated from
///   `tool_use` blocks (with `:sub` suffix for Skill / Task / Agent). Older
///   rows need a re-extract to backfill `session_stats.invocation_counts`.
/// - v3 (CQRS Phase 7.c): `SessionStats` extended with `is_sidechain`,
///   `commit_count`, `reedited_files_count`, `skills_used`. Migration 88
///   adds these columns to `session_stats`; indexer will re-extract rows
///   with stats_version < 3 on next scan.
/// - v4 (CQRS Phase 7.h): `session_stats` extended with every remaining
///   column from the legacy `sessions` table (42 additions in migration
///   89). The indexer_v2 writer now populates the full ParsedSession row
///   into `session_stats` so readers can migrate off `sessions` before
///   the IRREVERSIBLE DROP in 7.h.6. Older rows with stats_version < 4
///   get re-extracted on the next scan.
pub const STATS_VERSION: StatsVersion = StatsVersion(4);

/// Current rollup version. Bump when a rollup table adds or changes a
/// metric in a way that requires recomputation.
pub const ROLLUP_VERSION: RollupVersion = RollupVersion(1);
