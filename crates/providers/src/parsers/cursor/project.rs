// crates/providers/src/parsers/cursor/project.rs
//
// Project-name derivation from Cursor's hyphen-encoded directory names —
// exact port of agentsview's DecodeCursorProjectDir (cursor.go), including
// its unit-test table.

use std::path::Path;

/// Directory names very unlikely to appear inside usernames — checked first.
const HIGH_MARKERS: [&str; 4] = ["Documents", "Code", "projects", "repos"];
/// Names that could plausibly appear in usernames — fallback tier only.
const LOW_MARKERS: [&str; 4] = ["code", "src", "work", "dev"];

/// Derive the project from the path layout: the component preceding the
/// nearest `agent-transcripts` ancestor is the encoded project dir.
pub(super) fn project_from_path(path: &Path) -> String {
    let comps: Vec<&std::ffi::OsStr> = path.iter().collect();
    let decoded = comps
        .iter()
        .rposition(|c| c.to_str() == Some("agent-transcripts"))
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| comps[i].to_str())
        .map(decode_cursor_project_dir)
        .unwrap_or_default();
    if decoded.is_empty() {
        "unknown".to_string()
    } else {
        decoded
    }
}

/// Extract a clean project name from a Cursor-style hyphenated directory
/// name (Cursor encodes absolute paths by replacing `/` and `.` with `-`,
/// e.g. "Users-fiona-Documents-mcp-cursor-analytics"): a platform-prefix
/// minimum index plus a two-tier marker scan that handles multi-token
/// usernames.
pub(super) fn decode_cursor_project_dir(dir_name: &str) -> String {
    if dir_name.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = dir_name.split('-').collect();

    // Earliest position a marker could appear, from the platform root:
    //   macOS/Linux: Users-<name>-… / home-<name>-…  → min idx 2
    //   Windows:     C-Users-<name>-…                → min idx 3
    let min_idx = if parts.len() >= 3 && (parts[0] == "Users" || parts[0] == "home") {
        Some(2)
    } else if parts.len() >= 4 && parts[0].len() == 1 && parts[1] == "Users" {
        Some(3)
    } else {
        None
    };

    // Two-pass scan: high-confidence markers first, low-confidence as
    // fallback, so "Users-john-code-doe-Documents-app" finds Documents.
    if let Some(min_idx) = min_idx {
        for tier in [HIGH_MARKERS.as_slice(), LOW_MARKERS.as_slice()] {
            for (i, part) in parts.iter().enumerate().take(parts.len() - 1).skip(min_idx) {
                if !tier.contains(part) {
                    continue;
                }
                let result = parts[i + 1..].join("-");
                if !result.is_empty() {
                    return normalize_name(&result);
                }
            }
        }
    }

    // Fallback: last two components.
    if parts.len() >= 2 {
        return normalize_name(&parts[parts.len() - 2..].join("-"));
    }
    normalize_name(dir_name)
}

/// Dashes → underscores for consistent project names (Go NormalizeName).
fn normalize_name(s: &str) -> String {
    s.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_cursor_project_dir_two_tier_markers() {
        // Table ported verbatim from Go TestDecodeCursorProjectDir.
        let cases = [
            ("", ""),
            ("Users-fiona-Documents-project", "project"),
            ("Users-fiona-Documents-my-app", "my_app"),
            // Marker words inside project names must not truncate.
            ("Users-wesm-code-my-dev-tool", "my_dev_tool"),
            ("Users-wesm-code-work-bench", "work_bench"),
            ("Users-wesm-code-code-gen", "code_gen"),
            ("home-user-projects-my-app", "my_app"),
            ("C-Users-user-dev-my-project", "my_project"),
            // Multi-token usernames.
            ("Users-john-doe-Documents-my-app", "my_app"),
            ("home-john-doe-projects-my-super-app", "my_super_app"),
            ("C-Users-jane-smith-dev-project", "project"),
            // Low-confidence marker inside username; high marker wins.
            ("Users-john-code-doe-Documents-my-app", "my_app"),
            ("C-Users-jane-dev-smith-projects-my-app", "my_app"),
            ("Users-john-dev-my-Documents-app", "app"),
            // No recognized root — fallback to last two components.
            ("opt-builds-my-project", "my_project"),
        ];
        for (input, want) in cases {
            assert_eq!(decode_cursor_project_dir(input), want, "decode({input})");
        }
    }

    #[test]
    fn project_from_path_uses_encoded_ancestor() {
        let p = Path::new("/x/Users-fiona-Documents-my-app/agent-transcripts/a.txt");
        assert_eq!(project_from_path(p), "my_app");
        assert_eq!(project_from_path(Path::new("/tmp/a.txt")), "unknown");
    }
}
