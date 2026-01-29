---
status: done
date: 2026-01-29
---

# Phase 3B: Git Sync Orchestrator — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire up the existing git correlation building blocks into a working orchestrator that runs on server startup and via `POST /api/sync/git`.

**Architecture:** A `run_git_sync()` function in `crates/db/src/git_correlation.rs` iterates all sessions, groups by `project_path`, scans each repo for commits, correlates (Tier 2 for auto-sync, Tier 1+2 when skill data available), upserts results, and updates `index_metadata`. Called from server startup (after indexing) and from the sync route.

**Tech Stack:** Rust (Axum, sqlx, tokio), existing git_correlation module functions.

**Priorities (ordered):**
1. **Correctness** — never corrupt data, never lose existing Tier 1 links on re-sync
2. **Safety** — all errors contained per-repo, never panic, never block the server
3. **Robustness** — handle missing dirs, bare repos, corrupt repos, timeouts, empty DBs gracefully
4. **Performance** — group-by-repo dedup, single transaction per batch, parallel-ready scan, no N+1 queries

---

## Context: What Already Exists

All building blocks are implemented and tested:

| Function | Location | Status |
|----------|----------|--------|
| `scan_repo_commits()` | `crates/db/src/git_correlation.rs:112` | Done, tested |
| `tier1_match()` | `crates/db/src/git_correlation.rs:327` | Done, tested |
| `tier2_match()` | `crates/db/src/git_correlation.rs:381` | Done, tested |
| `correlate_session()` | `crates/db/src/git_correlation.rs:655` | Done, tested |
| `batch_upsert_commits()` | `crates/db/src/git_correlation.rs:428` | Done, tested |
| `batch_insert_session_commits()` | `crates/db/src/git_correlation.rs:472` | Done, tested |
| `count_commits_for_session()` | `crates/db/src/git_correlation.rs:606` | Done, tested |
| `update_session_commit_count()` | `crates/db/src/git_correlation.rs:618` | Done, tested |
| `update_git_sync_metadata_on_success()` | `crates/db/src/trends.rs:359` | Done, tested, **never called** |
| `SessionCorrelationInfo` | `crates/db/src/git_correlation.rs:639` | Done |
| `CommitSkillInvocation` | `crates/db/src/indexer_parallel.rs:68` | Done |

**Key signatures for reference:**

```rust
// git_correlation.rs:112 — returns ScanResult { commits, not_a_repo, error }
pub async fn scan_repo_commits(repo_path: &Path, since_timestamp: Option<i64>, limit: Option<usize>) -> ScanResult

// git_correlation.rs:655 — returns number of matches inserted
pub async fn correlate_session(db: &Database, session: &SessionCorrelationInfo, commits: &[GitCommit]) -> DbResult<usize>

// git_correlation.rs:428 — returns rows affected (uses tx: BEGIN...COMMIT)
pub async fn batch_upsert_commits(&self, commits: &[GitCommit]) -> DbResult<u64>

// git_correlation.rs:472 — checks existing tier before INSERT OR REPLACE (uses tx)
pub async fn batch_insert_session_commits(&self, matches: &[CorrelationMatch]) -> DbResult<u64>

// trends.rs:359 — updates index_metadata row (last_git_sync_at, commits_found, links_created)
pub async fn update_git_sync_metadata_on_success(&self, commits_found: i64, links_created: i64) -> DbResult<()>
```

**Database schema facts:**
- `sessions` table has `first_message_at INTEGER` column (migrations.rs:16) — exists in DB, but NOT in `SessionRow` struct or `list_projects()` SELECT
- `sessions` table has `project_path TEXT NOT NULL DEFAULT ''` — already indexed
- `index_metadata` table has `last_git_sync_at`, `commits_found`, `links_created` columns
- `commits` table: PRIMARY KEY on `hash`
- `session_commits` table: composite PK on `(session_id, commit_hash)`, has `tier` and `evidence` columns
- `Database` struct (`crates/db/src/lib.rs:46-49`): wraps `SqlitePool`, implements `Clone`
- `DbResult<T>` = `Result<T, DbError>` (lib.rs:43)

## What's Missing

1. **`run_git_sync()` orchestrator** — No function ties scan + correlate + metadata together
2. **`POST /api/sync/git` is a stub** — Returns 202 but spawns no work (`crates/server/src/routes/sync.rs:52`)
3. **No auto git-sync on startup** — Server never calls any git correlation code
4. **`first_message_at` not in `SessionRow`** — Query at `crates/db/src/queries.rs:240` doesn't SELECT it; Tier 2 needs it
5. **No query to fetch correlation-ready session data** — Need a lightweight query returning `(id, project_path, first_message_at, last_message_at)` for all sessions
6. **Frontend refresh button is broken** — StatusBar refresh icon (`src/components/StatusBar.tsx:43`) only re-fetches `/api/status` metadata; it does NOT trigger `POST /api/sync/git`. "Last synced" text disappears when `lastIndexedAt` is null (never indexed). The button should trigger a real re-index + git sync and invalidate dashboard queries on completion.

---

## Task 1: Add `first_message_at` to SessionRow and query

**Files:**
- Modify: `crates/db/src/queries.rs`

**Exact changes:**

### 1a. Add field to `SessionRow` struct (line 1283, before `commit_count`)

