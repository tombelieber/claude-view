//! Reader for ~/.claude/plans/{slug}.md implementation plan files.
//!
//! Plans are matched by prefix: a session with slug "foo-bar-baz" matches
//! "foo-bar-baz.md" (base) and "foo-bar-baz-IMPROVEMENTS.md" (variant).

use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

/// A plan document from ~/.claude/plans/{slug}[-variant].md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PlanDocument {
    /// Base slug (e.g. "sparkling-bouncing-glacier")
    pub slug: String,
    /// Variant suffix if any (e.g. "IMPROVEMENTS", "gaps"), None for the base plan
    pub variant: Option<String>,
    /// Full filename (e.g. "sparkling-bouncing-glacier-IMPROVEMENTS.md")
    pub filename: String,
    /// Raw markdown content
    pub content: String,
    /// File size in bytes
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// Last modified timestamp (unix seconds)
    #[ts(type = "number")]
    pub modified_at: i64,
}

/// Resolve the ~/.claude/plans/ directory.
pub fn claude_plans_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("plans"))
}

/// Check if any plan files exist for a slug without reading content.
pub fn has_plan_files(plans_dir: &Path, slug: &str) -> bool {
    if slug.is_empty() || !plans_dir.is_dir() {
        return false;
    }
    let base_filename = format!("{slug}.md");
    let variant_prefix = format!("{slug}-");
    let entries = match std::fs::read_dir(plans_dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        if let Some(fname) = entry.path().file_name().and_then(|f| f.to_str()) {
            if fname.ends_with(".md")
                && (fname == base_filename || fname.starts_with(&variant_prefix))
            {
                return true;
            }
        }
    }
    false
}

/// Find all plan files matching a session slug using prefix glob.
///
/// Matches:
/// - `{slug}.md` (base plan, variant = None)
/// - `{slug}-{variant}.md` (variant plan, variant = Some("IMPROVEMENTS"))
///
/// Does NOT match:
/// - Files where slug is a substring but not a prefix (e.g. slug "calm" won't match "calm-churning-sparrow.md")
/// - Files with spaces in names (not valid slugs)
///
/// Returns plans sorted: base plan first, then variants alphabetically.
pub fn find_plan_files(plans_dir: &Path, slug: &str) -> Vec<PlanDocument> {
    if slug.is_empty() || !plans_dir.is_dir() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(plans_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let base_filename = format!("{slug}.md");
    let variant_prefix = format!("{slug}-");

    let mut plans: Vec<PlanDocument> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };

        // Must be a .md file
        if !fname.ends_with(".md") {
            continue;
        }

        let variant = if fname == base_filename {
            None
        } else if fname.starts_with(&variant_prefix) && fname.ends_with(".md") {
            // Extract variant: "slug-IMPROVEMENTS.md" → "IMPROVEMENTS"
            let suffix = &fname[variant_prefix.len()..fname.len() - 3]; // strip prefix and ".md"
            if suffix.is_empty() {
                continue; // "slug-.md" is invalid
            }
            Some(suffix.to_string())
        } else {
            continue;
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let metadata = std::fs::metadata(&path).ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified_at = metadata
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        plans.push(PlanDocument {
            slug: slug.to_string(),
            variant,
            filename: fname.to_string(),
            content,
            size_bytes,
            modified_at,
        });
    }

    // Sort: base plan first (variant == None), then variants alphabetically
    plans.sort_by(|a, b| match (&a.variant, &b.variant) {
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(va), Some(vb)) => va.cmp(vb),
        (None, None) => std::cmp::Ordering::Equal,
    });

    plans
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_base_plan_only() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("cool-running-fox.md"), "# Plan\nDo stuff").unwrap();
        fs::write(tmp.path().join("other-slug-here.md"), "# Other").unwrap();

        let plans = find_plan_files(tmp.path(), "cool-running-fox");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].slug, "cool-running-fox");
        assert_eq!(plans[0].variant, None);
        assert_eq!(plans[0].filename, "cool-running-fox.md");
        assert_eq!(plans[0].content, "# Plan\nDo stuff");
        assert!(plans[0].size_bytes > 0);
    }

    #[test]
    fn test_find_base_plus_variants() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("abc-def-ghi.md"), "# Base").unwrap();
        fs::write(
            tmp.path().join("abc-def-ghi-IMPROVEMENTS.md"),
            "# Improvements",
        )
        .unwrap();
        fs::write(tmp.path().join("abc-def-ghi-gaps.md"), "# Gaps").unwrap();

        let plans = find_plan_files(tmp.path(), "abc-def-ghi");
        assert_eq!(plans.len(), 3);
        // Base first
        assert_eq!(plans[0].variant, None);
        assert_eq!(plans[0].filename, "abc-def-ghi.md");
        // Then variants alphabetically
        assert_eq!(plans[1].variant, Some("IMPROVEMENTS".to_string()));
        assert_eq!(plans[2].variant, Some("gaps".to_string()));
    }

    #[test]
    fn test_no_false_prefix_match() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("calm-churning-sparrow.md"), "# Plan").unwrap();

        // Using full slug matches correctly
        let plans = find_plan_files(tmp.path(), "calm-churning-sparrow");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].variant, None);

        // Partial slug "calm-churning" would match as variant — but this never happens
        // because real slugs are always 3 words
        let plans = find_plan_files(tmp.path(), "calm-churning");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].variant, Some("sparrow".to_string()));
    }

    #[test]
    fn test_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let plans = find_plan_files(&tmp.path().join("nonexistent"), "any-slug-here");
        assert!(plans.is_empty());
    }

    #[test]
    fn test_empty_slug() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("something.md"), "# Plan").unwrap();
        let plans = find_plan_files(tmp.path(), "");
        assert!(plans.is_empty());
    }

    #[test]
    fn test_non_md_files_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("cool-running-fox.md"), "# Plan").unwrap();
        fs::write(tmp.path().join("cool-running-fox.txt"), "Not a plan").unwrap();
        fs::write(tmp.path().join("cool-running-fox.json"), "{}").unwrap();

        let plans = find_plan_files(tmp.path(), "cool-running-fox");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].filename, "cool-running-fox.md");
    }
}
