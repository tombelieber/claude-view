# Reliability Release — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix 4 foundational issues to make claude-view enterprise-ready for DataCloak sandbox environments.

**Architecture:** Single config root (`CLAUDE_VIEW_DATA_DIR`) controls all writes. Session discovery rebuilt with content-based classification and full topology graph. Project path resolution uses `cwd` from JSONL instead of DFS filesystem walking.

**Tech Stack:** Rust (Axum, sqlx, Tantivy), React SPA (dotenvy already wired in server)

**Design doc:** `docs/plans/2026-02-24-reliability-release-design.md`

**Task dependencies:**
- Task 2 depends on Task 1 (Task 2 uses `lock_dir()` which Task 1 creates)
- Task 5 depends on Task 4 (Task 5 uses `classify_jsonl_file()` from Task 4)
- Task 7 depends on Tasks 5 and 6 (wires classification + cwd resolution into pipeline)

---

## Task 1: Single Config Root — `data_dir()` in paths.rs

**Files:**
- Modify: `crates/core/src/paths.rs` (entire file, currently 83 lines with existing test module at lines 56-83)
- Test: `crates/core/src/paths.rs` (replace existing test module)

**Step 1: Write failing tests for `data_dir()`**

The file already has a `#[cfg(test)] mod tests` block at line 56. **Replace** the existing test module (lines 56-83) with the expanded tests. Do NOT add a second `mod tests` — merge into the existing one:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_data_dir_uses_env_var_absolute() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-claude-view-data");
        let dir = data_dir();
        assert_eq!(dir, PathBuf::from("/tmp/test-claude-view-data"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_data_dir_resolves_relative_path() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "./.data");
        let dir = data_dir();
        assert!(dir.is_absolute(), "relative path should be resolved to absolute");
        assert!(dir.ends_with(".data"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_data_dir_falls_back_to_cache_dir() {
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
        let dir = data_dir();
        // Should end with "claude-view" in some cache directory
        assert!(dir.ends_with("claude-view"));
    }

    #[test]
    fn test_db_path_derives_from_data_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        let path = db_path().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-cv/claude-view.db"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_search_index_dir_derives_from_data_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        let path = search_index_dir().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-cv/search-index"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }

    #[test]
    fn test_lock_dir_derives_from_data_dir() {
        env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/test-cv");
        let path = lock_dir().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-cv/locks"));
        env::remove_var("CLAUDE_VIEW_DATA_DIR");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core paths::tests -- --test-threads=1`
Expected: FAIL — `data_dir`, `lock_dir` don't exist yet.

**Step 3: Rewrite `paths.rs` with single config root**

Replace the entire file body with:

```rust
use std::path::{Path, PathBuf};

/// Single source of truth for ALL claude-view write paths.
/// Set CLAUDE_VIEW_DATA_DIR to override (e.g., `./.data` for sandbox dev).
/// Falls back to platform cache dir (~/Library/Caches/claude-view on macOS).
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CLAUDE_VIEW_DATA_DIR") {
        let path = PathBuf::from(&dir);
        if path.is_relative() {
            std::env::current_dir()
                .expect("cannot determine current directory")
                .join(path)
        } else {
            path
        }
    } else {
        dirs::cache_dir()
            .map(|d| d.join("claude-view"))
            .expect("no platform cache directory found")
    }
}

pub fn db_path() -> Option<PathBuf> {
    Some(data_dir().join("claude-view.db"))
}

pub fn search_index_dir() -> Option<PathBuf> {
    Some(data_dir().join("search-index"))
}

pub fn lock_dir() -> Option<PathBuf> {
    Some(data_dir().join("locks"))
}

/// Remove all claude-view cache data (DB, WAL, search index).
pub fn remove_cache_data() -> Vec<String> {
    let dir = data_dir();
    let mut removed = Vec::new();

    for name in &["claude-view.db", "claude-view.db-wal", "claude-view.db-shm"] {
        let p = dir.join(name);
        if p.exists() {
            if std::fs::remove_file(&p).is_ok() {
                removed.push(format!("Removed {}", p.display()));
            }
        }
    }

    let idx = dir.join("search-index");
    if idx.exists() {
        if std::fs::remove_dir_all(&idx).is_ok() {
            removed.push(format!("Removed {}", idx.display()));
        }
    }

    removed
}

/// Remove lock files from data_dir/locks/.
pub fn remove_lock_files() -> Vec<String> {
    let mut removed = Vec::new();
    if let Some(dir) = lock_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "lock").unwrap_or(false) {
                    if std::fs::remove_file(&path).is_ok() {
                        removed.push(format!("Removed {}", path.display()));
                    }
                }
            }
        }
    }

    // Also clean up legacy /tmp/ lock files from older versions
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("claude-view-") && name_str.ends_with(".lock") {
                if std::fs::remove_file(entry.path()).is_ok() {
                    removed.push(format!("Removed legacy {}", entry.path().display()));
                }
            }
        }
    }

    removed
}
```

Then append the test module from Step 1.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core paths::tests -- --test-threads=1`
Expected: PASS (all 6 tests)

