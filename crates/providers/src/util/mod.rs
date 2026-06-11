// crates/providers/src/util/mod.rs

pub mod blocks;
pub mod jsonl;
pub mod time;

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