```rust
// Current (line 1282):
    duration_seconds: i32,
    commit_count: i32,

// Add between duration_seconds and commit_count:
    duration_seconds: i32,
    first_message_at: Option<i64>,
    commit_count: i32,
```

### 1b. Add `s.first_message_at` to `list_projects()` SELECT (line 259)

```sql
-- Current (line 258-259):
                s.files_read_count, s.files_edited_count, s.reedited_files_count,
                s.duration_seconds, s.commit_count

-- Change to:
                s.files_read_count, s.files_edited_count, s.reedited_files_count,
                s.duration_seconds, s.first_message_at, s.commit_count
```

### 1c. Update `FromRow` impl (after line 1299)

The `FromRow` impl for `SessionRow` starts at line 1286. Add the field read after `duration_seconds`:

```rust
// Add to the FromRow impl, after duration_seconds line:
first_message_at: row.try_get("first_message_at")?,
```

### 1d. Check if `SessionRow` → `SessionInfo` conversion uses all fields

The `first_message_at` field is only needed internally by `get_sessions_for_git_sync()` (Task 2). The `SessionInfo` struct (which is the API-facing type) does NOT need it — it's already exposed via the session detail endpoint if needed. So the `From<SessionRow> for SessionInfo` conversion can simply ignore it, OR we can add it to `SessionInfo` if the frontend wants it later. For now: **do not add to SessionInfo** — this is an internal-only field for git sync.

**Verification:** After this change, `cargo test -p db -- queries` should pass. The `list_projects()` query now returns `first_message_at` for each session.

**Why:** Tier 2 correlation (`tier2_match()` at git_correlation.rs:381) requires `session_start` and `session_end` timestamps. The session start is `first_message_at`. Currently `SessionRow` only has `last_message_at` (line 1250). The column exists in SQLite (migrations.rs:16) and is populated by the indexer — it just isn't read by the query.

---

## Task 2: Add lightweight query for git-sync session data

**Files:**
- Modify: `crates/db/src/git_correlation.rs` (add struct + impl block on Database, after line 631)

**Exact code to add (insert after line 631, before the `SessionCorrelationInfo` struct at line 637):**

```rust
// ============================================================================
// Git Sync Session Query
// ============================================================================

/// Lightweight session data for git correlation.
/// Contains only the 4 fields needed — no JOINs, no JSON arrays, no token sums.
#[derive(Debug, Clone)]
pub struct SessionSyncInfo {
    pub session_id: String,
    pub project_path: String,
    pub first_message_at: Option<i64>,
    pub last_message_at: Option<i64>,
}

impl Database {
    /// Fetch all sessions eligible for git correlation.
    ///
    /// Filters:
    /// - `project_path` must be non-empty (sessions without a project can't have a repo)
    /// - `last_message_at` must be non-NULL (need at least one timestamp for time window)
    ///
    /// This is deliberately lightweight: a single-table SELECT with no JOINs.
    /// For 500 sessions, this returns in <1ms vs list_projects() which takes ~50ms
    /// due to the turns LEFT JOIN and heavy column set.
    ///
    /// Performance: indexed on `project_path` (implicit via schema), and filtered by
    /// non-NULL `last_message_at` which has an index (`idx_sessions_last_message`).
    pub async fn get_sessions_for_git_sync(&self) -> DbResult<Vec<SessionSyncInfo>> {
        let rows: Vec<(String, String, Option<i64>, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT id, project_path, first_message_at, last_message_at
            FROM sessions
            WHERE project_path != '' AND last_message_at IS NOT NULL
            ORDER BY last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(session_id, project_path, first_message_at, last_message_at)| {
                SessionSyncInfo {
                    session_id,
                    project_path,
                    first_message_at,
                    last_message_at,
                }
            })
            .collect())
    }
}
```

**Why separate from Task 1:** Task 1 adds `first_message_at` to the heavy `list_projects()` query for completeness. Task 2 creates a purpose-built lightweight query that skips the `LEFT JOIN turns` subquery, the 30+ columns, and JSON array fields. The orchestrator calls this, not `list_projects()`.

**Verification:** `cargo test -p db -- git_correlation` — add a test that inserts sessions with/without project_path and timestamps, then asserts `get_sessions_for_git_sync()` returns only eligible ones.

---

## Task 3: Write `run_git_sync()` orchestrator

**Files:**
- Modify: `crates/db/src/git_correlation.rs` (add function after the `correlate_session` function, around line 706)

**Exact code:**

