// crates/db/src/indexer_parallel.rs
// Fast JSONL parsing with memory-mapped I/O and SIMD-accelerated scanning.
// Also contains the two-pass indexing pipeline: Pass 1 (index JSON) and Pass 2 (deep JSONL).

use memchr::memmem;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use vibe_recall_core::{read_all_session_indexes, resolve_project_path, ToolCounts};

use crate::Database;

/// Extended metadata extracted from JSONL deep parsing (Pass 2 only).
/// These fields are NOT available from sessions-index.json.
#[derive(Debug, Clone, Default)]
pub struct ExtendedMetadata {
    pub tool_counts: ToolCounts,
    pub skills_used: Vec<String>,
    pub files_touched: Vec<String>,
    pub last_message: String,
    pub turn_count: usize,
}

/// Read a file using memory-mapped I/O with fallback to regular read.
/// mmap is faster for large files (>64KB) because it avoids copying data through kernel buffers.
pub fn read_file_fast(path: &Path) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len() as usize;

    if len == 0 {
        return Ok(Vec::new());
    }

    // For small files, regular read is faster (no mmap overhead)
    if len < 64 * 1024 {
        return std::fs::read(path);
    }

    // Try mmap, fall back to regular read on failure
    // SAFETY: The file is read-only and we only hold the mapping briefly.
    // Claude Code appends to JSONL (never truncates), so the file won't shrink.
    match unsafe { memmap2::Mmap::map(&file) } {
        Ok(mmap) => Ok(mmap.to_vec()),
        Err(_) => std::fs::read(path),
    }
}

/// SIMD-accelerated line scanner that extracts only the fields NOT in sessions-index.json.
pub fn parse_bytes(data: &[u8]) -> ExtendedMetadata {
    let mut meta = ExtendedMetadata::default();
    let mut user_count = 0usize;
    let mut assistant_count = 0usize;
    let mut last_user_content: Option<String> = None;

    let user_finder = memmem::Finder::new(b"\"type\":\"user\"");
    let asst_finder = memmem::Finder::new(b"\"type\":\"assistant\"");

    // Tool name patterns for counting
    let read_finder = memmem::Finder::new(b"\"Read\"");
    let edit_finder = memmem::Finder::new(b"\"Edit\"");
    let write_finder = memmem::Finder::new(b"\"Write\"");
    let bash_finder = memmem::Finder::new(b"\"Bash\"");

    // File path pattern (from tool_use inputs)
    let file_path_finder = memmem::Finder::new(b"\"file_path\"");

    for line in split_lines_simd(data) {
        if line.is_empty() {
            continue;
        }

        if user_finder.find(line).is_some() {
            user_count += 1;
            // Extract content for last_message tracking
            if let Some(content) = extract_first_text_content(line) {
                last_user_content = Some(content);
            }
            // Check for skill invocations in user messages
            extract_skills_from_line(line, &mut meta.skills_used);
        } else if asst_finder.find(line).is_some() {
            assistant_count += 1;
            // Count tool usage
            if read_finder.find(line).is_some() {
                meta.tool_counts.read += count_occurrences(line, &read_finder);
            }
            if edit_finder.find(line).is_some() {
                meta.tool_counts.edit += count_occurrences(line, &edit_finder);
            }
            if write_finder.find(line).is_some() {
                meta.tool_counts.write += count_occurrences(line, &write_finder);
            }
            if bash_finder.find(line).is_some() {
                meta.tool_counts.bash += count_occurrences(line, &bash_finder);
            }
            // Extract file paths from tool_use inputs
            if file_path_finder.find(line).is_some() {
                extract_file_paths_from_line(line, &mut meta.files_touched);
            }
        }
    }

    meta.turn_count = user_count.min(assistant_count);
    meta.last_message = last_user_content
        .map(|c| truncate(&c, 200))
        .unwrap_or_default();

    // Deduplicate
    meta.skills_used.sort();
    meta.skills_used.dedup();
    meta.files_touched.sort();
    meta.files_touched.dedup();

    meta
}

