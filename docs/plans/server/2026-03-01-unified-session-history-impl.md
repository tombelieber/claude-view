# Unified Session History Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Combine live sessions (~/.claude/) and backup sessions (~/.claude-backup/) into one unified timeline with zero configuration.

**Architecture:** Pluggable `SessionSource` trait in crates/core. Two impls: `LiveSource` (existing logic, extracted) and `ClaudeBackupSource` (gzip decompression via flate2). Indexer takes `Vec<Box<dyn SessionSource>>`, dedup by UUID (live wins). One new SQLite column (`source`), one new field on `SessionInfo`, subtle "archived" badge in frontend.

**Tech Stack:** Rust (flate2 crate for gzip), SQLite migration, React/TypeScript (badge UI)

**Design doc:** `docs/plans/server/2026-03-01-unified-session-history-design.md`

---

### Task 1: Add `flate2` dependency and `SessionSource` trait

**Files:**
- Create: `crates/core/src/session_source.rs`
- Modify: `crates/core/src/lib.rs:1-42`
- Modify: `crates/core/Cargo.toml:8-19`

**Step 1: Add flate2 to crates/core/Cargo.toml**

Add under `[dependencies]`:
```toml
flate2 = "1"
```

**Step 2: Create `crates/core/src/session_source.rs`**

```rust
//! Pluggable session source abstraction.
//!
//! Each `SessionSource` impl knows how to discover session files and read them
//! into raw bytes. The indexer doesn't care where bytes come from — it just
//! parses them identically regardless of source.

use std::path::PathBuf;
use std::time::SystemTime;

/// Tag identifying which source a session came from.
/// Stored in SQLite, serialized to the API, drives the frontend badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSourceTag {
    /// Live session from ~/.claude/projects/
    Live,
    /// Archived session from backup (no longer in ~/.claude/)
    Archived,
}

impl SessionSourceTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Archived => "archived",
        }
    }
}

/// A session file discovered by a source — enough metadata to decide whether
/// to parse it (staleness check) and to read its bytes.
#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    pub uuid: String,
    pub project_hash: String,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub file_mtime: SystemTime,
    pub source_tag: SessionSourceTag,
}

/// Trait for session data sources. Implementations discover session files and
/// provide their raw JSONL bytes.
///
/// **Source order matters for dedup:** when multiple sources are passed to the
/// indexer, earlier sources win on UUID collision. Convention: LiveSource first.
pub trait SessionSource: Send + Sync {
    /// Human-readable name for logging (e.g. "live", "claude-backup").
    fn name(&self) -> &str;

    /// Discover all session files this source knows about.
    /// Returns an error only on fatal failures (e.g. permission denied on root dir).
    /// Missing directories should return Ok(empty vec).
    fn discover(&self) -> std::io::Result<Vec<DiscoveredSession>>;

    /// Read the raw JSONL bytes for a discovered session.
    /// For gzipped sources, this decompresses into memory.
    fn read_bytes(&self, session: &DiscoveredSession) -> std::io::Result<Vec<u8>>;
}
```

**Step 3: Register module in `crates/core/src/lib.rs`**

Add after `pub mod session_index;` (line 24):
```rust
pub mod session_source;
```

Add to the pub use block after `pub use session_index::*;` (line 40):
```rust
pub use session_source::*;
```

**Step 4: Verify it compiles**

Run: `cargo check -p claude-view-core`
Expected: compiles with no errors

**Step 5: Commit**

```bash
git add crates/core/src/session_source.rs crates/core/src/lib.rs crates/core/Cargo.toml
git commit -m "feat(core): add SessionSource trait and flate2 dependency

Pluggable abstraction for session data sources. Implementations
discover session files and provide raw JSONL bytes. Source order
determines dedup priority (live wins over archived)."
```

---

### Task 2: Implement `LiveSource`

**Files:**
- Modify: `crates/core/src/session_source.rs`

**Step 1: Write test for LiveSource discovery**

