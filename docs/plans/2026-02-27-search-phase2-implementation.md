# Search Phase 2: Regex Grep + Remaining Search Features — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add regex grep search (ripgrep engine) against raw JSONL files, `after:`/`before:` date qualifiers in Tantivy, and in-session Ctrl+F — completing all remaining Phase 2 search features.

**Architecture:** Regex toggle in existing search bar switches between Tantivy (existing BM25 full-text) and a new `/api/grep` endpoint powered by the `grep` crate (ripgrep's core engine). Date qualifiers extend the existing Tantivy query parser. In-session search is client-side only.

**Tech Stack:** `grep-regex` + `grep-searcher` + `grep-matcher` (ripgrep core crates), Tantivy `RangeQuery` for date filters, React hooks for in-session search.

**Design doc:** `docs/plans/2026-02-27-search-phase2-regex-grep-design.md`

---

## Feature Status

| # | Feature | Status | Notes |
|---|---------|--------|-------|
| 1 | Regex grep search | **Not started** — PRIMARY FEATURE (Tasks 1-8) |
| 2 | `after:`/`before:` date qualifiers | **Not started** (Task 9) |
| 3 | In-session Ctrl+F | **Not started** (Tasks 10-11) |
| 4 | SearchBar scoped mode | **Deferred** — existing Cmd+K workflow works |
| 5 | History → Tantivy wiring | **ALREADY DONE** — `sessions.rs:248-282` calls Tantivy, falls back to LIKE |

---

## Task 1: Add grep crate dependencies to workspace

**Files:**
- Modify: `Cargo.toml` (workspace root, line 28)
- Modify: `crates/search/Cargo.toml` (line 16)

**Step 1: Add grep crates to workspace dependencies**

In root `Cargo.toml`, add after `tantivy = "0.22"` (line 28):

```toml
# Grep (ripgrep core — Phase 2 regex search)
grep-matcher = "0.1"
grep-regex = "0.1"
grep-searcher = "0.1"
```

**Step 2: Add grep crates to search crate**

In `crates/search/Cargo.toml`, add after `chrono = { workspace = true }` (line 16):

```toml
grep-matcher = { workspace = true }
grep-regex = { workspace = true }
grep-searcher = { workspace = true }
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-search`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add Cargo.toml crates/search/Cargo.toml
git commit -m "chore(search): add grep crate dependencies for regex search"
```

---

## Task 2: Create grep response types

New types for the `/api/grep` endpoint, separate from existing Tantivy `SearchResponse`.

**Files:**
- Create: `crates/search/src/grep_types.rs`
- Modify: `crates/search/src/lib.rs:15-17` (add module declaration)

**Step 1: Create the types file**

Create `crates/search/src/grep_types.rs`:

```rust
use serde::Serialize;
use ts_rs::TS;

/// Response from a regex grep search across raw JSONL files.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct GrepResponse {
    /// The regex pattern that was searched.
    pub pattern: String,
    /// Total number of matching lines across all files.
    pub total_matches: usize,
    /// Number of distinct sessions with matches.
    pub total_sessions: usize,
    /// Time spent executing the search, in milliseconds.
    pub elapsed_ms: f64,
    /// True if results were capped at the limit.
    pub truncated: bool,
    /// Session-grouped results, sorted by modification time descending.
    pub results: Vec<GrepSessionHit>,
}

/// A session containing one or more grep line matches.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct GrepSessionHit {
    pub session_id: String,
    /// Decoded project name (e.g., "claude-view").
    pub project: String,
    /// Full filesystem path of the project.
    pub project_path: String,
    /// Unix timestamp (seconds) of the JSONL file's mtime.
    pub modified_at: i64,
    /// Individual line matches within this session file.
    pub matches: Vec<GrepLineMatch>,
}

/// A single line match from grep.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct GrepLineMatch {
    /// 1-based line number in the JSONL file.
    pub line_number: u64,
    /// The matched line content (truncated to 500 chars).
    pub content: String,
    /// Byte offset of match start within `content`.
    pub match_start: usize,
    /// Byte offset of match end within `content`.
    pub match_end: usize,
}
```

**Step 2: Register the module in lib.rs**

In `crates/search/src/lib.rs`, after line 17 (`pub mod types;`), add:

```rust
pub mod grep_types;
```

And after line 26 (`pub use types::{MatchHit, SearchResponse, SessionHit};`), add:

```rust
pub use grep_types::{GrepLineMatch, GrepResponse, GrepSessionHit};
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-search`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/search/src/grep_types.rs crates/search/src/lib.rs
git commit -m "feat(search): add GrepResponse types for regex search"
```

---

## Task 3: Implement grep engine

The core grep engine that searches `~/.claude/projects/` using ripgrep's library crates.

**Files:**
- Create: `crates/search/src/grep.rs`
- Modify: `crates/search/src/lib.rs` (add module declaration)

**Step 1: Write a failing test**