```rust
// ============================================================================
// Git Sync Orchestrator
// ============================================================================

/// Result of a full git sync run.
#[derive(Debug, Clone, Default)]
pub struct GitSyncResult {
    /// Number of unique repositories scanned.
    pub repos_scanned: u32,
    /// Total commits found across all repos.
    pub commits_found: u32,
    /// Total session-commit links created or updated.
    pub links_created: u32,
    /// Non-fatal errors encountered (one per failed repo).
    pub errors: Vec<String>,
}

/// Run the full git sync pipeline: scan repos → correlate sessions → update metadata.
///
/// # Algorithm
///
/// 1. Fetch all eligible sessions via `get_sessions_for_git_sync()` (lightweight, no JOINs)
/// 2. Group sessions by unique `project_path` using a HashMap
/// 3. For each unique project_path (deduped — avoids scanning same repo N times):
///    a. Call `scan_repo_commits(path, None, None)` — no time bound, default limit 100
///    b. If `not_a_repo`: skip silently (many sessions point to non-git dirs)
///    c. If `error`: log warning, record in `errors`, skip to next repo
///    d. Call `batch_upsert_commits()` — INSERT OR UPDATE into commits table (single tx)
///    e. Store commits in a HashMap<project_path, Vec<GitCommit>> for correlation phase
/// 4. For each session (iterate all sessions, not grouped):
///    a. Look up repo commits from the HashMap using session's project_path
///    b. Build `SessionCorrelationInfo` with empty `commit_skills` (Tier 2 only)
///    c. Call `correlate_session(db, &info, &commits)` — handles Tier 1+2, dedup, insert
///    d. Accumulate `links_created` count
/// 5. Call `update_git_sync_metadata_on_success(commits_found, links_created)` on trends.rs
/// 6. Return `GitSyncResult`
///
/// # Safety & Robustness
///
/// - **Per-repo error isolation:** A failing repo does NOT abort the sync.
///   Error is logged and recorded; remaining repos proceed normally.
/// - **Idempotent:** Safe to run multiple times. `batch_upsert_commits` uses ON CONFLICT UPDATE.
///   `batch_insert_session_commits` checks existing tier before replacing (only upgrades).
///   `update_session_commit_count` is a simple overwrite.
/// - **No data loss:** Existing Tier 1 links from pass_2 indexing are never downgraded.
///   The tier check at git_correlation.rs:497-499 ensures Tier 2 never overwrites Tier 1.
/// - **No blocking:** This function is `async` and yields at every I/O point.
///   `scan_repo_commits` uses `tokio::process::Command` with 10s timeout.
/// - **Transaction safety:** `batch_upsert_commits` wraps all INSERTs in BEGIN/COMMIT.
///   `batch_insert_session_commits` does the same. No partial writes.
///
/// # Performance
///
/// - **Group-by-repo dedup:** 500 sessions across 10 projects = 10 git commands, not 500.
///   Each `git log` takes ~200ms. Total scan phase: ~2 seconds.
/// - **Lightweight query:** `get_sessions_for_git_sync()` fetches 4 columns, no JOINs.
///   For 500 sessions: <1ms.
/// - **Batch writes:** All DB writes use transactions. 100 commits = 1 transaction, not 100.
/// - **No redundant scans:** commits are cached in HashMap by project_path.
///   Correlation reads from memory, not from DB or git again.
///
/// # Tier Strategy
///
/// Auto-sync (this function) produces **Tier 2 only** because we don't have
/// `CommitSkillInvocation` data at this stage — that data is extracted during pass_2
/// deep indexing of individual session JSONL files. The `commit_skills` field is set to
/// empty vec, so `tier1_match()` inside `correlate_session()` returns no matches.
///
/// Tier 1 links are created by `pass_2_deep_index()` when it encounters commit skill
/// invocations in the JSONL data. These are higher priority (tier=1) and will NOT be
/// overwritten by this function's Tier 2 links (ensured by the tier check in
/// `batch_insert_session_commits` at line 497-499).
pub async fn run_git_sync(db: &Database) -> DbResult<GitSyncResult> {
    let mut result = GitSyncResult::default();

    // Step 1: Fetch all eligible sessions
    let sessions = db.get_sessions_for_git_sync().await?;
    if sessions.is_empty() {
        tracing::debug!("Git sync: no eligible sessions found");
        // Still update metadata to record that sync ran successfully
        db.update_git_sync_metadata_on_success(0, 0).await?;
        return Ok(result);
    }

    tracing::info!("Git sync: {} eligible sessions", sessions.len());

    // Step 2: Group sessions by project_path to deduplicate repo scans
    let mut sessions_by_repo: std::collections::HashMap<String, Vec<&SessionSyncInfo>> =
        std::collections::HashMap::new();
    for session in &sessions {
        sessions_by_repo
            .entry(session.project_path.clone())
            .or_default()
            .push(session);
    }

    tracing::info!(
        "Git sync: {} unique project paths to scan",
        sessions_by_repo.len()
    );

    // Step 3: Scan each unique repo and upsert commits
    let mut commits_by_repo: std::collections::HashMap<String, Vec<GitCommit>> =
        std::collections::HashMap::new();

    for (project_path, _sessions) in &sessions_by_repo {
        let path = std::path::Path::new(project_path);
        let scan = scan_repo_commits(path, None, None).await;

        if scan.not_a_repo {
            // Not a git repo — skip silently. Many Claude sessions are in non-git dirs.
            continue;
        }

        if let Some(err) = &scan.error {
            // Git error (timeout, corrupt repo, permission denied, etc.)
            // Log warning but continue — don't abort the whole sync for one bad repo.
            tracing::warn!(
                "Git sync: error scanning {}: {}",
                project_path,
                err
            );
            result.errors.push(format!("{}: {}", project_path, err));
            continue;
        }

        if scan.commits.is_empty() {
            // Repo exists but has no commits (empty repo or all filtered out)
            continue;
        }

        result.repos_scanned += 1;
        result.commits_found += scan.commits.len() as u32;

        // Batch upsert all commits from this repo (single transaction)
        db.batch_upsert_commits(&scan.commits).await?;

        // Cache commits for correlation phase
        commits_by_repo.insert(project_path.clone(), scan.commits);
    }

    tracing::info!(
        "Git sync: scanned {} repos, found {} commits",
        result.repos_scanned,
        result.commits_found
    );

    // Step 4: Correlate each session with its repo's commits
    for session in &sessions {
        let commits = match commits_by_repo.get(&session.project_path) {
            Some(c) => c,
            None => continue, // No commits for this repo (non-git, error, or empty)
        };

        // Build correlation info — empty commit_skills = Tier 2 only
        let info = SessionCorrelationInfo {
            session_id: session.session_id.clone(),
            project_path: session.project_path.clone(),
            first_timestamp: session.first_message_at,
            last_timestamp: session.last_message_at,
            commit_skills: Vec::new(), // No skill data in auto-sync → Tier 2 only
        };

        match correlate_session(db, &info, commits).await {
            Ok(links) => {
                result.links_created += links as u32;
            }
            Err(e) => {
                // Per-session error: log and continue
                tracing::warn!(
                    "Git sync: correlation failed for session {}: {}",
                    session.session_id,
                    e
                );
                result.errors.push(format!(
                    "session {}: {}",
                    session.session_id,
                    e
                ));
            }
        }
    }

    // Step 5: Update metadata to record successful sync
    db.update_git_sync_metadata_on_success(
        result.commits_found as i64,
        result.links_created as i64,
    )
    .await?;

    tracing::info!(
        "Git sync complete: {} repos, {} commits, {} links, {} errors",
        result.repos_scanned,
        result.commits_found,
        result.links_created,
        result.errors.len()
    );

    Ok(result)
}
```

