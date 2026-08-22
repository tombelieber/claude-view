// crates/providers/src/util/mod.rs

pub mod blocks;
pub mod jsonl;
pub mod time;

/// Max bytes for whole-document reads of foreign session files. An
/// oversized file — hostile or organic — must fail its own parse, never
/// OOM the server (foreign data must not destabilize the CC pipeline).
pub const MAX_DOCUMENT_BYTES: u64 = 128 * 1024 * 1024;

/// `std::fs::read_to_string` with the [`MAX_DOCUMENT_BYTES`] cap.
pub fn read_to_string_capped<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<String> {
    let path = path.as_ref();
    let len = std::fs::metadata(path)?.len();
    if len > MAX_DOCUMENT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "session file {} is {len} bytes — exceeds the {}MB cap",
                path.display(),
                MAX_DOCUMENT_BYTES / (1024 * 1024)
            ),
        ));
    }
    std::fs::read_to_string(path)
}

/// First N chars of a string for previews, on a char boundary, single line.
pub fn preview(text: &str, max_chars: usize) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    line.chars().take(max_chars).collect::<String>().trim().to_string()
}

/// Derive a project display name from a cwd path: final component, with the
/// common `~/dev/org/name` shape collapsing to `name`.
pub fn project_from_cwd(cwd: &str) -> String {
    std::path::Path::new(cwd)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.to_string())
}
