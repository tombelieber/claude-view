---
status: done
date: 2026-02-10
---

# Git Sync SSE Progress

Replace fire-and-forget + HTTP polling with SSE real-time progress for git sync, replicating the proven pattern from rebuild index.

## Current State

| Layer | Pattern | Problem |
|-------|---------|---------|
| Backend | `POST /api/sync/git` returns 202, no progress feedback | Blind fire-and-forget |
| Frontend hook | `use-git-sync.ts` tracks HTTP state only | No progress |
| StatusBar UI | Polls `/api/status` every 1s for 30s, compares timestamps | Wastes bandwidth, 30s timeout arbitrary, no visibility |

## Target State

| Layer | Pattern | Benefit |
|-------|---------|---------|
| Backend | Atomic `GitSyncState` + SSE endpoint | Lock-free progress, single HTTP connection |
| Frontend hook | `useGitSyncProgress` via `EventSource` | Real-time events, auto-close on terminal state |
| StatusBar UI | Inline progress: "Scanning repo 3/10" -> "Linking commits..." -> toast summary | Interactive, informative |

---

## Phase A: Backend (Rust)

### Step 1: Create `crates/server/src/git_sync_state.rs`

Atomic progress state (adapted from `indexing_state.rs`):

> **Key divergence from indexing:** `IndexingState` has no `reset()` — it's created once per server start and runs a single indexing pass. `GitSyncState` needs `reset()` because users trigger multiple syncs via `POST /api/sync/git` without restarting the server. The same `Arc<GitSyncState>` lives in `AppState` for the server's lifetime, and `reset()` clears stale counters before each new sync.

```rust
pub enum GitSyncPhase {
    Idle = 0,
    Scanning = 1,     // Scanning repos for commits
    Correlating = 2,  // Linking commits to sessions
    Done = 3,
    Error = 4,
}

pub struct GitSyncState {
    phase: AtomicU8,
    repos_scanned: AtomicUsize,
    total_repos: AtomicUsize,
    commits_found: AtomicUsize,
    sessions_correlated: AtomicUsize,
    total_correlatable_sessions: AtomicUsize,
    links_created: AtomicUsize,
    error: RwLock<Option<String>>,
}
```

Same `Ordering::Relaxed` strategy. Same `set_error()` pattern that also sets phase to `Error`.

**`reset()` method** — must zero ALL counters and clear error before each new sync:

```rust
pub fn reset(&self) {
    self.phase.store(GitSyncPhase::Idle as u8, Ordering::Relaxed);
    self.repos_scanned.store(0, Ordering::Relaxed);
    self.total_repos.store(0, Ordering::Relaxed);
    self.commits_found.store(0, Ordering::Relaxed);
    self.sessions_correlated.store(0, Ordering::Relaxed);
    self.total_correlatable_sessions.store(0, Ordering::Relaxed);
    self.links_created.store(0, Ordering::Relaxed);
    if let Ok(mut guard) = self.error.write() {
        *guard = None;
    }
}
```

### Step 2: Register module in `crates/server/src/lib.rs`

Add `pub mod git_sync_state;` and `pub use git_sync_state::{GitSyncState, GitSyncPhase};`.

### Step 3: Add `git_sync: Arc<GitSyncState>` to `AppState`

In `crates/server/src/state.rs`:
- Add field to `AppState` struct
- Initialize in all constructors (`new()`, `new_with_indexing()`, `new_with_indexing_and_registry()`)

No new constructor variants needed — `GitSyncState` is created once in each `AppState` constructor and lives for the server's lifetime behind `Arc`. Route handlers `.clone()` the `Arc` to call `reset()` + update atomics across multiple user-triggered syncs. This differs from `IndexingState` which also lives in `Arc` but never resets (single run).

### Step 4: Add progress callback to `run_git_sync()`

In `crates/db/src/git_correlation.rs`, change signature:

```rust
pub async fn run_git_sync<F>(db: &Database, on_progress: F) -> DbResult<GitSyncResult>
where
    F: Fn(GitSyncProgress) + Send + 'static,
```

> **Why `Send + 'static` but NOT `Sync`?** The callback is called sequentially within
> `run_git_sync` (never shared across threads). `Send` is needed because the future
> is `Send` (held across `.await`). `Sync` is unnecessary.