**Step 5: Commit**

```bash
git add crates/core/src/paths.rs
git commit -m "feat: single config root CLAUDE_VIEW_DATA_DIR for all write paths"
```

---

## Task 2: Update Lock File Creation in Server

**Depends on:** Task 1 (uses `lock_dir()` from paths.rs)

**Files:**
- Modify: `crates/server/src/main.rs:401-412` (lock file creation)
- Modify: `crates/server/src/main.rs:206-220` (search index fallback)

**Step 1: Update lock file path in `main.rs`**

At line 401, change:
```rust
// OLD:
let lock_path = std::env::temp_dir().join(format!("claude-view-{}.lock", port));
```

To:
```rust
// NEW:
let lock_dir = claude_view_core::paths::lock_dir()
    .unwrap_or_else(|| std::env::temp_dir());
let _ = std::fs::create_dir_all(&lock_dir);
let lock_path = lock_dir.join(format!("claude-view-{}.lock", port));
```

**Step 2: Remove search index fallback in `main.rs`**

At lines 206-220, the search index dir already uses `paths::search_index_dir()`. Remove the `.unwrap_or_else(|| ...)` inline fallback — `search_index_dir()` now always returns `Some(...)`.

**Step 3: Add startup validation before DB open**

Before line 187 (`let db = Database::open_default().await?;`), add:

```rust
// Validate data directory is writable before proceeding
let data_dir = claude_view_core::paths::data_dir();
if let Err(e) = std::fs::create_dir_all(&data_dir) {
    eprintln!(
        "ERROR: Cannot create data directory: {}\n\
         Path: {}\n\
         Set CLAUDE_VIEW_DATA_DIR to a writable directory.",
        e,
        data_dir.display()
    );
    std::process::exit(1);
}
let probe = data_dir.join(".write-test");
if std::fs::write(&probe, b"ok").is_err() {
    eprintln!(
        "ERROR: Data directory is not writable: {}\n\
         Set CLAUDE_VIEW_DATA_DIR to a writable directory.",
        data_dir.display()
    );
    std::process::exit(1);
}
let _ = std::fs::remove_file(&probe);
tracing::info!("Data directory: {}", data_dir.display());
```

**Step 4: Run the server to verify it starts**

Run: `CLAUDE_VIEW_DATA_DIR=/tmp/cv-test cargo run -p claude-view-server`
Expected: Server starts, prints `Data directory: /tmp/cv-test`, creates DB and index there.

**Step 5: Verify failure case**

Run: `CLAUDE_VIEW_DATA_DIR=/nonexistent/readonly cargo run -p claude-view-server`
Expected: Prints error about non-writable directory, exits with code 1.

**Step 6: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "feat: startup validates writable data dir, lock files move to data_dir"
```

---

## Task 3: .env.example and .gitignore

**Files:**
- Modify: `.env.example` (already exists with RELAY_URL content — prepend, don't replace)
- Modify: `.gitignore` (`.env` already ignored at line 15 — only add `.data/`)

**Step 1: Update `.env.example`**

**Prepend** the `CLAUDE_VIEW_DATA_DIR` line above the existing RELAY_URL content. The file currently contains RELAY_URL with comments. Final result should be:

```
# Local data directory for sandbox/dev environments.
# Uncomment to keep all writes inside the repo (avoids ~/Library/Caches/).
# CLAUDE_VIEW_DATA_DIR=./.data

# Mobile relay server URL (WebSocket endpoint)
# Set this to enable mobile remote monitoring via phone.
# Local dev:  ws://localhost:47893/ws
# Production: wss://claude-view-relay.fly.dev/ws
RELAY_URL=wss://claude-view-relay.fly.dev/ws
```

**Step 2: Update `.gitignore`**

`.env` is already ignored (line 15). Only add `.data/` if not already present:

```
# Local data directory (sandbox/dev)
.data/
```

**Step 3: Commit**

```bash
git add .env.example .gitignore
git commit -m "feat: .env.example with CLAUDE_VIEW_DATA_DIR for sandbox dev"
```

---

## Task 4: Session Classification Types

**Files:**
- Modify: `crates/core/src/session_index.rs` (add new types after imports at line 11, before `SessionIndexFile` at line 15)
- Test: `crates/core/src/session_index.rs` (add `classification_tests` module inside existing `mod tests`)

**Step 1: Write failing tests for session classification**

Add inside the existing `#[cfg(test)] mod tests` block (after line 553, before the closing `}`):

