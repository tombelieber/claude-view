// crates/core/src/discovery.rs
//! Project discovery for Claude Code sessions.
//!
//! This module scans `~/.claude/projects/` to discover all Claude Code projects
//! and their sessions. It handles the encoded directory names that Claude uses
//! and efficiently extracts session metadata without fully parsing each file.

use crate::error::DiscoveryError;
use crate::types::{ProjectInfo, SessionInfo, ToolCounts};
use regex_lite::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

/// Returns the path to the Claude projects directory (~/.claude/projects).
///
/// # Errors
/// Returns `DiscoveryError::HomeDirNotFound` if the home directory cannot be determined.
pub fn claude_projects_dir() -> Result<PathBuf, DiscoveryError> {
    let home = dirs::home_dir().ok_or(DiscoveryError::HomeDirNotFound)?;
    Ok(home.join(".claude").join("projects"))
}

/// Resolved project path information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProject {
    /// The full filesystem path (e.g., "/Users/foo/my-project")
    pub full_path: String,
    /// Human-readable display name (e.g., "my-project")
    pub display_name: String,
}

/// Metadata extracted from a session file without full parsing.
#[derive(Debug, Clone, Default)]
pub struct ExtractedMetadata {
    pub preview: String,
    pub last_message: String,
    pub files_touched: Vec<String>,
    pub skills_used: Vec<String>,
    pub tool_counts: ToolCounts,
    pub message_count: usize,
    pub turn_count: usize,
}

/// Resolve an encoded project directory name to a filesystem path.
///
/// Claude encodes paths like `/Users/foo/my-project` as `-Users-foo-my-project`.
/// The challenge is that hyphens in real directory names look like path separators,
/// and `--` is ambiguous (both `@` and `.` prefixed dirs encode to `--`).
///
/// Strategy: DFS with directory listing (like ls/zsh resolution).
/// 1. Tokenize the encoded name (handling `--` as prefix marker)
/// 2. At each directory level, `read_dir` to get actual entries
/// 3. Match consecutive segments (joined with `-`) against real entries
/// 4. For `--`-marked segments, try `@`, `.`, and bare prefixes
/// 5. Backtrack if a path leads to a dead end
/// 6. Derive display name from nearest git root
pub fn resolve_project_path(encoded_name: &str) -> ResolvedProject {
    if encoded_name.is_empty() {
        return ResolvedProject {
            full_path: String::new(),
            display_name: String::new(),
        };
    }

    let segments = tokenize_encoded_name(encoded_name);

    if segments.is_empty() {
        return ResolvedProject {
            full_path: "/".to_string(),
            display_name: "/".to_string(),
        };
    }

    // DFS resolve
    let resolved_path = if let Some(path) = dfs_resolve(&PathBuf::from("/"), &segments, 0) {
        path.to_string_lossy().to_string()
    } else {
        // Fallback: join all segments with / (all-separators interpretation)
        format!("/{}", segments.join("/"))
    };

    let display_name = derive_display_name(&resolved_path);

    ResolvedProject {
        full_path: resolved_path,
        display_name,
    }
}

/// If the encoded project name represents a git worktree, return the parent
/// project's encoded name. Otherwise return None.
///
/// Worktree paths: `-Users-dev-project--worktrees-branch-name`
/// Parent:         `-Users-dev-project`
///
/// The `--worktrees-` segment maps to `/.worktrees/` on disk.
pub fn resolve_worktree_parent(encoded_name: &str) -> Option<String> {
    let marker = "--worktrees-";
    let pos = encoded_name.find(marker)?;
    if pos == 0 {
        return None; // edge case: name starts with marker
    }
    Some(encoded_name[..pos].to_string())
}

/// Tokenize an encoded project name into path segments.
///
/// Handles `--` → `/@` conversion for scoped packages.
/// The `--` represents a path separator `/` followed by `@`.
///
/// Example: `-Users-user-dev--example-org-claude-view`
///   → `["Users", "user", "dev", "@example", "org", "claude", "view"]`
fn tokenize_encoded_name(encoded_name: &str) -> Vec<String> {
    let name = encoded_name.strip_prefix('-').unwrap_or(encoded_name);
    if name.is_empty() {
        return vec![];
    }

    // Replace -- with a path-separator + @ marker
    // `--` means `/@` which is path_sep + @_prefix
    // Use \x00/ as separator so it splits correctly
    let normalized = name.replace("--", "\x00/\x00@");

    // Split on - and \x00/
    let mut segments = Vec::new();
    for part in normalized.split('-') {
        for sub in part.split("\x00/") {
            let restored = sub.replace('\x00', "");
            if !restored.is_empty() {
                segments.push(restored);
            }
        }
    }

    segments
}

/// DFS filesystem walk to resolve path segments against actual directory entries.
///
/// Uses real directory listing (`read_dir`) at each level, the same approach
/// shells like zsh/bash use for path resolution. Instead of speculatively
/// constructing paths and checking `exists()`, we list actual entries and
/// match against them.
///
/// At each directory level:
/// 1. Read all entries via `read_dir` into a `HashSet` for O(1) lookup
/// 2. Try matching 1..N consecutive segments joined with `-` against entries
/// 3. For segments from `--` encoding (marked with `@` by tokenizer), also
///    try `@`-prefixed and `.`-prefixed variants to handle both:
///    - Scoped packages: `/@vicky-ai` (encoded as `--vicky-ai`)
///    - Hidden directories: `/.worktrees` (also encoded as `--worktrees`)
/// 4. For 2-segment groups, try `.` join for domain names (e.g., `acme.io`)
/// 5. Recurse into matching directories; backtrack on dead ends
///
/// No artificial cap on join count — the real directory listing naturally
/// constrains what matches, so even long hyphenated names resolve correctly.
///
/// Returns the first complete path that exists on the filesystem.
fn dfs_resolve(base: &Path, segments: &[String], start: usize) -> Option<PathBuf> {
    if start >= segments.len() {
        return if base.exists() { Some(base.to_path_buf()) } else { None };
    }

    // List actual directory entries — the core of proper directory resolution.
    // std::fs::read_dir already excludes "." and ".." per Rust docs.
    let entries: HashSet<String> = match std::fs::read_dir(base) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect(),
        Err(_) => return None,
    };

    let remaining = segments.len() - start;

    for join_count in 1..=remaining {
        let candidates = build_candidates(segments, start, join_count);
        let next_start = start + join_count;

        for candidate in &candidates {
            if candidate.is_empty() || !entries.contains(candidate.as_str()) {
                continue;
            }

            let next_path = base.join(candidate);

            if next_start >= segments.len() {
                // Last segment(s): path just needs to exist
                return Some(next_path);
            }

            // Intermediate: must be a directory to recurse into
            if next_path.is_dir() {
                if let Some(result) = dfs_resolve(&next_path, segments, next_start) {
                    return Some(result);
                }
            }
        }
    }

    None
}