Progress enum (defined in the db crate, NOT server):

```rust
pub enum GitSyncProgress {
    ScanningStarted { total_repos: usize },
    RepoScanned { repos_done: usize, total_repos: usize, commits_in_repo: u32 },
    CorrelatingStarted { total_correlatable_sessions: usize },
    SessionCorrelated { sessions_done: usize, total_correlatable_sessions: usize, links_in_session: u32 },
}
```

Insert callback invocations at 4 points in `run_git_sync`:
1. After grouping sessions by repo -> `ScanningStarted { total_repos: sessions_by_repo.len() }`
2. After each `scan_repo_commits` (only when commits found, i.e. after `result.repos_scanned += 1`) -> `RepoScanned`
3. Before session correlation loop -> `CorrelatingStarted` (**see critical note below**)
4. After each `correlate_session` call (inside the `Ok` and `Err` arms, NOT after `continue`) -> `SessionCorrelated`

**Critical: `total_correlatable_sessions` must count only sessions that HAVE matching repos, not all eligible sessions.** Compute this before the correlation loop:

```rust
// Count sessions that actually have commits to correlate against
let correlatable_count = sessions.iter()
    .filter(|s| commits_by_repo.contains_key(&s.project_path))
    .count();
on_progress(GitSyncProgress::CorrelatingStarted {
    total_correlatable_sessions: correlatable_count,
});
```

This prevents the UX bug where progress shows "5/100" then jumps to done because 95 sessions had no commits.

**Gotcha:** `commits_found` must accumulate via `fetch_add`, not `store`, since the callback fires per-repo with that repo's count.

### Step 5: Create SSE endpoint `GET /api/sync/git/progress`

In `crates/server/src/routes/sync.rs`. Same pattern as `indexing_progress()` in `indexing.rs`:

SSE event names (deliberately different from indexing events):

| Event | When | Data |
|-------|------|------|
| `scanning` | Repos being scanned | `{phase, reposScanned, totalRepos, commitsFound}` |
| `correlating` | Linking commits to sessions | `{phase, sessionsCorrelated, totalCorrelatableSessions, commitsFound, linksCreated}` |
| `done` | Sync complete | `{phase, reposScanned, commitsFound, linksCreated}` |
| `error` | Sync failed | `{phase, message}` |

Polls atomics every 100ms. Stream terminates on `done` or `error`.

### Step 6: Wire progress callback in `trigger_git_sync`

In `crates/server/src/routes/sync.rs`:

1. Call `git_sync.reset()` BEFORE `tokio::spawn` — clears all counters + error from previous run so SSE clients never see stale data
2. Set phase to `Scanning` BEFORE `tokio::spawn` — so SSE clients that connect immediately see active state, not `Idle`
3. Build progress closure that updates atomics via match on `GitSyncProgress` variants:

```rust
let git_sync_cb = git_sync.clone();
let on_progress = move |p: GitSyncProgress| {
    match p {
        GitSyncProgress::ScanningStarted { total_repos } => {
            git_sync_cb.set_total_repos(total_repos);
        }
        GitSyncProgress::RepoScanned { repos_done, commits_in_repo, .. } => {
            git_sync_cb.set_repos_scanned(repos_done);
            git_sync_cb.add_commits_found(commits_in_repo as usize); // fetch_add, NOT store
        }
        GitSyncProgress::CorrelatingStarted { total_correlatable_sessions } => {
            git_sync_cb.set_phase(GitSyncPhase::Correlating);
            git_sync_cb.set_total_correlatable_sessions(total_correlatable_sessions);
        }
        GitSyncProgress::SessionCorrelated { sessions_done, links_in_session, .. } => {
            git_sync_cb.set_sessions_correlated(sessions_done);
            git_sync_cb.add_links_created(links_in_session as usize); // fetch_add, NOT store
        }
    }
};
```

4. Pass closure to `run_git_sync(&db, on_progress)`
5. On completion, set phase to `Done`; on error, call `set_error()`

**Race condition note:** If the sync finishes before the frontend opens EventSource, the SSE handler sees `Done` immediately and emits `done` on first poll -- correct behavior.