```rust
    // ========================================================================
    // Classification Tests
    // ========================================================================

    #[test]
    fn test_classify_normal_session() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","message":{{"content":"hello"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","uuid":"a1","message":{{"content":"hi"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert_eq!(c.start_type, StartType::User);
        assert_eq!(c.cwd.as_deref(), Some("/proj"));
        assert!(c.parent_id.is_none());
    }

    #[test]
    fn test_classify_forked_session() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","parentUuid":"p1","message":{{"content":"hello"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","uuid":"a1","message":{{"content":"hi"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert!(c.parent_id.is_some());
    }

    #[test]
    fn test_classify_file_history_snapshot() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"file-history-snapshot","messageId":"m1","snapshot":{{}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert_eq!(c.start_type, StartType::FileHistorySnapshot);
    }

    #[test]
    fn test_classify_resumed_session_with_preamble() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"file-history-snapshot","messageId":"m1","snapshot":{{}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"file-history-snapshot","messageId":"m2","snapshot":{{}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","message":{{"content":"continue"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","uuid":"a1","message":{{"content":"sure"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert_eq!(c.start_type, StartType::FileHistorySnapshot);
        assert_eq!(c.cwd.as_deref(), Some("/proj"));
    }

    #[test]
    fn test_classify_metadata_only_no_conversation() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert!(c.cwd.is_none());
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core classification_tests -- --test-threads=1`
Expected: FAIL — types and function don't exist yet.

**Step 3: Implement classification types and function**

Add after the existing imports (after line 11 `use crate::error::SessionIndexError;`), before `SessionIndexFile` struct (line 15):

```rust
use memchr::memmem;
use std::io::{BufRead, BufReader};
use std::fs::File;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKind {
    Conversation,  // has user + assistant lines
    MetadataOnly,  // file-history-snapshot, summary, etc.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartType {
    User,
    FileHistorySnapshot,
    QueueOperation,
    Progress,
    Summary,
    Assistant,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SessionClassification {
    pub kind: SessionKind,
    pub start_type: StartType,
    pub cwd: Option<String>,
    pub parent_id: Option<String>,
}

/// Classify a JSONL file by scanning its content.
/// Uses memmem SIMD pre-filter — only JSON-parses lines that match.
pub fn classify_jsonl_file(path: &Path) -> Result<SessionClassification, SessionIndexError> {
    let file = File::open(path).map_err(|e| SessionIndexError::io(path, e))?;
    let reader = BufReader::new(file);

    let user_finder = memmem::Finder::new(br#""type":"user""#);
    let user_finder_spaced = memmem::Finder::new(br#""type": "user""#);
    let assistant_finder = memmem::Finder::new(br#""type":"assistant""#);
    let assistant_finder_spaced = memmem::Finder::new(br#""type": "assistant""#);

    let mut start_type = StartType::Unknown;
    let mut has_user = false;
    let mut has_assistant = false;
    let mut cwd: Option<String> = None;
    let mut parent_id: Option<String> = None;
    let mut first_line = true;

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        let bytes = line.as_bytes();

        // Determine start_type from first non-empty line
        if first_line && !line.trim().is_empty() {
            first_line = false;
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                start_type = match obj.get("type").and_then(|v| v.as_str()) {
                    Some("user") => StartType::User,
                    Some("file-history-snapshot") => StartType::FileHistorySnapshot,
                    Some("queue-operation") => StartType::QueueOperation,
                    Some("progress") => StartType::Progress,
                    Some("summary") => StartType::Summary,
                    Some("assistant") => StartType::Assistant,
                    _ => StartType::Unknown,
                };
            }
        }

        // SIMD pre-filter: check for user/assistant type markers
        let is_user = user_finder.find(bytes).is_some()
            || user_finder_spaced.find(bytes).is_some();
        let is_assistant = assistant_finder.find(bytes).is_some()
            || assistant_finder_spaced.find(bytes).is_some();

        // Parse user lines to extract cwd and parentUuid
        if is_user && !has_user {
            has_user = true;
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                if cwd.is_none() {
                    cwd = obj.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                parent_id = obj.get("parentUuid").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
        }

        if is_assistant {
            has_assistant = true;
        }

        // Extract cwd from any line if we haven't found it yet
        if cwd.is_none() && line.contains("\"cwd\"") {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                cwd = obj.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
        }

        // Early exit: both conditions resolved
        if has_user && has_assistant && cwd.is_some() {
            break;
        }
    }

    let kind = if has_user && has_assistant {
        SessionKind::Conversation
    } else {
        SessionKind::MetadataOnly
    };

    Ok(SessionClassification {
        kind,
        start_type,
        cwd,
        parent_id,
    })
}
```

**Note on error handling:** `SessionIndexError` does NOT have an `IoError(String)` variant. The actual variant is `Io { path: PathBuf, source: std::io::Error }`. Use the constructor `SessionIndexError::io(path, e)` (defined in `crates/core/src/error.rs:94`) which auto-dispatches to `NotFound`, `PermissionDenied`, or `Io` based on error kind.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core classification_tests -- --test-threads=1`
Expected: PASS (all 5 tests)

**Step 5: Commit**

```bash
git add crates/core/src/session_index.rs
git commit -m "feat: content-based session classification with SIMD pre-filter"
```

---

## Task 5: Rewrite `discover_orphan_sessions()` with Classification

**Depends on:** Task 4 (uses `classify_jsonl_file()`, `SessionKind`)

**Files:**
- Modify: `crates/core/src/session_index.rs` — `SessionIndexEntry` struct (add 2 fields) and `discover_orphan_sessions()` function (lines 175-281)
- Modify: existing tests at lines 481-553

**Step 1: Add `session_cwd` and `parent_session_id` fields to `SessionIndexEntry`**

The struct is at lines 24-47 with `#[serde(rename_all = "camelCase")]`. Add two new optional fields **at the end** of the struct, before the closing `}`:

```rust
    #[serde(default)]
    pub session_cwd: Option<String>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
```

These use `#[serde(default)]` so existing JSON without these fields still deserializes correctly.

**CRITICAL: Also update the existing `SessionIndexEntry` struct literal in `read_all_session_indexes()`** (at approximately line 136). Adding new non-Default fields to the struct will cause a compile error at every construction site that doesn't list all fields. There are two construction sites in session_index.rs — the one in `read_all_session_indexes()` (line ~136) is NOT rewritten in Step 4. Add these two fields to that existing literal:

```rust
// In read_all_session_indexes(), find the SessionIndexEntry { ... } struct literal
// and add these two fields at the end (before the closing `}`):
            session_cwd: None,
            parent_session_id: None,
```

**Step 2: Write failing tests for new classification-aware discovery**

Add inside the existing `mod tests` block, after the existing `discover_orphan_sessions` tests (after line 553):

```rust
    #[test]
    fn test_discover_orphan_sessions_skips_metadata_files() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&proj_dir).unwrap();

        // Real session file (has user + assistant)
        let session = proj_dir.join("abc-123.jsonl");
        std::fs::write(&session, concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hello"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"hi"}}"#, "\n",
        )).unwrap();

        // Metadata-only file (should NOT count as session)
        let snapshot = proj_dir.join("fhs-456.jsonl");
        std::fs::write(&snapshot, concat!(
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#, "\n",
        )).unwrap();

        let results = discover_orphan_sessions(tmp.path()).unwrap();
        let entries: Vec<_> = results.into_iter().flat_map(|(_, v)| v).collect();

        // Only the real session should be discovered
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "abc-123");
        assert_eq!(entries[0].session_cwd.as_deref(), Some("/proj"));
    }

    #[test]
    fn test_discover_orphan_sessions_captures_parent_id() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&proj_dir).unwrap();

        let session = proj_dir.join("fork-789.jsonl");
        std::fs::write(&session, concat!(
            r#"{"type":"user","uuid":"u1","parentUuid":"parent-abc","message":{"content":"continue"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

        let results = discover_orphan_sessions(tmp.path()).unwrap();
        let entries: Vec<_> = results.into_iter().flat_map(|(_, v)| v).collect();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].parent_session_id.as_deref(), Some("parent-abc"));
    }
```

**Step 3: Run to verify failure**

Run: `cargo test -p claude-view-core test_discover_orphan_sessions_skips_metadata -- --test-threads=1`
Expected: FAIL — current function doesn't classify, new fields don't exist yet.

**Step 4: Rewrite `discover_orphan_sessions()` to use `classify_jsonl_file()`**

In the function body (lines 175-281), change the inner loop that builds `SessionIndexEntry` (currently lines 246-272). After the `file_path` assignment, add classification and filtering:

```rust
            // Classify the file content before including it
            let classification = match classify_jsonl_file(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to classify {}: {}", file_path.display(), e);
                    continue;
                }
            };

            // Only include actual conversation sessions
            if classification.kind != SessionKind::Conversation {
                continue;
            }

            let full_path = file_path.to_string_lossy().to_string();

            session_entries.push(SessionIndexEntry {
                session_id,
                full_path: Some(full_path),
                file_mtime: None,
                first_prompt: None,
                summary: None,
                message_count: None,
                created: None,
                modified: None,
                git_branch: None,
                project_path: None,
                is_sidechain: None,
                session_cwd: classification.cwd,
                parent_session_id: classification.parent_id,
            });
```

**Step 5: Update existing tests to match new behavior**

The 5 existing tests (lines 481-553) write bare `"{}"` to `.jsonl` files. These will now be classified as `MetadataOnly` and filtered out. Update the test fixtures to contain real `user`+`assistant` lines so they pass classification:

Replace `std::fs::write(proj.join("abc-123.jsonl"), "{}")` with:
```rust
std::fs::write(proj.join("abc-123.jsonl"), concat!(
    r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/proj"}"#, "\n",
    r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
))
```

Apply this to all test fixtures that write `.jsonl` files in the orphan discovery tests.

**Step 6: Run all discovery tests**

