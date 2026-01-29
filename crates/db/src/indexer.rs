// crates/db/src/indexer.rs
//! Indexer module: scan → diff → parse → store pipeline.
//!
//! Orchestrates the full indexing flow:
//! 1. `scan_files()` — discover all .jsonl files under a base directory
//! 2. `diff_against_db()` — compare discovered files against DB state
//! 3. `index_files()` — parse changed files, store in DB, report progress

use crate::{Database, DbResult};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, warn};
use vibe_recall_core::{extract_session_metadata, resolve_project_path, SessionInfo};

// ============================================================================
// Types
// ============================================================================

/// Information about a discovered .jsonl file.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Full path to the .jsonl file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time as Unix timestamp (seconds).
    pub modified_at: i64,
    /// Encoded project directory name (e.g., "-Users-foo-project-a").
    pub project_dir: String,
}

/// Result of scanning a base directory for .jsonl files.
#[derive(Debug)]
pub struct ScanResult {
    /// All discovered files.
    pub files: Vec<FileInfo>,
    /// Number of distinct project directories found.
    pub project_count: usize,
    /// Total size of all discovered files in bytes.
    pub total_size: u64,
}

/// Result of diffing discovered files against the database.
#[derive(Debug)]
pub struct DiffResult {
    /// Files not yet in the database.
    pub new_files: Vec<FileInfo>,
    /// Files whose size or mtime changed since last index.
    pub modified_files: Vec<FileInfo>,
    /// Count of files that have not changed.
    pub unchanged_count: usize,
    /// File paths that exist in the DB but no longer on disk.
    pub deleted_paths: Vec<String>,
}

// ============================================================================
// scan_files
// ============================================================================

/// Scan `base_dir` for all .jsonl session files.
///
/// Expects the structure:
/// ```text
/// base_dir/
///   <encoded-project-dir>/
///     <session-id>.jsonl
/// ```
///
/// Returns a `ScanResult` with all discovered files, project count, and total size.
pub async fn scan_files(base_dir: &Path) -> ScanResult {
    let mut files = Vec::new();
    let mut project_dirs = std::collections::HashSet::new();
    let mut total_size: u64 = 0;

    // Read project directories
    let mut entries = match fs::read_dir(base_dir).await {
        Ok(e) => e,
        Err(e) => {
            debug!("Cannot read base dir {:?}: {}", base_dir, e);
            return ScanResult {
                files,
                project_count: 0,
                total_size: 0,
            };
        }
    };

    while let Ok(Some(project_entry)) = entries.next_entry().await {
        let project_path = project_entry.path();

        // Skip non-directories (use async file_type to avoid blocking)
        let file_type = match project_entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }

        let project_dir = project_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Read session files inside this project dir
        let mut session_entries = match fs::read_dir(&project_path).await {
            Ok(e) => e,
            Err(e) => {
                debug!("Cannot read project dir {:?}: {}", project_path, e);
                continue;
            }
        };

        let mut found_in_project = false;

        while let Ok(Some(session_entry)) = session_entries.next_entry().await {
            let file_path = session_entry.path();

            // Only .jsonl files
            if file_path.extension().map(|e| e != "jsonl").unwrap_or(true) {
                continue;
            }

            let metadata = match fs::metadata(&file_path).await {
                Ok(m) => m,
                Err(_) => continue,
            };

            if !metadata.is_file() {
                continue;
            }

            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let size = metadata.len();
            total_size += size;
            found_in_project = true;

            files.push(FileInfo {
                path: file_path,
                size,
                modified_at,
                project_dir: project_dir.clone(),
            });
        }

        if found_in_project {
            project_dirs.insert(project_dir);
        }
    }

    ScanResult {
        files,
        project_count: project_dirs.len(),
        total_size,
    }
}

// ============================================================================
// diff_against_db
// ============================================================================

/// Compare discovered files against the database indexer state.
///
/// Returns a `DiffResult` describing which files are new, modified, unchanged,
/// or deleted (present in DB but not on disk).
pub async fn diff_against_db(files: &[FileInfo], db: &Database) -> DbResult<DiffResult> {
    let mut new_files = Vec::new();
    let mut modified_files = Vec::new();
    let mut unchanged_count: usize = 0;

    // Batch-load all indexer states in one query (avoids N+1 pattern)
    let all_states = db.get_all_indexer_states().await?;

    // Collect valid paths for stale detection
    let valid_paths: std::collections::HashSet<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();

    for file in files {
        let path_str = file.path.to_string_lossy().to_string();
        match all_states.get(&path_str) {
            None => {
                new_files.push(file.clone());
            }
            Some(entry) => {
                if entry.file_size != file.size as i64 || entry.modified_at != file.modified_at {
                    modified_files.push(file.clone());
                } else {
                    unchanged_count += 1;
                }
            }
        }
    }

    // Find deleted paths: entries in DB whose path is not in the scanned file set
    let deleted_paths: Vec<String> = all_states
        .keys()
        .filter(|path| !valid_paths.contains(path.as_str()))
        .cloned()
        .collect();

    Ok(DiffResult {
        new_files,
        modified_files,
        unchanged_count,
        deleted_paths,
    })
}