At the bottom of the new `crates/search/src/grep.rs` file, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_grep_finds_pattern_in_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("project-abc");
        fs::create_dir_all(&project_dir).unwrap();

        // Create a fake JSONL file
        let session_file = project_dir.join("sess123.jsonl");
        fs::write(
            &session_file,
            r#"{"role":"user","content":"add auth middleware"}
{"role":"assistant","content":"I'll implement the auth middleware now"}
{"role":"user","content":"looks good, ship it"}
"#,
        )
        .unwrap();

        let opts = GrepOptions {
            pattern: "auth.*middleware".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 500,
            project_filter: None,
        };

        let result = execute_grep(dir.path(), &opts).unwrap();
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.total_matches, 2, "should match lines 1 and 2");
        assert_eq!(result.results[0].session_id, "sess123");
        assert_eq!(result.results[0].matches.len(), 2);
        assert_eq!(result.results[0].matches[0].line_number, 1);
        assert_eq!(result.results[0].matches[1].line_number, 2);
    }

    #[test]
    fn test_grep_case_sensitive() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("project-abc");
        fs::create_dir_all(&project_dir).unwrap();

        let session_file = project_dir.join("sess456.jsonl");
        fs::write(
            &session_file,
            "Auth middleware is ready\nauth middleware is ready\n",
        )
        .unwrap();

        // Case-insensitive: matches both
        let opts = GrepOptions {
            pattern: "Auth".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 500,
            project_filter: None,
        };
        let result = execute_grep(dir.path(), &opts).unwrap();
        assert_eq!(result.total_matches, 2);

        // Case-sensitive: matches only first
        let opts_cs = GrepOptions {
            case_sensitive: true,
            ..opts
        };
        let result_cs = execute_grep(dir.path(), &opts_cs).unwrap();
        assert_eq!(result_cs.total_matches, 1);
    }

    #[test]
    fn test_grep_limit_truncates() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("project-abc");
        fs::create_dir_all(&project_dir).unwrap();

        let session_file = project_dir.join("sess789.jsonl");
        let content = "match this line\n".repeat(100);
        fs::write(&session_file, &content).unwrap();

        let opts = GrepOptions {
            pattern: "match".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 5,
            project_filter: None,
        };
        let result = execute_grep(dir.path(), &opts).unwrap();
        assert!(result.truncated);
        // Total collected should be capped at limit
        let total_collected: usize = result.results.iter().map(|r| r.matches.len()).sum();
        assert_eq!(total_collected, 5);
    }

    #[test]
    fn test_grep_project_filter() {
        let dir = tempfile::tempdir().unwrap();

        // Two projects
        let proj_a = dir.path().join("project-a");
        let proj_b = dir.path().join("project-b");
        fs::create_dir_all(&proj_a).unwrap();
        fs::create_dir_all(&proj_b).unwrap();

        fs::write(proj_a.join("s1.jsonl"), "hello world\n").unwrap();
        fs::write(proj_b.join("s2.jsonl"), "hello world\n").unwrap();

        // No filter: both match
        let opts = GrepOptions {
            pattern: "hello".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 500,
            project_filter: None,
        };
        let result = execute_grep(dir.path(), &opts).unwrap();
        assert_eq!(result.total_sessions, 2);

        // Filter to project-a only
        let opts_filtered = GrepOptions {
            project_filter: Some("project-a".to_string()),
            ..opts
        };
        let result_filtered = execute_grep(dir.path(), &opts_filtered).unwrap();
        assert_eq!(result_filtered.total_sessions, 1);
        assert_eq!(result_filtered.results[0].project, "project-a");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-search grep::tests`
Expected: FAIL — `execute_grep` and `GrepOptions` don't exist yet.

**Step 3: Implement the grep engine**

Create `crates/search/src/grep.rs`:

```rust
//! Regex grep engine for searching raw JSONL files.
//!
//! Uses the `grep` crate (ripgrep's core) for SIMD-accelerated regex matching
//! with mmap support. Searches `~/.claude/projects/` directory structure:
//!   projects_dir/
//!     <encoded-project-name>/
//!       <session-id>.jsonl
//!
//! Results are grouped by session, sorted by file modification time (newest first).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::SearcherBuilder;
use tracing::warn;

use crate::grep_types::{GrepLineMatch, GrepResponse, GrepSessionHit};
use crate::SearchError;

/// Options for a grep search.
#[derive(Debug, Clone)]
pub struct GrepOptions {
    /// Regex pattern to search for.
    pub pattern: String,
    /// Whether the search is case-sensitive (default: false).
    pub case_sensitive: bool,
    /// Whether to match whole words only (default: false).
    pub whole_word: bool,
    /// Maximum total line matches to return (default: 500).
    pub limit: usize,
    /// Optional: restrict to a single project directory name.
    pub project_filter: Option<String>,
}

/// Execute a grep search across all JSONL files under `projects_dir`.
///
/// `projects_dir` is typically `~/.claude/projects/`.
/// Returns results grouped by session, sorted by file mtime descending.
pub fn execute_grep(projects_dir: &Path, opts: &GrepOptions) -> Result<GrepResponse, SearchError> {
    let start = Instant::now();

    // Validate the regex pattern eagerly (fail fast before file I/O)
    RegexMatcherBuilder::new()
        .case_insensitive(!opts.case_sensitive)
        .word(opts.whole_word)
        .build(&opts.pattern)
        .map_err(|e| SearchError::Io(std::io::Error::other(format!("invalid regex: {e}"))))?;

    // Collect all .jsonl files to search
    let files = collect_jsonl_files(projects_dir, opts.project_filter.as_deref())?;

    if files.is_empty() {
        return Ok(GrepResponse {
            pattern: opts.pattern.clone(),
            total_matches: 0,
            total_sessions: 0,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            truncated: false,
            results: vec![],
        });
    }

    // Shared state for parallel search
    let total_matches = AtomicUsize::new(0);
    let limit_reached = AtomicBool::new(false);
    let session_hits: Mutex<HashMap<String, GrepSessionHit>> = Mutex::new(HashMap::new());

    // Determine parallelism — bounded by available_parallelism per CLAUDE.md
    let parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    // Clone pattern + options for per-thread matcher construction.
    // Each thread builds its own RegexMatcher to avoid contention on internal
    // mutable state — even though RegexMatcher is Sync, per-thread construction
    // is cheaper than sharing and avoids lock overhead. Pattern string is cheap to clone.
    let pattern = opts.pattern.clone();
    let case_sensitive = opts.case_sensitive;
    let whole_word = opts.whole_word;

    // Process files in parallel using scoped threads (no rayon dependency)
    std::thread::scope(|scope| {
        let chunk_size = (files.len() + parallelism - 1) / parallelism;
        let chunks: Vec<&[JsonlFile]> = files.chunks(chunk_size.max(1)).collect();

        for chunk in chunks {
            let pattern = pattern.clone();
            let total_matches = &total_matches;
            let limit_reached = &limit_reached;
            let session_hits = &session_hits;
            let limit = opts.limit;

            scope.spawn(move || {
                // Each thread builds its own RegexMatcher — avoids contention
                let matcher = RegexMatcherBuilder::new()
                    .case_insensitive(!case_sensitive)
                    .word(whole_word)
                    .build(&pattern)
                    .expect("pattern already validated above");

                // Each thread gets its own Searcher (also not thread-safe)
                let mut searcher = SearcherBuilder::new()
                    .line_number(true)
                    .build();

                for file in chunk {
                    if limit_reached.load(Ordering::Relaxed) {
                        break;
                    }

                    let mut file_matches: Vec<GrepLineMatch> = Vec::new();

                    let search_result = searcher.search_path(
                        &matcher,
                        &file.path,
                        UTF8(|line_num, line_content| {
                            if limit_reached.load(Ordering::Relaxed) {
                                return Ok(false); // Stop searching this file
                            }

                            // Find match position within the line
                            let mut match_start = 0;
                            let mut match_end = line_content.len();
                            if let Ok(Some(m)) = matcher.find(line_content.as_bytes()) {
                                match_start = m.start();
                                match_end = m.end();
                            }

                            // Truncate long lines — UTF-8 safe (never split a multibyte char)
                            let content = if line_content.len() > 500 {
                                let end = line_content
                                    .char_indices()
                                    .nth(500)
                                    .map(|(i, _)| i)
                                    .unwrap_or(line_content.len());
                                format!("{}...", line_content[..end].trim_end())
                            } else {
                                line_content.trim_end().to_string()
                            };

                            // Adjust match offsets if content was truncated
                            let adj_end = match_end.min(content.len());
                            let adj_start = match_start.min(adj_end);

                            file_matches.push(GrepLineMatch {
                                line_number: line_num,
                                content,
                                match_start: adj_start,
                                match_end: adj_end,
                            });

                            let prev = total_matches.fetch_add(1, Ordering::Relaxed);
                            if prev + 1 >= limit {
                                limit_reached.store(true, Ordering::Relaxed);
                                return Ok(false);
                            }

                            Ok(true)
                        }),
                    );

                    if let Err(e) = search_result {
                        warn!(
                            path = %file.path.display(),
                            error = %e,
                            "grep search error on file"
                        );
                        continue;
                    }

                    if !file_matches.is_empty() {
                        let hit = GrepSessionHit {
                            session_id: file.session_id.clone(),
                            project: file.project.clone(),
                            project_path: file.project_path.clone(),
                            modified_at: file.mtime,
                            matches: file_matches,
                        };

                        let mut map = session_hits.lock().unwrap();
                        map.insert(file.session_id.clone(), hit);
                    }
                }
            });
        }
    });

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let total = total_matches.load(Ordering::Relaxed);
    let truncated = limit_reached.load(Ordering::Relaxed);

    // Collect and sort by mtime descending (newest first)
    let map = session_hits.into_inner().unwrap();
    let mut results: Vec<GrepSessionHit> = map.into_values().collect();
    results.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    let total_sessions = results.len();

    Ok(GrepResponse {
        pattern: opts.pattern.clone(),
        total_matches: total,
        total_sessions,
        elapsed_ms,
        truncated,
        results,
    })
}

/// A JSONL file to search, with pre-extracted metadata.
#[derive(Debug)]
struct JsonlFile {
    path: std::path::PathBuf,
    session_id: String,
    project: String,
    project_path: String,
    mtime: i64,
}

/// Walk `projects_dir` and collect all `.jsonl` files.
///
/// Directory structure: `projects_dir/<project-dir>/<session-id>.jsonl`
fn collect_jsonl_files(
    projects_dir: &Path,
    project_filter: Option<&str>,
) -> Result<Vec<JsonlFile>, SearchError> {
    let mut files = Vec::new();

    let entries = std::fs::read_dir(projects_dir).map_err(|e| {
        SearchError::Io(std::io::Error::other(format!(
            "cannot read projects dir {}: {e}",
            projects_dir.display()
        )))
    })?;

    for entry in entries.flatten() {
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }

        let project_dir_name = match project_dir.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Apply project filter if specified
        if let Some(filter) = project_filter {
            if project_dir_name != filter {
                continue;
            }
        }

        // Decode project path using resolve_project_path_with_cwd
        // For grep, we use the directory name as-is since we don't have cwd context
        let resolved = claude_view_core::discovery::resolve_project_path_with_cwd(
            &project_dir_name,
            None,
        );

        let session_entries = match std::fs::read_dir(&project_dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!(
                    path = %project_dir.display(),
                    error = %e,
                    "cannot read project directory"
                );
                continue;
            }
        };

        for session_entry in session_entries.flatten() {
            let file_path = session_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            let session_id = match file_path.file_stem().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let mtime = file_path
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0)
                })
                .unwrap_or(0);

            files.push(JsonlFile {
                path: file_path,
                session_id,
                project: resolved.display_name.clone(),
                project_path: resolved.full_path.clone(),
                mtime,
            });
        }
    }

    // Sort by mtime descending so newest files are searched first
    files.sort_by(|a, b| b.mtime.cmp(&a.mtime));

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_grep_finds_pattern_in_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("project-abc");
        fs::create_dir_all(&project_dir).unwrap();

        let session_file = project_dir.join("sess123.jsonl");
        fs::write(
            &session_file,
            r#"{"role":"user","content":"add auth middleware"}
{"role":"assistant","content":"I'll implement the auth middleware now"}
{"role":"user","content":"looks good, ship it"}
"#,
        )
        .unwrap();

        let opts = GrepOptions {
            pattern: "auth.*middleware".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 500,
            project_filter: None,
        };

        let result = execute_grep(dir.path(), &opts).unwrap();
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.total_matches, 2, "should match lines 1 and 2");
        assert_eq!(result.results[0].session_id, "sess123");
        assert_eq!(result.results[0].matches.len(), 2);
        assert_eq!(result.results[0].matches[0].line_number, 1);
        assert_eq!(result.results[0].matches[1].line_number, 2);
    }

    #[test]
    fn test_grep_case_sensitive() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("project-abc");
        fs::create_dir_all(&project_dir).unwrap();

        let session_file = project_dir.join("sess456.jsonl");
        fs::write(
            &session_file,
            "Auth middleware is ready\nauth middleware is ready\n",
        )
        .unwrap();

        let opts = GrepOptions {
            pattern: "Auth".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 500,
            project_filter: None,
        };
        let result = execute_grep(dir.path(), &opts).unwrap();
        assert_eq!(result.total_matches, 2);

        let opts_cs = GrepOptions {
            case_sensitive: true,
            ..opts
        };
        let result_cs = execute_grep(dir.path(), &opts_cs).unwrap();
        assert_eq!(result_cs.total_matches, 1);
    }

    #[test]
    fn test_grep_limit_truncates() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("project-abc");
        fs::create_dir_all(&project_dir).unwrap();

        let session_file = project_dir.join("sess789.jsonl");
        let content = "match this line\n".repeat(100);
        fs::write(&session_file, &content).unwrap();

        let opts = GrepOptions {
            pattern: "match".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 5,
            project_filter: None,
        };
        let result = execute_grep(dir.path(), &opts).unwrap();
        assert!(result.truncated);
        let total_collected: usize = result.results.iter().map(|r| r.matches.len()).sum();
        assert_eq!(total_collected, 5);
    }

    #[test]
    fn test_grep_project_filter() {
        let dir = tempfile::tempdir().unwrap();

        // Use directory names WITHOUT hyphens to avoid resolve_project_path_with_cwd
        // splitting on '-' and returning only the last segment as display_name.
        // Real Claude Code dirs have leading dashes (e.g. "-Users-foo-project"),
        // but test dirs don't — so use single-segment names for predictability.
        let proj_a = dir.path().join("alpha");
        let proj_b = dir.path().join("beta");
        fs::create_dir_all(&proj_a).unwrap();
        fs::create_dir_all(&proj_b).unwrap();

        fs::write(proj_a.join("s1.jsonl"), "hello world\n").unwrap();
        fs::write(proj_b.join("s2.jsonl"), "hello world\n").unwrap();

        // No filter: both match
        let opts = GrepOptions {
            pattern: "hello".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 500,
            project_filter: None,
        };
        let result = execute_grep(dir.path(), &opts).unwrap();
        assert_eq!(result.total_sessions, 2);

        // Filter to alpha only
        let opts_filtered = GrepOptions {
            project_filter: Some("alpha".to_string()),
            ..opts
        };
        let result_filtered = execute_grep(dir.path(), &opts_filtered).unwrap();
        assert_eq!(result_filtered.total_sessions, 1);
        assert_eq!(result_filtered.results[0].project, "alpha");
    }
}
```

**Step 4: Register module in lib.rs**

In `crates/search/src/lib.rs`, after `pub mod grep_types;`, add:

```rust
pub mod grep;
```

And add to the re-exports:

```rust
pub use grep::{execute_grep, GrepOptions};
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p claude-view-search grep::tests`
Expected: All 4 tests pass.

**Step 6: Commit**

```bash
git add crates/search/src/grep.rs crates/search/src/lib.rs
git commit -m "feat(search): implement grep engine with ripgrep core crates"
```

---

## Task 4: Create `/api/grep` Axum route

**Files:**
- Create: `crates/server/src/routes/grep.rs`
- Modify: `crates/server/src/routes/mod.rs` (add module + route registration)

**Step 1: Create the route handler**

Create `crates/server/src/routes/grep.rs`:

```rust
//! Regex grep endpoint for searching raw JSONL files.
//!
//! - GET /grep?pattern=...&case_sensitive=...&whole_word=...&limit=...&project=...