Add at the bottom of `session_source.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_live_source_discovers_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let projects = tmp.path().join("projects");
        let project_dir = projects.join("-Users-test-myproject");
        fs::create_dir_all(&project_dir).unwrap();

        // Create two .jsonl files
        fs::write(project_dir.join("abc-123.jsonl"), b"test data").unwrap();
        fs::write(project_dir.join("def-456.jsonl"), b"more data").unwrap();
        // Non-jsonl files should be ignored
        fs::write(project_dir.join("sessions-index.json"), b"{}").unwrap();
        fs::write(project_dir.join("abc-123"), b"dir stub").unwrap();

        let source = LiveSource::new(tmp.path());
        let sessions = source.discover().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().all(|s| s.source_tag == SessionSourceTag::Live));
        assert!(sessions.iter().any(|s| s.uuid == "abc-123"));
        assert!(sessions.iter().any(|s| s.uuid == "def-456"));
        assert!(sessions.iter().all(|s| s.project_hash == "-Users-test-myproject"));
    }

    #[test]
    fn test_live_source_read_bytes() {
        let tmp = TempDir::new().unwrap();
        let projects = tmp.path().join("projects");
        let project_dir = projects.join("-Users-test-proj");
        fs::create_dir_all(&project_dir).unwrap();
        let content = b"{\"type\":\"user\"}\n{\"type\":\"assistant\"}\n";
        fs::write(project_dir.join("sess-1.jsonl"), content).unwrap();

        let source = LiveSource::new(tmp.path());
        let sessions = source.discover().unwrap();
        let bytes = source.read_bytes(&sessions[0]).unwrap();
        assert_eq!(bytes, content);
    }

    #[test]
    fn test_live_source_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let source = LiveSource::new(tmp.path().join("nonexistent"));
        let sessions = source.discover().unwrap();
        assert!(sessions.is_empty());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core session_source -- --nocapture`
Expected: FAIL — `LiveSource` not defined

**Step 3: Implement LiveSource**