// ============================================================================
// index_files
// ============================================================================

/// Index a set of files: parse metadata, store in DB, and call progress callback.
///
/// `on_progress` is called after each file with `(indexed_so_far, total)`.
///
/// At the end, removes stale sessions (files that exist in DB but not on disk).
pub async fn index_files<F>(
    files: &[FileInfo],
    all_valid_paths: &[String],
    db: &Database,
    on_progress: F,
) -> DbResult<()>
where
    F: Fn(usize, usize),
{
    let total = files.len();

    for (i, file) in files.iter().enumerate() {
        let path_str = file.path.to_string_lossy().to_string();

        // Extract session metadata
        let extracted = extract_session_metadata(&file.path).await;

        // Derive session ID from filename (stem)
        let session_id = file
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Resolve project path
        let resolved = resolve_project_path(&file.project_dir);

        // Build SessionInfo
        let session = SessionInfo {
            id: session_id,
            project: file.project_dir.clone(),
            project_path: resolved.full_path.clone(),
            file_path: path_str.clone(),
            modified_at: file.modified_at,
            size_bytes: file.size,
            preview: extracted.preview,
            last_message: extracted.last_message,
            files_touched: extracted.files_touched,
            skills_used: extracted.skills_used,
            tool_counts: extracted.tool_counts,
            message_count: extracted.message_count,
            turn_count: extracted.turn_count,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            // Phase 3: Atomic unit metrics (initialized to defaults, populated by deep indexing)
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            summary_text: None,
            parse_version: 0,
        };

        // Store in DB
        if let Err(e) = db
            .insert_session(&session, &file.project_dir, &resolved.display_name)
            .await
        {
            warn!("Failed to insert session {}: {}", path_str, e);
        }

        // Update indexer state
        if let Err(e) = db
            .update_indexer_state(&path_str, file.size as i64, file.modified_at)
            .await
        {
            warn!("Failed to update indexer state for {}: {}", path_str, e);
        }

        // Report progress
        on_progress(i + 1, total);
    }

    // Remove stale sessions
    db.remove_stale_sessions(all_valid_paths).await?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper: create a temp dir mimicking ~/.claude/projects/ structure.
    /// Returns (temp_dir, base_path) where base_path is the "projects" dir.
    async fn setup_test_dir(projects: &[(&str, &[&str])]) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().to_path_buf();

        for (project_name, sessions) in projects {
            let project_dir = base.join(project_name);
            fs::create_dir_all(&project_dir).await.unwrap();

            for session_file in *sessions {
                let file_path = project_dir.join(session_file);
                // Write minimal valid JSONL content
                let content = format!(
                    r#"{{"type":"user","message":{{"content":"Test session in {}"}}}}
{{"type":"assistant","message":{{"content":"Response"}}}}"#,
                    session_file
                );
                fs::write(&file_path, &content).await.unwrap();
            }
        }

        (temp_dir, base)
    }

    // ========================================================================
    // test_scan_files — finds .jsonl files in temp dir
    // ========================================================================

    #[tokio::test]
    async fn test_scan_files() {
        let (_tmp, base) = setup_test_dir(&[
            ("-Users-foo-project-a", &["abc123.jsonl", "def456.jsonl"]),
            ("-Users-foo-project-b", &["ghi789.jsonl"]),
        ])
        .await;

        let result = scan_files(&base).await;

        assert_eq!(result.files.len(), 3, "Should find 3 .jsonl files");
        assert_eq!(result.project_count, 2, "Should find 2 project dirs");
        assert!(result.total_size > 0, "Total size should be > 0");

        // All files should have valid metadata
        for f in &result.files {
            assert!(f.size > 0);
            assert!(f.modified_at > 0);
            assert!(f.path.extension().unwrap() == "jsonl");
            assert!(!f.project_dir.is_empty());
        }
    }

    // ========================================================================
    // test_diff_new_files — all files are "new" on first run
    // ========================================================================

    #[tokio::test]
    async fn test_diff_new_files() {
        let (_tmp, base) = setup_test_dir(&[
            ("-Users-test-proj", &["s1.jsonl", "s2.jsonl"]),
        ])
        .await;

        let db = Database::new_in_memory().await.unwrap();
        let scan = scan_files(&base).await;

        let diff = diff_against_db(&scan.files, &db).await.unwrap();

        assert_eq!(diff.new_files.len(), 2, "All files should be new on first run");
        assert_eq!(diff.modified_files.len(), 0, "No modified files on first run");
        assert_eq!(diff.unchanged_count, 0, "No unchanged files on first run");
        assert_eq!(diff.deleted_paths.len(), 0, "No deleted paths on first run");
    }

    // ========================================================================
    // test_diff_unchanged — no changes when mtime/size match
    // ========================================================================

    #[tokio::test]
    async fn test_diff_unchanged() {
        let (_tmp, base) = setup_test_dir(&[
            ("-Users-test-proj", &["s1.jsonl"]),
        ])
        .await;

        let db = Database::new_in_memory().await.unwrap();
        let scan = scan_files(&base).await;

        // Simulate having already indexed these files
        for f in &scan.files {
            let path_str = f.path.to_string_lossy().to_string();
            db.update_indexer_state(&path_str, f.size as i64, f.modified_at)
                .await
                .unwrap();
        }

        let diff = diff_against_db(&scan.files, &db).await.unwrap();

        assert_eq!(diff.new_files.len(), 0, "No new files");
        assert_eq!(diff.modified_files.len(), 0, "No modified files");
        assert_eq!(diff.unchanged_count, 1, "1 unchanged file");
        assert_eq!(diff.deleted_paths.len(), 0, "No deleted files");
    }

    // ========================================================================
    // test_diff_modified — detects changed mtime
    // ========================================================================

    #[tokio::test]
    async fn test_diff_modified() {
        let (_tmp, base) = setup_test_dir(&[
            ("-Users-test-proj", &["s1.jsonl"]),
        ])
        .await;

        let db = Database::new_in_memory().await.unwrap();
        let scan = scan_files(&base).await;
        let file = &scan.files[0];
        let path_str = file.path.to_string_lossy().to_string();

        // Record with an older mtime to simulate a modified file
        db.update_indexer_state(&path_str, file.size as i64, file.modified_at - 100)
            .await
            .unwrap();

        let diff = diff_against_db(&scan.files, &db).await.unwrap();

        assert_eq!(diff.new_files.len(), 0, "Not new");
        assert_eq!(diff.modified_files.len(), 1, "Should detect modified file");
        assert_eq!(diff.unchanged_count, 0, "Not unchanged");
    }

    // ========================================================================
    // test_diff_deleted — detects removed files
    // ========================================================================

    #[tokio::test]
    async fn test_diff_deleted() {
        let (_tmp, base) = setup_test_dir(&[
            ("-Users-test-proj", &["s1.jsonl"]),
        ])
        .await;

        let db = Database::new_in_memory().await.unwrap();

        // Pre-index a file that no longer exists on disk
        db.update_indexer_state("/old/deleted-session.jsonl", 1024, 1000000)
            .await
            .unwrap();

        let scan = scan_files(&base).await;
        let diff = diff_against_db(&scan.files, &db).await.unwrap();

        assert_eq!(diff.deleted_paths.len(), 1, "Should detect 1 deleted path");
        assert_eq!(diff.deleted_paths[0], "/old/deleted-session.jsonl");
    }

    // ========================================================================
    // test_index_calls_progress — progress callback called with correct counts
    // ========================================================================

    #[tokio::test]
    async fn test_index_calls_progress() {
        let (_tmp, base) = setup_test_dir(&[
            ("-Users-test-proj", &["s1.jsonl", "s2.jsonl", "s3.jsonl"]),
        ])
        .await;

        let db = Database::new_in_memory().await.unwrap();
        let scan = scan_files(&base).await;
        let valid_paths: Vec<String> = scan
            .files
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect();

        let progress_count = Arc::new(AtomicUsize::new(0));
        let last_indexed = Arc::new(AtomicUsize::new(0));
        let last_total = Arc::new(AtomicUsize::new(0));

        let pc = progress_count.clone();
        let li = last_indexed.clone();
        let lt = last_total.clone();

        index_files(&scan.files, &valid_paths, &db, move |indexed, total| {
            pc.fetch_add(1, Ordering::SeqCst);
            li.store(indexed, Ordering::SeqCst);
            lt.store(total, Ordering::SeqCst);
        })
        .await
        .unwrap();

        assert_eq!(
            progress_count.load(Ordering::SeqCst),
            3,
            "Progress callback should be called once per file"
        );
        assert_eq!(
            last_indexed.load(Ordering::SeqCst),
            3,
            "Last indexed count should equal total files"
        );
        assert_eq!(
            last_total.load(Ordering::SeqCst),
            3,
            "Total should be 3"
        );

        // Verify data was actually stored in the DB
        let projects = db.list_projects().await.unwrap();
        assert!(!projects.is_empty(), "Should have stored at least one project");
    }
}
