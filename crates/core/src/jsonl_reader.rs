//! Transparent reader for `.jsonl` and `.jsonl.gz` session files.
//!
//! Part of the JSONL-first architecture (see
//! `docs/plans/2026-04-16-hardcut-jsonl-first-design.md`). Works in
//! concert with `session_catalog` — the catalog tells you **which**
//! file backs a session; this module tells you **how** to read it
//! regardless of whether it is plain or gzip-compressed.
//!
//! Design choices:
//! - **Individual bad JSONL lines are silently dropped.** This
//!   matches the semantics of the existing indexer, which tolerates
//!   malformed lines without aborting the whole session. A corrupt
//!   line should never take out an entire session view.
//! - **`read_all` buffers the whole file.** At the measured p95
//!   session size (813 KB live / 278 KB gz-on-disk) the full buffer
//!   fits in L2 cache and the parse runs at ~850 MB/s. Streaming line
//!   iterators add lifetime complexity without a perf win at these
//!   sizes. Add a streaming variant only if p99+ sessions become a
//!   user-visible latency issue.
//!
//! # Example
//!
//! ```ignore
//! use claude_view_core::jsonl_reader;
//! use serde_json::Value;
//! use std::path::Path;
//!
//! let rows: Vec<Value> = jsonl_reader::read_all(
//!     Path::new("/home/user/.claude/projects/proj/abc.jsonl"),
//!     false,
//! )?;
//! # Ok::<(), std::io::Error>(())
//! ```

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use serde::de::DeserializeOwned;

/// Open a session file for reading, transparently decompressing
/// `.jsonl.gz` when `is_compressed` is true.
///
/// Returns a boxed `Read` so callers don't need to branch on the
/// concrete type. The `Send` bound is omitted — callers that need
/// cross-thread usage should wrap the result themselves.
pub fn open_reader(path: &Path, is_compressed: bool) -> std::io::Result<Box<dyn Read>> {
    let file = File::open(path)?;
    if is_compressed {
        Ok(Box::new(GzDecoder::new(BufReader::new(file))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

/// Read the full session into memory and return a vector of typed
/// lines. Individual parse errors are silently skipped — see the
/// module docs for rationale.
pub fn read_all<T: DeserializeOwned>(path: &Path, is_compressed: bool) -> std::io::Result<Vec<T>> {
    let mut reader = open_reader(path, is_compressed)?;
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    Ok(text
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<T>(l).ok())
        .collect())
}

/// Count the number of parseable lines without retaining them.
/// Convenience wrapper around [`read_all`] for callers that only need
/// a tally.
pub fn count_parseable<T: DeserializeOwned>(
    path: &Path,
    is_compressed: bool,
) -> std::io::Result<usize> {
    let rows: Vec<T> = read_all(path, is_compressed)?;
    Ok(rows.len())
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::io::Write;

    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tempfile::tempdir;

    fn make_jsonl(path: &Path, lines: &[&str]) {
        let mut f = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
    }

    fn make_gz(path: &Path, lines: &[&str]) {
        let f = fs::File::create(path).unwrap();
        let mut enc = GzEncoder::new(f, Compression::default());
        for line in lines {
            writeln!(enc, "{}", line).unwrap();
        }
        enc.finish().unwrap();
    }

    #[test]
    fn reads_plain_jsonl() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("s.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"user","content":"hi"}"#,
                r#"{"type":"assistant","content":"hello"}"#,
                "",
                r#"{"type":"system"}"#,
            ],
        );
        let lines: Vec<serde_json::Value> = read_all(&path, false).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["type"], "user");
        assert_eq!(lines[1]["type"], "assistant");
    }

    #[test]
    fn reads_gzipped_jsonl() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("s.jsonl.gz");
        make_gz(
            &path,
            &[
                r#"{"type":"user","content":"hi"}"#,
                r#"{"type":"assistant","content":"hello"}"#,
            ],
        );
        let lines: Vec<serde_json::Value> = read_all(&path, true).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["type"], "user");
    }

    #[test]
    fn tolerates_bad_lines() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("bad.jsonl");
        make_jsonl(
            &path,
            &[
                r#"{"type":"user"}"#,
                r#"not-valid-json"#,
                r#"{"type":"assistant"}"#,
            ],
        );
        let lines: Vec<serde_json::Value> = read_all(&path, false).unwrap();
        assert_eq!(lines.len(), 2, "bad line is silently skipped");
    }

    #[test]
    fn count_parseable_returns_valid_count() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("c.jsonl");
        make_jsonl(
            &path,
            &[r#"{"a":1}"#, r#"oops"#, r#"{"a":2}"#, r#"{"a":3}"#],
        );
        let n = count_parseable::<serde_json::Value>(&path, false).unwrap();
        assert_eq!(n, 3);
    }
}
