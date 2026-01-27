// crates/db/src/indexer_parallel.rs
// Fast JSONL parsing with memory-mapped I/O and SIMD-accelerated scanning.
// Also contains the two-pass indexing pipeline: Pass 1 (index JSON) and Pass 2 (deep JSONL).

use memchr::memmem;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use vibe_recall_core::{
    read_all_session_indexes, resolve_project_path, ClassifyResult, Registry, ToolCounts,
};

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

/// A raw tool_use extracted from JSONL, before classification.
#[derive(Debug, Clone)]
pub struct RawInvocation {
    pub name: String,
    pub input: Option<serde_json::Value>,
    pub byte_offset: usize,
    pub timestamp: i64,
}

/// Result of parse_bytes(): deep metadata plus raw tool invocations.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub deep: ExtendedMetadata,
    pub raw_invocations: Vec<RawInvocation>,
    pub turns: Vec<vibe_recall_core::RawTurn>,
    pub models_seen: Vec<String>,
}

/// Read a file using memory-mapped I/O with fallback to regular read.
/// mmap is faster for large files (>64KB) because it avoids copying data through kernel buffers.
///
/// NOTE: This returns `Vec<u8>` (a heap copy). For zero-copy parsing, see
/// `pass_2_deep_index` which mmaps and parses inline without copying.
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
///
/// Returns a `ParseResult` containing both the deep metadata (tool counts, skills, etc.)
/// and raw tool_use invocations for downstream classification.
pub fn parse_bytes(data: &[u8]) -> ParseResult {
    let mut result = ParseResult::default();
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

    // SIMD pre-filter for tool_use blocks (only parse JSON for lines containing this)
    let tool_use_finder = memmem::Finder::new(b"\"tool_use\"");

    // Hoisted finders for helper functions (avoid rebuilding SIMD tables per line)
    let content_finder = memmem::Finder::new(b"\"content\":\"");
    let text_finder = memmem::Finder::new(b"\"text\":\"");
    let skill_name_finder = memmem::Finder::new(b"\"skill\":\"");
    let file_path_value_finder = memmem::Finder::new(b"\"file_path\":\"");

    for (byte_offset, line) in split_lines_with_offsets(data) {
        if line.is_empty() {
            continue;
        }

        if user_finder.find(line).is_some() {
            user_count += 1;
            // Extract content for last_message tracking
            if let Some(content) =
                extract_first_text_content(line, &content_finder, &text_finder)
            {
                last_user_content = Some(content);
            }
            // Check for skill invocations in user messages
            extract_skills_from_line(line, &skill_name_finder, &mut result.deep.skills_used);
        } else if asst_finder.find(line).is_some() {
            assistant_count += 1;
            // Count tool usage
            if read_finder.find(line).is_some() {
                result.deep.tool_counts.read += count_occurrences(line, &read_finder);
            }
            if edit_finder.find(line).is_some() {
                result.deep.tool_counts.edit += count_occurrences(line, &edit_finder);
            }
            if write_finder.find(line).is_some() {
                result.deep.tool_counts.write += count_occurrences(line, &write_finder);
            }
            if bash_finder.find(line).is_some() {
                result.deep.tool_counts.bash += count_occurrences(line, &bash_finder);
            }
            // Extract file paths from tool_use inputs
            if file_path_finder.find(line).is_some() {
                extract_file_paths_from_line(
                    line,
                    &file_path_value_finder,
                    &mut result.deep.files_touched,
                );
            }
            // Extract raw tool_use invocations (SIMD pre-filter: only parse JSON if "tool_use" present)
            if tool_use_finder.find(line).is_some() {
                extract_raw_invocations(line, byte_offset, &mut result.raw_invocations);
            }
        }
    }

    result.deep.turn_count = user_count.min(assistant_count);
    result.deep.last_message = last_user_content
        .map(|c| truncate(&c, 200))
        .unwrap_or_default();

    // Deduplicate
    result.deep.skills_used.sort();
    result.deep.skills_used.dedup();
    result.deep.files_touched.sort();
    result.deep.files_touched.dedup();

    result
}