### Step 6b: Update `main.rs` call sites

`crates/server/src/main.rs` has `run_git_sync_logged()` which calls `run_git_sync(&db)` for both initial and periodic sync. These run in the background with no UI consumer.

**Pass a no-op callback:**

```rust
async fn run_git_sync_logged(db: &Database, label: &str) {
    let start = Instant::now();
    tracing::info!(sync_type = label, "Starting git sync");

    match vibe_recall_db::git_correlation::run_git_sync(db, |_| {}).await {
        // ... existing match arms unchanged
    }
}
```

**Why no-op, not wired to `GitSyncState`?** If the periodic sync wrote to the same `GitSyncState` atomics, a user who opens the dashboard mid-periodic-sync would see phantom progress for a sync they didn't trigger. The SSE endpoint is for user-initiated syncs only. Periodic sync reports via `tracing::info!` (already existing).

### Step 7: Register the SSE route

```rust
.route("/sync/git/progress", get(git_sync_progress))
```

### Step 8: Update route docs in `routes/mod.rs`

Add `GET /api/sync/git/progress` to the doc comment.

---

## Phase B: Frontend Hook

### Step 9: Create `src/hooks/use-git-sync-progress.ts`

Modeled directly on `use-indexing-progress.ts`:

```typescript
export type GitSyncPhase = 'idle' | 'scanning' | 'correlating' | 'done' | 'error'

export interface GitSyncProgress {
  phase: GitSyncPhase
  reposScanned: number
  totalRepos: number
  commitsFound: number
  sessionsCorrelated: number
  totalCorrelatableSessions: number
  linksCreated: number
  errorMessage?: string
}

const INITIAL_STATE: GitSyncProgress = {
  phase: 'idle',
  reposScanned: 0,
  totalRepos: 0,
  commitsFound: 0,
  sessionsCorrelated: 0,
  totalCorrelatableSessions: 0,
  linksCreated: 0,
}
```

- `sseUrl()` bypasses Vite proxy in dev mode (port 5173 -> `localhost:47892`)
- EventSource listeners for `scanning`, `correlating`, `done`, `error`
- Same error event handling pattern (MessageEvent vs plain Event)
- Closes on terminal state or unmount

**Do NOT extract a shared `sseUrl()` helper with `use-indexing-progress.ts` -- YAGNI. Two concrete instances is fine.**

### Step 10: Verify `src/hooks/use-git-sync.ts` — no changes needed

The hook is already a clean HTTP-only trigger (POST + status tracking). It has NO polling logic — all polling currently lives in `StatusBar.tsx`. The hook's existing API surface is correct as-is:

- `triggerSync()` -> `POST /api/sync/git` (returns `boolean`)
- `status: SyncStatus` (`'idle' | 'running' | 'success' | 'conflict' | 'error'`) — keep all existing values, no renames
- `isLoading: boolean`
- `error: string | null`
- `response: SyncAcceptedResponse | null`
- `reset()`

Completion detection moves to the SSE hook. The HTTP hook's `'success'` status (meaning "POST returned 202") is still useful to distinguish from `'conflict'` and `'error'`.

---

## Phase C: Frontend UI (StatusBar)

### Step 11: Update `src/components/StatusBar.tsx`

**Delete:**
- Entire `pollForCompletion` callback (lines 41-118)
- `prevStatusRef` for delta tracking
- `setTimeout(poll, 1000)` retry loop
- `lastToastRef` dedup — **replaced by `doneHandledRef`** (see below)

**Add:**
```tsx
const [sseEnabled, setSseEnabled] = useState(false)
const progress = useGitSyncProgress(sseEnabled)

// Guard against React strict-mode double-firing of the terminal-state effect.
// Reset to false when sseEnabled transitions to true (new sync started).
const doneHandledRef = useRef(false)
```

**Derive `isSyncing` from SSE phase** (replaces HTTP-only `isSyncing`):
```tsx
const isSseActive = sseEnabled && progress.phase !== 'idle' && progress.phase !== 'done' && progress.phase !== 'error'
const isSpinning = isStatusLoading || isSyncing || isSseActive
```