/// Split data into lines using SIMD-accelerated newline search.
fn split_lines_simd(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut start = 0;
    let mut positions = memchr::memchr_iter(b'\n', data).chain(std::iter::once(data.len()));

    std::iter::from_fn(move || {
        if start > data.len() {
            return None;
        }
        positions.next().map(|end| {
            let line = &data[start..end];
            start = end + 1;
            line
        })
    })
}

/// Count occurrences of a pattern in a line.
fn count_occurrences(line: &[u8], finder: &memmem::Finder) -> usize {
    let mut count = 0;
    let mut start = 0;
    while start < line.len() {
        if let Some(pos) = finder.find(&line[start..]) {
            count += 1;
            start += pos + finder.needle().len();
        } else {
            break;
        }
    }
    count
}

/// Extract the first text content from a JSONL line (best-effort, no full JSON parse).
fn extract_first_text_content(line: &[u8]) -> Option<String> {
    // Look for "content":"..." pattern (simple string content)
    let text_finder = memmem::Finder::new(b"\"content\":\"");
    if let Some(pos) = text_finder.find(line) {
        let start = pos + b"\"content\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    // or "text":"..." in content blocks
    let text_finder2 = memmem::Finder::new(b"\"text\":\"");
    if let Some(pos) = text_finder2.find(line) {
        let start = pos + b"\"text\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    None
}

/// Extract a JSON-escaped string value starting from after the opening quote.
fn extract_quoted_string(data: &[u8]) -> Option<String> {
    let mut end = 0;
    let mut escaped = false;
    for &b in data {
        if escaped {
            escaped = false;
            end += 1;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            end += 1;
            continue;
        }
        if b == b'"' {
            break;
        }
        end += 1;
    }

    if end > 0 {
        String::from_utf8(data[..end].to_vec()).ok()
    } else {
        None
    }
}

/// Extract skill names from a user message line (looking for "skill":"..." patterns).
fn extract_skills_from_line(line: &[u8], skills: &mut Vec<String>) {
    let skill_name_finder = memmem::Finder::new(b"\"skill\":\"");
    let mut start = 0;
    while start < line.len() {
        if let Some(pos) = skill_name_finder.find(&line[start..]) {
            let begin = start + pos + b"\"skill\":\"".len();
            if let Some(name) = extract_quoted_string(&line[begin..]) {
                if !name.is_empty() {
                    skills.push(name);
                }
            }
            start = begin;
        } else {
            break;
        }
    }
}

/// Extract file_path values from tool_use inputs.
fn extract_file_paths_from_line(line: &[u8], paths: &mut Vec<String>) {
    let finder = memmem::Finder::new(b"\"file_path\":\"");
    let mut start = 0;
    while start < line.len() {
        if let Some(pos) = finder.find(&line[start..]) {
            let begin = start + pos + b"\"file_path\":\"".len();
            if let Some(path) = extract_quoted_string(&line[begin..]) {
                if !path.is_empty() {
                    paths.push(path);
                }
            }
            start = begin;
        } else {
            break;
        }
    }
}

/// Truncate a string to at most `max_len` characters (not bytes).
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect::<String>() + "..."
    }
}