/// Split data into lines using SIMD-accelerated newline search.
#[cfg(test)]
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

/// Split data into lines with byte offsets using SIMD-accelerated newline search.
/// Returns `(byte_offset, line_slice)` for each line.
fn split_lines_with_offsets(data: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    let mut start = 0;
    let mut positions = memchr::memchr_iter(b'\n', data).chain(std::iter::once(data.len()));

    std::iter::from_fn(move || {
        if start > data.len() {
            return None;
        }
        positions.next().map(|end| {
            let offset = start;
            let line = &data[start..end];
            start = end + 1;
            (offset, line)
        })
    })
}

/// Extract raw tool_use invocations from a JSONL line by parsing it as JSON.
///
/// Only called on lines that already passed the SIMD `"tool_use"` pre-filter.
/// Extracts all `{"type": "tool_use", "name": ..., "input": ...}` blocks from
/// the assistant message's content array.
fn extract_raw_invocations(line: &[u8], byte_offset: usize, out: &mut Vec<RawInvocation>) {
    // Parse the line as JSON
    let value: serde_json::Value = match serde_json::from_slice(line) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Extract timestamp from the top-level object (if present)
    let timestamp = value
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Navigate to the content array: could be at .message.content or .content
    let content = value
        .get("message")
        .and_then(|m| m.get("content"))
        .or_else(|| value.get("content"));

    let content_arr = match content.and_then(|c| c.as_array()) {
        Some(arr) => arr,
        None => return,
    };

    for block in content_arr {
        if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
            continue;
        }
        let name = match block.get("name").and_then(|n| n.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let input = block.get("input").cloned();

        out.push(RawInvocation {
            name,
            input,
            byte_offset,
            timestamp,
        });
    }
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
fn extract_first_text_content(
    line: &[u8],
    content_finder: &memmem::Finder,
    text_finder: &memmem::Finder,
) -> Option<String> {
    // Look for "content":"..." pattern (simple string content)
    if let Some(pos) = content_finder.find(line) {
        let start = pos + b"\"content\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    // or "text":"..." in content blocks
    if let Some(pos) = text_finder.find(line) {
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
fn extract_skills_from_line(
    line: &[u8],
    skill_name_finder: &memmem::Finder,
    skills: &mut Vec<String>,
) {
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
fn extract_file_paths_from_line(line: &[u8], finder: &memmem::Finder, paths: &mut Vec<String>) {
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
/// Uses zero-copy mmap: the file is memory-mapped and `parse_bytes` runs
/// directly on the mapped pages. The mmap stays alive for the duration of
/// parsing, then drops — no heap copy is ever made.
///
/// If `registry` is `Some`, raw tool_use invocations are classified and
/// batch-inserted into the `invocations` table.
///
/// Calls `on_file_done(indexed_so_far, total)` after each file completes.
pub async fn pass_2_deep_index<F>(
    db: &Database,
    registry: Option<&Registry>,
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

    // Clone registry into an Arc so we can share across spawned tasks.
    let registry: Option<Arc<Registry>> = registry.map(|r| Arc::new(r.clone()));

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
        let registry = registry.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("Semaphore error: {}", e))?;

            let path = std::path::PathBuf::from(&file_path);

            // Zero-copy mmap + parse in a blocking thread.
            // The mmap stays alive while parse_bytes runs on the mapped pages,
            // then drops after parsing — no heap allocation for the file content.
            let parse_result = tokio::task::spawn_blocking(move || {
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => return ParseResult::default(),
                };
                let metadata = match file.metadata() {
                    Ok(m) => m,
                    Err(_) => return ParseResult::default(),
                };
                let len = metadata.len() as usize;
                if len == 0 {
                    return ParseResult::default();
                }
                // Small files: regular read (mmap overhead not worth it)
                if len < 64 * 1024 {
                    match std::fs::read(&path) {
                        Ok(data) => return parse_bytes(&data),
                        Err(_) => return ParseResult::default(),
                    }
                }
                // Large files: zero-copy mmap — parse directly from mapped pages
                // SAFETY: Read-only mapping. Claude Code appends to JSONL (never truncates).
                match unsafe { memmap2::Mmap::map(&file) } {
                    Ok(mmap) => parse_bytes(&mmap), // zero-copy! mmap drops after parse
                    Err(_) => match std::fs::read(&path) {
                        Ok(data) => parse_bytes(&data),
                        Err(_) => ParseResult::default(),
                    },
                }
            })
            .await
            .map_err(|e| format!("spawn_blocking join error: {}", e))?;

            let meta = &parse_result.deep;

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

            // Classify raw invocations and batch-insert if registry is available
            if let Some(ref registry) = registry {
                let classified: Vec<(String, i64, String, String, String, i64)> = parse_result
                    .raw_invocations
                    .iter()
                    .filter_map(|raw| {
                        let result = vibe_recall_core::classify_tool_use(
                            &raw.name,
                            &raw.input,
                            registry,
                        );
                        match result {
                            ClassifyResult::Valid { invocable_id, .. } => Some((
                                file_path.clone(),
                                raw.byte_offset as i64,
                                invocable_id,
                                id.clone(),
                                String::new(), // project filled by caller if needed
                                raw.timestamp,
                            )),
                            _ => None, // Rejected and Ignored are discarded
                        }
                    })
                    .collect();

                if !classified.is_empty() {
                    db.batch_insert_invocations(&classified)
                        .await
                        .map_err(|e| {
                            format!("Failed to insert invocations for {}: {}", id, e)
                        })?;
                }
            }

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

/// Type alias for the shared registry holder used by the server.
pub type RegistryHolder = Arc<RwLock<Option<Registry>>>;

/// Full background indexing orchestrator: Pass 1 (index JSON) + Registry build
/// in parallel, then Pass 2 (deep JSONL) with registry available.
///
/// This is the main entry point for background indexing. It runs Pass 1 and
/// registry construction concurrently via `tokio::join!`, then Pass 2
/// sequentially with the registry available for invocation classification.
///
/// If `registry_holder` is provided, the built registry is stored in it so
/// API routes can access it after indexing completes.
pub async fn run_background_index<F>(
    claude_dir: &Path,
    db: &Database,
    registry_holder: Option<RegistryHolder>,
    on_pass1_done: impl FnOnce(usize, usize),
    on_file_done: F,
    on_complete: impl FnOnce(usize),
) -> Result<(), String>
where
    F: Fn(usize, usize) + Send + Sync + 'static,
{
    // Pass 1 and Registry build are independent — run in parallel.
    let claude_dir_owned = claude_dir.to_path_buf();
    let (pass1_result, registry) = tokio::join!(
        pass_1_read_indexes(claude_dir, db),
        vibe_recall_core::build_registry(&claude_dir_owned),
    );

    let (projects, sessions) = pass1_result?;
    on_pass1_done(projects, sessions);

    // Seed invocables into the DB so invocations can reference them (FK constraint).
    let invocable_tuples: Vec<(String, Option<String>, String, String, String)> = registry
        .all_invocables()
        .map(|info| {
            (
                info.id.clone(),
                info.plugin_name.clone(),
                info.name.clone(),
                info.kind.to_string(),
                info.description.clone(),
            )
        })
        .collect();
    if !invocable_tuples.is_empty() {
        db.batch_upsert_invocables(&invocable_tuples)
            .await
            .map_err(|e| format!("Failed to seed invocables: {}", e))?;
    }

    // Pass 2: use the registry for invocation classification
    let indexed = pass_2_deep_index(db, Some(&registry), on_file_done).await?;

    // Store registry in shared holder for API routes to use
    if let Some(holder) = registry_holder {
        if let Ok(mut guard) = holder.write() {
            *guard = Some(registry);
        }
    }

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
        let result = parse_bytes(b"");
        assert_eq!(result.deep.turn_count, 0);
        assert!(result.deep.last_message.is_empty());
        assert!(result.deep.tool_counts.is_empty());
        assert!(result.raw_invocations.is_empty());
    }

    #[test]
    fn test_parse_bytes_counts_tools() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Edit"}]}}
{"type":"user","message":{"content":"thanks"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.turn_count, 2);
        assert_eq!(result.deep.tool_counts.read, 1);
        assert_eq!(result.deep.tool_counts.edit, 1);
        assert_eq!(result.deep.tool_counts.bash, 1);
        assert_eq!(result.deep.tool_counts.write, 0);
    }

    #[test]
    fn test_parse_bytes_last_message() {
        let data = br#"{"type":"user","message":{"content":"first question"}}
{"type":"assistant","message":{"content":"answer 1"}}
{"type":"user","message":{"content":"second question"}}
{"type":"assistant","message":{"content":"answer 2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.last_message, "second question");
    }

    #[test]
    fn test_parse_bytes_extracts_raw_invocations() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","timestamp":1706200000,"message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}
{"type":"user","message":{"content":"run tests"}}
{"type":"assistant","timestamp":1706200100,"message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}},{"type":"text","text":"Done!"}]}}
"#;
        let result = parse_bytes(data);

        // Should extract 3 raw invocations: Read, Edit, Bash
        assert_eq!(
            result.raw_invocations.len(),
            3,
            "Should extract 3 tool_use invocations"
        );

        // First line (user) is at offset 0, second line (assistant with Read+Edit) starts after first newline
        assert_eq!(result.raw_invocations[0].name, "Read");
        assert_eq!(
            result.raw_invocations[0]
                .input
                .as_ref()
                .and_then(|v| v.get("file_path"))
                .and_then(|v| v.as_str()),
            Some("/src/main.rs")
        );
        assert_eq!(result.raw_invocations[0].timestamp, 1706200000);

        assert_eq!(result.raw_invocations[1].name, "Edit");
        assert_eq!(result.raw_invocations[1].timestamp, 1706200000);

        assert_eq!(result.raw_invocations[2].name, "Bash");
        assert_eq!(
            result.raw_invocations[2]
                .input
                .as_ref()
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str()),
            Some("cargo test")
        );
        assert_eq!(result.raw_invocations[2].timestamp, 1706200100);

        // Byte offsets: first two invocations share the same line offset,
        // third invocation is on a different line
        assert_eq!(
            result.raw_invocations[0].byte_offset,
            result.raw_invocations[1].byte_offset,
            "Read and Edit are on the same JSONL line"
        );
        assert_ne!(
            result.raw_invocations[0].byte_offset,
            result.raw_invocations[2].byte_offset,
            "Bash is on a different JSONL line"
        );
    }

    #[test]
    fn test_parse_bytes_no_invocations_without_tool_use() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":"Just text, no tools."}}
"#;
        let result = parse_bytes(data);
        assert!(
            result.raw_invocations.is_empty(),
            "No tool_use blocks, no invocations"
        );
    }

    #[test]
    fn test_parse_bytes_timestamp_defaults_to_zero() {
        // No timestamp field on the assistant message
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/foo"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.raw_invocations.len(), 1);
        assert_eq!(
            result.raw_invocations[0].timestamp, 0,
            "Missing timestamp should default to 0"
        );
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
        let finder = memmem::Finder::new(b"\"file_path\":\"");
        let mut paths = Vec::new();
        extract_file_paths_from_line(line, &finder, &mut paths);
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

        // Run Pass 2 (no registry)
        let progress = Arc::new(AtomicUsize::new(0));
        let progress_clone = progress.clone();
        let indexed = pass_2_deep_index(&db, None, move |done, _total| {
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
        let first_run = pass_2_deep_index(&db, None, |_, _| {}).await.unwrap();
        assert_eq!(first_run, 1);

        // Run Pass 2 again — should skip because deep_indexed_at is set
        let second_run = pass_2_deep_index(&db, None, |_, _| {}).await.unwrap();
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
            None, // no registry holder
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