**Design decisions:**

| Decision | Choice | Why |
|----------|--------|-----|
| Tier 2 only for auto-sync | `commit_skills: Vec::new()` | Skill data is only available during pass_2 JSONL parsing. Auto-sync has no access to raw JSONL. |
| Tier 1 preserved | `batch_insert_session_commits` checks `m.tier < existing_tier` at line 499 | Higher-priority Tier 1 links from pass_2 are never overwritten by Tier 2. |
| Group by project_path | `HashMap<String, Vec<&SessionSyncInfo>>` | 500 sessions / 10 projects = 10 git scans, not 500. |
| Per-repo error isolation | `tracing::warn!` + `result.errors.push()` + `continue` | One corrupt repo doesn't abort 9 healthy ones. |
| scan_repo_commits(path, None, None) | No time filtering, default limit 100 | `tier2_match()` handles the time window internally. We want all commits available for matching. |
| Metadata update on empty DB | `update_git_sync_metadata_on_success(0, 0)` | Records that sync ran so `lastGitSyncAt` is non-null in the StatusBar. |
| `commits_by_repo` cache | In-memory HashMap of scan results | Avoids re-reading from DB or re-running git for correlation phase. |

**Verification:** `cargo test -p db -- run_git_sync` — tests from Task 6 cover this.

---

## Task 4: Wire sync route to actually run git sync

**Files:**
- Modify: `crates/server/src/routes/sync.rs`

**Problem with current design:** The file imports `tokio::sync::Mutex` (line 13) and uses `mutex.try_lock()` (line 49). `tokio::sync::Mutex` returns `tokio::sync::MutexGuard` which IS `Send` — so it CAN be moved across `tokio::spawn`. This is the correct choice. The current code already uses `tokio::sync::Mutex`, not `std::sync::Mutex`.

**Exact replacement — replace the ENTIRE `trigger_git_sync` function (lines 43-73) with:**

```rust
/// POST /api/sync/git - Trigger git commit scanning.
///
/// Returns:
/// - 202 Accepted: Sync started in background (no sync was running)
/// - 409 Conflict: Sync already in progress
///
/// The sync runs as a background tokio task. Poll GET /api/status for completion
/// (the `lastGitSyncAt` field updates when sync finishes).
///
/// # Concurrency Safety
///
/// Uses a `tokio::sync::Mutex<()>` with `try_lock()`:
/// - If lock acquired: spawn background task that holds the guard for its lifetime
/// - If lock busy: return 409 immediately (no blocking, no queuing)
/// - Guard is moved into the spawned task via `tokio::spawn` (safe because
///   `tokio::sync::MutexGuard` is `Send`, unlike `std::sync::MutexGuard`)
/// - When the background task completes (success or error), the guard drops,
///   allowing the next sync request to proceed
pub async fn trigger_git_sync(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Response> {
    let mutex = get_sync_mutex();

    // Try to acquire without blocking — fail fast if sync is already running
    match mutex.try_lock() {
        Ok(guard) => {
            // We got the lock — spawn background task that holds it
            let db = state.db.clone();
            tokio::spawn(async move {
                // Hold the mutex guard for the entire duration of the sync.
                // When this task completes (success or error), _guard drops and
                // the mutex is released, allowing the next POST /api/sync/git.
                let _guard = guard;

                tracing::info!("Git sync triggered via API");
                match vibe_recall_db::git_correlation::run_git_sync(&db).await {
                    Ok(result) => {
                        tracing::info!(
                            "Git sync complete: {} repos, {} commits, {} links, {} errors",
                            result.repos_scanned,
                            result.commits_found,
                            result.links_created,
                            result.errors.len(),
                        );
                    }
                    Err(e) => {
                        tracing::error!("Git sync failed: {}", e);
                    }
                }
            });

            let response = SyncAcceptedResponse {
                message: "Git sync initiated".to_string(),
                status: "accepted".to_string(),
            };

            Ok((StatusCode::ACCEPTED, Json(response)).into_response())
        }
        Err(_) => {
            // Mutex is locked — sync already running
            Err(ApiError::Conflict(
                "Git sync already in progress. Please wait for it to complete.".to_string(),
            ))
        }
    }
}
```

