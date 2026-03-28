//! Rolling JSONL debug log for live event streams.
//!
//! Each `DebugEventLog` writes one JSON line per event to a file on disk.
//! A background task drains an mpsc channel so file IO never blocks the
//! mutation path. When the line count exceeds `MAX_LINES`, the file is
//! truncated to `KEEP_LINES` (most-recent history preserved).
//!
//! **Debug-only:** callers gate construction behind `cfg!(debug_assertions)`.

use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Maximum lines before rotation triggers.
const MAX_LINES: usize = 1000;
/// Lines to keep after rotation (tail of file).
const KEEP_LINES: usize = 500;
/// Channel capacity — generous to avoid backpressure.
const CHANNEL_CAP: usize = 256;

/// Handle used by route handlers to append log lines without blocking.
#[derive(Clone)]
pub struct DebugEventLog {
    tx: mpsc::Sender<String>,
}

impl DebugEventLog {
    /// Create a new debug log targeting `path`.
    ///
    /// Spawns a background tokio task that owns the file. The `.debug/`
    /// parent directory is created if it doesn't exist.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (tx, rx) = mpsc::channel(CHANNEL_CAP);
        tokio::spawn(writer_task(path, rx));
        Self { tx }
    }

    /// Enqueue a line to be written. Fire-and-forget — drops silently if
    /// the channel is full (backpressure = acceptable loss for debug logs).
    pub fn append(&self, line: String) {
        let _ = self.tx.try_send(line);
    }
}

/// Background task: drains the channel and appends lines to the file.
/// Rotates when `line_count` exceeds `MAX_LINES`.
async fn writer_task(path: PathBuf, mut rx: mpsc::Receiver<String>) {
    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    // Count existing lines so rotation is accurate across restarts.
    let mut line_count = count_lines(&path).await;

    // Open file in append mode (create if missing).
    let open = || {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
    };

    while let Some(line) = rx.recv().await {
        // Write one line (sync IO is fine — small writes, background task).
        if let Ok(mut file) = open() {
            use std::io::Write;
            let _ = writeln!(file, "{}", line);
            line_count += 1;
        }

        // Rotate when limit exceeded.
        if line_count > MAX_LINES {
            if rotate(&path, KEEP_LINES).await.is_ok() {
                line_count = KEEP_LINES;
            }
        }
    }
}

/// Count newlines in an existing file (0 if missing).
async fn count_lines(path: &Path) -> usize {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => contents.lines().count(),
        Err(_) => 0,
    }
}

/// Keep only the last `keep` lines of the file.
async fn rotate(path: &Path, keep: usize) -> std::io::Result<()> {
    let contents = tokio::fs::read_to_string(path).await?;
    let lines: Vec<&str> = contents.lines().collect();
    if lines.len() <= keep {
        return Ok(());
    }
    let tail = &lines[lines.len() - keep..];
    let truncated = tail.join("\n") + "\n";
    tokio::fs::write(path, truncated).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn appends_lines_to_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.jsonl");
        let log = DebugEventLog::new(path.clone());

        log.append(r#"{"ts":1,"msg":"hello"}"#.into());
        log.append(r#"{"ts":2,"msg":"world"}"#.into());

        // Give writer task time to flush.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("hello"));
        assert!(lines[1].contains("world"));
    }

    #[tokio::test]
    async fn creates_parent_directory() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("deep").join("test.jsonl");
        let log = DebugEventLog::new(path.clone());

        log.append(r#"{"ok":true}"#.into());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(path.exists());
    }

    #[tokio::test]
    async fn rotate_keeps_tail() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("rot.jsonl");

        // Write 10 lines.
        let mut content = String::new();
        for i in 0..10 {
            content.push_str(&format!("line-{i}\n"));
        }
        tokio::fs::write(&path, &content).await.unwrap();

        rotate(&path, 3).await.unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = after.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line-7");
        assert_eq!(lines[1], "line-8");
        assert_eq!(lines[2], "line-9");
    }
}
