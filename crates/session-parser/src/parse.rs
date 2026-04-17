//! Stage A — JSONL bytes → typed `SessionDoc`.
//!
//! Splits on `\n`, skips blank lines, and deserialises each remaining
//! line into a `StatsLine`. A single malformed line returns an error
//! with the 1-based line number; callers that want the v1 "drop-and-
//! continue" semantics must wrap this and discard the `Err`.
//!
//! The parser takes a `ParserVersion` argument so future protocol
//! changes can fan out here without widening the signature.

use claude_view_core::session_stats::StatsLine;

use crate::doc::SessionDoc;
use crate::version::ParserVersion;

/// Errors returned by `parse_jsonl`.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// A single line failed to deserialise into `StatsLine`. The line
    /// number is 1-based and counts all newline-separated segments,
    /// including empty ones.
    #[error("malformed json at line {line}: {source}")]
    MalformedJson {
        line: usize,
        source: serde_json::Error,
    },
}

/// Parse a JSONL byte slice into a `SessionDoc`.
///
/// - Splits on `\n`; empty segments (including a trailing newline) are
///   skipped.
/// - Never panics on arbitrary input — see `tests/properties.rs`.
/// - Returns `Err(MalformedJson)` on the first unparseable non-empty
///   line.
pub fn parse_jsonl(bytes: &[u8], _v: ParserVersion) -> Result<SessionDoc, ParseError> {
    let mut lines = Vec::new();
    for (idx, line_bytes) in bytes.split(|&b| b == b'\n').enumerate() {
        if line_bytes.is_empty() {
            continue;
        }
        match serde_json::from_slice::<StatsLine>(line_bytes) {
            Ok(line) => lines.push(line),
            Err(e) => {
                return Err(ParseError::MalformedJson {
                    line: idx + 1,
                    source: e,
                });
            }
        }
    }
    Ok(SessionDoc { lines })
}
