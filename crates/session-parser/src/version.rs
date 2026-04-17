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
pub const STATS_VERSION: StatsVersion = StatsVersion(1);

/// Current rollup version. Bump when a rollup table adds or changes a
/// metric in a way that requires recomputation.
pub const ROLLUP_VERSION: RollupVersion = RollupVersion(1);