use crate::error::ApiResult;
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use claude_view_core::discovery::claude_projects_dir;
use claude_view_search::grep::{execute_grep, GrepOptions};
use claude_view_search::grep_types::GrepResponse;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct GrepQuery {
    /// Regex pattern to search for. Required.
    pub pattern: Option<String>,
    /// Case-sensitive search (default: false).
    pub case_sensitive: Option<bool>,
    /// Match whole words only (default: false).
    pub whole_word: Option<bool>,
    /// Maximum total line matches to return (default: 500, max: 2000).
    pub limit: Option<usize>,
    /// Optional: restrict search to a single project.
    pub project: Option<String>,
}

/// Build the grep sub-router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/grep", get(grep_handler))
}

/// GET /api/grep — Execute a regex grep search across raw JSONL files.
async fn grep_handler(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<GrepQuery>,
) -> ApiResult<Json<GrepResponse>> {
    let pattern = query
        .pattern
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    if pattern.is_empty() {
        return Err(crate::error::ApiError::BadRequest(
            "query parameter 'pattern' is required".to_string(),
        ));
    }

    // Uses existing `impl From<DiscoveryError> for ApiError` — routes through
    // ApiError::Discovery which handles all DiscoveryError variants correctly.
    let projects_dir = claude_projects_dir()?;

    let opts = GrepOptions {
        pattern,
        case_sensitive: query.case_sensitive.unwrap_or(false),
        whole_word: query.whole_word.unwrap_or(false),
        limit: query.limit.unwrap_or(500).min(2000),
        project_filter: query.project,
    };

    // Run grep on a blocking thread — it does file I/O
    let response = tokio::task::spawn_blocking(move || execute_grep(&projects_dir, &opts))
        .await
        .map_err(|e| crate::error::ApiError::Internal(format!("grep task failed: {e}")))?
        .map_err(|e| crate::error::ApiError::Internal(format!("grep failed: {e}")))?;

    Ok(Json(response))
}
```

**Step 2: Register the route in mod.rs**

In `crates/server/src/routes/mod.rs`, add `pub mod grep;` after `pub mod facets;` (line ~7).

In the `api_routes()` function, add before the `.with_state(state)` line (~125):

```rust
        .nest("/api", grep::router())
```

Add to the doc comment at the top (~89):

```rust
/// - GET /api/grep?pattern=...&case_sensitive=...&whole_word=...&limit=...&project=... — Regex grep search
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/server/src/routes/grep.rs crates/server/src/routes/mod.rs
git commit -m "feat(search): add GET /api/grep endpoint for regex search"
```

---

## Task 5: Add frontend grep types and `useGrep` hook

**Files:**
- Modify: `src/types/generated/index.ts` (add grep type exports)
- Create: `src/hooks/use-grep.ts`

**Step 1: Generate TypeScript types**

Run: `cargo test -p claude-view-search -- --ignored ts_export 2>/dev/null; true`

If ts-rs auto-generation doesn't produce the files, manually create them:

Create `src/types/generated/GrepResponse.ts`:

```typescript
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { GrepSessionHit } from "./GrepSessionHit";

export type GrepResponse = {
  pattern: string
  totalMatches: number
  totalSessions: number
  elapsedMs: number
  truncated: boolean
  results: GrepSessionHit[]
}
```

Create `src/types/generated/GrepSessionHit.ts`:

```typescript
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { GrepLineMatch } from "./GrepLineMatch";

export type GrepSessionHit = {
  sessionId: string
  project: string
  projectPath: string
  modifiedAt: number
  matches: GrepLineMatch[]
}
```

Create `src/types/generated/GrepLineMatch.ts`:

```typescript
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
export type GrepLineMatch = {
  lineNumber: number
  content: string
  matchStart: number
  matchEnd: number
}
```

**Step 2: Add exports to index.ts**

In `src/types/generated/index.ts`, add at the bottom:

```typescript
// Grep search types (Phase 2)
export type { GrepResponse } from './GrepResponse'
export type { GrepSessionHit } from './GrepSessionHit'
export type { GrepLineMatch } from './GrepLineMatch'
```

**Step 3: Create the useGrep hook**

Create `src/hooks/use-grep.ts`:

```typescript
import { useQuery } from '@tanstack/react-query'
import { useState, useEffect } from 'react'
import type { GrepResponse } from '../types/generated'

interface UseGrepOptions {
  caseSensitive?: boolean
  wholeWord?: boolean
  limit?: number
  project?: string
  enabled?: boolean
}