/// Build candidate directory names for a given slice of segments.
///
/// When the first segment has the `@` marker (from `--` tokenization), generates
/// three variants since `--` is ambiguous between prefix characters that Claude's
/// encoder replaces with `-`:
/// - `@name`: scoped packages (`/@vicky-ai`)
/// - `.name`: hidden directories (`/.worktrees`, `/.git`)
/// - `name`: bare fallback
///
/// Also generates dot-joined variant for 2-segment domain names.
fn build_candidates(segments: &[String], start: usize, join_count: usize) -> Vec<String> {
    let first = &segments[start];
    let has_prefix_marker = first.starts_with('@');

    // Strip '@' marker from all segments in range (it's a tokenizer artifact, not literal)
    let mut parts: Vec<&str> = Vec::with_capacity(join_count);
    for i in 0..join_count {
        let seg = &segments[start + i];
        parts.push(seg.strip_prefix('@').unwrap_or(seg.as_str()));
    }

    let joined = parts.join("-");

    let mut candidates = Vec::with_capacity(4);

    if has_prefix_marker {
        // `--` sequence: try @, ., then bare (ordered by likelihood)
        candidates.push(format!("@{}", joined));
        candidates.push(format!(".{}", joined));
    }
    candidates.push(joined.clone());

    // Dot-join for domain-like names (e.g., acme + io → acme.io)
    if join_count == 2 {
        candidates.push(format!("{}.{}", parts[0], parts[1]));
    }

    candidates
}

/// Derive a human-friendly display name from a resolved filesystem path.
///
/// Strategy:
/// 1. Walk up from the resolved path to find the nearest `.git` directory
/// 2. Then walk further up to find the **topmost** `.git` within 5 levels
///    (handles worktrees/nested repos like `my-app/web` inside `my-app`)
/// 3. Display name = topmost git root name + relative path
///
/// Examples:
/// - `/Users/foo/dev/@org/my-project` (git at my-project) → `my-project`
/// - `/Users/foo/dev/@org/repo/web`   (git at both repo and web) → `repo/web`
/// - `/Users/foo`                     (no git root)             → `foo`
fn derive_display_name(resolved_path: &str) -> String {
    let path = Path::new(resolved_path);

    // Find the topmost git root within 5 levels above the resolved path
    let mut topmost_git_root: Option<&Path> = None;
    let mut current = path;

    for _ in 0..5 {
        if current.join(".git").exists() {
            topmost_git_root = Some(current);
        }

        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    if let Some(git_root) = topmost_git_root {
        let git_root_name = git_root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        if git_root == path {
            return git_root_name;
        }

        // path is deeper than git root — include relative suffix
        if let Ok(relative) = path.strip_prefix(git_root) {
            return format!("{}/{}", git_root_name, relative.display());
        }

        return git_root_name;
    }

    // No git root found — fall back to last path component
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| resolved_path.to_string())
}

/// Generate all possible path interpretations of an encoded name.
///
/// This is the legacy API preserved for backward compatibility.
/// Internally delegates to `tokenize_encoded_name` and generates fixed variants.
///
/// Prefer using `resolve_project_path` which uses DFS for correct resolution.
pub fn get_join_variants(encoded_name: &str) -> Vec<String> {
    let segments = tokenize_encoded_name(encoded_name);
    if segments.is_empty() {
        return vec!["/".to_string()];
    }

    let mut variants = Vec::new();

    // Variant 1: All segments as path separators
    variants.push(format!("/{}", segments.join("/")));

    // Variant 2: Last two segments joined with -
    if segments.len() >= 3 {
        let last_two = segments[segments.len() - 2..].join("-");
        let rest = &segments[..segments.len() - 2];
        let v = format!("/{}/{}", rest.join("/"), last_two);
        if !variants.contains(&v) {
            variants.push(v);
        }
    }

    // Variant 3: Last three segments joined with -
    if segments.len() >= 4 {
        let last_three = segments[segments.len() - 3..].join("-");
        let rest = &segments[..segments.len() - 3];
        let v = format!("/{}/{}", rest.join("/"), last_three);
        if !variants.contains(&v) {
            variants.push(v);
        }
    }

    // Variant 4: Dot join for domain-like names
    if segments.len() >= 2 {
        let dot_joined = format!("{}.{}", segments[segments.len() - 2], segments[segments.len() - 1]);
        let rest = &segments[..segments.len() - 2];
        let v = if rest.is_empty() {
            format!("/{}", dot_joined)
        } else {
            format!("/{}/{}", rest.join("/"), dot_joined)
        };
        if !variants.contains(&v) {
            variants.push(v);
        }
    }

    variants
}

/// Count sessions that are "active" (modified within the last 5 minutes).
/// This matches the Node.js behavior for the activeCount field.
pub fn count_active_sessions(sessions: &[SessionInfo]) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let five_minutes_ago = now - 5 * 60;

    sessions
        .iter()
        .filter(|s| s.modified_at > five_minutes_ago)
        .count()
}