**Key correctness details:**
- `tokio::sync::MutexGuard` is `Send` — moving into `tokio::spawn` compiles correctly
- The guard is held for the ENTIRE background task duration — not just the lock acquisition
- 202 response returns IMMEDIATELY (before sync starts) — non-blocking to the client
- On error, the guard still drops (Rust's RAII), so the mutex is always released
- No race condition: `try_lock()` is atomic, and only one `spawn` can succeed at a time

**Also update imports at top of file (line 1-17) — add `vibe_recall_db::git_correlation`:**

The import `vibe_recall_db::git_correlation::run_git_sync` is used inline as a fully-qualified path in the spawn block. No new `use` import needed at module level — the `vibe_recall_db` crate is already a dependency of `vibe_recall_server`.

**Also update the State parameter (line 44) — change `State(_state)` to `State(state)`:**

The current code has `State(_state)` (unused). Change to `State(state)` since we now need `state.db`.

**Verification:** `cargo test -p server -- sync` should pass. The existing `test_sync_git_accepted` test (line 115-125) should still work because `run_git_sync` on an in-memory DB with no sessions returns immediately.

---

## Task 5: Add auto git-sync on server startup

**Files:**
- Modify: `crates/server/src/main.rs`

**Exact change — modify the background indexing `tokio::spawn` block (lines 93-125):**

Current code (lines 119-124):
```rust
        .await;

        if let Err(e) = result {
            idx_state.set_error(e);
        }
    });
```

Replace with:
```rust
        .await;

        match result {
            Ok(_) => {
                // Auto git-sync: correlate commits with sessions after indexing completes.
                // This populates commit_count for all sessions so the dashboard shows
                // git data on first load without requiring a manual sync.
                //
                // Performance: iterates unique project_paths (~10 repos) and runs
                // `git log` per repo (~200ms each). Total: ~2 seconds for 10 projects.
                // Acceptable as a post-indexing background task.
                tracing::info!("Starting auto git sync...");
                match vibe_recall_db::git_correlation::run_git_sync(&idx_db).await {
                    Ok(sync_result) => {
                        tracing::info!(
                            "Auto git sync complete: {} repos, {} commits, {} links",
                            sync_result.repos_scanned,
                            sync_result.commits_found,
                            sync_result.links_created,
                        );
                        if !sync_result.errors.is_empty() {
                            tracing::warn!(
                                "Auto git sync had {} errors: {:?}",
                                sync_result.errors.len(),
                                sync_result.errors,
                            );
                        }
                    }
                    Err(e) => {
                        // Git sync failure is non-fatal — the server works fine without it.
                        // Users can manually trigger via POST /api/sync/git later.
                        tracing::warn!("Auto git sync failed (non-fatal): {}", e);
                    }
                }
            }
            Err(e) => {
                idx_state.set_error(e);
            }
        }
    });
```

**Why this placement:**
- Runs AFTER `run_background_index` succeeds — all sessions are in the DB
- Runs INSIDE the existing background `tokio::spawn` — does NOT block the HTTP server
- Does NOT run if indexing failed (`Err(e)` branch) — no point syncing with corrupt data
- The server is already serving requests while this runs (started at line 82)

**Safety:**
- Git sync failure is logged as `warn!`, not `error!` — it's non-fatal
- The server continues operating normally regardless of git sync outcome
- The `IndexingStatus::Done` is set BEFORE git sync starts (by `on_complete` callback at line 116-118). This is correct — the TUI progress spinner should finish when indexing completes, not wait for git sync.

**Performance note:** The TUI spinner at lines 152-193 already finishes when `IndexingStatus::Done` is set. Git sync runs after that — invisible to the user in the terminal. The StatusBar in the browser will show "Last synced: just now" once git sync updates `index_metadata`.

**Verification:** `cargo build -p server` should compile. Start the server with `bun dev` and verify "Auto git sync complete" appears in the terminal output after "Deep index complete".

---

## Task 6: Tests

**Files:**
- Modify: `crates/db/src/git_correlation.rs` (add tests to existing `#[cfg(test)] mod tests` block at line 712)

**Exact tests to add (append inside the `mod tests` block before the closing `}`):**

```rust
    // ========================================================================
    // get_sessions_for_git_sync tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_sessions_for_git_sync_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let sessions = db.get_sessions_for_git_sync().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_get_sessions_for_git_sync_filters_correctly() {
        let db = Database::new_in_memory().await.unwrap();

        // Session 1: eligible (has project_path and last_message_at)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', '/home/user/project-a', 1000, 2000, '/tmp/s1.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        // Session 2: ineligible (empty project_path)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, last_message_at, file_path)
             VALUES ('s2', 'p2', '', 3000, '/tmp/s2.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        // Session 3: ineligible (NULL last_message_at)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, file_path)
             VALUES ('s3', 'p3', '/home/user/project-b', '/tmp/s3.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        // Session 4: eligible (has project_path and last_message_at, no first_message_at)
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, last_message_at, file_path)
             VALUES ('s4', 'p4', '/home/user/project-a', 4000, '/tmp/s4.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        let sessions = db.get_sessions_for_git_sync().await.unwrap();
        assert_eq!(sessions.len(), 2);

        // Ordered by last_message_at DESC
        assert_eq!(sessions[0].session_id, "s4");
        assert_eq!(sessions[0].project_path, "/home/user/project-a");
        assert_eq!(sessions[0].first_message_at, None);
        assert_eq!(sessions[0].last_message_at, Some(4000));

        assert_eq!(sessions[1].session_id, "s1");
        assert_eq!(sessions[1].project_path, "/home/user/project-a");
        assert_eq!(sessions[1].first_message_at, Some(1000));
        assert_eq!(sessions[1].last_message_at, Some(2000));
    }

    // ========================================================================
    // run_git_sync tests
    // ========================================================================

    #[tokio::test]
    async fn test_run_git_sync_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let result = run_git_sync(&db).await.unwrap();

        assert_eq!(result.repos_scanned, 0);
        assert_eq!(result.commits_found, 0);
        assert_eq!(result.links_created, 0);
        assert!(result.errors.is_empty());

        // Metadata should still be updated (records that sync ran)
        let meta = db.get_index_metadata().await.unwrap();
        assert!(meta.last_git_sync_at.is_some());
    }

    #[tokio::test]
    async fn test_run_git_sync_non_git_dirs() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert sessions pointing to a temp dir that is NOT a git repo
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, 1000, 2000, '/tmp/s1.jsonl')"
        )
        .bind(dir)
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db).await.unwrap();

        // Non-git dir is silently skipped
        assert_eq!(result.repos_scanned, 0);
        assert_eq!(result.commits_found, 0);
        assert_eq!(result.links_created, 0);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_run_git_sync_with_real_repo() {
        let db = Database::new_in_memory().await.unwrap();

        // Create a temp git repo with a commit
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        // git init + configure + create a commit
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config name");

        std::fs::write(repo_path.join("file.txt"), "hello").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        // Get the commit timestamp
        let output = std::process::Command::new("git")
            .args(["log", "-1", "--format=%at"])
            .current_dir(repo_path)
            .output()
            .expect("git log timestamp");
        let commit_ts: i64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap();

        // Insert a session whose time window includes the commit
        let dir_str = repo_path.to_str().unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')"
        )
        .bind(dir_str)
        .bind(commit_ts - 600) // session started 10 min before commit
        .bind(commit_ts + 600) // session ended 10 min after commit
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db).await.unwrap();

        assert_eq!(result.repos_scanned, 1);
        assert_eq!(result.commits_found, 1);
        assert_eq!(result.links_created, 1); // Tier 2 match
        assert!(result.errors.is_empty());

        // Verify the link was created in the DB
        let commits = db.get_commits_for_session("s1").await.unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].1, 2); // Tier 2

        // Verify session commit_count was updated
        let count = db.count_commits_for_session("s1").await.unwrap();
        assert_eq!(count, 1);

        // Verify metadata was updated
        let meta = db.get_index_metadata().await.unwrap();
        assert!(meta.last_git_sync_at.is_some());
        assert_eq!(meta.commits_found, Some(1));
        assert_eq!(meta.links_created, Some(1));
    }

    #[tokio::test]
    async fn test_run_git_sync_deduplicates_repos() {
        let db = Database::new_in_memory().await.unwrap();

        // Create a temp git repo
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config name");

        std::fs::write(repo_path.join("file.txt"), "hello").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        // Insert TWO sessions pointing to the SAME repo
        let dir_str = repo_path.to_str().unwrap();
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')"
        )
        .bind(dir_str)
        .bind(now - 7200)
        .bind(now + 7200)
        .execute(db.pool())
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s2', 'p1', ?1, ?2, ?3, '/tmp/s2.jsonl')"
        )
        .bind(dir_str)
        .bind(now - 3600)
        .bind(now + 3600)
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db).await.unwrap();

        // Only 1 repo scanned despite 2 sessions
        assert_eq!(result.repos_scanned, 1);
        // Both sessions should get linked
        assert_eq!(result.links_created, 2);
    }

    #[tokio::test]
    async fn test_run_git_sync_nonexistent_dir() {
        let db = Database::new_in_memory().await.unwrap();

        // Session pointing to a directory that doesn't exist
        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', '/nonexistent/path/abc123', 1000, 2000, '/tmp/s1.jsonl')"
        )
        .execute(db.pool())
        .await
        .unwrap();

        let result = run_git_sync(&db).await.unwrap();

        // Should skip gracefully — not_a_repo for nonexistent dirs
        assert_eq!(result.repos_scanned, 0);
        assert_eq!(result.links_created, 0);
        assert!(result.errors.is_empty()); // not_a_repo is not an error, it's silent skip
    }

    #[tokio::test]
    async fn test_run_git_sync_idempotent() {
        let db = Database::new_in_memory().await.unwrap();

        // Create a temp git repo with a commit
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("git init");

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .expect("git config email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .expect("git config name");

        std::fs::write(repo_path.join("file.txt"), "hello").unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        let dir_str = repo_path.to_str().unwrap();
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path)
             VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')"
        )
        .bind(dir_str)
        .bind(now - 7200)
        .bind(now + 7200)
        .execute(db.pool())
        .await
        .unwrap();

        // Run sync TWICE
        let result1 = run_git_sync(&db).await.unwrap();
        let result2 = run_git_sync(&db).await.unwrap();

        // First run creates a link
        assert_eq!(result1.links_created, 1);
        // Second run: link already exists at same tier, so 0 new links
        // (batch_insert_session_commits checks existing tier at line 497-499)
        assert_eq!(result2.links_created, 0);

        // Only 1 link total in DB
        let count = db.count_commits_for_session("s1").await.unwrap();
        assert_eq!(count, 1);
    }
```

**Also add a `use` import for `chrono` in the test module (if not already present):**

Check if `chrono::Utc` is available. Since `trends.rs` uses `Utc::now()`, the `chrono` crate is already a dependency of `vibe_recall_db`. The test module can use it directly.

**Verification:** `cargo test -p db -- git_correlation` — all new and existing tests should pass.

---

## Task 7: Fix frontend refresh button to trigger real sync

**Files:**
- Modify: `src/components/StatusBar.tsx`
- The existing `src/hooks/use-git-sync.ts` hook is already implemented and provides `triggerSync()`, `status`, `isLoading` — **reuse it, don't reinvent**

### Current state analysis

**StatusBar.tsx (59 lines):**
- Line 10: `const { data: status, isLoading: isStatusLoading, refetch } = useStatus()`
- Line 17-18: Only shows "Last synced" when `status?.lastIndexedAt` is truthy
- Line 43-55: Refresh button calls `refetch()` which just GETs `/api/status`

**use-git-sync.ts (101 lines) — already exists and is correct:**
- `triggerSync()` — POSTs to `/api/sync/git`, handles 202/409/error
- `status` — `'idle' | 'running' | 'success' | 'conflict' | 'error'`
- `isLoading` — true while POST is in flight
- Already invalidates `['status']` query on success (line 61)
- **Problem:** Only invalidates `['status']`, not dashboard/projects queries

### Exact changes to StatusBar.tsx

**Replace the entire file content with:**

```tsx
import { RefreshCw } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { useStatus, formatRelativeTime } from '../hooks/use-status'
import { useGitSync } from '../hooks/use-git-sync'
import { useQueryClient } from '@tanstack/react-query'

interface StatusBarProps {
  projects: ProjectSummary[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const { data: status, isLoading: isStatusLoading } = useStatus()
  const { triggerSync, isLoading: isSyncing } = useGitSync()
  const queryClient = useQueryClient()
  const totalSessions = projects.reduce((sum, p) => sum + p.sessionCount, 0)

  // Format sessions count from status (index metadata) or fallback to project count
  const sessionsIndexed = status?.sessionsIndexed ? Number(status.sessionsIndexed) : totalSessions

  // Format last synced time — show "Not yet synced" when null instead of hiding
  const lastSyncedText = status?.lastIndexedAt
    ? formatRelativeTime(status.lastIndexedAt)
    : null

  const isSpinning = isStatusLoading || isSyncing

  const handleRefresh = async () => {
    if (isSpinning) return

    const started = await triggerSync()
    if (started) {
      // Sync was accepted (202) — the background task is running.
      // Poll status more frequently until lastGitSyncAt updates.
      // Also invalidate data queries so dashboard/session views refresh.
      setTimeout(() => {
        queryClient.invalidateQueries({ queryKey: ['status'] })
        queryClient.invalidateQueries({ queryKey: ['dashboard-stats'] })
        queryClient.invalidateQueries({ queryKey: ['projects'] })
      }, 2000) // Wait 2s for sync to likely complete, then refresh
    }
    // If not started (409 conflict), useGitSync already set status='conflict'
  }

  return (
    <footer
      className="h-8 bg-white border-t border-gray-200 px-4 flex items-center justify-between text-xs text-gray-500"
      role="contentinfo"
      aria-label="Data freshness status"
    >
      <div className="flex items-center gap-1.5">
        {isStatusLoading ? (
          <span className="animate-pulse">Loading status...</span>
        ) : isSyncing ? (
          <span className="animate-pulse">Syncing...</span>
        ) : lastSyncedText ? (
          <>
            <span>Last synced: {lastSyncedText}</span>
            <span aria-hidden="true">&middot;</span>
            <span aria-label={`${sessionsIndexed} sessions indexed`}>
              {sessionsIndexed.toLocaleString()} sessions
            </span>
          </>
        ) : (
          <span>Not yet synced &middot; {projects.length} projects &middot; {totalSessions} sessions</span>
        )}
      </div>

      <button
        type="button"
        onClick={handleRefresh}
        disabled={isSpinning}
        className="p-1 -mr-1 rounded hover:bg-gray-100 transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 disabled:opacity-50 cursor-pointer"
        aria-label={isSyncing ? 'Sync in progress' : 'Trigger sync'}
        title={isSyncing ? 'Sync in progress...' : 'Sync data'}
      >
        <RefreshCw
          className={`w-3.5 h-3.5 ${isSpinning ? 'animate-spin' : ''}`}
          aria-hidden="true"
        />
      </button>
    </footer>
  )
}
```

### Changes explained

| Change | What | Why |
|--------|------|-----|
| Import `useGitSync` | Line 4: `import { useGitSync } from '../hooks/use-git-sync'` | Reuse existing hook that handles POST /api/sync/git correctly |
| Import `useQueryClient` | Line 5: `import { useQueryClient } from '@tanstack/react-query'` | Needed to invalidate dashboard/projects queries after sync |
| Remove `refetch` | No longer destructured from `useStatus()` | Replaced by `triggerSync()` which does POST, not GET |
| Add `isSyncing` state | From `useGitSync().isLoading` | Shows spinner during actual sync POST |
| `handleRefresh` async | Calls `triggerSync()` then invalidates queries after 2s delay | Gives the backend time to complete sync before refreshing UI data |
| "Not yet synced" text | Else branch now shows `Not yet synced · N projects · N sessions` | Instead of hiding freshness info when `lastIndexedAt` is null |
| "Syncing..." text | New branch when `isSyncing` is true | Visual feedback that sync is running |
| Button aria-label | Dynamic: "Sync in progress" vs "Trigger sync" | Accessibility for screen readers |

### Why the 2s delay strategy

The sync is a background task (202 response = "started, not finished"). We can't know exactly when it completes. Options considered:

| Strategy | Pros | Cons | Chosen? |
|----------|------|------|---------|
| Poll /api/status every 500ms until lastGitSyncAt changes | Precise | Complex, requires tracking prev value | No |
| Invalidate after fixed 2s delay | Simple, works for typical sync (~2s for 10 repos) | May miss if sync takes >2s | **Yes** |
| Use the existing refetchInterval (30s) | Zero code | User waits up to 30s for visual update | No (too slow) |
| SSE/WebSocket push | Real-time | Over-engineered for this use case | No |

The 2s delay is good enough: the status query's existing `refetchInterval: 30_000` (use-status.ts:39) will eventually catch up even if the 2s delay is too early. The user sees "Syncing..." immediately, then "Last synced: just now" within a few seconds.

**Verification:** Start the app with `bun dev`, click the refresh icon in the footer. Should show "Syncing..." with spinning icon, then update to "Last synced: just now" within a few seconds.

---

## Task 8: Update PROGRESS.md

**Files:**
- Modify: `docs/plans/PROGRESS.md`

**What:**
- Add a row in the Plan File Index table:

```
| Phase 3B: Git Sync Orchestrator | 2026-01-29 | done | Wire git sync orchestrator, fix sync route, auto-sync on startup, fix frontend refresh |
```

- Update the "At a Glance" table if Phase 3 has a row — mark git sync gaps as addressed
- Move the Phase 3B entry from "Queued Work" to the completed section

**Also update this plan file's frontmatter from `draft` to `done`:**

```yaml
---
status: done
date: 2026-01-29
---
```

---

## Summary

| Task | Files | Description | Acceptance Criteria |
|------|-------|-------------|---------------------|
| 1 | `crates/db/src/queries.rs` | Add `first_message_at` to SessionRow + list_projects() SELECT + FromRow impl | `cargo test -p db -- queries` passes; field is populated |
| 2 | `crates/db/src/git_correlation.rs` | Add `SessionSyncInfo` struct + `get_sessions_for_git_sync()` query (4 columns, no JOINs) | Returns eligible sessions; filters empty project_path and NULL timestamps |
| 3 | `crates/db/src/git_correlation.rs` | Add `GitSyncResult` struct + `run_git_sync()` orchestrator (scan→correlate→metadata) | Groups by repo, isolates errors, updates metadata, returns accurate counts |
| 4 | `crates/server/src/routes/sync.rs` | Replace stub with real background task holding tokio::sync::MutexGuard across spawn | POST returns 202 and sync actually runs; 409 when already running |
| 5 | `crates/server/src/main.rs` | Add `run_git_sync()` call after `run_background_index` succeeds | commit_count populated after server start; failure is non-fatal |
| 6 | `crates/db/src/git_correlation.rs` | 7 tests: empty DB, non-git dirs, real repo, dedup, nonexistent dir, idempotent, filter query | `cargo test -p db -- git_correlation` all pass |
| 7 | `src/components/StatusBar.tsx` | Reuse `useGitSync` hook, replace refetch() with triggerSync(), add "Not yet synced" fallback | Button triggers POST /api/sync/git, shows spinner, updates after sync completes |
| 8 | `docs/plans/PROGRESS.md` + this file | Add Phase 3B entry, mark as done | Docs current |

## Task Dependencies

```
Task 1 ──┐
         ├──→ Task 3 ──→ Task 4 ──→ Task 5
Task 2 ──┘                  │
                            └──→ Task 6
Task 7 (frontend — independent, can run in parallel with Tasks 1-6)
Task 8 (docs — run last)
```

## Invariants to Protect

These MUST hold true after all tasks are implemented:

1. **Tier 1 links are never downgraded to Tier 2.** Ensured by `batch_insert_session_commits()` at git_correlation.rs:497-499 which checks `m.tier < existing_tier` before replacing.

2. **A failing repo never aborts the entire sync.** `run_git_sync()` catches errors per-repo and per-session, logs them, and continues.

3. **The HTTP server is never blocked by git sync.** Auto-sync runs inside an already-spawned background task (main.rs line 93). API sync runs in a new `tokio::spawn` (sync.rs).

4. **Only one git sync runs at a time.** `tokio::sync::Mutex<()>` with `try_lock()` in sync.rs. The guard is held for the entire background task duration.

5. **`index_metadata.last_git_sync_at` is only updated on successful sync completion.** Ensured by `update_git_sync_metadata_on_success()` being called only at the end of `run_git_sync()` after all repos are processed.

6. **The frontend always shows sync status.** "Not yet synced" when `lastIndexedAt` is null; "Last synced: X ago" otherwise. Never a blank footer.

## Dependencies

- Phase 3 backend (steps 1-27) — **DONE**
- All git correlation building blocks — **DONE**
- Phase 3 frontend wiring fixes (separate from this plan) — **IN PROGRESS**