export function useGrep(pattern: string, options: UseGrepOptions = {}) {
  const {
    caseSensitive = false,
    wholeWord = false,
    limit = 500,
    project,
    enabled = true,
  } = options

  // Debounce pattern by 300ms (slightly longer than Tantivy — regex compile + full scan)
  const [debouncedPattern, setDebouncedPattern] = useState(pattern)

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedPattern(pattern), 300)
    return () => clearTimeout(timer)
  }, [pattern])

  const queryResult = useQuery<GrepResponse>({
    queryKey: ['grep', debouncedPattern, caseSensitive, wholeWord, limit, project],
    queryFn: async () => {
      const params = new URLSearchParams()
      params.set('pattern', debouncedPattern)
      if (caseSensitive) params.set('case_sensitive', 'true')
      if (wholeWord) params.set('whole_word', 'true')
      params.set('limit', String(limit))
      if (project) params.set('project', project)

      const res = await fetch(`/api/grep?${params}`)
      if (!res.ok) {
        const text = await res.text()
        throw new Error(text || `Grep failed: ${res.statusText}`)
      }
      return res.json()
    },
    enabled: enabled && debouncedPattern.trim().length > 0,
    staleTime: 30_000,
    gcTime: 5 * 60_000,
  })

  return {
    ...queryResult,
    debouncedPattern,
    isDebouncing: pattern !== debouncedPattern,
  }
}
```

**Step 4: Verify frontend compiles**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && bun run build`
Expected: No TypeScript errors.

**Step 5: Commit**

```bash
git add src/types/generated/GrepResponse.ts src/types/generated/GrepSessionHit.ts \
  src/types/generated/GrepLineMatch.ts src/types/generated/index.ts \
  src/hooks/use-grep.ts
git commit -m "feat(search): add useGrep hook and GrepResponse types"
```

---

## Task 6: Build GrepResults tree view component

VSCode-style collapsible tree view for grep results: project > session > line hits.

**Files:**
- Create: `src/components/GrepResults.tsx`

**Step 1: Create the component**

Create `src/components/GrepResults.tsx`:

```tsx
import { useState, useMemo } from 'react'
import { ChevronDown, ChevronRight, FileText } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import type { GrepResponse, GrepSessionHit, GrepLineMatch } from '../types/generated'

interface GrepResultsProps {
  data: GrepResponse
}

export function GrepResults({ data }: GrepResultsProps) {
  // Group results by project
  const grouped = useMemo(() => {
    const map = new Map<string, GrepSessionHit[]>()
    for (const hit of data.results) {
      const existing = map.get(hit.project) ?? []
      existing.push(hit)
      map.set(hit.project, existing)
    }
    return Array.from(map.entries()).map(([project, sessions]) => ({
      project,
      sessions,
      totalMatches: sessions.reduce((sum, s) => sum + s.matches.length, 0),
    }))
  }, [data.results])

  return (
    <div className="flex flex-col gap-1">
      {/* Summary */}
      <div className="text-xs text-gray-500 dark:text-gray-400 px-2 py-1">
        {data.totalMatches.toLocaleString()} results in{' '}
        {data.totalSessions.toLocaleString()} sessions ({data.elapsedMs.toFixed(0)}ms)
        {data.truncated && (
          <span className="ml-1 text-amber-600 dark:text-amber-400">(truncated)</span>
        )}
      </div>

      {/* Project groups */}
      {grouped.map((group) => (
        <ProjectGroup key={group.project} {...group} />
      ))}
    </div>
  )
}

function ProjectGroup({
  project,
  sessions,
  totalMatches,
}: {
  project: string
  sessions: GrepSessionHit[]
  totalMatches: number
}) {
  const [expanded, setExpanded] = useState(true)

  return (
    <div>
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 w-full px-2 py-1 text-left text-sm font-medium text-gray-800 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 rounded transition-colors"
      >
        {expanded ? (
          <ChevronDown className="w-4 h-4 shrink-0" />
        ) : (
          <ChevronRight className="w-4 h-4 shrink-0" />
        )}
        <span className="truncate">{project}</span>
        <span className="ml-auto text-xs text-gray-400 dark:text-gray-500 shrink-0">
          {totalMatches} results
        </span>
      </button>

      {expanded && (
        <div className="ml-4">
          {sessions.map((session) => (
            <SessionGroup key={session.sessionId} session={session} />
          ))}
        </div>
      )}
    </div>
  )
}

function SessionGroup({ session }: { session: GrepSessionHit }) {
  const [expanded, setExpanded] = useState(false)
  const navigate = useNavigate()

  const dateStr = useMemo(() => {
    if (session.modifiedAt <= 0) return ''
    const d = new Date(session.modifiedAt * 1000)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }, [session.modifiedAt])

  const displayMatches = expanded ? session.matches : session.matches.slice(0, 3)

  return (
    <div>
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 w-full px-2 py-0.5 text-left text-xs text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 rounded transition-colors"
      >
        {expanded ? (
          <ChevronDown className="w-3 h-3 shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 shrink-0" />
        )}
        <FileText className="w-3 h-3 shrink-0 text-gray-400" />
        <span className="truncate font-mono">
          {session.sessionId.slice(0, 8)}
        </span>
        <span className="ml-1 text-gray-400">{dateStr}</span>
        <span className="ml-auto text-gray-400 shrink-0">
          ({session.matches.length} results)
        </span>
      </button>

      <div className="ml-6">
        {displayMatches.map((match, i) => (
          <LineMatch
            key={`${match.lineNumber}-${i}`}
            match={match}
            sessionId={session.sessionId}
            onClick={() => navigate(`/sessions/${session.sessionId}`)}
          />
        ))}
        {!expanded && session.matches.length > 3 && (
          <button
            onClick={() => setExpanded(true)}
            className="px-2 py-0.5 text-xs text-blue-600 dark:text-blue-400 hover:underline"
          >
            {session.matches.length - 3} more results...
          </button>
        )}
      </div>
    </div>
  )
}

function LineMatch({
  match,
  sessionId,
  onClick,
}: {
  match: GrepLineMatch
  sessionId: string
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className="flex items-start gap-2 w-full px-2 py-0.5 text-left text-xs hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded transition-colors group"
    >
      <span className="text-gray-400 shrink-0 font-mono tabular-nums w-10 text-right">
        L{match.lineNumber}
      </span>
      <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
        <HighlightedContent
          content={match.content}
          matchStart={match.matchStart}
          matchEnd={match.matchEnd}
        />
      </span>
    </button>
  )
}

function HighlightedContent({
  content,
  matchStart,
  matchEnd,
}: {
  content: string
  matchStart: number
  matchEnd: number
}) {
  if (matchStart >= matchEnd || matchStart >= content.length) {
    return <>{content}</>
  }

  const before = content.slice(0, matchStart)
  const matched = content.slice(matchStart, matchEnd)
  const after = content.slice(matchEnd)

  return (
    <>
      {before}
      <mark className="bg-yellow-200 dark:bg-yellow-800/60 text-inherit rounded-sm px-0.5">
        {matched}
      </mark>
      {after}
    </>
  )
}
```

**Step 2: Verify frontend compiles**

Run: `bun run build`
Expected: No TypeScript errors.

**Step 3: Commit**

```bash
git add src/components/GrepResults.tsx
git commit -m "feat(search): add GrepResults tree view component"
```

---

## Task 7: Add regex toggle to CommandPalette and wire grep mode

Add VSCode-style `[Aa]` `[Ab]` `[.*]` toggle buttons to the CommandPalette search input. When `.*` is active, switch from Tantivy search to grep.

**Files:**
- Modify: `src/components/CommandPalette.tsx`
- Modify: `src/components/SearchResults.tsx`

**Step 1: Read current CommandPalette and SearchResults implementations**

Read `src/components/CommandPalette.tsx` and `src/components/SearchResults.tsx` in full to understand current structure. The changes must:

1. Add three toggle buttons (`[Aa]` case, `[Ab]` word, `[.*]` regex) next to the search input
2. Store toggle state: `isRegexMode`, `isCaseSensitive`, `isWholeWord` (local state, not Zustand — session-only per VSCode behavior)
3. When regex mode ON: call `useGrep` instead of `useSearch`
4. When regex mode ON: render `<GrepResults>` instead of `<SearchResultCard>` list
5. Pass `isCaseSensitive` and `isWholeWord` to `useGrep` options

**Step 2: Add toggle buttons and mode switching to CommandPalette**

Add state variables near the top of the CommandPalette component:

Add after `const [selectedIndex, setSelectedIndex] = useState(0)` (line 64 of `CommandPalette.tsx`):

```tsx
const [isRegexMode, setIsRegexMode] = useState(false)
const [isCaseSensitive, setIsCaseSensitive] = useState(false)
const [isWholeWord, setIsWholeWord] = useState(false)
```

