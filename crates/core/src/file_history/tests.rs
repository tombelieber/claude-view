#![cfg(test)]

use super::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[test]
fn test_parse_backup_filename() {
    use super::helpers::parse_backup_filename;

    assert_eq!(
        parse_backup_filename("abc123@v1"),
        Some(("abc123".to_string(), 1))
    );
    assert_eq!(
        parse_backup_filename("abc123@v12"),
        Some(("abc123".to_string(), 12))
    );
    assert_eq!(parse_backup_filename("noversion"), None);
    assert_eq!(parse_backup_filename("hash@x1"), None);
}

#[test]
fn test_scan_file_history_basic() {
    let tmp = tempfile::tempdir().unwrap();
    let session_dir = tmp.path().join("sess-1");
    fs::create_dir_all(&session_dir).unwrap();

    fs::write(session_dir.join("abc@v1"), "line1\nline2\n").unwrap();
    fs::write(session_dir.join("abc@v2"), "line1\nline2\nline3\n").unwrap();

    // Map is keyed by hash (not backupFileName) -- matches extract_file_path_map output
    let map = HashMap::from([("abc".to_string(), "src/main.rs".to_string())]);

    let result = scan_file_history(tmp.path(), "sess-1", &map);
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].file_path, "src/main.rs");
    assert_eq!(result.files[0].versions.len(), 2);
    assert_eq!(result.files[0].stats.added, 1);
    assert_eq!(result.files[0].stats.removed, 0);
    assert_eq!(result.summary.total_files, 1);
}

#[test]
fn test_scan_file_history_missing_session() {
    let tmp = tempfile::tempdir().unwrap();
    let result = scan_file_history(tmp.path(), "nonexistent", &HashMap::new());
    assert!(result.files.is_empty());
}

#[test]
fn test_scan_file_history_single_version() {
    let tmp = tempfile::tempdir().unwrap();
    let session_dir = tmp.path().join("sess-2");
    fs::create_dir_all(&session_dir).unwrap();

    fs::write(session_dir.join("xyz@v1"), "a\nb\nc\n").unwrap();

    let result = scan_file_history(tmp.path(), "sess-2", &HashMap::new());
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].stats.added, 3); // 3 lines, all "new"
    assert_eq!(result.files[0].stats.removed, 0);
}

#[test]
fn test_compute_diff_basic() {
    let tmp = tempfile::tempdir().unwrap();
    let session_dir = tmp.path().join("sess-d");
    fs::create_dir_all(&session_dir).unwrap();

    fs::write(session_dir.join("h1@v1"), "alpha\nbeta\ngamma\n").unwrap();
    fs::write(
        session_dir.join("h1@v2"),
        "alpha\nbeta modified\ngamma\ndelta\n",
    )
    .unwrap();

    let result = compute_diff(tmp.path(), "sess-d", "h1", 1, 2, "test.rs", 3).unwrap();

    assert_eq!(result.from_version, 1);
    assert_eq!(result.to_version, 2);
    assert_eq!(result.stats.added, 2); // "beta modified" + "delta"
    assert_eq!(result.stats.removed, 1); // "beta"
    assert!(!result.hunks.is_empty());
}

#[test]
fn test_compute_diff_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let session_dir = tmp.path().join("sess-m");
    fs::create_dir_all(&session_dir).unwrap();

    let result = compute_diff(tmp.path(), "sess-m", "nope", 1, 2, "test.rs", 3);
    assert!(result.is_err());
}

#[test]
fn test_files_sorted_by_version_count_desc() {
    let tmp = tempfile::tempdir().unwrap();
    let session_dir = tmp.path().join("sess-s");
    fs::create_dir_all(&session_dir).unwrap();

    // File A: 1 version
    fs::write(session_dir.join("aaa@v1"), "a\n").unwrap();
    // File B: 3 versions
    fs::write(session_dir.join("bbb@v1"), "b\n").unwrap();
    fs::write(session_dir.join("bbb@v2"), "b\nc\n").unwrap();
    fs::write(session_dir.join("bbb@v3"), "b\nc\nd\n").unwrap();
    // File C: 2 versions
    fs::write(session_dir.join("ccc@v1"), "c\n").unwrap();
    fs::write(session_dir.join("ccc@v2"), "c\nd\n").unwrap();

    let result = scan_file_history(tmp.path(), "sess-s", &HashMap::new());
    assert_eq!(result.files.len(), 3);
    assert_eq!(result.files[0].versions.len(), 3); // bbb first
    assert_eq!(result.files[1].versions.len(), 2); // ccc second
    assert_eq!(result.files[2].versions.len(), 1); // aaa last
}