**Wire handleRefresh:**
```tsx
const handleRefresh = async () => {
  if (isSpinning) return
  doneHandledRef.current = false
  const started = await triggerSync()
  if (started) setSseEnabled(true)
}
```

**Handle retry** (with Retry button in toast — preserves existing UX):
```tsx
const handleRetry = useCallback(async () => {
  resetSync()
  doneHandledRef.current = false
  const started = await triggerSync()
  if (started) setSseEnabled(true)
}, [triggerSync, resetSync])
```

**React to SSE terminal states:**
```tsx
useEffect(() => {
  if (progress.phase === 'done' && !doneHandledRef.current) {
    doneHandledRef.current = true
    toast.success('Sync completed', {
      description: `${progress.reposScanned} repos | ${progress.commitsFound} commits | ${progress.linksCreated} links`,
    })
    queryClient.invalidateQueries({ queryKey: ['status'] })
    queryClient.invalidateQueries({ queryKey: ['dashboard-stats'] })
    queryClient.invalidateQueries({ queryKey: ['projects'] })
    setSseEnabled(false)
    resetSync()
  } else if (progress.phase === 'error' && !doneHandledRef.current) {
    doneHandledRef.current = true
    toast.error('Sync failed', {
      description: progress.errorMessage ?? 'Unknown error',
      duration: 6000,
      action: {
        label: 'Retry',
        onClick: handleRetry,
      },
    })
    setSseEnabled(false)
  }
}, [progress.phase, progress.reposScanned, progress.commitsFound, progress.linksCreated, progress.errorMessage, queryClient, resetSync, handleRetry])
```