Add the `useGrep` import and hook call (**unconditionally** — `enabled` gates the query, per React's rules of hooks):

```tsx
import { useGrep } from '../hooks/use-grep'
import { GrepResults } from './GrepResults'

// Inside component, after useSearch call (line 70) (MUST be unconditional — React rules of hooks):
// The `enabled` param prevents the actual fetch when regex mode is off.
// NOTE: Both useSearch and useGrep have internal debounce timers (200ms and 300ms respectively).
// When regex mode is off, useGrep's enabled=false prevents the fetch but the debounce timer
// still runs (React rules of hooks). This is functionally harmless — two setTimeout calls
// with no network request. Acceptable for MVP.
const grepResult = useGrep(query, {
  caseSensitive: isCaseSensitive,
  wholeWord: isWholeWord,
  enabled: isRegexMode && isOpen,
})
```

Add toggle buttons inside the search input container, **between the `<input>` and the `{showLoading && <Loader2>}` spinner** (NOT after the X close button — that would break the visual layout):

```tsx
<div className="flex items-center gap-0.5 mr-2">
  <ToggleButton
    active={isCaseSensitive}
    onClick={() => setIsCaseSensitive(!isCaseSensitive)}
    title="Match Case"
    label="Aa"
  />
  <ToggleButton
    active={isWholeWord}
    onClick={() => setIsWholeWord(!isWholeWord)}
    title="Match Whole Word"
    label="Ab"
  />
  <ToggleButton
    active={isRegexMode}
    onClick={() => setIsRegexMode(!isRegexMode)}
    title="Use Regular Expression"
    label=".*"
  />
</div>
```

Add the `ToggleButton` helper component at the bottom of the file:

```tsx
function ToggleButton({
  active,
  onClick,
  title,
  label,
}: {
  active: boolean
  onClick: () => void
  title: string
  label: string
}) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation()
        onClick()
      }}
      title={title}
      className={`px-1.5 py-0.5 text-xs font-mono rounded border transition-colors ${
        active
          ? 'bg-blue-100 dark:bg-blue-900/40 border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300'
          : 'bg-transparent border-transparent text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
      }`}
    >
      {label}
    </button>
  )
}
```

In the results rendering section (lines 441–469 of `CommandPalette.tsx`), replace the entire `{hasLiveResults && (...)}` block with a conditional that renders either grep results or the existing Tantivy results.

**Current code to replace (lines 441–469):**
```tsx
{hasLiveResults && (
  <div className="py-2 border-b border-slate-200/80 dark:border-white/[0.06]">
    <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider">
      {searchResults.totalSessions} {searchResults.totalSessions === 1 ? 'session' : 'sessions'}, {searchResults.totalMatches} {searchResults.totalMatches === 1 ? 'match' : 'matches'}
      <span className="ml-2 normal-case tracking-normal">({searchResults.elapsedMs}ms)</span>
    </p>
    <div className="px-3 py-1 space-y-1">
      {searchResults.sessions.map((hit, i) => {
        const itemIndex = searchResultsStartIndex + i
        return (
          <SearchResultCard
            key={hit.sessionId}
            hit={hit}
            isSelected={selectedIndex === itemIndex}
            onSelect={() => handleSelectSearchResult(hit.sessionId)}
          />
        )
      })}
    </div>
    {searchResults.totalSessions > searchResults.sessions.length && (
      <button
        onClick={() => handleSelect(query.trim())}
        className="w-full px-4 py-2 text-xs text-emerald-600 dark:text-emerald-400 hover:text-emerald-500 dark:hover:text-emerald-300 transition-colors text-center"
      >
        View all {searchResults.totalSessions} results
      </button>
    )}
  </div>
)}
```

**Replace with:**
```tsx
{isRegexMode ? (
  grepResult.data ? (
    <GrepResults data={grepResult.data} />
  ) : grepResult.isLoading ? (
    <div className="p-4 text-center text-gray-500">Searching...</div>
  ) : grepResult.error ? (
    <div className="p-4 text-center text-red-500">
      {grepResult.error instanceof Error ? grepResult.error.message : 'Grep failed'}
    </div>
  ) : null
) : (
  hasLiveResults && (
    <div className="py-2 border-b border-slate-200/80 dark:border-white/[0.06]">
      <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider">
        {searchResults.totalSessions} {searchResults.totalSessions === 1 ? 'session' : 'sessions'}, {searchResults.totalMatches} {searchResults.totalMatches === 1 ? 'match' : 'matches'}
        <span className="ml-2 normal-case tracking-normal">({searchResults.elapsedMs}ms)</span>
      </p>
      <div className="px-3 py-1 space-y-1">
        {searchResults.sessions.map((hit, i) => {
          const itemIndex = searchResultsStartIndex + i
          return (
            <SearchResultCard
              key={hit.sessionId}
              hit={hit}
              isSelected={selectedIndex === itemIndex}
              onSelect={() => handleSelectSearchResult(hit.sessionId)}
            />
          )
        })}
      </div>
      {searchResults.totalSessions > searchResults.sessions.length && (
        <button
          onClick={() => handleSelect(query.trim())}
          className="w-full px-4 py-2 text-xs text-emerald-600 dark:text-emerald-400 hover:text-emerald-500 dark:hover:text-emerald-300 transition-colors text-center"
        >
          View all {searchResults.totalSessions} results
        </button>
      )}
    </div>
  )
)}
```

**Important keyboard nav fix:** When `isRegexMode` is true, the existing `totalItems` and `searchSessionCount` will be 0 (since `useSearch` returns no results when grep is active). This means ArrowUp/ArrowDown keyboard nav will not work for grep results. For MVP, this is acceptable — grep results use click navigation. If keyboard nav is needed later, add a `grepItemCount` to `totalItems` when regex mode is on.

**Step 3: Add same toggles to SearchResults page and propagate URL params**

In `src/components/SearchResults.tsx`, add the same toggle state and conditional rendering. When regex mode is active on the search results page, use `useGrep` and render `<GrepResults>`.

**URL param propagation:** Replace the `handleSelect` callback in `CommandPalette.tsx` (lines 204–218) with this version that adds toggle state to the search URL. Note: the callback parameter is `searchQuery` (different from the component state `query` — this is the trimmed text passed by callers like the "View all" button).

**Current code (lines 204–218):**
```tsx
const handleSelect = useCallback((searchQuery: string) => {
  addRecentSearch(searchQuery)
  onClose()
  const searchUrl = new URLSearchParams()
  searchUrl.set('q', searchQuery)
  const currentParams = new URLSearchParams(location.search)
  const project = currentParams.get('project')
  const branch = currentParams.get('branch')
  const scopeParts: string[] = []
  if (project) scopeParts.push(`project:${project}`)
  if (branch) scopeParts.push(`branch:${branch}`)
  if (scopeParts.length > 0) searchUrl.set('scope', scopeParts.join(' '))
  navigate(`/search?${searchUrl}`)
}, [addRecentSearch, onClose, navigate, location.search])
```

**Replace with:**
```tsx
const handleSelect = useCallback((searchQuery: string) => {
  addRecentSearch(searchQuery)
  onClose()
  const searchUrl = new URLSearchParams()
  searchUrl.set('q', searchQuery)
  const currentParams = new URLSearchParams(location.search)
  const project = currentParams.get('project')
  const branch = currentParams.get('branch')
  const scopeParts: string[] = []
  if (project) scopeParts.push(`project:${project}`)
  if (branch) scopeParts.push(`branch:${branch}`)
  if (scopeParts.length > 0) searchUrl.set('scope', scopeParts.join(' '))
  // Propagate toggle state to SearchResults page
  if (isRegexMode) searchUrl.set('regex', '1')
  if (isCaseSensitive) searchUrl.set('cs', '1')
  if (isWholeWord) searchUrl.set('ww', '1')
  navigate(`/search?${searchUrl}`)
}, [addRecentSearch, onClose, navigate, location.search, isRegexMode, isCaseSensitive, isWholeWord])
```

In `SearchResults.tsx`, read these params back:

```tsx
const isRegexMode = searchParams.get('regex') === '1'
const isCaseSensitive = searchParams.get('cs') === '1'
const isWholeWord = searchParams.get('ww') === '1'
```

**Step 4: Verify frontend compiles and renders**

Run: `bun run build`
Expected: No TypeScript errors.

**Step 5: Manual test**

1. Start the server: `cargo run -p claude-view-server`
2. Open http://localhost:47892
3. Press Cmd+K → see three toggle buttons next to input
4. Click `.*` toggle → it highlights blue
5. Type `auth.*middleware` → grep results appear as tree view
6. Click `.*` toggle off → Tantivy results appear (existing behavior)

**Step 6: Commit**

```bash
git add src/components/CommandPalette.tsx src/components/SearchResults.tsx
git commit -m "feat(search): add regex toggle and grep mode to search UI"
```

---

## Task 8: End-to-end test — regex grep

**Step 1: Build and start server**

Run: `cargo build -p claude-view-server && cargo run -p claude-view-server`

**Step 2: Test API directly**

```bash
curl -s 'http://localhost:47892/api/grep?pattern=brainstorming' | jq '.totalMatches, .totalSessions, .elapsedMs'
```

Expected: Matches across multiple sessions, < 200ms.

**Step 3: Test regex pattern**

```bash
curl -s 'http://localhost:47892/api/grep?pattern=auth.*middleware' | jq '.totalMatches, .totalSessions'
```

Expected: Non-zero matches if any sessions contain that pattern.

**Step 4: Test case sensitivity**

```bash
curl -s 'http://localhost:47892/api/grep?pattern=TODO&case_sensitive=true' | jq '.totalMatches'
curl -s 'http://localhost:47892/api/grep?pattern=TODO&case_sensitive=false' | jq '.totalMatches'
```

Expected: Case-insensitive returns more matches.

**Step 5: Test in browser**

1. Open http://localhost:47892
2. Press Cmd+K
3. Toggle `.*` on
4. Type `brainstorming` → see tree view with project > session > line results
5. Click a line → navigates to session detail
6. Toggle `.*` off → Tantivy results reappear

**Step 6: Commit test verification**

```bash
git add -A && git commit -m "test: verify regex grep search end-to-end"
```

---

## Task 9: Add `after:`/`before:` date qualifiers to Tantivy

**Files:**
- Modify: `crates/search/src/query.rs:59-84` (qualifier parsing)
- Modify: `crates/search/src/query.rs:309-344` (qualifier-to-query conversion)
- Test: `crates/search/src/lib.rs` (add test)

**Step 1: Write a failing test**

In `crates/search/src/lib.rs`, add in the `mod tests` block:

```rust
#[test]
fn test_search_after_before_date_qualifiers() {
    let idx = SearchIndex::open_in_ram().expect("create index");

    let docs = vec![
        SearchDocument {
            session_id: "sess-old".to_string(),
            project: "test".to_string(),
            branch: String::new(),
            model: String::new(),
            role: "user".to_string(),
            content: "brainstorming ideas for the startup".to_string(),
            turn_number: 1,
            timestamp: 1704067200, // 2024-01-01
            skills: vec![],
        },
        SearchDocument {
            session_id: "sess-new".to_string(),
            project: "test".to_string(),
            branch: String::new(),
            model: String::new(),
            role: "user".to_string(),
            content: "brainstorming ideas for the product".to_string(),
            turn_number: 1,
            timestamp: 1740787200, // 2025-02-28 (approximately)
            skills: vec![],
        },
    ];

    idx.index_session("sess-old", &docs[..1]).expect("index old");
    idx.index_session("sess-new", &docs[1..]).expect("index new");
    idx.commit().expect("commit");
    idx.reader.reload().expect("reload");

    // after: should exclude old session
    let r1 = idx.search("brainstorming after:2025-01-01", None, 10, 0).expect("after");
    assert_eq!(r1.total_sessions, 1, "after: should find only the new session");
    assert_eq!(r1.sessions[0].session_id, "sess-new");

    // before: should exclude new session
    let r2 = idx.search("brainstorming before:2025-01-01", None, 10, 0).expect("before");
    assert_eq!(r2.total_sessions, 1, "before: should find only the old session");
    assert_eq!(r2.sessions[0].session_id, "sess-old");

    // Combined: narrow window that includes neither
    let r3 = idx.search("brainstorming after:2024-06-01 before:2024-07-01", None, 10, 0).expect("range");
    assert_eq!(r3.total_sessions, 0, "narrow range should find nothing");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-search test_search_after_before_date_qualifiers`
Expected: FAIL — `after:` and `before:` are not recognized as qualifiers.

**Step 3: Extend qualifier parsing**

In `crates/search/src/query.rs`, modify `parse_query_string()`:

Change the `known_keys` array (line 63) to include date qualifiers:

```rust
let known_keys = ["project", "branch", "model", "role", "skill", "after", "before"];
```

**Step 4: Add date qualifier handling to the query builder**

In `crates/search/src/query.rs`, after the qualifier term queries loop (after line 344, before the empty check at line 347), add:

```rust
        // Date range qualifiers: after: and before: create RangeQuery on timestamp.
        // NOTE: This is a SEPARATE loop from the qualifier term queries above — do NOT
        // merge into the first loop's `_ => {}` arm. The first loop handles text/string
        // qualifiers; this one handles date range qualifiers.
        for qual in &qualifiers {
            match qual.key.as_str() {
                "after" | "before" => {
                    // Parse date string to unix timestamp
                    let ts = match NaiveDate::parse_from_str(&qual.value, "%Y-%m-%d") {
                        Ok(date) => {
                            let datetime = date
                                .and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_utc()
                                .timestamp();
                            datetime
                        }
                        Err(_) => continue, // Invalid date format — skip silently
                    };

                    let range_query = if qual.key == "after" {
                        RangeQuery::new_i64_bounds(
                            "timestamp".to_string(),
                            std::ops::Bound::Excluded(ts),
                            std::ops::Bound::Unbounded,
                        )
                    } else {
                        // before: exclusive upper bound
                        RangeQuery::new_i64_bounds(
                            "timestamp".to_string(),
                            std::ops::Bound::Unbounded,
                            std::ops::Bound::Excluded(ts),
                        )
                    };
                    sub_queries.push((Occur::Must, Box::new(range_query)));
                }
                _ => {} // Other qualifiers already handled above
            }
        }
```

**Required imports — add these to the top of `query.rs`:**

1. Add `RangeQuery` to the existing tantivy import (line 5). Change:
   ```rust
   use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, TermQuery};
   ```
   to:
   ```rust
   use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, RangeQuery, TermQuery};
   ```

2. Add `chrono::NaiveDate` import after the existing imports (after line 13):
   ```rust
   use chrono::NaiveDate;
   ```
   `chrono` IS already a dependency of `claude-view-search` (Cargo.toml line 17) but is NOT currently imported in `query.rs`.

**Step 5: Run test to verify it passes**

Run: `cargo test -p claude-view-search test_search_after_before_date_qualifiers`
Expected: PASS.

**Step 6: Run all search tests**

Run: `cargo test -p claude-view-search`
Expected: All pass. No regressions.

**Step 7: Commit**

```bash
git add crates/search/src/query.rs crates/search/src/lib.rs
git commit -m "feat(search): add after: and before: date range qualifiers"
```

---

## Task 10: Create `useInSessionSearch` hook

Client-side in-session search (Ctrl+F on session detail page). Messages are already in React state — no API call needed.

**Files:**
- Create: `src/hooks/use-in-session-search.ts`

**Step 1: Create the hook**

Create `src/hooks/use-in-session-search.ts`:

```typescript
import { useState, useEffect, useMemo, useCallback, useRef } from 'react'
import type { Message } from '../types/generated'

interface InSessionSearchResult {
  /** Index of the message in the messages array */
  messageIndex: number
  /** Character offset of match start within the message content */
  matchStart: number
  /** Character offset of match end */
  matchEnd: number
}

interface UseInSessionSearchReturn {
  /** Current search query */
  query: string
  /** Set the search query */
  setQuery: (q: string) => void
  /** All matches */
  matches: InSessionSearchResult[]
  /** Currently focused match index (0-based) */
  activeIndex: number
  /** Total number of matches */
  totalCount: number
  /** Navigate to next match */
  next: () => void
  /** Navigate to previous match */
  prev: () => void
  /** Clear search and reset state */
  clear: () => void
  /** Whether search bar is visible */
  isOpen: boolean
  /** Open the search bar */
  open: () => void
  /** Close the search bar */
  close: () => void
}

export function useInSessionSearch(messages: Message[]): UseInSessionSearchReturn {
  const [query, setQuery] = useState('')
  const [activeIndex, setActiveIndex] = useState(0)
  const [isOpen, setIsOpen] = useState(false)

  // Debounce query by 150ms
  const [debouncedQuery, setDebouncedQuery] = useState('')
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), 150)
    return () => clearTimeout(timer)
  }, [query])

  // Find all matches across messages
  const matches = useMemo(() => {
    if (!debouncedQuery.trim()) return []

    const results: InSessionSearchResult[] = []
    const needle = debouncedQuery.toLowerCase()

    for (let i = 0; i < messages.length; i++) {
      const content = messages[i].content?.toLowerCase() ?? ''
      let startPos = 0
      while (startPos < content.length) {
        const idx = content.indexOf(needle, startPos)
        if (idx === -1) break
        results.push({
          messageIndex: i,
          matchStart: idx,
          matchEnd: idx + needle.length,
        })
        startPos = idx + 1
      }
    }

    return results
  }, [messages, debouncedQuery])

  // Reset active index when matches change
  useEffect(() => {
    setActiveIndex(0)
  }, [matches.length])

  const next = useCallback(() => {
    if (matches.length === 0) return
    setActiveIndex((prev) => (prev + 1) % matches.length)
  }, [matches.length])

  const prev = useCallback(() => {
    if (matches.length === 0) return
    setActiveIndex((prev) => (prev - 1 + matches.length) % matches.length)
  }, [matches.length])

  const clear = useCallback(() => {
    setQuery('')
    setActiveIndex(0)
  }, [])

  const open = useCallback(() => {
    setIsOpen(true)
  }, [])

  const close = useCallback(() => {
    setIsOpen(false)
    clear()
  }, [clear])

  return {
    query,
    setQuery,
    matches,
    activeIndex,
    totalCount: matches.length,
    next,
    prev,
    clear,
    isOpen,
    open,
    close,
  }
}
```

**Step 2: Verify frontend compiles**

Run: `bun run build`
Expected: No TypeScript errors.

**Step 3: Commit**

```bash
git add src/hooks/use-in-session-search.ts
git commit -m "feat(search): add useInSessionSearch hook for in-session Ctrl+F"
```

---

## Task 11: Wire in-session search into ConversationView

Add Ctrl+F handler and mini search bar to session detail page.

**Files:**
- Modify: `src/components/ConversationView.tsx`

**Critical constraints identified from codebase audit:**
- The component uses `react-virtuoso` `<Virtuoso>` for message rendering — DOM elements outside the viewport do NOT exist, so `document.getElementById()` fails for off-screen messages
- Virtuoso's `firstItemIndex` starts at `FIRST_ITEM_START - filteredMessages.length` (~999900), NOT 0 — element IDs based on Virtuoso index don't match 0-based array positions
- The component has 4 early-return branches (lines 350, 370, 381, 394) — hooks MUST be declared before all of them (React rules of hooks)
- There is no single `messages` variable — use `filteredMessages` (what Virtuoso renders in compact mode)
- There is NO existing `VirtuosoHandle` ref — one must be added

**Step 1: Read ConversationView.tsx in full**

Read the entire file to understand its structure before modifying.

**Step 2: Add imports**

At the top of `ConversationView.tsx`, add:

```tsx
import { useInSessionSearch } from '../hooks/use-in-session-search'
import type { VirtuosoHandle } from 'react-virtuoso'
```

**Step 3: Add hook and Virtuoso ref — BEFORE early returns**

The hook depends on `filteredMessages` (defined at line 259) and MUST be placed BEFORE the early-return branches at lines 350/370/381/394. The valid placement window is **lines 263–348**. Add immediately after `hiddenCount` (line 263):

```tsx
// In-session search — placed after filteredMessages is defined (line 259)
// but before any early returns (lines 350+) to satisfy React rules of hooks
const inSessionSearch = useInSessionSearch(filteredMessages)
const virtuosoRef = useRef<VirtuosoHandle>(null)
```

**NOTE:** `useRef` has no data dependency and could go anywhere before early returns, but placing it next to `inSessionSearch` keeps the in-session search declarations together.

**Step 4: Add Ctrl+F handler to existing keydown useEffect**

The existing `handleKeyDown` is inside a `useEffect` at lines 200-219. Add Ctrl+F and Escape handlers inside the `handleKeyDown` function body, and add `inSessionSearch` to the dependency array:

```tsx
useEffect(() => {
  const handleKeyDown = (e: KeyboardEvent) => {
    const modifierKey = e.metaKey || e.ctrlKey

    // Ctrl+F / Cmd+F: open in-session search
    if (modifierKey && e.key.toLowerCase() === 'f') {
      e.preventDefault()
      inSessionSearch.open()
      return
    }

    // Escape: close in-session search if open
    if (e.key === 'Escape' && inSessionSearch.isOpen) {
      e.preventDefault()
      inSessionSearch.close()
      return
    }

    if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'e') {
      e.preventDefault()
      handleExportHtml()
    } else if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'p') {
      e.preventDefault()
      handleExportPdf()
    } else if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'r') {
      e.preventDefault()
      handleResume()
    }
  }

  window.addEventListener('keydown', handleKeyDown)
  return () => window.removeEventListener('keydown', handleKeyDown)
}, [handleExportHtml, handleExportPdf, handleResume, inSessionSearch])
```

**Step 5: Wire Virtuoso ref**

Find the existing `<Virtuoso` component (~line 618) and add the `ref` prop:

```tsx
<Virtuoso
  ref={virtuosoRef}
  data={filteredMessages}
  firstItemIndex={firstItemIndex}
  // ... rest of existing props unchanged
```

**Step 6: Render mini search bar**

Insert the search bar **inside the main return's flex-col container**, after the header `</div>` (line ~609) and before the two-column `<div className="flex-1 flex overflow-hidden">` (line ~612). Do NOT add it to early-return branches:

```tsx
{inSessionSearch.isOpen && (
  <div className="flex items-center gap-2 px-4 py-2 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 shadow-sm">
    <input
      type="text"
      value={inSessionSearch.query}
      onChange={(e) => inSessionSearch.setQuery(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault()
          inSessionSearch.next()
        } else if (e.key === 'Enter' && e.shiftKey) {
          e.preventDefault()
          inSessionSearch.prev()
        } else if (e.key === 'Escape') {
          e.preventDefault()
          inSessionSearch.close()
        }
      }}
      placeholder="Find in conversation..."
      className="flex-1 px-3 py-1 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded outline-none focus:border-blue-500 dark:focus:border-blue-400 text-gray-900 dark:text-gray-100"
      autoFocus
    />
    <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums min-w-[4rem] text-center">
      {inSessionSearch.totalCount > 0
        ? `${inSessionSearch.activeIndex + 1}/${inSessionSearch.totalCount}`
        : 'No results'}
    </span>
    <button
      onClick={inSessionSearch.prev}
      disabled={inSessionSearch.totalCount === 0}
      className="p-1 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 disabled:opacity-30"
      title="Previous (Shift+Enter)"
    >
      ▲
    </button>
    <button
      onClick={inSessionSearch.next}
      disabled={inSessionSearch.totalCount === 0}
      className="p-1 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 disabled:opacity-30"
      title="Next (Enter)"
    >
      ▼
    </button>
    <button
      onClick={inSessionSearch.close}
      className="p-1 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
      title="Close (Escape)"
    >
      ✕
    </button>
  </div>
)}
```

**Step 7: Scroll to active match using Virtuoso API**

Use Virtuoso's `scrollToIndex` API instead of `document.getElementById()` — Virtuoso virtualizes the DOM, so off-screen elements don't exist in the DOM tree.

**IMPORTANT:** Because `ConversationView` sets `firstItemIndex` (starting at ~999900 for reverse infinite scroll), `scrollToIndex` expects the **virtual index**, NOT a 0-based array index. The virtual index = `firstItemIndex + arrayIndex`. Components without `firstItemIndex` (like `RichPane`, `ActionLogTab`) can pass raw 0-based indices, but `ConversationView` cannot.

```tsx
useEffect(() => {
  if (inSessionSearch.totalCount === 0) return
  const match = inSessionSearch.matches[inSessionSearch.activeIndex]
  if (!match) return
  // scrollToIndex needs the VIRTUAL index because ConversationView uses firstItemIndex.
  // Virtual index = firstItemIndex + 0-based array position.
  virtuosoRef.current?.scrollToIndex({
    index: firstItemIndex + match.messageIndex,
    behavior: 'smooth',
    align: 'center',
  })
}, [inSessionSearch.activeIndex, inSessionSearch.matches, inSessionSearch.totalCount, firstItemIndex])
```

**Step 8: Add match highlighting to messages**

In the Virtuoso `itemContent` callback, use the 0-based array index (derived from Virtuoso's index minus `firstItemIndex`) to check for matches. Inside the `itemContent` callback (~line 625):

```tsx
itemContent={(index, message) => {
  // Convert Virtuoso index to 0-based array position
  const arrayIndex = index - firstItemIndex
  const hasMatch = inSessionSearch.matches.some(m => m.messageIndex === arrayIndex)
  const isActiveMatch = inSessionSearch.matches[inSessionSearch.activeIndex]?.messageIndex === arrayIndex

  return (
    <div className={`max-w-4xl mx-auto px-6 pb-4 ${
      isActiveMatch
        ? 'bg-yellow-100 dark:bg-yellow-900/20 ring-1 ring-yellow-300 dark:ring-yellow-700 rounded'
        : hasMatch
          ? 'bg-yellow-50 dark:bg-yellow-900/10 rounded'
          : ''
    }`}>
      <ErrorBoundary key={message.uuid || index}>
        <MessageTyped
          message={message}
          messageIndex={index}
          // ... rest of existing props
        />
      </ErrorBoundary>
    </div>
  )
}}
```

**Step 9: Verify frontend compiles**

Run: `bun run build`
Expected: No TypeScript errors.

**Step 10: Manual test**

1. Navigate to a session detail page
2. Press Ctrl+F → search bar appears between header and messages
3. Type "auth" → matching messages get yellow background, counter shows `1/5`
4. Press Enter → Virtuoso scrolls to next matching message smoothly
5. Press Shift+Enter → scrolls to previous
6. Press Escape → search bar closes, highlights removed

**Step 11: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "feat(search): add in-session Ctrl+F search with highlight and scroll"
```

---

## Task 12: Bump schema version and final verification

**Files:**
- Modify: `crates/search/src/lib.rs:33`

**Step 1: Bump SEARCH_SCHEMA_VERSION**

In `crates/search/src/lib.rs`, change line 33:

```rust
// Current:
pub const SEARCH_SCHEMA_VERSION: u32 = 6;

// Change to:
pub const SEARCH_SCHEMA_VERSION: u32 = 7;
// Version 7: Phase 2 — after:/before: date qualifiers
```

**Note:** The grep feature doesn't require a schema version bump (it reads raw JSONL, not the Tantivy index). Only the date qualifier feature benefits from a re-index if timestamp data was previously missing.

**Step 2: Run all search tests**

Run: `cargo test -p claude-view-search`
Expected: All pass.

**Step 3: Run server tests**

Run: `cargo test -p claude-view-server`
Expected: All pass.

**Step 4: Full build check**

Run: `cargo build -p claude-view-server && bun run build`
Expected: Both succeed.

**Step 5: End-to-end manual test**

1. Start server: `cargo run -p claude-view-server`
2. Wait for re-index (schema bump triggers it)
3. Cmd+K → type `brainstorming` → Tantivy results appear
4. Toggle `.*` → grep results appear as tree view
5. Toggle `.*` off → Tantivy results reappear
6. Type `brainstorming after:2026-02-01` → only recent sessions
7. Navigate to a session → press Ctrl+F → in-session search works
8. All three features confirmed working

**Step 6: Commit**

```bash
git add crates/search/src/lib.rs
git commit -m "chore(search): bump schema version to 7 for Phase 2 date qualifiers"
```

---

## Summary of All Tasks

| Task | Feature | Effort | Files |
|------|---------|--------|-------|
| 1 | Grep deps | 2 min | `Cargo.toml`, `crates/search/Cargo.toml` |
| 2 | Grep types | 5 min | `crates/search/src/grep_types.rs`, `lib.rs` |
| 3 | Grep engine | 20 min | `crates/search/src/grep.rs`, `lib.rs` |
| 4 | Grep API route | 10 min | `crates/server/src/routes/grep.rs`, `mod.rs` |
| 5 | Frontend types + hook | 10 min | `src/types/generated/*`, `src/hooks/use-grep.ts` |
| 6 | GrepResults component | 15 min | `src/components/GrepResults.tsx` |
| 7 | Regex toggle UI | 20 min | `CommandPalette.tsx`, `SearchResults.tsx` |
| 8 | E2E grep test | 10 min | Manual verification |
| 9 | Date qualifiers | 10 min | `query.rs`, `lib.rs` (test) |
| 10 | In-session hook | 10 min | `src/hooks/use-in-session-search.ts` |
| 11 | Ctrl+F UI | 15 min | `ConversationView.tsx` |
| 12 | Schema bump + verify | 5 min | `lib.rs` |

**Total: ~2.5 hours of implementation time.**

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| B1 | `RegexMatcher` per-thread rationale was factually wrong (`!Sync` claim) | Blocker | `RegexMatcher` IS Sync. Updated comments: per-thread construction avoids contention on internal mutable state, not a Sync issue. Fix (build per thread) was already correct; only the stated reason was wrong. |
| B2 | `&line_content[..500]` panics on multibyte char boundary (CJK, emoji) | Blocker | Replaced with `char_indices().nth(500)` for char-safe truncation. |
| B3 | `resolve_project_path_with_cwd("project-a", None)` returns `display_name="a"`, not `"project-a"` | Blocker | Changed test directory names from `project-a`/`project-b` to `alpha`/`beta` (no hyphens). |
| B4 | Plan used `ApiError::Internal` for `DiscoveryError` when `From<DiscoveryError>` impl exists | Blocker | Replaced manual `map_err` with `?` operator — `ApiError::Discovery` handles it automatically. |
| B5 | Plan referenced `searchQuery` variable — actual variable is `query` | Blocker | Changed `useGrep(searchQuery, ...)` to `useGrep(query, ...)`. |
| B6 | "only when regex mode is active" implies conditional hook call (violates rules of hooks) | Blocker | Changed to "unconditionally — enabled gates the query". Added `enabled: isRegexMode && isOpen`. |
| B7 | `RangeQuery` not in `tantivy::query` import list at line 5 of `query.rs` | Blocker | Added explicit instruction to add `RangeQuery` to the import. |
| B8 | `chrono::NaiveDate` used but not imported in `query.rs` — plan said "if not already present" | Blocker | Replaced vague wording with explicit: "Add `use chrono::NaiveDate;` — it is NOT currently imported in this file". |
| B9 | ConversationView has no single `messages` variable — plan referenced it | Blocker | Changed hook input to `useInSessionSearch(filteredMessages)` — the array Virtuoso actually renders. |
| B10 | `document.getElementById()` fails with Virtuoso — off-screen elements don't exist in DOM | Blocker | Replaced with `VirtuosoHandle` ref + `scrollToIndex()` API. |
| B11 | Virtuoso `firstItemIndex` starts at ~999900, not 0 — index math was wrong | Blocker | **REVISED (Round 2):** `scrollToIndex` needs the VIRTUAL index when `firstItemIndex` is set. Changed to `firstItemIndex + match.messageIndex`. Added `firstItemIndex` to useEffect deps. |
| B12 | Hook placement after early returns violates rules of hooks | Blocker | Specified placement before early returns at lines 350/370/381/394. |
| B13 | `filteredMessages` not in scope at originally stated hook placement (lines 86-131) | Blocker | **NEW (Round 2):** `filteredMessages` is defined at line 259. Updated placement to "lines 263–348" — after `filteredMessages` definition, before early returns. |
| B15 | `scrollToIndex` receives wrong index — 0-based vs virtual index | Blocker | **NEW (Round 2):** Components with `firstItemIndex` (like ConversationView) need virtual index = `firstItemIndex + arrayIndex`. Fixed code block and added explanatory comment. |
| W2 | Keyboard nav for grep results unspecified | Warning | Documented that grep results don't support keyboard nav in MVP. |
| W3 | Toggle placement ambiguous in CommandPalette | Warning | Specified "between `<input>` and spinner, NOT after X button". |
| W4 | URL param propagation missing for regex/cs/ww | Warning | Added `handleSelect` code to propagate `regex`, `cs`, `ww` params on navigation. |
| W5 | `RangeQuery` imported but code used fully-qualified `tantivy::query::RangeQuery` | Warning | **NEW (Round 2):** Changed code blocks to use short `RangeQuery::new_i64_bounds()` to match the import. |
| W6 | Task 7 CommandPalette instructions were fragment-level — high risk of merge conflicts | Warning | **NEW (Round 3):** Replaced `/* keep as-is */` placeholder with full before/after diff showing exact lines 441–469. |
| W7 | Dual debounce timers (useSearch 200ms + useGrep 300ms) running simultaneously | Warning | **NEW (Round 3):** Added inline note: both timers run but useGrep's `enabled=false` prevents network request. Harmless for MVP. |
| W8 | `handleSelect` dep array change could cause stale closures | Warning | **NEW (Round 3):** Provided full before/after diff with explicit dep array: `[addRecentSearch, onClose, navigate, location.search, isRegexMode, isCaseSensitive, isWholeWord]`. |
| M1 | ts-rs generated types had imports at bottom — actual codebase pattern has imports at top | Minor | **NEW (Round 3):** Moved `import type` lines to top of `GrepResponse.ts` and `GrepSessionHit.ts`. Matched actual ts-rs header format. |
| M2 | `searchQuery` callback param vs `query` state var naming confusion | Minor | **NEW (Round 3):** Added explicit note in handleSelect section clarifying that `searchQuery` is the callback parameter, distinct from component state `query`. |
| M3 | Task 1 line number said ~37, actual tantivy line is 28 | Minor | **NEW (Round 3):** Fixed to exact line numbers: root `Cargo.toml` line 28, `crates/search/Cargo.toml` line 16. |
