// crates/db/src/indexer_parallel/parser/file_io.rs
// File-level JSONL reading with mmap for large files.

use super::core::parse_bytes;
use crate::indexer_parallel::types::*;

/// Parse a JSONL file from disk, using mmap for large files.
/// Returns a default ParseResult on any I/O error.
pub(crate) fn parse_file_bytes(path: &std::path::Path) -> ParseResult {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ParseResult::default(),
    };
    let len = match file.metadata() {
        Ok(m) => m.len() as usize,
        Err(_) => return ParseResult::default(),
    };
    if len == 0 {
        return ParseResult::default();
    }
    if len < 64 * 1024 {
        match std::fs::read(path) {
            Ok(data) => return parse_bytes(&data),
            Err(_) => return ParseResult::default(),
        }
    }
    match unsafe { memmap2::Mmap::map(&file) } {
        Ok(mmap) => parse_bytes(&mmap),
        Err(_) => match std::fs::read(path) {
            Ok(data) => parse_bytes(&data),
            Err(_) => ParseResult::default(),
        },
    }
}