/// Pass 1: Read sessions-index.json files and insert/update sessions in DB.
///
/// This is extremely fast (<10ms) because it reads pre-computed JSON indexes
/// that Claude Code maintains. No JSONL parsing is needed.
///
/// Returns `(num_projects, num_sessions)`.
pub async fn pass_1_read_indexes(
    claude_dir: &Path,
    db: &Database,
) -> Result<(usize, usize), String> {
    let all_indexes = read_all_session_indexes(claude_dir).map_err(|e| e.to_string())?;

    let mut total_projects = 0usize;
    let mut total_sessions = 0usize;

    for (project_encoded, entries) in &all_indexes {
        if entries.is_empty() {
            continue;
        }

        total_projects += 1;

        // Resolve the project path and display name from the encoded directory name
        let resolved = resolve_project_path(project_encoded);
        let project_display_name = &resolved.display_name;
        let project_path = &resolved.full_path;

        for entry in entries {
            // Parse modified ISO string to unix timestamp
            let modified_at = entry
                .modified
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            // Derive file_path from full_path or construct from session_id
            let file_path = entry.full_path.clone().unwrap_or_else(|| {
                claude_dir
                    .join("projects")
                    .join(project_encoded)
                    .join(format!("{}.jsonl", &entry.session_id))
                    .to_string_lossy()
                    .to_string()
            });

            // Stat the file for size_bytes if the path exists
            let size_bytes = std::fs::metadata(&file_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let preview = entry.first_prompt.as_deref().unwrap_or("");
            let summary = entry.summary.as_deref();
            let message_count = entry.message_count.unwrap_or(0) as i32;
            let git_branch = entry.git_branch.as_deref();
            let is_sidechain = entry.is_sidechain.unwrap_or(false);

            // Use project_path from the entry if available, else from resolved path
            let entry_project_path = entry.project_path.as_deref().unwrap_or(project_path);

            db.insert_session_from_index(
                &entry.session_id,
                project_encoded,
                project_display_name,
                entry_project_path,
                &file_path,
                preview,
                summary,
                message_count,
                modified_at,
                git_branch,
                is_sidechain,
                size_bytes,
            )
            .await
            .map_err(|e| format!("Failed to insert session {}: {}", entry.session_id, e))?;

            total_sessions += 1;
        }
    }

    Ok((total_projects, total_sessions))
}

/// Pass 2: Parallel deep JSONL parsing for extended metadata.
///
/// Processes sessions where `deep_indexed_at IS NULL`. For each session,
/// reads the JSONL file with memory-mapped I/O, extracts tool counts,
/// skills, files touched, last message, and turn count using SIMD-accelerated
/// scanning, then updates the database.
///
/// Calls `on_file_done(indexed_so_far, total)` after each file completes.
pub async fn pass_2_deep_index<F>(
    db: &Database,
    on_file_done: F,
) -> Result<usize, String>
where
    F: Fn(usize, usize) + Send + Sync + 'static,
{
    let sessions = db
        .get_sessions_needing_deep_index()
        .await
        .map_err(|e| format!("Failed to query sessions needing deep index: {}", e))?;

    if sessions.is_empty() {
        return Ok(0);
    }

    let total = sessions.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let on_file_done = Arc::new(on_file_done);

    // Limit parallelism to available CPU cores
    let parallelism = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(parallelism));

    let mut handles = Vec::with_capacity(total);

    for (id, file_path) in sessions {
        let db = db.clone();
        let sem = semaphore.clone();
        let counter = counter.clone();
        let on_done = on_file_done.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("Semaphore error: {}", e))?;

            let path = std::path::PathBuf::from(&file_path);

            // Read and parse in a blocking thread
            let meta = tokio::task::spawn_blocking(move || match read_file_fast(&path) {
                Ok(data) => parse_bytes(&data),
                Err(_) => ExtendedMetadata::default(),
            })
            .await
            .map_err(|e| format!("spawn_blocking join error: {}", e))?;

            // Serialize vec fields to JSON strings
            let files_touched =
                serde_json::to_string(&meta.files_touched).unwrap_or_else(|_| "[]".to_string());
            let skills_used =
                serde_json::to_string(&meta.skills_used).unwrap_or_else(|_| "[]".to_string());

            db.update_session_deep_fields(
                &id,
                &meta.last_message,
                meta.turn_count as i32,
                meta.tool_counts.edit as i32,
                meta.tool_counts.read as i32,
                meta.tool_counts.bash as i32,
                meta.tool_counts.write as i32,
                &files_touched,
                &skills_used,
            )
            .await
            .map_err(|e| format!("Failed to update deep fields for {}: {}", id, e))?;

            let indexed = counter.fetch_add(1, Ordering::Relaxed) + 1;
            on_done(indexed, total);

            Ok::<(), String>(())
        });

        handles.push(handle);
    }

    // Await all tasks
    let mut errors = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => errors.push(e),
            Err(e) => errors.push(format!("Task join error: {}", e)),
        }
    }

    if !errors.is_empty() {
        tracing::warn!(
            "pass_2_deep_index encountered {} errors: {:?}",
            errors.len(),
            errors
        );
    }

    Ok(counter.load(Ordering::Relaxed))
}