#[test]
fn test_validate_file_hash() {
    assert!(validate_file_hash("abc123def456").is_ok());
    assert!(validate_file_hash("").is_err());
    assert!(validate_file_hash("../../../etc/passwd").is_err());
    assert!(validate_file_hash("hash/with/slashes").is_err());
    assert!(validate_file_hash("hash\\backslash").is_err());
    assert!(validate_file_hash("..").is_err());
}

// ====================================================================
// parse_backup_filename edge cases
// ====================================================================

#[test]
fn test_parse_backup_filename_edge_cases() {
    use super::helpers::parse_backup_filename;

    // Empty string
    assert_eq!(parse_backup_filename(""), None);

    // Hash containing '@' -- rfind picks the last '@'
    assert_eq!(
        parse_backup_filename("hash@extra@v1"),
        Some(("hash@extra".to_string(), 1))
    );

    // Version 0
    assert_eq!(
        parse_backup_filename("hash@v0"),
        Some(("hash".to_string(), 0))
    );

    // Trailing content after version number -- parse fails
    assert_eq!(parse_backup_filename("hash@v1.bak"), None);

    // Real-world hex hash
    assert_eq!(
        parse_backup_filename("be8cf68e283682af@v1"),
        Some(("be8cf68e283682af".to_string(), 1))
    );
}

// ====================================================================
// extract_file_path_map tests
// ====================================================================

#[test]
fn test_extract_file_path_map_happy_path() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let line = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/main.rs":{"backupFileName":"abc123@v1","version":1,"backupTime":"2026-01-01T00:00:00Z"},"/src/lib.rs":{"backupFileName":"def456@v2","version":2,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
    fs::write(tmp.path(), format!("{line}\n")).unwrap();

    let map = extract_file_path_map(tmp.path());
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("abc123").unwrap(), "/src/main.rs");
    assert_eq!(map.get("def456").unwrap(), "/src/lib.rs");
}

#[test]
fn test_extract_file_path_map_nonexistent_file() {
    let map = extract_file_path_map(Path::new("/tmp/does-not-exist-ever-12345.jsonl"));
    assert!(map.is_empty());
}

#[test]
fn test_extract_file_path_map_empty_file() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), "").unwrap();

    let map = extract_file_path_map(tmp.path());
    assert!(map.is_empty());
}

#[test]
fn test_extract_file_path_map_mixed_valid_invalid_lines() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let valid_snapshot = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/main.rs":{"backupFileName":"abc123@v1","version":1,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
    let malformed_json = r#"{broken"#;
    let assistant_line = r#"{"type":"assistant","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]}}"#;
    let snapshot_missing_tracked = r#"{"type":"file-history-snapshot","snapshot":{}}"#;

    let content = format!(
        "{valid_snapshot}\n{malformed_json}\n{assistant_line}\n{snapshot_missing_tracked}\n"
    );
    fs::write(tmp.path(), content).unwrap();

    let map = extract_file_path_map(tmp.path());
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("abc123").unwrap(), "/src/main.rs");
}

#[test]
fn test_extract_file_path_map_later_snapshot_overwrites_earlier() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let snapshot1 = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/old_path.rs":{"backupFileName":"samehash@v1","version":1,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
    let snapshot2 = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/new_path.rs":{"backupFileName":"samehash@v2","version":2,"backupTime":"2026-01-01T01:00:00Z"}}}}"#;

    let content = format!("{snapshot1}\n{snapshot2}\n");
    fs::write(tmp.path(), content).unwrap();

    let map = extract_file_path_map(tmp.path());
    // Both snapshots map hash "samehash" -- second one wins
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("samehash").unwrap(), "/src/new_path.rs");
}

#[test]
fn test_extract_file_path_map_missing_backup_file_name() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // Entry has version and backupTime but no backupFileName
    let snapshot = r#"{"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{"/src/main.rs":{"version":1,"backupTime":"2026-01-01T00:00:00Z"}}}}"#;
    fs::write(tmp.path(), format!("{snapshot}\n")).unwrap();

    let map = extract_file_path_map(tmp.path());
    assert!(map.is_empty());
}