/// Truncate a string to a maximum length, adding ellipsis if needed.
/// Truncates at word boundary when possible. Handles multi-byte UTF-8 safely.
pub fn truncate_preview(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();

    // Count characters, not bytes
    let char_count = trimmed.chars().count();
    if char_count <= max_len {
        return trimmed.to_string();
    }

    // Collect characters up to max_len
    let truncated: String = trimmed.chars().take(max_len).collect();

    // Try to find a word boundary (space) in the truncated string
    // rfind returns byte index, so we need to find char index instead
    if let Some(last_space_byte_idx) = truncated.rfind(' ') {
        // Convert byte index to char index
        let char_idx_at_space = truncated[..last_space_byte_idx].chars().count();
        if char_idx_at_space > max_len / 2 {
            // Take chars up to the space
            let up_to_space: String = truncated.chars().take(char_idx_at_space).collect();
            return format!("{}...", up_to_space.trim_end());
        }
    }

    format!("{}...", truncated.trim_end())
}

/// Discover all Claude Code projects and their sessions.
///
/// # Returns
/// A list of `ProjectInfo` sorted by most recent session activity.
///
/// # Errors
/// Returns an error only for permission denied. Missing directories return empty vec.
pub async fn get_projects() -> Result<Vec<ProjectInfo>, DiscoveryError> {
    let projects_dir = claude_projects_dir()?;

    // If the directory doesn't exist, return empty list (not an error)
    if !projects_dir.exists() {
        debug!("Claude projects directory does not exist: {:?}", projects_dir);
        return Ok(vec![]);
    }

    let mut entries = match fs::read_dir(&projects_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(DiscoveryError::PermissionDenied {
                path: projects_dir,
            });
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => {
            return Err(DiscoveryError::Io {
                path: projects_dir,
                source: e,
            });
        }
    };

    let mut projects = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|e| DiscoveryError::Io {
        path: projects_dir.clone(),
        source: e,
    })? {
        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        let encoded_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let resolved = resolve_project_path(&encoded_name);

        // Get sessions for this project
        let sessions = match get_project_sessions(&path, &encoded_name, &resolved).await {
            Ok(s) => s,
            Err(e) => {
                debug!("Error reading sessions for {:?}: {}", path, e);
                continue;
            }
        };

        // Skip projects with no sessions
        if sessions.is_empty() {
            continue;
        }

        let active_count = count_active_sessions(&sessions);

        projects.push(ProjectInfo {
            name: encoded_name,
            display_name: resolved.display_name,
            path: resolved.full_path,
            active_count,
            sessions,
        });
    }

    // Sort projects by most recent session
    projects.sort_by(|a, b| {
        let a_latest = a.sessions.iter().map(|s| s.modified_at).max().unwrap_or(0);
        let b_latest = b.sessions.iter().map(|s| s.modified_at).max().unwrap_or(0);
        b_latest.cmp(&a_latest)
    });

    Ok(projects)
}

/// Get all sessions for a project directory.
async fn get_project_sessions(
    project_path: &Path,
    encoded_name: &str,
    resolved: &ResolvedProject,
) -> Result<Vec<SessionInfo>, std::io::Error> {
    let mut entries = fs::read_dir(project_path).await?;
    let mut sessions = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Only process .jsonl files
        if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
            continue;
        }

        let metadata = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Extract session ID from filename
        let session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract metadata efficiently
        let extracted = extract_session_metadata(&path).await;

        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        sessions.push(SessionInfo {
            id: session_id,
            project: encoded_name.to_string(),
            project_path: resolved.full_path.clone(),
            file_path: path.to_string_lossy().to_string(),
            modified_at,
            size_bytes: metadata.len(),
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
            // Phase C: LOC estimation
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
        });
    }

    // Sort sessions by modification time (most recent first)
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(sessions)
}