Run: `cargo test -p claude-view-core session_index -- --test-threads=1`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/core/src/session_index.rs
git commit -m "feat: discover_orphan_sessions filters by content, builds session topology"
```

---

## Task 6: Delete DFS Resolve, Use `cwd` from JSONL

**Files:**
- Modify: `crates/core/src/discovery.rs` — rewrite `resolve_project_path()`, delete DFS and related dead code
- Modify: `crates/db/src/indexer.rs:13` — update import
- Modify: `crates/db/src/indexer_parallel.rs:14` — update import
- Delete tests across discovery.rs

**IMPORTANT — Backward compatibility strategy:**

`ResolvedProject` struct keeps `full_path: String` (not `Option<String>`). This avoids breaking 5+ callsites across 3 crates. When cwd is unavailable, `full_path` gets the naive segment-join path (same as old DFS fallback). A new function `resolve_project_path_with_cwd()` is the preferred path for callers that have cwd from classification.

**Step 1: Write failing tests for new resolution**

Add inside the existing test module:

```rust
    // ============================================================================
    // cwd-based Resolution Tests
    // ============================================================================

    #[test]
    fn test_resolve_project_path_with_cwd_some() {
        let result = resolve_project_path_with_cwd("-Users-dev-claude-view", Some("/Users/dev/claude-view"));
        assert_eq!(result.full_path, "/Users/dev/claude-view");
        assert_eq!(result.display_name, "claude-view");
    }

    #[test]
    fn test_resolve_project_path_with_cwd_none_uses_naive_decode() {
        let result = resolve_project_path_with_cwd("-Users-dev-my-project", None);
        // Without cwd, falls back to naive segment join (no DFS)
        assert!(result.full_path.starts_with('/'));
        assert!(!result.display_name.is_empty());
    }

    #[test]
    fn test_resolve_project_path_backward_compat() {
        // Old function signature still works — returns String full_path
        let result = resolve_project_path("-tmp");
        assert_eq!(result.full_path, "/tmp");
        assert_eq!(result.display_name, "tmp");
    }
```

**Step 2: Run to verify failure**

Run: `cargo test -p claude-view-core test_resolve_project_path_with_cwd -- --test-threads=1`
Expected: FAIL — new function doesn't exist yet.

**Step 3: Rewrite `resolve_project_path()` — delete DFS, add cwd variant**

Replace the current `resolve_project_path()` function (lines 60-91) with:

```rust
/// Resolve an encoded project directory name to a filesystem path.
/// Uses naive segment join (no DFS filesystem walking — sandbox-safe).
/// Prefer `resolve_project_path_with_cwd()` when cwd from JSONL is available.
pub fn resolve_project_path(encoded_name: &str) -> ResolvedProject {
    resolve_project_path_with_cwd(encoded_name, None)
}

