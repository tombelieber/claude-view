//! File I/O for incremental JSONL tailing.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::finders::TailFinders;
use super::parse_line::parse_single_line;
use super::types::LiveLine;

/// Read new JSONL lines appended since `offset`.
///
/// Returns the parsed lines and the new byte offset to pass on the next call.
/// This function uses synchronous I/O and should be called from
/// `tokio::task::spawn_blocking`.
pub fn parse_tail(
    path: &Path,
    offset: u64,
    finders: &TailFinders,
) -> std::io::Result<(Vec<LiveLine>, u64)> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_len = metadata.len();

    if offset > file_len {
        // File was replaced (new file smaller than stored offset).
        // Reset to start and read the entire new file.
        tracing::warn!(
            path = %path.display(),
            old_offset = offset,
            new_file_len = file_len,
            "File replaced (offset > size) — resetting to start"
        );
        return parse_tail(path, 0, finders);
    }
    if offset == file_len {
        return Ok((Vec::new(), offset));
    }

    file.seek(SeekFrom::Start(offset))?;

    let to_read = (file_len - offset) as usize;
    let mut buf = vec![0u8; to_read];
    file.read_exact(&mut buf)?;

    // Find the last newline — anything after it is a partial write and must be
    // excluded so we don't try to parse an incomplete JSON object.
    let last_newline = buf.iter().rposition(|&b| b == b'\n');
    let (complete, new_offset) = match last_newline {
        Some(pos) => (&buf[..=pos], offset + pos as u64 + 1),
        None => {
            // No complete line yet
            return Ok((Vec::new(), offset));
        }
    };

    let mut lines = Vec::new();
    for raw_line in complete.split(|&b| b == b'\n') {
        if raw_line.is_empty() {
            continue;
        }
        let line = parse_single_line(raw_line, finders);
        lines.push(line);
    }

    Ok((lines, new_offset))
}
