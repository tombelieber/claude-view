# Reliability Release — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix 4 foundational issues to make claude-view enterprise-ready for DataCloak sandbox environments.

**Architecture:** Single config root (`CLAUDE_VIEW_DATA_DIR`) controls all writes. Session discovery rebuilt with content-based classification and full topology graph. Project path resolution uses `cwd` from JSONL instead of DFS filesystem walking.

**Tech Stack:** Rust (Axum, sqlx, Tantivy), React SPA, dotenv

**Design doc:** `docs/plans/2026-02-24-reliability-release-design.md`

---

## Task 1: Single Config Root — `data_dir()` in paths.rs

**Files:**
- Modify: `crates/core/src/paths.rs` (entire file, currently 54 lines)
- Test: `crates/core/src/paths.rs` (add inline tests)

**Step 1: Write failing tests for `data_dir()`**

Add at the bottom of `crates/core/src/paths.rs`:

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
- Modify: `.env.example`
- Modify: `.gitignore`

**Step 1: Update `.env.example`**

Add `CLAUDE_VIEW_DATA_DIR` line:

```
CLAUDE_VIEW_DATA_DIR=./.data
RELAY_URL=wss://claude-view-relay.fly.dev/ws
```

**Step 2: Update `.gitignore`**

Add these entries (if not already present):

```
# Local data directory (sandbox/dev)
.data/

# Environment overrides
.env
```

**Step 3: Commit**

```bash
git add .env.example .gitignore
git commit -m "feat: .env.example with CLAUDE_VIEW_DATA_DIR for sandbox dev"
```

---

## Task 4: Session Classification Types

**Files:**
- Modify: `crates/core/src/session_index.rs` (add new types near top)
- Test: `crates/core/src/session_index.rs` (inline tests)

**Step 1: Write failing tests for session classification**

```rust
#[cfg(test)]
mod classification_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_classify_normal_session() {
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
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","parentUuid":"p1","message":{{"content":"hello"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","uuid":"a1","message":{{"content":"hi"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert!(c.parent_id.is_some());
    }

    #[test]
    fn test_classify_file_history_snapshot() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"file-history-snapshot","messageId":"m1","snapshot":{{}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert_eq!(c.start_type, StartType::FileHistorySnapshot);
    }

    #[test]
    fn test_classify_resumed_session_with_preamble() {
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
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert!(c.cwd.is_none());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core classification_tests -- --test-threads=1`
Expected: FAIL — types and function don't exist yet.

**Step 3: Implement classification types and function**

Add near the top of `session_index.rs`:

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
    let file = File::open(path).map_err(|e| SessionIndexError::IoError(e.to_string()))?;
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

**Files:**
- Modify: `crates/core/src/session_index.rs:175-281` (rewrite function)
- Modify: existing tests at lines 481-553

**Step 1: Write failing test for new classification-aware discovery**

```rust
#[test]
fn test_discover_orphan_sessions_skips_metadata_files() {
    let tmp = tempfile::tempdir().unwrap();
    let proj_dir = tmp.path().join("projects").join("test-project");
    std::fs::create_dir_all(&proj_dir).unwrap();

    // Real session file
    let session = proj_dir.join("abc-123.jsonl");
    std::fs::write(&session, concat!(
        r#"{"type":"user","uuid":"u1","message":{"content":"hello"},"cwd":"/proj"}"#, "\n",
        r#"{"type":"assistant","uuid":"a1","message":{"content":"hi"}}"#, "\n",
    )).unwrap();

    // Metadata file (should not count as session)
    let snapshot = proj_dir.join("fhs-456.jsonl");
    std::fs::write(&snapshot, concat!(
        r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#, "\n",
    )).unwrap();

    let results = discover_orphan_sessions(tmp.path()).unwrap();
    let entries: Vec<_> = results.into_iter().flat_map(|(_, v)| v).collect();

    // Only the real session should be discovered
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].session_id, "abc-123");
}

#[test]
fn test_discover_orphan_sessions_classifies_forks() {
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
    // The entry should have classification data available
    // (exact field depends on whether we store classification in SessionIndexEntry)
}
```

**Step 2: Run to verify failure**

Run: `cargo test -p claude-view-core test_discover_orphan_sessions_skips_metadata -- --test-threads=1`
Expected: FAIL — current function doesn't classify.

**Step 3: Rewrite `discover_orphan_sessions()` to use `classify_jsonl_file()`**

The function at line 175 currently creates `SessionIndexEntry` for every `.jsonl` file. Change it to:
1. Call `classify_jsonl_file()` on each `.jsonl` file
2. Only include files where `kind == Conversation`
3. Store `cwd`, `parent_id`, `start_type` in the entry metadata

**Step 4: Update existing tests to match new behavior**

The 5 existing tests (lines 481-553) need updating since metadata files are now filtered out. Rewrite test fixtures to include proper `user`+`assistant` content.