> **Why `doneHandledRef`?** SSE delivers terminal events exactly once per connection, but React 18 strict mode double-fires effects in development. The ref prevents duplicate toasts. It's reset to `false` in `handleRefresh`/`handleRetry` before starting a new sync.
>
> **Why full deps array?** ESLint `exhaustive-deps` requires all referenced values. `queryClient` and `resetSync` are stable refs (won't trigger re-fires), and the progress fields are only read inside the `phase === 'done'` branch which fires once per sync. The `doneHandledRef` guard makes this safe regardless.

**Inline progress display** (replaces `isSyncing ? 'Syncing...'` text):
```tsx
{isSseActive ? (
  <span className="animate-pulse text-xs">
    {progress.phase === 'scanning'
      ? progress.totalRepos > 0
        ? `Scanning repo ${progress.reposScanned}/${progress.totalRepos}...`
        : 'Scanning repos...'
      : progress.phase === 'correlating'
        ? progress.totalCorrelatableSessions > 0
          ? `Linking sessions ${progress.sessionsCorrelated}/${progress.totalCorrelatableSessions}... (${progress.linksCreated} links)`
          : `Linking commits... (${progress.linksCreated} links)`
        : 'Starting sync...'}
  </span>
) : /* existing normal display */}
```

---

## Testing

### Step 13: Unit tests for `GitSyncState`

Copy test structure from `indexing_state.rs`. Test cases:

1. `initial_state_is_idle_with_zeroes` — all counters 0, phase Idle, error None
2. `phase_transitions` — Idle -> Scanning -> Correlating -> Done -> Idle (reset cycle)
3. `counter_increments_store` — `set_repos_scanned`, `set_total_repos`, `set_sessions_correlated`, `set_total_correlatable_sessions`
4. `counter_increments_fetch_add` — `add_commits_found` and `add_links_created` accumulate correctly across multiple calls
5. `error_state` — `set_error()` sets phase to Error AND stores message, overwrite works
6. `reset_clears_everything` — set all fields to non-zero, call `reset()`, verify all zero and error cleared
7. `thread_safety_concurrent_access` — 8 threads × 100 iterations: mix of `add_commits_found`, `add_links_created`, `set_repos_scanned`; verify `commits_found == 800` and `links_created == 800`
8. `from_u8_invalid_returns_none` — values 5, 255 return None
9. `default_impl` — `GitSyncState::default()` matches `new()`

### Step 14: SSE endpoint tests

Three test cases (copy pattern from `indexing.rs` tests):

1. **`test_sse_done_emits_done_event`** — Set `GitSyncState` to `Done` with known values (`repos_scanned=3, commits_found=42, links_created=7`), verify SSE stream body contains `event: done` with expected JSON fields
2. **`test_sse_error_emits_error_event`** — Call `set_error("disk full")`, verify SSE body contains `event: error` and `"disk full"`
3. **`test_sse_content_type`** — Verify response header `Content-Type: text/event-stream`

**Test helper:** Add `create_app_with_git_sync(db, git_sync)` in `lib.rs`, following the exact pattern of `create_app_with_indexing(db, indexing)`. Tests pre-configure a `GitSyncState` (set phase, counters, error), pass it to the helper, then assert on the SSE stream output:

```rust
pub fn create_app_with_git_sync(db: Database, git_sync: Arc<GitSyncState>) -> Router {
    let state = AppState { git_sync, ..AppState::new(db) };
    api_routes().with_state(Arc::new(state))
}
```

### Step 15: Verify all `run_git_sync` call sites compile

The signature change (added callback param) will cause compile errors at **3 call sites**:

| File | Call site | Fix |
|------|-----------|-----|
| `crates/server/src/routes/sync.rs` | `trigger_git_sync` | Wire progress closure to `GitSyncState` atomics (Step 6) |
| `crates/server/src/main.rs` | `run_git_sync_logged` (initial) | Pass no-op `\|_\| {}` (Step 6b) |
| `crates/server/src/main.rs` | `run_git_sync_logged` (periodic) | Same no-op — shares `run_git_sync_logged` |

Run `cargo check -p vibe-recall-server` to verify. Then `cargo check -p vibe-recall-db` for the db crate changes.

---

## Gotchas

1. **`commits_found` and `links_created` accumulation:** Use `fetch_add` (via `add_commits_found` / `add_links_created`) in the atomics, not `store`, since callbacks fire per-repo/per-session with incremental counts
2. **POST 202 vs SSE connect race:** If sync finishes before EventSource connects, SSE handler sees `Done` immediately and emits `done` on first poll -- correct behavior
3. **Multiple tabs:** Both see same events from same `GitSyncState` -- fine, atomics are read-only from SSE handler
4. **Vite proxy:** Must use `sseUrl()` bypass in dev mode
5. **`AppState` constructors:** All must initialize `git_sync: Arc::new(GitSyncState::new())` -- compile error catches this
6. **`reset()` before `tokio::spawn`:** Must call `git_sync.reset()` then set phase to `Scanning` BEFORE spawning, so the SSE client never sees stale Done/Error from a previous run
7. **`main.rs` periodic sync uses no-op callback:** Never wire periodic sync to `GitSyncState` — would show phantom progress for a sync the user didn't trigger
8. **`total_correlatable_sessions` vs `sessions.len()`:** Only count sessions that have matching repos in `commits_by_repo`, not all eligible sessions. Otherwise progress jumps from 5/100 to done.
9. **React strict-mode double-fire:** Use `doneHandledRef` guard in the terminal-state `useEffect` to prevent duplicate toasts in development
10. **ESLint exhaustive-deps:** Include all accessed values in the deps array (`queryClient`, `resetSync`, `handleRetry`, progress fields). The `doneHandledRef` guard makes this safe regardless of extra fires.

## Key Files

| File | Action |
|------|--------|
| `crates/server/src/git_sync_state.rs` | **Create** -- atomic progress state with `reset()` method |
| `crates/server/src/lib.rs` | Modify -- register module, add `create_app_with_git_sync` (for tests) |
| `crates/server/src/state.rs` | Modify -- add `git_sync` field to `AppState` |
| `crates/db/src/git_correlation.rs` | Modify -- add `GitSyncProgress` enum + callback param to `run_git_sync` |
| `crates/server/src/routes/sync.rs` | Modify -- SSE handler, wire callback, register route |
| `crates/server/src/routes/mod.rs` | Modify -- update route docs |
| `crates/server/src/main.rs` | Modify -- pass no-op `\|_\| {}` to `run_git_sync` in `run_git_sync_logged` |
| `src/hooks/use-git-sync-progress.ts` | **Create** -- EventSource hook |
| `src/hooks/use-git-sync.ts` | **No changes** -- already a clean HTTP trigger |
| `src/components/StatusBar.tsx` | Modify -- replace polling with SSE, inline progress, Retry button |