Add above the `#[cfg(test)]` block in `session_source.rs`:
```rust
use std::path::Path;

/// Live sessions from ~/.claude/projects/{hash}/{uuid}.jsonl
pub struct LiveSource {
    claude_dir: PathBuf,
}

impl LiveSource {
    pub fn new(claude_dir: impl Into<PathBuf>) -> Self {
        Self {
            claude_dir: claude_dir.into(),
        }
    }
}

impl SessionSource for LiveSource {
    fn name(&self) -> &str {
        "live"
    }

    fn discover(&self) -> std::io::Result<Vec<DiscoveredSession>> {
        let projects_dir = self.claude_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for project_entry in std::fs::read_dir(&projects_dir)?.flatten() {
            let project_path = project_entry.path();
            if !project_path.is_dir() {
                continue;
            }
            let project_hash = project_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let entries = match std::fs::read_dir(&project_path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for file_entry in entries.flatten() {
                let file_path = file_entry.path();
                if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let uuid = file_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let metadata = match file_entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                sessions.push(DiscoveredSession {
                    uuid,
                    project_hash: project_hash.clone(),
                    file_path,
                    file_size: metadata.len(),
                    file_mtime: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    source_tag: SessionSourceTag::Live,
                });
            }
        }

        Ok(sessions)
    }

    fn read_bytes(&self, session: &DiscoveredSession) -> std::io::Result<Vec<u8>> {
        std::fs::read(&session.file_path)
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core session_source -- --nocapture`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/session_source.rs
git commit -m "feat(core): implement LiveSource for ~/.claude/projects/"
```

---

### Task 3: Implement `ClaudeBackupSource`

**Files:**
- Modify: `crates/core/src/session_source.rs`

**Step 1: Write tests for ClaudeBackupSource**

Add to the `tests` module:
```rust
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn write_gzipped(path: &std::path::Path, content: &[u8]) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = GzEncoder::new(file, Compression::fast());
        encoder.write_all(content).unwrap();
        encoder.finish().unwrap();
    }

    #[test]
    fn test_backup_source_discovers_gz_files() {
        let tmp = TempDir::new().unwrap();
        let machine_dir = tmp.path().join("machines").join("my-machine").join("projects");
        let project_dir = machine_dir.join("-Users-test-proj");
        fs::create_dir_all(&project_dir).unwrap();

        write_gzipped(&project_dir.join("aaa-111.jsonl.gz"), b"data1");
        write_gzipped(&project_dir.join("bbb-222.jsonl.gz"), b"data2");
        // Non-gz files should be ignored
        fs::write(project_dir.join("not-gz.jsonl"), b"skip").unwrap();

        // Write manifest
        fs::write(
            tmp.path().join("manifest.json"),
            r#"{"version":"3.0.0","mode":"github"}"#,
        ).unwrap();

        let source = ClaudeBackupSource::detect_at(tmp.path()).unwrap();
        let sessions = source.discover().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().all(|s| s.source_tag == SessionSourceTag::Archived));
        assert!(sessions.iter().any(|s| s.uuid == "aaa-111"));
        assert!(sessions.iter().any(|s| s.uuid == "bbb-222"));
    }

    #[test]
    fn test_backup_source_read_bytes_decompresses() {
        let tmp = TempDir::new().unwrap();
        let machine_dir = tmp.path().join("machines").join("m1").join("projects");
        let project_dir = machine_dir.join("-Users-test-proj");
        fs::create_dir_all(&project_dir).unwrap();

        let original = b"{\"type\":\"user\",\"message\":\"hello\"}\n";
        write_gzipped(&project_dir.join("sess-1.jsonl.gz"), original);

        fs::write(
            tmp.path().join("manifest.json"),
            r#"{"version":"3.0.0"}"#,
        ).unwrap();

        let source = ClaudeBackupSource::detect_at(tmp.path()).unwrap();
        let sessions = source.discover().unwrap();
        let bytes = source.read_bytes(&sessions[0]).unwrap();
        assert_eq!(bytes, original);
    }

    #[test]
    fn test_backup_source_no_manifest_returns_none() {
        let tmp = TempDir::new().unwrap();
        assert!(ClaudeBackupSource::detect_at(tmp.path()).is_none());
    }

    #[test]
    fn test_backup_source_multiple_machines() {
        let tmp = TempDir::new().unwrap();
        for machine in &["machine-a", "machine-b"] {
            let project_dir = tmp.path().join("machines").join(machine).join("projects").join("-proj");
            fs::create_dir_all(&project_dir).unwrap();
            write_gzipped(&project_dir.join(format!("{}-sess.jsonl.gz", machine)), b"data");
        }
        fs::write(tmp.path().join("manifest.json"), r#"{"version":"3.0.0"}"#).unwrap();

        let source = ClaudeBackupSource::detect_at(tmp.path()).unwrap();
        let sessions = source.discover().unwrap();
        assert_eq!(sessions.len(), 2);
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core session_source -- --nocapture`
Expected: FAIL — `ClaudeBackupSource` not defined

**Step 3: Implement ClaudeBackupSource**

Add to `session_source.rs` (above `#[cfg(test)]`):
```rust
use flate2::read::GzDecoder;
use std::io::Read;

/// Archived sessions from ~/.claude-backup/machines/*/projects/{hash}/{uuid}.jsonl.gz
///
/// Auto-detected via manifest.json. If ~/.claude-backup doesn't exist or has no
/// manifest, this source is silently skipped — zero impact on users without backup.
pub struct ClaudeBackupSource {
    backup_dir: PathBuf,
}

impl ClaudeBackupSource {
    /// Try to detect a claude-backup installation at the default path.
    /// Returns None if ~/.claude-backup/manifest.json doesn't exist.
    pub fn detect() -> Option<Self> {
        let home = dirs::home_dir()?;
        Self::detect_at(&home.join(".claude-backup"))
    }

    /// Try to detect a claude-backup installation at a specific path.
    /// Returns None if manifest.json doesn't exist or is invalid.
    pub fn detect_at(backup_dir: &Path) -> Option<Self> {
        let manifest = backup_dir.join("manifest.json");
        if !manifest.exists() {
            return None;
        }
        // Validate it's parseable JSON (don't need the contents)
        let content = std::fs::read_to_string(&manifest).ok()?;
        serde_json::from_str::<serde_json::Value>(&content).ok()?;
        Some(Self {
            backup_dir: backup_dir.to_path_buf(),
        })
    }

    /// Count total .jsonl.gz files without fully walking (for startup log).
    pub fn session_count(&self) -> usize {
        self.discover().map(|v| v.len()).unwrap_or(0)
    }
}

impl SessionSource for ClaudeBackupSource {
    fn name(&self) -> &str {
        "claude-backup"
    }

    fn discover(&self) -> std::io::Result<Vec<DiscoveredSession>> {
        let machines_dir = self.backup_dir.join("machines");
        if !machines_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        // Walk: machines/*/projects/{hash}/{uuid}.jsonl.gz
        for machine_entry in std::fs::read_dir(&machines_dir)?.flatten() {
            let projects_dir = machine_entry.path().join("projects");
            if !projects_dir.is_dir() {
                continue;
            }

            for project_entry in std::fs::read_dir(&projects_dir)?.flatten() {
                let project_path = project_entry.path();
                if !project_path.is_dir() {
                    continue;
                }
                let project_hash = project_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let entries = match std::fs::read_dir(&project_path) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                for file_entry in entries.flatten() {
                    let file_path = file_entry.path();
                    let file_name = file_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    // Must end with .jsonl.gz
                    if !file_name.ends_with(".jsonl.gz") {
                        continue;
                    }

                    // UUID = filename without .jsonl.gz
                    let uuid = file_name.trim_end_matches(".jsonl.gz").to_string();

                    let metadata = match file_entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    sessions.push(DiscoveredSession {
                        uuid,
                        project_hash: project_hash.clone(),
                        file_path,
                        file_size: metadata.len(),
                        file_mtime: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                        source_tag: SessionSourceTag::Archived,
                    });
                }
            }
        }

        Ok(sessions)
    }

    fn read_bytes(&self, session: &DiscoveredSession) -> std::io::Result<Vec<u8>> {
        let file = std::fs::File::open(&session.file_path)?;
        let mut decoder = GzDecoder::new(std::io::BufReader::new(file));
        // Pre-allocate ~4x the compressed size (typical gzip ratio for JSONL)
        let mut bytes = Vec::with_capacity(session.file_size as usize * 4);
        decoder.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core session_source -- --nocapture`
Expected: all 7 tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/session_source.rs
git commit -m "feat(core): implement ClaudeBackupSource for ~/.claude-backup/

Auto-detects backup via manifest.json. Walks machines/*/projects/
for .jsonl.gz files. Decompresses via flate2 in read_bytes()."
```

---

### Task 4: Add `source` column to SQLite schema

**Files:**
- Modify: `crates/db/src/migrations.rs` (append migration 40)
- Modify: `crates/db/src/queries/row_types.rs:432-667` (SessionRow + into_session_info)
- Modify: `crates/db/src/queries/sessions.rs:16-199` (UPSERT_SESSION_SQL + execute_upsert)
- Modify: `crates/db/src/indexer_parallel.rs:41-105` (ParsedSession struct)
- Modify: `crates/core/src/types.rs:230-364` (SessionInfo struct)

**Step 1: Add migration 40**

Append to the `MIGRATIONS` array in `crates/db/src/migrations.rs` (before the closing `];`):
```rust
    // Migration 40: Session source — 'live' (default) or 'archived' (from backup)
    "ALTER TABLE sessions ADD COLUMN source TEXT NOT NULL DEFAULT 'live'",
```

**Step 2: Add `source` to `ParsedSession`**

In `crates/db/src/indexer_parallel.rs`, add after `total_cost_usd: f64,` (line 104):
```rust
    pub source: String,
```

**Step 3: Add `source` to `SessionRow`**

In `crates/db/src/queries/row_types.rs`, add after `longest_task_preview: Option<String>,` (line 502):
```rust
    pub(crate) source: String,
```

In the `FromRow` impl (around line 577), add before the closing `})`:
```rust
            source: row.try_get("source").unwrap_or_else(|_| "live".to_string()),
```

**Step 4: Add `source` to `SessionInfo`**

In `crates/core/src/types.rs`, add after `longest_task_preview: Option<String>,` (line 363, before the closing `}`):
```rust
    // Session source
    #[serde(default = "default_source")]
    pub source: String,
```

Add the default function near the struct:
```rust
fn default_source() -> String {
    "live".to_string()
}
```

**Step 5: Wire `source` through `into_session_info`**

In `crates/db/src/queries/row_types.rs`, in `into_session_info()` (around line 664), add before the closing `}`:
```rust
            source: self.source,
```

**Step 6: Add `source` to UPSERT SQL and bind**

In `crates/db/src/queries/sessions.rs`:

a) Add `source` to the INSERT column list (line 39, after `total_cost_usd`):
```
        primary_model, total_task_time_seconds,
        longest_task_seconds, longest_task_preview, total_cost_usd,
        source
```

b) Add `?64` to the VALUES list (after `?63`):
```
        ?56, ?57, ?58, ?59, ?60, ?61, ?62, ?63, ?64
```

c) Add `source = excluded.source` to ON CONFLICT (after `total_cost_usd = excluded.total_cost_usd`, line 114):
```
        total_cost_usd = excluded.total_cost_usd,
        source = excluded.source
```

d) Update the doc comment from "63 bind parameters" to "64 bind parameters" (line 15).

e) Add bind in `execute_upsert_parsed_session()` after `.bind(s.total_cost_usd) // ?63` (line 194):
```rust
        .bind(&s.source) // ?64
```

**Step 7: Set `source` in the indexer's ParsedSession construction**

In `crates/db/src/indexer_parallel.rs`, in the `ParsedSession { ... }` block (around line 3182, after `total_cost_usd:`):
```rust
                source: "live".to_string(), // Default; overridden by source tag in Task 5
```

**Step 8: Update test helpers that construct SessionInfo**

Search for `make_session` and `make_test_session` in tests — add `source: "live".to_string()` to each. Key locations:
- `crates/server/src/routes/sessions.rs:637`
- `crates/server/src/routes/turns.rs:362`
- `crates/server/src/routes/export.rs:258`
- `crates/server/src/routes/projects.rs:112`
- `crates/db/tests/queries_shared.rs:5`
- `crates/db/src/queries/dashboard.rs:977`
- `crates/core/src/types.rs:1238`
- `crates/core/src/patterns/mod.rs:199,273`
- `crates/server/src/routes/insights.rs:297` (the `into_session_info` impl)

**Step 9: Verify it compiles and tests pass**

Run: `cargo check` then `cargo test`
Expected: compiles, all tests pass (migration adds column with DEFAULT 'live')

**Step 10: Commit**

```bash
git add crates/db/src/migrations.rs crates/db/src/queries/row_types.rs \
  crates/db/src/queries/sessions.rs crates/db/src/indexer_parallel.rs \
  crates/core/src/types.rs crates/server/src/routes/ crates/db/tests/ \
  crates/db/src/queries/dashboard.rs crates/core/src/patterns/
git commit -m "feat(db): add source column to sessions table (migration 40)

Tracks whether a session is 'live' (from ~/.claude/) or 'archived'
(from backup). Threaded through ParsedSession -> SessionRow ->
SessionInfo -> API response."
```

---

### Task 5: Wire sources into `scan_and_index_all`

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs:2866-3200` (scan_and_index_all signature + body)
- Modify: `crates/server/src/main.rs:14,406-490` (startup + re-scan loop)
- Modify: `crates/db/Cargo.toml` (add claude-view-core dep if not present)

**Step 1: Change `scan_and_index_all` to accept sources**

In `crates/db/src/indexer_parallel.rs`, change the signature (line 2866):

From:
```rust
pub async fn scan_and_index_all<F, T>(
    claude_dir: &Path,
    db: &Database,
    hints: &HashMap<String, IndexHints>,
    search_index: Option<Arc<claude_view_search::SearchIndex>>,
    registry: Option<Arc<Registry>>,
    on_file_done: F,
    on_total_known: T,
) -> Result<(usize, usize), String>
```

To:
```rust
pub async fn scan_and_index_all<F, T>(
    sources: &[Box<dyn claude_view_core::SessionSource>],
    db: &Database,
    hints: &HashMap<String, IndexHints>,
    search_index: Option<Arc<claude_view_search::SearchIndex>>,
    registry: Option<Arc<Registry>>,
    on_file_done: F,
    on_total_known: T,
) -> Result<(usize, usize), String>
```

**Step 2: Replace hardcoded directory walk with source discovery + dedup**

Replace lines 2879-2930 (the project directory walk) with:

```rust
    // When the search index was rebuilt (schema version mismatch), force re-parse
    let force_search_reindex = search_index
        .as_ref()
        .map(|idx| idx.needs_full_reindex)
        .unwrap_or(false);

    if force_search_reindex {
        tracing::info!(
            "Search index was rebuilt — forcing full re-parse to repopulate search data"
        );
    }

    // Discover sessions from all sources, dedup by UUID (earlier sources win)
    let mut seen_uuids = std::collections::HashSet::new();
    let mut files: Vec<(std::path::PathBuf, String, String, String)> = Vec::new(); // (path, project_encoded, session_id, source_tag)

    for source in sources {
        match source.discover() {
            Ok(discovered) => {
                let before = files.len();
                for session in discovered {
                    if seen_uuids.insert(session.uuid.clone()) {
                        files.push((
                            session.file_path,
                            session.project_hash,
                            session.uuid,
                            session.source_tag.as_str().to_string(),
                        ));
                    }
                }
                let added = files.len() - before;
                tracing::info!(
                    source = source.name(),
                    discovered = added,
                    "Session source discovery complete"
                );
            }
            Err(e) => {
                tracing::warn!(source = source.name(), error = %e, "Session source discovery failed");
            }
        }
    }
```

**Step 3: Thread source_tag through the parse loop**

In the `for (path, project_encoded, session_id) in files` loop (line 2961), change the destructuring to:
```rust
    for (path, project_encoded, session_id, source_tag) in files {
```

And in the `ParsedSession { ... }` construction (around line 3182), change:
```rust
                source: "live".to_string(),
```
to:
```rust
                source: source_tag,
```

**Step 4: Use sources for byte reading in parse_file_bytes**

The existing `parse_file_bytes()` function reads directly from disk (mmap or fs::read). For backup sessions (`.jsonl.gz`), this won't work — we need the source's `read_bytes()`.

Store a reference to the sources vec and look up the right source by source_tag. However, since `parse_file_bytes` runs in `spawn_blocking`, the simplest approach is: detect `.jsonl.gz` extension and decompress inline.

Add a new function after `parse_file_bytes` (line 647):
```rust
/// Parse a session file, handling both raw .jsonl and .jsonl.gz.
fn parse_session_file(path: &std::path::Path) -> ParseResult {
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    if file_name.ends_with(".jsonl.gz") {
        // Decompress gzipped backup file
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return ParseResult::default(),
        };
        let mut decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
        let mut data = Vec::new();
        if decoder.read_to_end(&mut data).is_err() {
            return ParseResult::default();
        }
        parse_bytes(&data)
    } else {
        parse_file_bytes(path)
    }
}
```

Then in the spawn_blocking call (around line 3008), change:
```rust
            let parse_result =
                tokio::task::spawn_blocking(move || parse_file_bytes(&path_for_parse))
```
to:
```rust
            let parse_result =
                tokio::task::spawn_blocking(move || parse_session_file(&path_for_parse))
```

Add `use std::io::Read;` at the top of the file if not already present.

**Step 5: Update main.rs — build sources vec at startup**

In `crates/server/src/main.rs`, add import (around line 14):
```rust
use claude_view_core::{LiveSource, ClaudeBackupSource, SessionSource};
```

Replace the initial `scan_and_index_all` call (around lines 416-428). Change from:
```rust
        match scan_and_index_all(
            &claude_dir,
            &idx_db,
            &hints,
            ...
```
to:
```rust
        // Build session sources
        let mut sources: Vec<Box<dyn SessionSource>> = vec![
            Box::new(LiveSource::new(&claude_dir)),
        ];
        if let Some(backup) = ClaudeBackupSource::detect() {
            let count = backup.session_count();
            tracing::info!(sessions = count, "claude-backup detected, adding archived sessions");
            sources.push(Box::new(backup));
        }

        match scan_and_index_all(
            &sources,
            &idx_db,
            &hints,
            ...
```

Do the same for the re-scan loop (around lines 481-489):
```rust
        // Build sources for re-scan (same logic)
        let mut rescan_sources: Vec<Box<dyn SessionSource>> = vec![
            Box::new(LiveSource::new(&claude_dir)),
        ];
        if let Some(backup) = ClaudeBackupSource::detect() {
            rescan_sources.push(Box::new(backup));
        }

        match scan_and_index_all(
            &rescan_sources,
            &idx_db,
            &hints,
            ...
```

**Step 6: Add `flate2` to crates/db/Cargo.toml**

```toml
flate2 = "1"
```

**Step 7: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: compiles

**Step 8: Commit**

```bash
git add crates/db/src/indexer_parallel.rs crates/server/src/main.rs crates/db/Cargo.toml
git commit -m "feat: wire SessionSource into scan_and_index_all

Indexer now accepts Vec<Box<dyn SessionSource>> instead of a hardcoded
path. Discovers from all sources, dedup by UUID (live wins).
Handles .jsonl.gz decompression for backup sessions.
Auto-detects ~/.claude-backup at startup."
```

---

### Task 6: Add `source` to frontend types and UI badge

**Files:**
- Modify: `apps/web/src/types/generated/SessionInfo.ts:7-18`
- Modify: `apps/web/src/types/generated/SessionDetail.ts:9-28`
- Modify: `apps/web/src/components/SessionCard.tsx:145-200`
- Modify: `apps/web/src/components/CompactSessionTable.tsx`

**Step 1: Add `source` field to TS types**

In `apps/web/src/types/generated/SessionInfo.ts`, add at the end of the type (before the closing `};`):
```typescript
source?: string,
```

In `apps/web/src/types/generated/SessionDetail.ts`, add at the end of the type (before the closing `};`):
```typescript
source?: string,
```

Note: These files say "Do not edit manually" (ts-rs generated), but ts-rs generates from the Rust `SessionInfo` struct. After Task 4, running `cargo test -p claude-view-core` will regenerate these. If not auto-regenerated, add manually — the field is optional so it's backward-compatible either way.

**Step 2: Add archive badge to SessionCard**

In `apps/web/src/components/SessionCard.tsx`, find the project label area (around line 169 after `const projectLabel`). In the JSX, find where the session date/time is rendered. Add the badge right after the date display.

Look for the timestamp rendering (search for `formatDistanceToNow` or the date display). Add after it:
```tsx
{session.source === 'archived' && (
  <span
    className="inline-flex items-center text-[10px] font-medium px-1.5 py-0.5 rounded bg-zinc-100 text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400"
    title="From backup archive — this session is no longer in ~/.claude/"
  >
    archived
  </span>
)}
```

**Step 3: Add archive indicator to CompactSessionTable**

In `apps/web/src/components/CompactSessionTable.tsx`, find where the session preview/title is rendered in the table row. Add after the title text:
```tsx
{row.original.source === 'archived' && (
  <span className="ml-1.5 text-[10px] text-zinc-400 dark:text-zinc-500">
    archived
  </span>
)}
```

**Step 4: Build frontend**

Run: `cd apps/web && bun run build`
Expected: builds successfully

**Step 5: Commit**

```bash
git add apps/web/src/types/generated/SessionInfo.ts \
  apps/web/src/types/generated/SessionDetail.ts \
  apps/web/src/components/SessionCard.tsx \
  apps/web/src/components/CompactSessionTable.tsx
git commit -m "feat(web): add archived badge for backup sessions

Shows subtle 'archived' badge on sessions from backup. No separate
tab or filter — archived sessions appear in the unified list."
```

---

### Task 7: Integration test — end-to-end verification

**Files:**
- Test only (no new files)

**Step 1: Run full Rust test suite for changed crates**

Run:
```bash
cargo test -p claude-view-core session_source && \
cargo test -p claude-view-db migrations:: && \
cargo test -p claude-view-server
```
Expected: all pass

**Step 2: Manual verification with real data**

Run:
```bash
cargo run -p claude-view-server
```

Then check in browser at `http://localhost:47892`:
1. Sessions list loads (no regression)
2. If ~/.claude-backup exists with sessions, archived sessions appear with badge
3. Search works across both live and archived sessions
4. No duplicate sessions (same UUID should not appear twice)

**Step 3: Check logs for backup detection**

Look for in server output:
```
INFO claude-backup detected, adding archived sessions
INFO source=live discovered=5249 Session source discovery complete
INFO source=claude-backup discovered=600 Session source discovery complete
```

**Step 4: Verify staleness skip on re-scan**

Wait 2 minutes for periodic re-scan. Check logs — should show nearly all sessions skipped (staleness check).

**Step 5: Commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: integration fixes for unified session history"
```
