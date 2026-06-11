// crates/providers/src/util/jsonl.rs
//
// Resilient JSONL reading shared by every line-oriented parser.
//
// Foreign agents write transcripts live; readers race partial lines and the
// occasional corrupt record. Policy (mirrors the proven agentsview design):
// skip malformed lines but COUNT them (surfaced as meta.malformed_lines —
// never hidden), cap single lines at 64 MB, and detect a truncated final
// line (no trailing newline) without treating it as corruption.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Max single-line size — matches the most permissive known producer.
pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

/// Outcome of reading a JSONL file tolerantly.
pub struct JsonlRead {
    /// Parsed values, one per valid line.
    pub values: Vec<serde_json::Value>,
    /// Lines skipped because they failed to parse or exceeded the cap.
    /// A trailing partial line (live write in progress) is NOT counted.
    pub malformed: u32,
}

/// Read every parseable JSON line from `path`.
pub fn read_values(path: &Path) -> std::io::Result<JsonlRead> {
    let file = File::open(path)?;
    read_values_from(BufReader::new(file))
}

/// Reader-generic core (testable without touching disk).
pub fn read_values_from<R: Read>(reader: BufReader<R>) -> std::io::Result<JsonlRead> {
    let mut values = Vec::new();
    let mut malformed: u32 = 0;
    let mut buf = Vec::new();
    let mut reader = reader;

    loop {
        buf.clear();
        let n = reader
            .by_ref()
            .take(MAX_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }
        let ended_with_newline = buf.last() == Some(&b'\n');
        if buf.len() > MAX_LINE_BYTES {
            // Oversize line: drain the remainder, count once, move on.
            malformed += 1;
            if !ended_with_newline {
                skip_to_newline(&mut reader)?;
            }
            continue;
        }
        let line = trim_line(&buf);
        if line.is_empty() {
            continue;
        }
        match serde_json::from_slice::<serde_json::Value>(line) {
            Ok(v) => values.push(v),
            Err(_) => {
                if ended_with_newline {
                    malformed += 1;
                }
                // No trailing newline → live partial write; skip silently.
            }
        }
    }

    Ok(JsonlRead { values, malformed })
}

fn skip_to_newline<R: Read>(reader: &mut BufReader<R>) -> std::io::Result<()> {
    let mut sink = Vec::new();
    loop {
        sink.clear();
        let n = reader
            .by_ref()
            .take(MAX_LINE_BYTES as u64)
            .read_until(b'\n', &mut sink)?;
        if n == 0 || sink.last() == Some(&b'\n') {
            return Ok(());
        }
    }
}

fn trim_line(buf: &[u8]) -> &[u8] {
    let mut s = buf;
    while let Some((last, rest)) = s.split_last() {
        if *last == b'\n' || *last == b'\r' {
            s = rest;
        } else {
            break;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn read_str(s: &str) -> JsonlRead {
        read_values_from(BufReader::new(Cursor::new(s.as_bytes().to_vec()))).unwrap()
    }

    #[test]
    fn skips_and_counts_malformed_lines() {
        let r = read_str("{\"a\":1}\nnot json\n{\"b\":2}\n");
        assert_eq!(r.values.len(), 2);
        assert_eq!(r.malformed, 1);
    }

    #[test]
    fn trailing_partial_line_is_not_malformed() {
        let r = read_str("{\"a\":1}\n{\"b\":");
        assert_eq!(r.values.len(), 1);
        assert_eq!(r.malformed, 0);
    }

    #[test]
    fn blank_lines_ignored() {
        let r = read_str("\n\n{\"a\":1}\n\n");
        assert_eq!(r.values.len(), 1);
        assert_eq!(r.malformed, 0);
    }
}
