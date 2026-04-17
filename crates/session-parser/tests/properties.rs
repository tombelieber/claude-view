//! Property tests for the session-parser crate.
//!
//! Written TDD-style against `parse_jsonl`, which MUST never panic on
//! arbitrary byte input — a corrupt file from `~/.claude/projects/` must
//! never take out a user's session listing.

use claude_view_session_parser::{parse_jsonl, PARSER_VERSION};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// parse_jsonl must return Ok or Err — never panic — on any byte
    /// slice, including invalid UTF-8, embedded NULs, truncated JSON,
    /// and adversarial nesting.
    #[test]
    fn parse_jsonl_never_panics(data: Vec<u8>) {
        let _ = parse_jsonl(&data, PARSER_VERSION);
    }
}