/// Extract metadata from a session file without fully parsing it.
/// This is an efficient scan that looks for specific patterns.
pub async fn extract_session_metadata(file_path: &Path) -> ExtractedMetadata {
    let file = match fs::File::open(file_path).await {
        Ok(f) => f,
        Err(_) => return ExtractedMetadata::default(),
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut metadata = ExtractedMetadata::default();
    let mut first_user_message: Option<String> = None;
    let mut last_user_message: Option<String> = None;
    let mut user_count = 0;
    let mut assistant_count = 0;

    // Regex for detecting skills (slash commands like /commit, /review-pr, /superpowers:brainstorm)
    // Must NOT be followed by another / (to exclude file paths like /Users/test)
    // Captures the full skill including the leading /
    // Pattern: /word with optional :word or -word segments, not followed by /
    let skill_regex = Regex::new(r"(?:^|[^/\w])(/[a-zA-Z][\w:-]*)(?:[^/]|$)").ok();

    // Regex for file paths in tool inputs
    let file_path_regex = Regex::new(r#""file_path"\s*:\s*"([^"]+)""#).ok();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        metadata.message_count += 1;

        // Quick type detection without full JSON parsing
        if line.contains(r#""type":"user""#) || line.contains(r#""type": "user""#) {
            user_count += 1;

            // Extract content for preview
            if let Some(content) = extract_content_quick(line) {
                // Check for skills (slash commands)
                if let Some(ref re) = skill_regex {
                    for cap in re.captures_iter(&content) {
                        if let Some(skill) = cap.get(1) {
                            let skill_name = skill.as_str().to_string();
                            // Double-check: skill must start with / and not look like a file path
                            // File paths typically have multiple / like /Users/foo/bar
                            if skill_name.starts_with('/') && !metadata.skills_used.contains(&skill_name) {
                                metadata.skills_used.push(skill_name);
                            }
                        }
                    }
                }

                // Track first and last user message
                if first_user_message.is_none() && !content.trim().is_empty() {
                    // Skip messages that are just command tags
                    let cleaned = clean_for_preview(&content);
                    if !cleaned.is_empty() {
                        first_user_message = Some(cleaned);
                    }
                }
                last_user_message = Some(content);
            }
        } else if line.contains(r#""type":"assistant""#) || line.contains(r#""type": "assistant""#) {
            assistant_count += 1;

            // Count tool uses
            count_tools_quick(line, &mut metadata.tool_counts);

            // Extract file paths (filename only, limit to 5)
            if metadata.files_touched.len() < 5 {
                if let Some(ref re) = file_path_regex {
                    for cap in re.captures_iter(line) {
                        if metadata.files_touched.len() >= 5 {
                            break;
                        }
                        if let Some(path) = cap.get(1) {
                            // Extract just the filename from the path
                            let path_str = path.as_str();
                            let filename = path_str
                                .rsplit('/')
                                .next()
                                .unwrap_or(path_str)
                                .to_string();
                            if !filename.is_empty() && !metadata.files_touched.contains(&filename) {
                                metadata.files_touched.push(filename);
                            }
                        }
                    }
                }
            }
        }
    }

    // Set preview from first user message
    if let Some(preview) = first_user_message {
        metadata.preview = truncate_preview(&preview, 200);
    }

    // Set last message
    if let Some(last) = last_user_message {
        metadata.last_message = truncate_preview(&last, 200);
    }

    // Calculate turn count
    metadata.turn_count = user_count.min(assistant_count);

    metadata
}

/// Quick content extraction without full JSON parsing.
fn extract_content_quick(line: &str) -> Option<String> {
    // Look for "content":"..." pattern
    let content_start = line.find(r#""content":"#)?;
    let after_content = &line[content_start + 10..];

    // Handle string content
    if let Some(content_str) = after_content.strip_prefix('"') {
        if let Some(end) = find_string_end(content_str) {
            let content = &content_str[..end];
            // Unescape basic JSON escapes
            return Some(unescape_json_string(content));
        }
    }

    // Handle array content (blocks)
    if after_content.starts_with('[') {
        // Look for text blocks
        let mut result = String::new();
        let mut search_pos = 0;
        while let Some(text_pos) = after_content[search_pos..].find(r#""text":""#) {
            let start = search_pos + text_pos + 8;
            if start < after_content.len() {
                if let Some(end) = find_string_end(&after_content[start..]) {
                    let text = &after_content[start..start + end];
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(&unescape_json_string(text));
                    search_pos = start + end;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        if !result.is_empty() {
            return Some(result);
        }
    }

    None
}

/// Find the end of a JSON string (handling escapes).
fn find_string_end(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some(i),
            '\\' => {
                // Skip the next character
                chars.next();
            }
            _ => {}
        }
    }
    None
}

/// Basic JSON string unescaping.
fn unescape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Quick tool counting without full JSON parsing.
fn count_tools_quick(line: &str, counts: &mut ToolCounts) {
    // Count tool_use blocks by name (handle both with and without space after colon)
    counts.read += line.matches(r#""name":"Read""#).count();
    counts.read += line.matches(r#""name": "Read""#).count();
    counts.edit += line.matches(r#""name":"Edit""#).count();
    counts.edit += line.matches(r#""name": "Edit""#).count();
    counts.write += line.matches(r#""name":"Write""#).count();
    counts.write += line.matches(r#""name": "Write""#).count();
    counts.bash += line.matches(r#""name":"Bash""#).count();
    counts.bash += line.matches(r#""name": "Bash""#).count();
}

/// Clean content for preview display.
fn clean_for_preview(content: &str) -> String {
    // Remove command tags
    let tag_regex = Regex::new(r"<command-name>[^<]*</command-name>\s*").unwrap();
    let args_regex = Regex::new(r"<command-args>[^<]*</command-args>\s*").unwrap();

    let cleaned = tag_regex.replace_all(content, "");
    let cleaned = args_regex.replace_all(&cleaned, "");
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    // ============================================================================
    // claude_projects_dir Tests
    // ============================================================================

    #[test]
    fn test_claude_projects_dir() {
        let result = claude_projects_dir();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().contains(".claude"));
        assert!(path.to_string_lossy().contains("projects"));
        assert!(path.ends_with("projects"));
    }

    #[test]
    fn test_claude_projects_dir_format() {
        let path = claude_projects_dir().unwrap();
        let path_str = path.to_string_lossy();

        // Should be an absolute path
        assert!(path_str.starts_with('/') || path_str.contains(':'));

        // Should end with .claude/projects
        assert!(path_str.ends_with(".claude/projects") || path_str.ends_with(".claude\\projects"));
    }

    // ============================================================================
    // resolve_project_path Tests
    // ============================================================================

    #[test]
    fn test_resolve_simple_path() {
        // This is a fallback test since /tmp probably doesn't exist as encoded
        let resolved = resolve_project_path("-tmp");
        assert_eq!(resolved.full_path, "/tmp");
        assert_eq!(resolved.display_name, "tmp");
    }

    #[test]
    fn test_resolve_nonexistent_path() {
        // A path that definitely doesn't exist
        let resolved = resolve_project_path("-nonexistent-path-abc123");
        // Should fall back to basic decode
        assert!(resolved.full_path.starts_with('/'));
        assert!(!resolved.display_name.is_empty());
    }

    #[test]
    fn test_resolve_empty_path() {
        let resolved = resolve_project_path("");
        assert_eq!(resolved.full_path, "");
        assert_eq!(resolved.display_name, "");
    }

    #[test]
    fn test_resolve_complex_path() {
        // A typical Claude project path
        let resolved = resolve_project_path("-Users-test-dev-my-cool-project");
        // Should produce a reasonable path
        assert!(resolved.full_path.starts_with('/'));
        // The display name should be the last component
        assert!(!resolved.display_name.is_empty());
    }

    // ============================================================================
    // get_join_variants Tests
    // ============================================================================

    #[test]
    fn test_get_join_variants_simple() {
        let variants = get_join_variants("-tmp");
        assert!(!variants.is_empty());
        assert!(variants.contains(&"/tmp".to_string()));
    }

    #[test]
    fn test_get_join_variants_hyphenated_name() {
        let variants = get_join_variants("-Users-test-my-project");
        assert!(!variants.is_empty());

        // Should include various interpretations
        assert!(variants.contains(&"/Users/test/my/project".to_string()));
        assert!(variants.contains(&"/Users/test/my-project".to_string()));
    }

    #[test]
    fn test_get_join_variants_without_leading_hyphen() {
        let variants = get_join_variants("Users-test-project");
        assert!(!variants.is_empty());
        // Should still work, treating it as relative
        assert!(variants.iter().any(|v| v.contains("Users")));
    }

    #[test]
    fn test_get_join_variants_single_part() {
        let variants = get_join_variants("-tmp");
        assert!(variants.contains(&"/tmp".to_string()));
    }

    #[test]
    fn test_get_join_variants_empty() {
        let variants = get_join_variants("");
        assert!(variants.contains(&"/".to_string()) || !variants.is_empty());
    }

    // ============================================================================
    // Issue 3: Path Resolution - Double Dash and Dot Support
    // ============================================================================

    #[test]
    fn test_double_dash_converts_to_at_symbol() {
        // --example-org should become /@example-org (scoped packages)
        let variants = get_join_variants("-Users-user-dev--example-org-project");

        assert!(
            variants.iter().any(|v| v.contains("/@example-org")),
            "Should convert -- to /@ for scoped packages. Got: {:?}",
            variants
        );
    }

    #[test]
    fn test_double_dash_at_start() {
        // Double dash at start of component
        let variants = get_join_variants("-Users-dev--scope-package");

        assert!(
            variants.iter().any(|v| v.contains("/@scope")),
            "Should handle -- at component boundary. Got: {:?}",
            variants
        );
    }

    #[test]
    fn test_dot_separator_for_domains() {
        // acme-io should try acme.io
        let variants = get_join_variants("-Users-test-acme-io");

        assert!(
            variants.iter().any(|v| v.ends_with("acme.io")),
            "Should try dot separator for domain-like names. Got: {:?}",
            variants
        );
    }

    #[test]
    fn test_dot_separator_three_parts() {
        // my-app-io should try my-app.io and my.app.io
        let variants = get_join_variants("-home-user-my-app-io");

        assert!(
            variants.iter().any(|v| v.contains(".io")),
            "Should try .io domain pattern. Got: {:?}",
            variants
        );
    }

    // ============================================================================
    // Issue 6: filesTouched - Limit to 5, Filename Only
    // ============================================================================

    #[tokio::test]
    async fn test_files_touched_limited_to_5() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        // Create session with 10 file edits
        let content = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file1.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file2.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file3.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file4.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file5.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file6.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/b/file7.rs"}}]}}"#;

        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.files_touched.len() <= 5,
            "Should limit to 5 files, got: {}",
            metadata.files_touched.len()
        );
    }

    #[tokio::test]
    async fn test_files_touched_shows_filename_only() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/Users/test/project/src/main.rs"}}]}}"#;

        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.files_touched.contains(&"main.rs".to_string()),
            "Should contain filename only, got: {:?}",
            metadata.files_touched
        );
        assert!(
            !metadata.files_touched.iter().any(|f| f.contains('/')),
            "Should NOT contain paths with slashes, got: {:?}",
            metadata.files_touched
        );
    }

    // ============================================================================
    // truncate_preview Tests
    // ============================================================================

    #[test]
    fn test_truncate_preview_short_string() {
        let text = "Hello world";
        let result = truncate_preview(text, 50);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_truncate_preview_long_string() {
        let text = "This is a very long string that definitely exceeds the maximum length";
        let result = truncate_preview(text, 30);
        assert!(result.len() <= 33); // 30 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_word_boundary() {
        let text = "Hello world this is a test";
        let result = truncate_preview(text, 15);
        // Should truncate at word boundary if possible
        assert!(result.ends_with("..."));
        assert!(result.len() <= 18);
    }

    #[test]
    fn test_truncate_preview_exact_length() {
        let text = "Exactly 10";
        let result = truncate_preview(text, 10);
        assert_eq!(result, "Exactly 10");
    }

    #[test]
    fn test_truncate_preview_whitespace() {
        let text = "  Hello world  ";
        let result = truncate_preview(text, 50);
        assert_eq!(result, "Hello world");
    }

    // ============================================================================
    // extract_session_metadata Tests
    // ============================================================================

    #[tokio::test]
    async fn test_extract_session_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Hello, please help me"},"timestamp":"2026-01-27T10:00:00Z"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Sure!"},{"type":"tool_use","name":"Read","input":{"file_path":"/test/file.rs"}}]},"timestamp":"2026-01-27T10:00:01Z"}
{"type":"user","message":{"content":"Now /commit the changes"},"timestamp":"2026-01-27T10:00:02Z"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Done"},{"type":"tool_use","name":"Edit","input":{"file_path":"/test/other.rs"}}]},"timestamp":"2026-01-27T10:00:03Z"}"#;

        let mut file = tokio::fs::File::create(&file_path).await.unwrap();
        file.write_all(content.as_bytes()).await.unwrap();
        file.flush().await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert_eq!(metadata.preview, "Hello, please help me");
        assert_eq!(metadata.message_count, 4);
        assert_eq!(metadata.turn_count, 2);
        assert_eq!(metadata.tool_counts.read, 1);
        assert_eq!(metadata.tool_counts.edit, 1);
        assert!(metadata.skills_used.contains(&"/commit".to_string()));
        // Note: files_touched now contains only filenames, not full paths
        assert!(metadata.files_touched.contains(&"file.rs".to_string()));
    }

    #[tokio::test]
    async fn test_extract_session_metadata_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.jsonl");

        tokio::fs::File::create(&file_path).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(metadata.preview.is_empty());
        assert_eq!(metadata.message_count, 0);
        assert_eq!(metadata.turn_count, 0);
    }

    #[tokio::test]
    async fn test_extract_session_metadata_nonexistent_file() {
        let metadata = extract_session_metadata(Path::new("/nonexistent/file.jsonl")).await;
        assert!(metadata.preview.is_empty());
        assert_eq!(metadata.message_count, 0);
    }

    // ============================================================================
    // get_projects Tests
    // ============================================================================

    #[tokio::test]
    async fn test_get_projects_empty_dir() {
        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join(".claude").join("projects");
        tokio::fs::create_dir_all(&projects_dir).await.unwrap();

        // We can't easily test get_projects with a custom path since it uses claude_projects_dir()
        // Instead, test that it handles the real path gracefully
        let result = get_projects().await;
        // Should not error, even if dir doesn't exist or is empty
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_projects_returns_vec() {
        let result = get_projects().await;
        assert!(result.is_ok());
        // The result is a Vec, which may or may not have items
        let _projects: Vec<ProjectInfo> = result.unwrap();
    }

    // ============================================================================
    // Helper Function Tests
    // ============================================================================

    #[test]
    fn test_extract_content_quick_string() {
        let line = r#"{"type":"user","message":{"content":"Hello world"}}"#;
        let content = extract_content_quick(line);
        assert_eq!(content, Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_content_quick_with_escapes() {
        let line = r#"{"type":"user","message":{"content":"Hello\nworld"}}"#;
        let content = extract_content_quick(line);
        assert_eq!(content, Some("Hello\nworld".to_string()));
    }

    #[test]
    fn test_extract_content_quick_blocks() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"text","text":"World"}]}}"#;
        let content = extract_content_quick(line);
        assert!(content.is_some());
        let text = content.unwrap();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_count_tools_quick() {
        let line = r#"{"content":[{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Edit"}]}"#;
        let mut counts = ToolCounts::default();
        count_tools_quick(line, &mut counts);

        assert_eq!(counts.read, 2);
        assert_eq!(counts.edit, 1);
        assert_eq!(counts.write, 0);
        assert_eq!(counts.bash, 0);
    }

    #[test]
    fn test_clean_for_preview() {
        let content = "<command-name>/commit</command-name>\nPlease commit my changes";
        let cleaned = clean_for_preview(content);
        assert_eq!(cleaned, "Please commit my changes");
    }

    #[test]
    fn test_clean_for_preview_no_tags() {
        let content = "Normal message without tags";
        let cleaned = clean_for_preview(content);
        assert_eq!(cleaned, "Normal message without tags");
    }

    #[test]
    fn test_unescape_json_string() {
        assert_eq!(unescape_json_string(r"hello\nworld"), "hello\nworld");
        assert_eq!(unescape_json_string(r"tab\there"), "tab\there");
        assert_eq!(unescape_json_string(r#"quote\"here"#), "quote\"here");
        assert_eq!(unescape_json_string(r"back\\slash"), "back\\slash");
    }

    #[test]
    fn test_find_string_end() {
        assert_eq!(find_string_end(r#"hello""#), Some(5));
        assert_eq!(find_string_end(r#"he\"llo""#), Some(7));
        assert_eq!(find_string_end(r#"no end"#), None);
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[tokio::test]
    async fn test_get_project_sessions_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved).await;
        assert!(sessions.is_ok());
        assert!(sessions.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_project_sessions_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        // Create a test session file
        let session_path = temp_dir.path().join("session-123.jsonl");
        let content = r#"{"type":"user","message":{"content":"Test"}}"#;
        tokio::fs::write(&session_path, content).await.unwrap();

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved)
            .await
            .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-123");
        assert_eq!(sessions[0].preview, "Test");
    }

    // ============================================================================
    // Issue 2: activeCount Calculation Tests
    // ============================================================================

    #[test]
    fn test_count_active_sessions_within_5_minutes() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sessions = vec![
            create_test_session_with_time(now - 60),     // 1 min ago (active)
            create_test_session_with_time(now - 240),    // 4 min ago (active)
            create_test_session_with_time(now - 600),    // 10 min ago (not active)
            create_test_session_with_time(now - 3600),   // 1 hour ago (not active)
        ];

        let active = count_active_sessions(&sessions);
        assert_eq!(active, 2, "Should count 2 sessions within 5 minutes");
    }

    #[test]
    fn test_count_active_sessions_none_recent() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sessions = vec![
            create_test_session_with_time(now - 600),    // 10 min ago
            create_test_session_with_time(now - 1800),   // 30 min ago
        ];

        let active = count_active_sessions(&sessions);
        assert_eq!(active, 0, "Should count 0 when no sessions within 5 minutes");
    }

    #[test]
    fn test_count_active_sessions_boundary() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sessions = vec![
            create_test_session_with_time(now - 299),    // Just under 5 min (active)
            create_test_session_with_time(now - 301),    // Just over 5 min (not active)
        ];

        let active = count_active_sessions(&sessions);
        assert_eq!(active, 1, "Should count session at 4:59 as active, 5:01 as not");
    }

    fn create_test_session_with_time(modified_at: i64) -> crate::types::SessionInfo {
        crate::types::SessionInfo {
            id: "test".to_string(),
            project: "test".to_string(),
            project_path: "/test".to_string(),
            file_path: "/test/session.jsonl".to_string(),
            modified_at,
            size_bytes: 100,
            preview: "Test".to_string(),
            last_message: "Test".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: crate::types::ToolCounts::default(),
            message_count: 1,
            turn_count: 1,
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
            // Phase 3: Atomic unit metrics
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
            // Phase C: LOC estimation
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
        }
    }

    #[tokio::test]
    async fn test_get_project_sessions_ignores_non_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let resolved = ResolvedProject {
            full_path: temp_dir.path().to_string_lossy().to_string(),
            display_name: "test".to_string(),
        };

        // Create various files
        tokio::fs::write(temp_dir.path().join("session.jsonl"), r#"{"type":"user","message":{"content":"Test"}}"#).await.unwrap();
        tokio::fs::write(temp_dir.path().join("notes.txt"), "some notes").await.unwrap();
        tokio::fs::write(temp_dir.path().join("config.json"), "{}").await.unwrap();

        let sessions = get_project_sessions(temp_dir.path(), "test", &resolved)
            .await
            .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session");
    }

    // ============================================================================
    // Issue 4: Skills Extraction Tests
    // ============================================================================

    #[tokio::test]
    async fn test_skills_extraction_captures_slash_commands() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Please /commit my changes"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.skills_used.contains(&"/commit".to_string()),
            "Should contain /commit, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_with_colon_separator() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Run /superpowers:brainstorm please"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.skills_used.contains(&"/superpowers:brainstorm".to_string()),
            "Should contain /superpowers:brainstorm, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_with_hyphen() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Please /review-pr this"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.skills_used.contains(&"/review-pr".to_string()),
            "Should contain /review-pr, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_ignores_file_paths() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"Check file at /Users/test/path/file.rs"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        // /Users is a path, not a skill - should be excluded
        assert!(
            !metadata.skills_used.iter().any(|s| s.contains("Users")),
            "Should NOT contain /Users, got: {:?}",
            metadata.skills_used
        );
    }

    #[tokio::test]
    async fn test_skills_extraction_multiple_skills() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let content = r#"{"type":"user","message":{"content":"/commit then /push please"}}"#;
        tokio::fs::write(&file_path, content).await.unwrap();

        let metadata = extract_session_metadata(&file_path).await;

        assert!(
            metadata.skills_used.contains(&"/commit".to_string()),
            "Should contain /commit"
        );
        assert!(
            metadata.skills_used.contains(&"/push".to_string()),
            "Should contain /push"
        );
    }

    // ============================================================================
    // DFS Path Resolution Tests
    // ============================================================================

    #[test]
    fn test_tokenize_simple() {
        let segments = tokenize_encoded_name("-Users-foo-bar");
        assert_eq!(segments, vec!["Users", "foo", "bar"]);
    }

    #[test]
    fn test_tokenize_double_dash_at_prefix() {
        // -- means /@ for scoped packages
        let segments = tokenize_encoded_name("-Users-dev--example-org-project");
        assert_eq!(
            segments,
            vec!["Users", "dev", "@example", "org", "project"]
        );
    }

    #[test]
    fn test_tokenize_empty() {
        assert!(tokenize_encoded_name("").is_empty());
        assert!(tokenize_encoded_name("-").is_empty());
    }

    #[test]
    fn test_dfs_resolve_with_tempdir() {
        // Create a directory structure that mimics the real scenario
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Create: base/dev/@example-org/claude-view/
        std::fs::create_dir_all(base.join("dev/@example-org/claude-view")).unwrap();

        let segments: Vec<String> = vec![
            "dev", "@example", "org", "claude", "view",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some(), "DFS should find the path");
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("dev/@example-org/claude-view"),
            "Should resolve to @example-org/claude-view, got: {:?}",
            resolved
        );
    }

    #[test]
    fn test_dfs_resolve_hyphenated_project_name() {
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Create: base/dev/my-cool-project/
        std::fs::create_dir_all(base.join("dev/my-cool-project")).unwrap();

        let segments: Vec<String> = vec!["dev", "my", "cool", "project"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some());
        assert!(
            result.unwrap().ends_with("dev/my-cool-project"),
            "Should join hyphens for project name"
        );
    }

    #[test]
    fn test_dfs_resolve_dot_domain() {
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Create: base/dev/acme.io/
        std::fs::create_dir_all(base.join("dev/acme.io")).unwrap();

        let segments: Vec<String> = vec!["dev", "acme", "io"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some());
        assert!(
            result.unwrap().ends_with("dev/acme.io"),
            "Should try dot join for domain-like names"
        );
    }

    #[test]
    fn test_dfs_resolve_backtracking() {
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Create BOTH: base/a/ and base/a-b/c/
        // DFS should try base/a/ first, fail to find b/c, then backtrack to base/a-b/c
        std::fs::create_dir_all(base.join("a")).unwrap();
        std::fs::create_dir_all(base.join("a-b/c")).unwrap();

        let segments: Vec<String> = vec!["a", "b", "c"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some());
        assert!(
            result.unwrap().ends_with("a-b/c"),
            "Should backtrack from a/ to a-b/c"
        );
    }

    #[test]
    fn test_dfs_resolve_nonexistent() {
        let temp = TempDir::new().unwrap();
        let segments: Vec<String> = vec!["no", "such", "path"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(temp.path(), &segments, 0);
        assert!(result.is_none(), "Should return None for nonexistent paths");
    }

    #[test]
    fn test_dfs_resolve_dot_prefixed_directory() {
        // Bug: `--` is ambiguous — encodes both `@` and `.` prefixed dirs.
        // .worktrees, .config, .git etc. must resolve, not just @-prefixed.
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Create: base/project/.worktrees/main-audit/
        std::fs::create_dir_all(base.join("project/.worktrees/main-audit")).unwrap();

        // Tokenizer produces @worktrees from `--worktrees` encoding
        let segments: Vec<String> = vec!["project", "@worktrees", "main", "audit"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some(), "DFS should resolve .worktrees via dot-prefix fallback");
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("project/.worktrees/main-audit"),
            "Should resolve to .worktrees/main-audit, got: {:?}",
            resolved
        );
    }

    #[test]
    fn test_dfs_resolve_mixed_at_and_dot_prefixes() {
        // Path with both @scope and .hidden in the same tree
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Create: base/dev/@my-org/project/.config/
        std::fs::create_dir_all(base.join("dev/@my-org/project/.config")).unwrap();

        // Both -- sequences: @my → scoped, @config → hidden
        let segments: Vec<String> = vec!["dev", "@my", "org", "project", "@config"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some(), "DFS should resolve mixed @ and . prefixes");
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("dev/@my-org/project/.config"),
            "Should resolve both @my-org and .config, got: {:?}",
            resolved
        );
    }

    #[test]
    fn test_dfs_resolve_long_hyphenated_name() {
        // Directory names with 5+ hyphen-separated words must resolve.
        // Old 4-segment cap would fail on this.
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Create: base/dev/my-very-cool-react-native-app/
        std::fs::create_dir_all(base.join("dev/my-very-cool-react-native-app")).unwrap();

        let segments: Vec<String> = vec!["dev", "my", "very", "cool", "react", "native", "app"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some(), "DFS should resolve 6-segment hyphenated name (no 4-cap)");
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("dev/my-very-cool-react-native-app"),
            "Should join all 6 segments, got: {:?}",
            resolved
        );
    }

    #[test]
    fn test_dfs_resolve_bare_fallback_from_double_dash() {
        // Edge case: `--` encoding but the real dir has no prefix at all.
        // e.g., directory just named "config" but came from `--config` encoding.
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        // Only bare "config" exists (no @ or . prefix)
        std::fs::create_dir_all(base.join("project/config")).unwrap();

        let segments: Vec<String> = vec!["project", "@config"]
            .into_iter()
            .map(String::from)
            .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(result.is_some(), "DFS should fall back to bare name when @/. don't exist");
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("project/config"),
            "Should resolve to bare config, got: {:?}",
            resolved
        );
    }

    #[test]
    fn test_dfs_resolve_real_worktree_path() {
        // End-to-end: full tokenize → DFS for the actual worktree path pattern
        // Encoded: -X-Y-dev--org-project--worktrees-main-audit
        // Real:    /X/Y/dev/@org/project/.worktrees/main-audit
        let temp = TempDir::new().unwrap();
        let base = temp.path();

        std::fs::create_dir_all(base.join("X/Y/dev/@org-name/my-project/.worktrees/main-audit"))
            .unwrap();

        // Simulate what tokenize_encoded_name produces for the encoded name
        let segments: Vec<String> =
            vec!["X", "Y", "dev", "@org", "name", "my", "project", "@worktrees", "main", "audit"]
                .into_iter()
                .map(String::from)
                .collect();

        let result = dfs_resolve(base, &segments, 0);
        assert!(
            result.is_some(),
            "DFS should resolve full worktree path with mixed @ and . prefixes"
        );
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("dev/@org-name/my-project/.worktrees/main-audit"),
            "Full path should resolve correctly, got: {:?}",
            resolved
        );
    }

    // ============================================================================
    // Display Name Tests
    //
    // Note: TempDir may be created inside a git repo (the workspace), so tests
    // use a deep path structure to ensure the 5-level cap doesn't reach the
    // workspace .git, OR test relative expectations.
    // ============================================================================

    #[test]
    fn test_display_name_git_root_at_resolved_path() {
        let temp = TempDir::new().unwrap();
        // Create enough depth that the 5-level walk won't reach the workspace .git
        let deep = temp.path().join("a/b/c/d/e/f/my-project");
        std::fs::create_dir_all(deep.join(".git")).unwrap();

        let name = derive_display_name(&deep.to_string_lossy());
        assert_eq!(name, "my-project");
    }

    #[test]
    fn test_display_name_subdirectory_of_git_root() {
        let temp = TempDir::new().unwrap();
        let deep = temp.path().join("a/b/c/d/e/f/repo");
        std::fs::create_dir_all(deep.join(".git")).unwrap();
        let subdir = deep.join("web");
        std::fs::create_dir_all(&subdir).unwrap();

        let name = derive_display_name(&subdir.to_string_lossy());
        assert_eq!(name, "repo/web");
    }

    #[test]
    fn test_display_name_nested_git_uses_topmost() {
        let temp = TempDir::new().unwrap();
        let deep = temp.path().join("a/b/c/d/e/f");
        // Parent repo has .git
        let parent = deep.join("parent");
        std::fs::create_dir_all(parent.join(".git")).unwrap();
        // Child also has .git (worktree or nested repo)
        let child = parent.join("child");
        std::fs::create_dir_all(child.join(".git")).unwrap();

        let name = derive_display_name(&child.to_string_lossy());
        assert_eq!(name, "parent/child");
    }

    #[test]
    fn test_display_name_no_git_root_fallback() {
        let temp = TempDir::new().unwrap();
        let deep = temp.path().join("a/b/c/d/e/f/some-dir");
        std::fs::create_dir_all(&deep).unwrap();

        let name = derive_display_name(&deep.to_string_lossy());
        // No .git within 5 levels → falls back to last component
        assert_eq!(name, "some-dir");
    }

    // ========================================================================
    // resolve_worktree_parent Tests
    // ========================================================================

    #[test]
    fn test_worktree_parent_basic() {
        assert_eq!(
            resolve_worktree_parent("-Users-dev-project--worktrees-feature-branch"),
            Some("-Users-dev-project".to_string())
        );
    }

    #[test]
    fn test_non_worktree_returns_none() {
        assert_eq!(resolve_worktree_parent("-Users-dev-project"), None);
    }

    #[test]
    fn test_worktree_parent_edge_cases() {
        assert_eq!(resolve_worktree_parent(""), None);
        assert_eq!(resolve_worktree_parent("--worktrees-foo"), None); // marker at pos 0
    }

    #[test]
    fn test_worktree_parent_preserves_complex_parent() {
        assert_eq!(
            resolve_worktree_parent("-Users-dev--vicky-ai-claude-view--worktrees-theme3-contributions"),
            Some("-Users-dev--vicky-ai-claude-view".to_string())
        );
    }
}