**Step 5: Run all discovery tests**

Run: `cargo test -p claude-view-core session_index -- --test-threads=1`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/core/src/session_index.rs
git commit -m "feat: discover_orphan_sessions filters by content, builds session topology"
```

---

## Task 6: Delete DFS Resolve, Use `cwd` from JSONL

**Files:**
- Modify: `crates/core/src/discovery.rs:60-205` (delete `dfs_resolve`, `tokenize_encoded_name`, fallback)
- Modify: `crates/core/src/discovery.rs:259-299` (keep `derive_display_name`)
- Delete tests: lines 1532-1758 (DFS and tokenization tests)
- Add new tests for `cwd`-based resolution

**Step 1: Write failing tests for new resolution**

```rust
#[test]
fn test_resolve_from_cwd() {
    let result = resolve_project_from_cwd("/Users/dev/claude-view");
    assert_eq!(result.full_path, Some("/Users/dev/claude-view".to_string()));
}

#[test]
fn test_resolve_from_cwd_none_shows_encoded_name() {
    let result = resolve_project_path_with_cwd("-Users-dev-claude-view", None);
    // Should show encoded name honestly, not a guess
    assert_eq!(result.display_name, "-Users-dev-claude-view");
    assert!(!result.resolved);
}

#[test]
fn test_resolve_from_cwd_with_value() {
    let result = resolve_project_path_with_cwd("-Users-dev-claude-view", Some("/Users/dev/claude-view"));
    assert_eq!(result.full_path, Some("/Users/dev/claude-view".to_string()));
    assert!(result.resolved);
}
```

**Step 2: Run to verify failure**

Run: `cargo test -p claude-view-core test_resolve_from_cwd -- --test-threads=1`
Expected: FAIL — functions don't exist yet.

**Step 3: Rewrite `resolve_project_path()` to accept optional `cwd`**

```rust
/// Resolve project path. Primary source: cwd from JSONL.
/// Fallback: show encoded directory name (never guess).
pub fn resolve_project_path_with_cwd(encoded_name: &str, cwd: Option<&str>) -> ResolvedProject {
    match cwd {
        Some(path) => ResolvedProject {
            full_path: Some(path.to_string()),
            display_name: derive_display_name(path),
            resolved: true,
        },
        None => ResolvedProject {
            full_path: None,
            display_name: encoded_name.to_string(),
            resolved: false,
        },
    }
}
```

**Step 4: Delete dead code**

Remove:
- `tokenize_encoded_name()` (lines 116-139)
- `dfs_resolve()` (lines 162-205)
- Silent fallback in old `resolve_project_path()` (lines 60-91)
- All DFS tests (lines 1532-1758)
- All tokenization tests (lines 1532-1551)

Keep:
- `derive_display_name()` (lines 259-299) — still walks up to find `.git` root
- Display name tests (lines 1797-1844)

**Step 5: Update all callers of `resolve_project_path()`**

Search for all callsites. Each one needs to pass the `cwd` from session classification data. The main caller is in the indexing pipeline where sessions are processed.

Run: `grep -rn "resolve_project_path" crates/` to find all callsites.

**Step 6: Run remaining tests**

Run: `cargo test -p claude-view-core discovery -- --test-threads=1`
Expected: PASS (display name tests still work, DFS tests deleted)

**Step 7: Commit**

```bash
git add crates/core/src/discovery.rs
git commit -m "feat: project path from JSONL cwd, delete DFS resolve (sandbox-proof)"
```

---

## Task 7: Wire Everything into Indexing Pipeline

**Files:**
- Modify: `crates/server/src/main.rs:246-269` (background indexing)
- Modify: wherever sessions are inserted into DB (trace the pipeline)

**Step 1: Find the indexing pipeline entry point**

Run: `grep -rn "discover_orphan_sessions\|read_all_session_indexes" crates/server/`

The background indexing task (main.rs ~line 253) calls into session discovery. Update it to:
1. Call the new classification-aware `discover_orphan_sessions()`
2. Pass `cwd` from classification to `resolve_project_path_with_cwd()`
3. Store session topology (parent_id, kind, start_type) in the DB

**Step 2: Update DB schema if needed**

Check if `sessions` table needs new columns for `kind`, `start_type`, `parent_id`, `cwd`. If so, add a migration.

**Step 3: End-to-end test**

Run: `CLAUDE_VIEW_DATA_DIR=/tmp/cv-e2e cargo run -p claude-view-server`
Verify:
- Server starts with data in `/tmp/cv-e2e/`
- Session count is accurate (not inflated)
- Project names resolve correctly
- No files written outside `/tmp/cv-e2e/`

**Step 4: Commit**

```bash
git add crates/server/src/main.rs crates/db/
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
```

**Step 2: Verify with real data**

```bash
CLAUDE_VIEW_DATA_DIR=./.data cargo run -p claude-view-server
```

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
