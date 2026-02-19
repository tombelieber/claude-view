use std::io;
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Default chunk size for backward reading: 8KB.
const CHUNK_SIZE: u64 = 8 * 1024;

/// Read the last `n` lines from a file without loading the entire file.
///
/// Strategy: seek to EOF, read backwards in 8KB chunks, find newlines.
/// Returns lines in chronological order (oldest first).
///
/// Edge cases handled:
/// - `n == 0` returns an empty vec
/// - Empty file returns an empty vec
/// - If the file has fewer than `n` lines, all lines are returned
/// - A trailing newline at EOF does not produce an empty last line
/// - Lines longer than the chunk size are assembled correctly
pub async fn tail_lines(path: &Path, n: usize) -> io::Result<Vec<String>> {
    if n == 0 {
        return Ok(Vec::new());
    }

    let mut file = tokio::fs::File::open(path).await?;
    let file_len = file.metadata().await?.len();

    if file_len == 0 {
        return Ok(Vec::new());
    }

    // Read backwards in chunks, collecting bytes from EOF toward BOF.
    // We need to find enough newline-delimited lines. To get `n` lines,
    // we need `n` newlines (each line ends with \n), but we also need the
    // start of the nth-from-last line. A trailing \n at EOF does not start
    // a new (empty) line.
    //
    // Algorithm:
    // 1. Read chunks from the end backward.
    // 2. Count newlines encountered. We need `n + 1` newlines to delimit
    //    `n` complete lines (the extra one is the boundary before the first
    //    of the n lines). BUT if there's a trailing newline, it doesn't
    //    count as starting a new line, so we need `n + 1` for trailing or
    //    `n` for non-trailing.
    // 3. Simpler: just read enough to have at least n lines, then split
    //    and take the last n.

    let mut collected: Vec<u8> = Vec::new();
    let mut remaining = file_len;

    // We need n+1 newlines to fully delimit n lines from the end
    // (the +1 is for the boundary before the first included line).
    // To be safe with trailing newlines, we look for n+1 newlines.
    let target_newlines = n + 1;
    let mut newline_count = 0usize;

    while remaining > 0 {
        let chunk_len = remaining.min(CHUNK_SIZE);
        let offset = remaining - chunk_len;

        file.seek(io::SeekFrom::Start(offset)).await?;

        let mut buf = vec![0u8; chunk_len as usize];
        file.read_exact(&mut buf).await?;

        // Count newlines in this chunk.
        for &byte in &buf {
            if byte == b'\n' {
                newline_count += 1;
            }
        }

        // Prepend this chunk to collected bytes.
        buf.append(&mut collected);
        collected = buf;

        remaining = offset;

        // If we've found enough newlines, we definitely have n lines.
        if newline_count >= target_newlines {
            break;
        }
    }

    // Convert to string and split into lines.
    let text = String::from_utf8_lossy(&collected);
    let text = text.as_ref();

    // Strip a single trailing newline so it doesn't produce an empty last element.
    let text = text.strip_suffix('\n').unwrap_or(text);

    if text.is_empty() {
        return Ok(Vec::new());
    }

    let all_lines: Vec<&str> = text.split('\n').collect();

    // Take only the last n lines.
    let start = all_lines.len().saturating_sub(n);

    Ok(all_lines[start..]
        .iter()
        .map(|s| s.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn tail_0_lines_returns_empty() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "line1").unwrap();
        writeln!(f, "line2").unwrap();
        f.flush().unwrap();

        let result = tail_lines(f.path(), 0).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn tail_fewer_than_n() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "alpha").unwrap();
        writeln!(f, "beta").unwrap();
        writeln!(f, "gamma").unwrap();
        f.flush().unwrap();

        let result = tail_lines(f.path(), 100).await.unwrap();
        assert_eq!(result, vec!["alpha", "beta", "gamma"]);
    }

    #[tokio::test]
    async fn tail_exact() {
        let mut f = NamedTempFile::new().unwrap();
        for i in 0..100 {
            writeln!(f, "line{}", i).unwrap();
        }
        f.flush().unwrap();

        let result = tail_lines(f.path(), 100).await.unwrap();
        assert_eq!(result.len(), 100);
        assert_eq!(result[0], "line0");
        assert_eq!(result[99], "line99");
    }

    #[tokio::test]
    async fn tail_last_5() {
        let mut f = NamedTempFile::new().unwrap();
        for i in 0..1000 {
            writeln!(f, "line{}", i).unwrap();
        }
        f.flush().unwrap();

        let result = tail_lines(f.path(), 5).await.unwrap();
        assert_eq!(
            result,
            vec!["line995", "line996", "line997", "line998", "line999"]
        );
    }

    #[tokio::test]
    async fn tail_empty_file() {
        let f = NamedTempFile::new().unwrap();
        let result = tail_lines(f.path(), 10).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn tail_large_lines() {
        // Lines longer than the 8KB chunk size.
        let mut f = NamedTempFile::new().unwrap();
        let big_line_a = "A".repeat(10_000);
        let big_line_b = "B".repeat(12_000);
        let big_line_c = "C".repeat(9_000);
        writeln!(f, "{}", big_line_a).unwrap();
        writeln!(f, "{}", big_line_b).unwrap();
        writeln!(f, "{}", big_line_c).unwrap();
        f.flush().unwrap();

        let result = tail_lines(f.path(), 2).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], big_line_b);
        assert_eq!(result[1], big_line_c);
    }

    #[tokio::test]
    async fn tail_no_trailing_newline() {
        // File without a trailing newline should still work.
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "line1\nline2\nline3").unwrap(); // no trailing \n
        f.flush().unwrap();

        let result = tail_lines(f.path(), 2).await.unwrap();
        assert_eq!(result, vec!["line2", "line3"]);
    }

    #[tokio::test]
    async fn tail_single_line_no_newline() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "only line").unwrap();
        f.flush().unwrap();

        let result = tail_lines(f.path(), 5).await.unwrap();
        assert_eq!(result, vec!["only line"]);
    }

    #[tokio::test]
    async fn tail_single_line_with_newline() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "only line").unwrap();
        f.flush().unwrap();

        let result = tail_lines(f.path(), 5).await.unwrap();
        assert_eq!(result, vec!["only line"]);
    }

    #[tokio::test]
    async fn tail_large_file_performance() {
        // Create a 10MB+ temp file with many lines
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.jsonl");

        let mut content = String::new();
        let line = format!(
            "{{\"type\":\"assistant\",\"content\":\"{}\"}}\n",
            "x".repeat(200)
        );
        // Each line is ~220 bytes. 10MB / 220 = ~47,000 lines
        for _ in 0..50_000 {
            content.push_str(&line);
        }
        tokio::fs::write(&path, &content).await.unwrap();

        let start = std::time::Instant::now();
        let lines = tail_lines(&path, 10).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(lines.len(), 10);
        // Must complete in <50ms (plan says <10ms, but be generous for CI)
        assert!(
            elapsed.as_millis() < 50,
            "tail_lines on 10MB file took {}ms, expected <50ms",
            elapsed.as_millis()
        );
    }
}