/// Resolve project path. Primary source: cwd from JSONL.
/// Fallback: naive segment join of encoded name (never DFS, never guess).
pub fn resolve_project_path_with_cwd(encoded_name: &str, cwd: Option<&str>) -> ResolvedProject {
    if let Some(path) = cwd {
        return ResolvedProject {
            full_path: path.to_string(),
            display_name: derive_display_name(path),
        };
    }

    // No cwd available — naive decode from encoded name
    if encoded_name.is_empty() {
        return ResolvedProject {
            full_path: String::new(),
            display_name: String::new(),
        };
    }

    // Simple decode: strip leading -, split on -, join with /
    let name = encoded_name.strip_prefix('-').unwrap_or(encoded_name);
    if name.is_empty() {
        return ResolvedProject {
            full_path: "/".to_string(),
            display_name: "/".to_string(),
        };
    }

    let segments: Vec<&str> = name.split('-').collect();
    let resolved_path = format!("/{}", segments.join("/"));
    let display_name = derive_display_name(&resolved_path);

    ResolvedProject {
        full_path: resolved_path,
        display_name,
    }
}
```

**Step 4: Delete dead code**

Remove ALL of the following functions **including their doc comment blocks** immediately above each function:
- `tokenize_encoded_name()` (delete lines **109-139**, includes doc comment at 109-115)
- `dfs_resolve()` (delete lines **141-205**, includes doc comment at 141-161)
- `build_candidates()` (delete lines **207-245**, includes doc comment at 207-216)
- `get_join_variants()` (delete lines **301-353**, includes doc comment at 301-306)

**Note:** These ranges are relative to the original file before any Step 4 deletions. Delete in order from bottom to top (highest line number first) to avoid line-number drift between deletions.

**Step 5: Delete dead tests**

Delete these test sections (all reference deleted functions):
- `resolve_project_path` tests that test DFS behavior (lines 876-912) — replace with the new tests from Step 1
- `get_join_variants` tests (lines 914-1005) — `get_join_variants()` is deleted
- Double dash / dot tests that use `get_join_variants` (lines 955-1005) — also deleted
- DFS Path Resolution Tests (lines 1527-1786) — ALL of these test `dfs_resolve()` and `tokenize_encoded_name()`

**Keep:**
- `derive_display_name()` (lines 259-299) — still walks up to find `.git` root
- Display Name Tests (lines 1788-1843)

**Step 6: Update import statements in dependent crates**

The deleted `get_join_variants` is NOT imported anywhere outside its own module. But `resolve_project_path` IS imported, and the new `resolve_project_path_with_cwd` must be made available.

In `crates/core/src/lib.rs:34`, the `pub use discovery::*;` already re-exports everything public. The new `resolve_project_path_with_cwd()` is automatically available. No change needed to `lib.rs`.

Callers that import `resolve_project_path` by name:

**`crates/db/src/indexer.rs:13`** — Currently:
```rust
use claude_view_core::{extract_session_metadata, resolve_project_path, SessionInfo};
```
No change needed — `resolve_project_path` still exists with the same signature.

**`crates/db/src/indexer_parallel.rs:14`** — The current import block (lines 12-16) is:
```rust
use claude_view_core::{
    classify_work_type, count_ai_lines, discover_orphan_sessions, read_all_session_indexes,
    resolve_project_path, resolve_worktree_parent, ClassificationInput, ClassifyResult, Registry,
    ToolCounts,
};
```
Add `resolve_project_path_with_cwd` and **preserve `ToolCounts`**:
```rust
use claude_view_core::{
    classify_work_type, count_ai_lines, discover_orphan_sessions, read_all_session_indexes,
    resolve_project_path, resolve_project_path_with_cwd, resolve_worktree_parent,
    ClassificationInput, ClassifyResult, Registry, ToolCounts,
};
```

**`crates/core/src/discovery.rs:454`** (inside `get_projects()`) — calls `resolve_project_path(&encoded_name)`. No cwd is available at this callsite (it's listing directories, not parsing JSONL). No change needed — it keeps using the backward-compat function.

**`crates/server/src/live/manager.rs:1378`** — calls `claude_view_core::discovery::resolve_project_path(&project_encoded)`. No cwd is available at this callsite. No change needed.

**Step 7: Run remaining tests**

Run: `cargo test -p claude-view-core discovery -- --test-threads=1`
Expected: PASS (display name tests still work, DFS tests deleted, new cwd tests pass)

**Step 8: Commit**

```bash
git add crates/core/src/discovery.rs crates/db/src/indexer_parallel.rs
git commit -m "feat: project path from JSONL cwd, delete DFS resolve (sandbox-proof)"
```

---

## Task 7: Wire Everything into Indexing Pipeline

**Depends on:** Tasks 5 and 6

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — wires cwd into `insert_project_sessions()`
- Modify: `crates/db/src/migrations.rs` — add Migrations 33-36 (one ALTER per entry)
- Modify: `crates/db/src/queries/sessions.rs` — add new `update_session_topology()` function (do NOT change `insert_session_from_index` — it has 28+ call sites)

**IMPORTANT:** The plan previously said to modify `crates/server/src/main.rs:246-269`. This is WRONG. `main.rs` only spawns the background task — the actual session discovery and insertion happens in `crates/db/src/indexer_parallel.rs`:
- `pass_1_read_indexes()` at line 1838 calls `read_all_session_indexes()` (line 1842) and `discover_orphan_sessions()` (line 1937)
- `insert_project_sessions()` at line 1848 calls `resolve_project_path()` (lines 1858, 1863)

**Step 1: Add Migrations 33-36 to `crates/db/src/migrations.rs`**

The last migration is Migration 32. Add **four separate entries** at the end of the `MIGRATIONS` array — one `ALTER TABLE` per entry. This is the established project pattern (see migrations 29-31 comment: "separate entries so each ALTER is independently idempotent"). Using a single entry with multiple statements would cause `sqlx::query()` to execute only the first statement (the framework only calls `raw_sql()` for migrations containing `BEGIN;`/`BEGIN\n`).

```rust
    // Migrations 33-36: Session topology fields for reliability release.
    // Separate entries so each ALTER is independently idempotent (branch collision safety).
    // Migration 33: session_cwd — raw cwd from JSONL (source of truth for project path)
    r#"ALTER TABLE sessions ADD COLUMN session_cwd TEXT;"#,
    // Migration 34: parent_session_id — parentUuid from first user line (fork/continuation)
    r#"ALTER TABLE sessions ADD COLUMN parent_session_id TEXT;"#,
    // Migration 35: session_kind — 'conversation' or 'metadata_only' (content classification)
    r#"ALTER TABLE sessions ADD COLUMN session_kind TEXT;"#,
    // Migration 36: start_type — first line type (user, file-history-snapshot, etc.)
    r#"ALTER TABLE sessions ADD COLUMN start_type TEXT;"#,
```

All columns are nullable with implicit `DEFAULT NULL` — safe for existing rows.

**Step 2: Update `insert_project_sessions()` in `indexer_parallel.rs`**

In `insert_project_sessions()` (line 1848), the function calls `resolve_project_path()` at lines 1858 and 1863. Update these to use cwd from the session entry when available:

At line 1856-1865, change:
```rust
// OLD:
let (effective_encoded, effective_resolved) =
    if let Some(parent_encoded) = resolve_worktree_parent(project_encoded) {
        let resolved = resolve_project_path(&parent_encoded);
        (parent_encoded, resolved)
    } else {
        (
            project_encoded.to_string(),
            resolve_project_path(project_encoded),
        )
    };
