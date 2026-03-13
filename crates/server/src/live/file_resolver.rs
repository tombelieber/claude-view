//! File path resolution and verification for VerifiedFile construction.

use std::path::Path;

use crate::live::state::{FileSourceKind, VerifiedFile};

/// Resolve a detected file reference to a VerifiedFile.
/// Returns None if the resolved path does not exist on disk.
pub fn resolve_file_path(
    raw_path: &str,
    kind: FileSourceKind,
    cwd: Option<&str>,
    project_dir: Option<&str>,
) -> Option<VerifiedFile> {
    let absolute = if raw_path.starts_with('/') {
        raw_path.to_string()
    } else if let Some(cwd) = cwd {
        format!("{}/{}", cwd.trim_end_matches('/'), raw_path)
    } else {
        return None;
    };

    if !Path::new(&absolute).exists() {
        return None;
    }

    let display_name = if let Some(proj) = project_dir {
        let prefix = format!("{}/", proj.trim_end_matches('/'));
        if absolute.starts_with(&prefix) {
            absolute[prefix.len()..].to_string()
        } else {
            Path::new(&absolute)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| absolute.clone())
        }
    } else {
        Path::new(&absolute)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| absolute.clone())
    };

    Some(VerifiedFile {
        path: absolute,
        kind,
        display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_resolution_at_mention_with_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("auth.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "// test").unwrap();

        let cwd = dir.path().to_str().unwrap();
        let result =
            resolve_file_path("src/auth.rs", FileSourceKind::Mention, Some(cwd), Some(cwd));
        let vf = result.expect("file exists, should resolve");
        assert_eq!(vf.display_name, "src/auth.rs");
        assert_eq!(vf.path, file.to_str().unwrap());
    }

    #[test]
    fn file_resolution_nonexistent_returns_none() {
        let result = resolve_file_path(
            "does_not_exist.rs",
            FileSourceKind::Mention,
            Some("/tmp"),
            Some("/tmp"),
        );
        assert!(result.is_none(), "non-existent file should return None");
    }

    #[test]
    fn file_resolution_absolute_path_keeps_as_is() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello").unwrap();

        let abs = file.to_str().unwrap();
        let result = resolve_file_path(
            abs,
            FileSourceKind::Pasted,
            Some("/irrelevant"),
            Some("/irrelevant"),
        );
        let vf = result.expect("file exists, should resolve");
        assert_eq!(vf.path, abs);
        assert_eq!(vf.display_name, "test.txt");
    }

    #[test]
    fn file_dedup_by_absolute_path() {
        use std::collections::HashSet;
        let mut seen: HashSet<String> = HashSet::new();
        let path = "/tmp/some/file.rs".to_string();
        assert!(seen.insert(path.clone()), "first insert should succeed");
        assert!(!seen.insert(path.clone()), "duplicate should be rejected");
    }
}