/// Full background indexing orchestrator: Pass 1 (index JSON) then Pass 2 (deep JSONL).
///
/// This is the main entry point for background indexing. It runs both passes
/// sequentially and reports progress via callbacks.
pub async fn run_background_index<F>(
    claude_dir: &Path,
    db: &Database,
    on_pass1_done: impl FnOnce(usize, usize),
    on_file_done: F,
    on_complete: impl FnOnce(usize),
) -> Result<(), String>
where
    F: Fn(usize, usize) + Send + Sync + 'static,
{
    // Pass 1: fast index JSON reads
    let (projects, sessions) = pass_1_read_indexes(claude_dir, db).await?;
    on_pass1_done(projects, sessions);

    // Pass 2: parallel deep JSONL parsing
    let indexed = pass_2_deep_index(db, on_file_done).await?;
    on_complete(indexed);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_parse_bytes_empty() {
        let meta = parse_bytes(b"");
        assert_eq!(meta.turn_count, 0);
        assert!(meta.last_message.is_empty());
        assert!(meta.tool_counts.is_empty());
    }

    #[test]
    fn test_parse_bytes_counts_tools() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Edit"}]}}
{"type":"user","message":{"content":"thanks"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}
"#;
        let meta = parse_bytes(data);
        assert_eq!(meta.turn_count, 2);
        assert_eq!(meta.tool_counts.read, 1);
        assert_eq!(meta.tool_counts.edit, 1);
        assert_eq!(meta.tool_counts.bash, 1);
        assert_eq!(meta.tool_counts.write, 0);
    }

    #[test]
    fn test_parse_bytes_last_message() {
        let data = br#"{"type":"user","message":{"content":"first question"}}
{"type":"assistant","message":{"content":"answer 1"}}
{"type":"user","message":{"content":"second question"}}
{"type":"assistant","message":{"content":"answer 2"}}
"#;
        let meta = parse_bytes(data);
        assert_eq!(meta.last_message, "second question");
    }

    #[test]
    fn test_read_file_fast_nonexistent() {
        let result = read_file_fast(std::path::Path::new("/nonexistent/file.jsonl"));
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_fast_small_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello world").unwrap();
        let data = read_file_fast(tmp.path()).unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    #[test]
    fn test_split_lines_simd() {
        let data = b"line1\nline2\nline3";
        let lines: Vec<&[u8]> = split_lines_simd(data).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], b"line1");
        assert_eq!(lines[1], b"line2");
        assert_eq!(lines[2], b"line3");
    }

    #[test]
    fn test_extract_file_paths() {
        let line = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}"#;
        let mut paths = Vec::new();
        extract_file_paths_from_line(line, &mut paths);
        assert!(paths.contains(&"/src/main.rs".to_string()));
        assert!(paths.contains(&"/src/lib.rs".to_string()));
    }

    // ============================================================================
    // Pass 1 / Pass 2 / run_background_index Integration Tests
    // ============================================================================

    /// Helper: create a temp claude dir with sessions-index.json and JSONL files.
    fn setup_test_claude_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();
        let project_dir = claude_dir.join("projects").join("test-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Create a JSONL file
        let jsonl_path = project_dir.join("sess-001.jsonl");
        let jsonl_content = br#"{"type":"user","message":{"content":"hello world"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}}]}}
{"type":"user","message":{"content":"now edit it"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}
"#;
        std::fs::write(&jsonl_path, jsonl_content).unwrap();

        // Create sessions-index.json
        let index = format!(
            r#"[
            {{
                "sessionId": "sess-001",
                "fullPath": "{}",
                "firstPrompt": "hello world",
                "summary": "Test session about editing",
                "messageCount": 4,
                "modified": "2026-01-25T17:18:30.718Z",
                "gitBranch": "main",
                "isSidechain": false
            }}
        ]"#,
            jsonl_path.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(project_dir.join("sessions-index.json"), index).unwrap();

        (tmp, claude_dir)
    }

    #[tokio::test]
    async fn test_pass_1_reads_and_inserts() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        assert_eq!(projects, 1, "Should find 1 project");
        assert_eq!(sessions, 1, "Should find 1 session");

        // Verify the session is in the DB
        let db_projects = db.list_projects().await.unwrap();
        assert_eq!(db_projects.len(), 1);
        assert_eq!(db_projects[0].sessions.len(), 1);
        assert_eq!(db_projects[0].sessions[0].id, "sess-001");
        assert_eq!(db_projects[0].sessions[0].preview, "hello world");
        assert_eq!(
            db_projects[0].sessions[0].summary.as_deref(),
            Some("Test session about editing")
        );
    }

    #[tokio::test]
    async fn test_pass_1_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();
        std::fs::create_dir_all(claude_dir.join("projects")).unwrap();

        let db = Database::new_in_memory().await.unwrap();
        let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        assert_eq!(projects, 0);
        assert_eq!(sessions, 0);
    }

    #[tokio::test]
    async fn test_pass_2_fills_deep_fields() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        // Run Pass 1 first to populate sessions
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        // Verify deep_indexed is false before Pass 2
        let projects = db.list_projects().await.unwrap();
        assert!(
            !projects[0].sessions[0].deep_indexed,
            "Should not be deep indexed yet"
        );

        // Run Pass 2
        let progress = Arc::new(AtomicUsize::new(0));
        let progress_clone = progress.clone();
        let indexed = pass_2_deep_index(&db, move |done, _total| {
            progress_clone.store(done, Ordering::Relaxed);
        })
        .await
        .unwrap();

        assert_eq!(indexed, 1, "Should deep-index 1 session");
        assert_eq!(
            progress.load(Ordering::Relaxed),
            1,
            "Progress should report 1"
        );

        // Verify deep fields are populated
        let projects = db.list_projects().await.unwrap();
        let session = &projects[0].sessions[0];
        assert!(session.deep_indexed, "Should be deep indexed now");
        assert_eq!(session.turn_count, 2, "Should have 2 turns");
        assert_eq!(session.tool_counts.read, 1);
        assert_eq!(session.tool_counts.edit, 1);
        assert_eq!(session.last_message, "now edit it");
    }

    #[tokio::test]
    async fn test_pass_2_skips_already_indexed() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        // Run Pass 1 then Pass 2
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        let first_run = pass_2_deep_index(&db, |_, _| {}).await.unwrap();
        assert_eq!(first_run, 1);

        // Run Pass 2 again — should skip because deep_indexed_at is set
        let second_run = pass_2_deep_index(&db, |_, _| {}).await.unwrap();
        assert_eq!(second_run, 0, "Should skip already deep-indexed sessions");
    }

    #[tokio::test]
    async fn test_run_background_index_full_pipeline() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        let pass1_result = Arc::new(std::sync::Mutex::new((0usize, 0usize)));
        let complete_result = Arc::new(AtomicUsize::new(0));

        let p1 = pass1_result.clone();
        let cr = complete_result.clone();

        run_background_index(
            &claude_dir,
            &db,
            move |projects, sessions| {
                *p1.lock().unwrap() = (projects, sessions);
            },
            |_done, _total| {},
            move |total| {
                cr.store(total, Ordering::Relaxed);
            },
        )
        .await
        .unwrap();

        let (projects, sessions) = *pass1_result.lock().unwrap();
        assert_eq!(projects, 1);
        assert_eq!(sessions, 1);
        assert_eq!(complete_result.load(Ordering::Relaxed), 1);

        // Verify full pipeline result
        let db_projects = db.list_projects().await.unwrap();
        assert_eq!(db_projects.len(), 1);
        let session = &db_projects[0].sessions[0];
        assert!(session.deep_indexed);
        assert_eq!(session.turn_count, 2);
        assert_eq!(session.tool_counts.read, 1);
        assert_eq!(session.tool_counts.edit, 1);
    }
}