```

To:
```rust
// NEW: Use cwd from entry if available (from classification in discover_orphan_sessions)
// For indexed sessions (from sessions-index.json), session_cwd may be None — that's ok.
let entry_cwd = entries.first().and_then(|e| e.session_cwd.as_deref());

let (effective_encoded, effective_resolved) =
    if let Some(parent_encoded) = resolve_worktree_parent(project_encoded) {
        let resolved = resolve_project_path_with_cwd(&parent_encoded, entry_cwd);
        (parent_encoded, resolved)
    } else {
        (
            project_encoded.to_string(),
            resolve_project_path_with_cwd(project_encoded, entry_cwd),
        )
    };
```

**Step 3: Add `update_session_topology()` in `crates/db/src/queries/sessions.rs`**

**DO NOT modify `insert_session_from_index` — it has 28+ call sites across `git_correlation.rs`, `trends.rs`, `system.rs`, and test files. Changing its signature would require updating all of them.**

Instead, add a new small function after `insert_session_from_index` (after line 378):

```rust
/// Update session topology fields discovered via content classification.
/// Called after insert_session_from_index for sessions discovered by
/// discover_orphan_sessions() which have cwd and parent_id from JSONL.
pub async fn update_session_topology(
    &self,
    id: &str,
    session_cwd: Option<&str>,
    parent_session_id: Option<&str>,
) -> DbResult<()> {
    sqlx::query(
        "UPDATE sessions SET \
         session_cwd = COALESCE(?1, session_cwd), \
         parent_session_id = COALESCE(?2, parent_session_id) \
         WHERE id = ?3",
    )
    .bind(session_cwd)
    .bind(parent_session_id)
    .bind(id)
    .execute(self.pool())
    .await?;
    Ok(())
}
```

**Update `insert_project_sessions()` in `crates/db/src/indexer_parallel.rs`:**

After the existing `db.insert_session_from_index(...)` call (after line 1918), add a topology update for sessions that have classification data:

```rust
// Update topology fields if this entry has cwd or parent_id from classification
if entry.session_cwd.is_some() || entry.parent_session_id.is_some() {
    db.update_session_topology(
        &entry.session_id,
        entry.session_cwd.as_deref(),
        entry.parent_session_id.as_deref(),
    )
    .await
    .map_err(|e| format!("Failed to update topology {}: {}", entry.session_id, e))?;
}
```

`session_kind` and `start_type` columns are created by the migration but not populated yet — they remain NULL until a future iteration adds that data to `SessionIndexEntry`.

**Step 4: End-to-end test**

Run: `CLAUDE_VIEW_DATA_DIR=/tmp/cv-e2e cargo run -p claude-view-server`

**Note:** Browser verification requires the frontend. Run `bun run dev` in a separate terminal, or `bun run build && bun run preview`.

Verify:
- Server starts with data in `/tmp/cv-e2e/`
- Session count is accurate (not inflated by metadata files)
- Project names resolve correctly
- No files written outside `/tmp/cv-e2e/`

**Step 5: Commit**

```bash
git add crates/db/src/migrations.rs crates/db/src/indexer_parallel.rs crates/db/src/queries/sessions.rs
git commit -m "feat: wire session topology + cwd resolution into indexing pipeline"
```

---

## Task 8: Hook Install Documentation

**Files:**
- Modify: `README.md`

**Step 1: Add hook installation section to README**

Add a "Setup for Corporate/Sandbox Environments" section with the copy-paste one-liner for hook installation. Place it after the main installation instructions.

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add hook install one-liner for sandbox environments"
```

---

## Task 9: Integration Verification

**Files:** None (verification only)

**Step 1: Run full test suite for changed crates**

```bash
cargo test -p claude-view-core session_index -- --test-threads=1
cargo test -p claude-view-core discovery -- --test-threads=1
cargo test -p claude-view-core paths -- --test-threads=1
cargo test -p claude-view-db -- --test-threads=1
```

**Step 2: Verify with real data**

```bash
CLAUDE_VIEW_DATA_DIR=./.data bun run dev
```

(Runs both Vite dev server and Rust backend. For SSE testing, use `bun run preview` instead.)

Open browser, verify:
- Session count matches reality (~2,130 main sessions, not 1,660)
- Project names show correctly (not truncated/wrong)
- Data directory only in `.data/` — no writes to `/tmp/` or `~/Library/Caches/`

**Step 3: Clean uninstall test**

```bash
rm -rf .data
# Verify: zero traces remain
```

**Step 4: Final commit with version bump if needed**

