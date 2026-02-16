//! File position tracker for incremental JSONL reads.
//!
//! Tracks the byte offset into a file so that successive calls to
//! `read_new_lines` return only the lines appended since the last read.
//! Handles file truncation by resetting the position to 0.

use std::path::PathBuf;
use tokio::io::{self, AsyncReadExt, AsyncSeekExt};

/// Tracks a byte offset into a file for incremental line-by-line reading.
///
/// Used by Mission Control's live terminal monitor to tail JSONL session
/// files without re-reading the entire file on each poll.
pub struct FilePositionTracker {
    /// Current read position (byte offset from start of file).
    position: u64,
    /// Path to the file being tracked.
    path: PathBuf,
}

impl FilePositionTracker {
    /// Create a new tracker starting at position 0.
    ///
    /// The first call to `read_new_lines` will read from the beginning of
    /// the file, which is useful for sending the full scrollback buffer
    /// to a newly connected viewer.
    pub fn new(path: PathBuf) -> Self {
        Self { position: 0, path }
    }

    /// Create a new tracker starting at the current end of the file.
    ///
    /// This is useful for new watchers that have already been sent the
    /// scrollback — subsequent calls to `read_new_lines` will only return
    /// lines appended after this point.
    pub async fn new_at_end(path: PathBuf) -> io::Result<Self> {
        let metadata = tokio::fs::metadata(&path).await?;
        Ok(Self {
            position: metadata.len(),
            path,
        })
    }

    /// Read all new complete lines appended since the last read.
    ///
    /// - Reads bytes from `self.position` to current EOF.
    /// - Updates `self.position` to the end of the last complete line.
    /// - Returns only complete lines (those terminated by `\n`).
    /// - An incomplete trailing line (no `\n`) is NOT returned and will
    ///   be picked up on the next call once the line is complete.
    /// - If the file has been truncated (current size < position), the
    ///   position resets to 0 and the file is re-read from the start.
    pub async fn read_new_lines(&mut self) -> io::Result<Vec<String>> {
        let mut file = tokio::fs::File::open(&self.path).await?;
        let metadata = file.metadata().await?;
        let file_len = metadata.len();

        // Handle truncation: if the file is now smaller than our position,
        // reset to 0 and re-read from the beginning.
        if file_len < self.position {
            self.position = 0;
        }

        // Nothing new to read
        if file_len == self.position {
            return Ok(Vec::new());
        }

        // Seek to current position and read only new bytes
        file.seek(std::io::SeekFrom::Start(self.position)).await?;
        let mut buf = Vec::with_capacity((file_len - self.position) as usize);
        file.read_to_end(&mut buf).await?;

        self.read_new_lines_from_bytes(&buf)
    }

    /// Parse complete lines from a byte slice, advancing position accordingly.
    fn read_new_lines_from_bytes(&mut self, bytes: &[u8]) -> io::Result<Vec<String>> {
        if bytes.is_empty() {
            return Ok(Vec::new());
        }

        // Find the last newline — everything up to and including it is "complete"
        let last_newline = bytes.iter().rposition(|&b| b == b'\n');

        let complete_bytes = match last_newline {
            Some(pos) => &bytes[..=pos],
            None => {
                // No newline found — the entire chunk is an incomplete line.
                // Don't advance position; we'll pick it up next time.
                return Ok(Vec::new());
            }
        };

        // Advance position by the number of complete bytes consumed
        self.position += complete_bytes.len() as u64;

        // Split into lines, filtering out empty lines from trailing \n
        let lines: Vec<String> = complete_bytes
            .split(|&b| b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| String::from_utf8_lossy(line).into_owned())
            .collect();

        Ok(lines)
    }

    /// Get the current byte position.
    #[allow(dead_code)]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the tracked file path.
    #[allow(dead_code)]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn read_new_lines_from_empty_file() {
        let tmp = NamedTempFile::new().unwrap();
        let mut tracker = FilePositionTracker::new(tmp.path().to_path_buf());

        let lines = tracker.read_new_lines().await.unwrap();
        assert!(lines.is_empty());
        assert_eq!(tracker.position(), 0);
    }

    #[tokio::test]
    async fn read_new_lines_after_append() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write initial content
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap();
            write!(f, "line1\nline2\n").unwrap();
        }

        let mut tracker = FilePositionTracker::new(path.clone());

        // First read: should get both lines
        let lines = tracker.read_new_lines().await.unwrap();
        assert_eq!(lines, vec!["line1", "line2"]);

        // Second read with no changes: should get nothing
        let lines = tracker.read_new_lines().await.unwrap();
        assert!(lines.is_empty());

        // Append more content
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            write!(f, "line3\nline4\n").unwrap();
        }

        // Third read: should get only the new lines
        let lines = tracker.read_new_lines().await.unwrap();
        assert_eq!(lines, vec!["line3", "line4"]);
    }

    #[tokio::test]
    async fn read_handles_truncation() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write initial content
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap();
            write!(f, "original-line1\noriginal-line2\n").unwrap();
        }

        let mut tracker = FilePositionTracker::new(path.clone());

        // Read initial content
        let lines = tracker.read_new_lines().await.unwrap();
        assert_eq!(lines.len(), 2);
        assert!(tracker.position() > 0);

        // Truncate and write shorter content
        {
            let mut f = std::fs::File::create(&path).unwrap(); // truncates
            write!(f, "new\n").unwrap();
        }

        // Position is now past EOF — should detect truncation, reset, and re-read
        let lines = tracker.read_new_lines().await.unwrap();
        assert_eq!(lines, vec!["new"]);
        assert_eq!(tracker.position(), 4); // "new\n" = 4 bytes
    }

    #[tokio::test]
    async fn incomplete_line_not_returned() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write a line WITHOUT a trailing newline
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap();
            write!(f, "partial-line").unwrap();
        }

        let mut tracker = FilePositionTracker::new(path.clone());

        // Should return nothing — the line is incomplete
        let lines = tracker.read_new_lines().await.unwrap();
        assert!(lines.is_empty());
        assert_eq!(tracker.position(), 0); // position should NOT advance

        // Now complete the line
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            write!(f, " continued\n").unwrap();
        }

        // Now the full line should be returned
        let lines = tracker.read_new_lines().await.unwrap();
        assert_eq!(lines, vec!["partial-line continued"]);
    }

    #[tokio::test]
    async fn new_at_end_skips_existing_content() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write initial content
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap();
            write!(f, "existing1\nexisting2\n").unwrap();
        }

        // Create tracker at end — should skip existing content
        let mut tracker = FilePositionTracker::new_at_end(path.clone()).await.unwrap();
        let lines = tracker.read_new_lines().await.unwrap();
        assert!(lines.is_empty());

        // Append new content
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            write!(f, "new-line\n").unwrap();
        }

        // Should only get the new content
        let lines = tracker.read_new_lines().await.unwrap();
        assert_eq!(lines, vec!["new-line"]);
    }

    #[tokio::test]
    async fn mixed_complete_and_incomplete_lines() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write two complete lines and one incomplete
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap();
            write!(f, "complete1\ncomplete2\nincomplete").unwrap();
        }

        let mut tracker = FilePositionTracker::new(path.clone());

        // Should return only the two complete lines
        let lines = tracker.read_new_lines().await.unwrap();
        assert_eq!(lines, vec!["complete1", "complete2"]);

        // Position should be after "complete2\n" (10 + 10 = 20 bytes)
        assert_eq!(tracker.position(), 20);
    }
}
