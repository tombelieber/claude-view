//! Content-hash staleness helpers (design doc §2.2 / SOTA §4).
//!
//! Two cheap blake3 hashes are taken over a file's head+tail and its
//! mid-region. Together they detect the overwhelming majority of edits
//! we see in practice — appends touch the tail, mid-file mutations
//! touch the mid. Neither reads the whole file, so the staleness check
//! stays cheap even for multi-MB sessions.
//!
//! Both helpers take an `&Path` and perform their own `File::open` —
//! the caller never needs to manage a handle.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const WINDOW_BYTES: u64 = 64 * 1024;

/// Hash the first 64 KB and the last 64 KB of a file into a single
/// 32-byte digest. For files smaller than 64 KB the whole file is
/// hashed once (the tail window is skipped when it would overlap the
/// head).
pub fn blake3_head_tail(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    let mut hasher = blake3::Hasher::new();

    let head_len = core::cmp::min(WINDOW_BYTES, size);
    if head_len > 0 {
        let mut head = vec![0u8; head_len as usize];
        f.read_exact(&mut head)?;
        hasher.update(&head);
    }

    if size > head_len {
        let tail_len = core::cmp::min(WINDOW_BYTES, size - head_len);
        f.seek(SeekFrom::End(-(tail_len as i64)))?;
        let mut tail = vec![0u8; tail_len as usize];
        f.read_exact(&mut tail)?;
        hasher.update(&tail);
    }

    Ok(*hasher.finalize().as_bytes())
}

/// Hash up to 64 KB of the file's mid-region, line-aligned to the next
/// `\n` after `size/2` when one exists within a 64 KB scan window.
/// Sibling to `blake3_head_tail`; together they give a cheap but
/// edit-sensitive staleness signal for JSONL session files.
pub fn blake3_mid(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();

    if size == 0 {
        return Ok(*blake3::Hasher::new().finalize().as_bytes());
    }

    let start = size / 2;
    f.seek(SeekFrom::Start(start))?;

    // Scan forward up to WINDOW_BYTES for the next '\n' so the hashed
    // window starts on a line boundary. If we don't find one inside the
    // scan window, fall back to the raw mid offset.
    let remaining = size - start;
    let scan_cap = core::cmp::min(WINDOW_BYTES, remaining) as usize;
    let mut scan_buf = vec![0u8; scan_cap];
    let scan_read = f.read(&mut scan_buf)?;

    let aligned_offset = scan_buf[..scan_read]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| p as u64 + 1)
        .unwrap_or(0);

    let hash_start = start + aligned_offset;
    if hash_start >= size {
        return Ok(*blake3::Hasher::new().finalize().as_bytes());
    }

    f.seek(SeekFrom::Start(hash_start))?;
    let hash_cap = core::cmp::min(WINDOW_BYTES, size - hash_start) as usize;
    let mut hash_buf = vec![0u8; hash_cap];
    let n = f.read(&mut hash_buf)?;
    Ok(*blake3::Hasher::new()
        .update(&hash_buf[..n])
        .finalize()
        .as_bytes())
}