```bash
git add -A
git commit -m "test: reliability release integration verification"
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Blocker | Severity | Fix Applied |
|---|---------|----------|-------------|
| B1 | `SessionIndexError::IoError(String)` doesn't exist | Blocker | Changed to `SessionIndexError::io(path, e)` constructor (error.rs:94) in Task 4 classify function |
| B2 | `SessionIndexEntry` has no `kind` field for filtering | Blocker | Added `session_cwd` and `parent_session_id` fields to `SessionIndexEntry` in Task 5 Step 1. Classification called inline in `discover_orphan_sessions()` — only `Conversation` entries pushed |
| B3 | `ResolvedProject.full_path` changed from `String` to `Option<String>` breaks 5+ callsites | Blocker | Kept `full_path: String` unchanged. `resolve_project_path()` now uses naive decode (no DFS). New `resolve_project_path_with_cwd()` added for callers with cwd. Zero callsite breakage |
| B4 | `resolve_project_from_cwd()` tested but never defined | Blocker | Removed undefined function. Tests now use `resolve_project_path_with_cwd()` directly, which is the actual API |
| B5 | 4 callsites across 3 crates — plan said "grep to find them" | Blocker | All 4 callsites listed explicitly with per-site analysis: discovery.rs:454 (no change), indexer.rs:251 (no change), indexer_parallel.rs:1858+1863 (updated to use `_with_cwd`), manager.rs:1378 (no change) |
| B6 | `get_join_variants()` calls deleted `tokenize_encoded_name()` | Blocker | Added `get_join_variants()` (307-353) and its tests (914-1005) to deletion list in Task 6 Step 4 |
| B7 | `indexer.rs:13` and `indexer_parallel.rs:14` have explicit imports | Blocker | `indexer.rs` unchanged (function still exists). `indexer_parallel.rs:14` updated to also import `resolve_project_path_with_cwd` |
| B8 | Task 7 grep targets `crates/server/` — zero matches | Blocker | Changed to grep `crates/`. Identified `indexer_parallel.rs:1842,1937` as real call sites |
| B9 | Task 7 says modify `main.rs:246-269` — wrong file | Blocker | Redirected to `crates/db/src/indexer_parallel.rs:pass_1_read_indexes()` and `insert_project_sessions()` |
| B10 | Migration says "add if needed" — no SQL, no number | Blocker | Specified Migration 33-36: four separate `ALTER TABLE sessions ADD COLUMN` entries |
| B11 | No integration strategy for `SessionClassification` → pipeline | Blocker | `SessionIndexEntry` gets `session_cwd` and `parent_session_id` fields. `discover_orphan_sessions()` populates them from `classify_jsonl_file()`. `insert_project_sessions()` reads `session_cwd` for path resolution |
| B12 | Task 2 uses `lock_dir()` from Task 1 — no dependency stated | Blocker | Added explicit dependency graph at plan header: Task 2→1, Task 5→4, Task 7→5+6 |
| B13 | Migration 33 multi-statement entry — `sqlx::query()` only executes first statement | Blocker | Split into 4 separate entries (migrations 33-36), matching established pattern from migrations 29-31. Multi-statement requires `BEGIN;`/`BEGIN\n` to trigger `raw_sql()` — bare multiple `ALTER TABLE` statements in one entry are silently truncated to first statement only |
| B14 | `SessionIndexEntry` struct literal in `read_all_session_indexes()` at line 136 not updated when adding 2 new fields — compile error | Blocker | Added explicit instruction in Task 5 Step 1 to update the line 136 struct literal with `session_cwd: None, parent_session_id: None` |
| B15 | Task 7 Step 3 vague — "check and update" without showing exact signature, SQL, or call site | Blocker | `insert_session_from_index` has 28+ call sites across 5 files (`git_correlation.rs` x11, `system.rs` x4, `trends.rs`, test files) — changing its signature would break all of them. Rewrote Step 3 to add a NEW `update_session_topology()` function instead; zero existing call sites touched; only `indexer_parallel.rs` gets one new follow-up call |

### Warnings Also Fixed

| # | Warning | Fix Applied |
|---|---------|-------------|
| W1 | paths.rs is 83 lines, not 54; existing test module collision | Updated Task 1 to say 83 lines, merge into existing `mod tests` instead of creating second one |
| W3 | DFS test deletion range 1532-1758 misses two tests ending at 1786 | Extended deletion to cover full range 1527-1786 |
| W4 | `build_candidates()` becomes dead code after DFS delete | Added to deletion list in Task 6 Step 4 |
| W5 | Task 6 deletion ranges omit doc comment blocks above each function (4 orphaned `///` blocks) | Updated all 4 deletion ranges to include doc comments; added note to delete bottom-to-top |
| W6 | `.env.example` replacement would delete existing RELAY_URL comments | Task 3 now says "prepend" not "replace" |
| W7 | `.env` already in .gitignore | Task 3 notes `.env` already covered, only `.data/` needs adding |
| W8 | Task 6 import quote omits `ToolCounts` — editing block could accidentally drop it | Updated Task 6 Step 6 to show full 5-line import block preserving `ToolCounts` |
| W9 | Task 7 Step 5 commit lists `lib.rs` instead of `queries/sessions.rs` | Fixed `git add` to include `crates/db/src/queries/sessions.rs` (where `insert_session_from_index` lives) |
