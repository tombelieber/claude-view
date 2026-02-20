---
status: done
date: 2026-02-03
---

# Cold Start UX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** When a user with 50-100 GB of data runs `npx claude-view` for the first time, they see immediate progress feedback with bandwidth stats — not a blank screen.

**Architecture:** Extend existing SSE `/api/indexing/progress` with bandwidth fields (bytes processed, throughput, total size). Add a frontend cold-start overlay that shows a progress bar with data throughput. Enhance TUI output with bandwidth display.

**Tech Stack:** Rust (Axum SSE, atomics), React (EventSource hook), indicatif (terminal)

---

## Context

**Current cold start flow:**
1. Server binds port instantly (good)
2. Pass 1 reads `sessions-index.json` files (<1s for any data size)
3. Pass 2 deep-indexes every JSONL file (the slow part)
4. Frontend polls `/api/status` every 10s (too slow for cold start)
5. SSE `/api/indexing/progress` emits `deep-progress` events with `indexed/total` session counts

**Problem:** For a 100 GB dataset (~56,000 sessions), Pass 2 takes ~38 seconds. The user sees session counts ticking up but has no sense of:
- How much data is being processed (GB)
- How fast it's going (GB/s)
- How long the whole thing will take

**Desired cold start experience:**
```
$ npx claude-view

  claude-view v0.3.0

  Scanning your Claude Code history...
  Found 56,000 sessions across 42 projects (1.2s)

  Browse now → http://localhost:47892

  Deep indexing your conversations...
  ████████████░░░░░░░░  23.4 GB / 52.1 GB   (2.7 GB/s)
  12,847 / 56,000 sessions

  ✓ Deep index complete — 56,000 sessions, 52.1 GB processed (38.2s)
```

**Frontend cold start overlay:**
```
┌─────────────────────────────────────────────┐
│  Setting up claude-view...                  │
│                                             │
│  ████████████░░░░░░░░  45%                  │
│  23.4 GB / 52.1 GB  ·  2.7 GB/s            │
│  12,847 / 56,000 sessions                   │
│                                             │
│  You can browse sessions while we finish.   │
│  Analytics will appear as indexing completes.│
└─────────────────────────────────────────────┘
```

---

### Task 1: Extend IndexingState with Bandwidth Atomics

**Files:**
- Modify: `crates/server/src/indexing_state.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_bandwidth_tracking() {
    let state = IndexingState::new();
    assert_eq!(state.bytes_processed(), 0);
    assert_eq!(state.bytes_total(), 0);

    state.set_bytes_total(52_100_000_000); // 52.1 GB
    state.add_bytes_processed(1_000_000_000); // 1 GB
    state.add_bytes_processed(500_000_000); // 0.5 GB

    assert_eq!(state.bytes_total(), 52_100_000_000);
    assert_eq!(state.bytes_processed(), 1_500_000_000);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p server -- indexing_state::test_bandwidth_tracking -v`
Expected: FAIL — `bytes_processed()` method doesn't exist

**Step 3: Add atomic fields to IndexingState**

Add to the `IndexingState` struct:

```rust
bytes_processed: AtomicU64,  // cumulative bytes of JSONL parsed so far
bytes_total: AtomicU64,      // total bytes of all JSONL files to parse
```

Add methods:

```rust
pub fn bytes_processed(&self) -> u64 {
    self.bytes_processed.load(Ordering::Relaxed)
}
pub fn add_bytes_processed(&self, bytes: u64) {
    self.bytes_processed.fetch_add(bytes, Ordering::Relaxed);
}
pub fn bytes_total(&self) -> u64 {
    self.bytes_total.load(Ordering::Relaxed)
}
pub fn set_bytes_total(&self, bytes: u64) {
    self.bytes_total.store(bytes, Ordering::Relaxed);
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p server -- indexing_state::test_bandwidth_tracking -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/server/src/indexing_state.rs
git commit -m "feat(server): add bandwidth tracking atomics to IndexingState"
```

---

### Task 2: Feed Bandwidth Data from Indexer

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs`
- Modify: `crates/server/src/main.rs`

**Step 1: Compute total JSONL bytes before Pass 2**

In `pass_2_deep_index()`, after determining which sessions need deep indexing, sum their `size_bytes` fields. Pass this total to a new callback (or set it on IndexingState directly).

The `on_file_done` callback signature should be extended (or a new `on_bytes_progress` callback added) to report bytes processed per file.

**Step 2: Set bytes_total before Pass 2 starts**

In `main.rs`, in the `on_pass1_done` callback or just before Pass 2 begins:

```rust
// Sum total bytes of sessions that need deep indexing
let total_bytes: u64 = sessions_to_index.iter().map(|s| s.size_bytes).sum();
indexing_state.set_bytes_total(total_bytes);
```

**Step 3: Report bytes_processed per file completion**

In the `on_file_done` callback (or alongside it), add:

```rust
indexing_state.add_bytes_processed(session.size_bytes);
```

This gives us file-level granularity — good enough. Each JSONL file completes in milliseconds, so the progress bar updates frequently.

**Step 4: Run existing tests**

Run: `cargo test -p db -- indexer_parallel`
Expected: All existing tests still pass

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs crates/server/src/main.rs
git commit -m "feat(server): feed bytes_processed/bytes_total from indexer to IndexingState"
```

---

### Task 3: Extend SSE Events with Bandwidth Fields

**Files:**
- Modify: `crates/server/src/routes/indexing.rs`

**Step 1: Write the test**

Add a test that verifies `deep-progress` events include `bytes_processed` and `bytes_total` fields.

**Step 2: Extend `deep-progress` SSE event payload**

Current:
```json
{"status": "deep-indexing", "indexed": 12847, "total": 56000}
```

New:
```json
{
  "status": "deep-indexing",
  "indexed": 12847,
  "total": 56000,
  "bytes_processed": 23400000000,
  "bytes_total": 52100000000
}
```

Also extend the `done` event:
```json
{
  "status": "done",
  "indexed": 56000,
  "total": 56000,
  "bytes_processed": 52100000000,
  "bytes_total": 52100000000
}
```

**Step 3: Run test to verify**

Run: `cargo test -p server -- routes::indexing`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/server/src/routes/indexing.rs
git commit -m "feat(server): add bandwidth fields to SSE deep-progress events"
```

---

### Task 4: Enhance TUI Progress Display with Bandwidth

**Files:**
- Modify: `crates/server/src/main.rs` (TUI spinner section)

**Step 1: Update spinner template**

Change the Pass 2 spinner from:
```
⠒ Deep indexing 15/42 sessions...
```

To:
```
⠒ Deep indexing 23.4 GB / 52.1 GB  (2.7 GB/s)  12847/56000 sessions
```

**Step 2: Compute throughput**

Track a start timestamp when Pass 2 begins. In the spinner update loop:

```rust
let elapsed = start.elapsed().as_secs_f64();
let processed = indexing_state.bytes_processed();
let throughput = if elapsed > 0.0 { processed as f64 / elapsed } else { 0.0 };
```

Format as human-readable: `format_bytes(processed)` / `format_bytes(total)` (`format_bytes(throughput)`/s)

**Step 3: Add `format_bytes` helper**

```rust
fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_000_000_000;
    const MB: u64 = 1_000_000;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
```

**Step 4: Update completion line**

```
✓ Deep index complete — 56,000 sessions, 52.1 GB processed (38.2s)
```

**Step 5: Run `cargo check -p server`**

Expected: Compiles cleanly

**Step 6: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "feat(server): TUI progress shows bandwidth throughput during deep indexing"
```

---

### Task 5: Frontend — Add `useIndexingProgress` SSE Hook

**Files:**
- Create: `src/hooks/use-indexing-progress.ts`

**Step 1: Write the hook**

```typescript
import { useState, useEffect, useRef } from "react";

export interface IndexingProgress {
  status: "idle" | "reading-indexes" | "ready" | "deep-indexing" | "done" | "error";
  // Pass 1 results
  projects: number;
  sessions: number;
  // Pass 2 progress
  indexed: number;
  total: number;
  bytesProcessed: number;
  bytesTotal: number;
  // Computed
  throughputBytesPerSec: number;
  isFirstRun: boolean;
  errorMessage?: string;
}

export function useIndexingProgress(): IndexingProgress {
  const [progress, setProgress] = useState<IndexingProgress>({
    status: "idle",
    projects: 0,
    sessions: 0,
    indexed: 0,
    total: 0,
    bytesProcessed: 0,
    bytesTotal: 0,
    throughputBytesPerSec: 0,
    isFirstRun: true,
  });

  const startTimeRef = useRef<number | null>(null);

  useEffect(() => {
    const es = new EventSource("/api/indexing/progress");

    es.addEventListener("status", (e) => {
      const data = JSON.parse(e.data);
      setProgress((prev) => ({ ...prev, status: data.status }));
    });

    es.addEventListener("ready", (e) => {
      const data = JSON.parse(e.data);
      startTimeRef.current = Date.now();
      setProgress((prev) => ({
        ...prev,
        status: "ready",
        projects: data.projects ?? prev.projects,
        sessions: data.sessions ?? prev.sessions,
      }));
    });

    es.addEventListener("deep-progress", (e) => {
      const data = JSON.parse(e.data);
      const elapsed = startTimeRef.current
        ? (Date.now() - startTimeRef.current) / 1000
        : 1;
      const throughput = elapsed > 0 ? (data.bytes_processed ?? 0) / elapsed : 0;

      setProgress((prev) => ({
        ...prev,
        status: "deep-indexing",
        indexed: data.indexed,
        total: data.total,
        bytesProcessed: data.bytes_processed ?? 0,
        bytesTotal: data.bytes_total ?? 0,
        throughputBytesPerSec: throughput,
      }));
    });

    es.addEventListener("done", (e) => {
      const data = JSON.parse(e.data);
      setProgress((prev) => ({
        ...prev,
        status: "done",
        indexed: data.indexed,
        total: data.total,
        bytesProcessed: data.bytes_processed ?? prev.bytesTotal,
        bytesTotal: data.bytes_total ?? prev.bytesTotal,
        isFirstRun: false,
      }));
      es.close();
    });

    es.addEventListener("error", (e) => {
      try {
        const data = JSON.parse((e as MessageEvent).data);
        setProgress((prev) => ({
          ...prev,
          status: "error",
          errorMessage: data.message,
        }));
      } catch {
        // SSE connection error, not a data event
      }
    });

    return () => es.close();
  }, []);

  return progress;
}
```

**Step 2: Write tests**

```typescript
// src/hooks/__tests__/use-indexing-progress.test.ts
// Test initial state, SSE event handling, throughput computation
```

**Step 3: Commit**

```bash
git add src/hooks/use-indexing-progress.ts src/hooks/__tests__/use-indexing-progress.test.ts
git commit -m "feat(frontend): add useIndexingProgress SSE hook with bandwidth tracking"
```

---

### Task 6: Frontend — Cold Start Overlay Component

**Files:**
- Create: `src/components/ColdStartOverlay.tsx`
- Modify: `src/App.tsx`

**Step 1: Build the overlay component**

A non-blocking banner/overlay that appears when `status !== "done"`. Shows:
- Progress bar (bytes-based, not session-based — smoother for large files)
- `23.4 GB / 52.1 GB` with percentage
- `2.7 GB/s` throughput
- `12,847 / 56,000 sessions` count
- "You can browse sessions while we finish." message

Design notes:
- Use existing design system (match StatusBar, LoadingStates patterns)
- Banner at top of page, NOT a blocking modal — user can browse underneath
- Dismissible after Pass 1 completes (user knows they can browse)
- Auto-hides when `status === "done"`

**Step 2: Add `formatBytes` utility**

```typescript
export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`;
  return `${bytes} bytes`;
}
```

**Step 3: Wire into App.tsx**

```tsx
const progress = useIndexingProgress();

return (
  <>
    {progress.status !== "done" && <ColdStartOverlay progress={progress} />}
    {/* existing app content */}
  </>
);
```

**Step 4: Write component tests**

Test rendering at each status phase (reading-indexes, ready, deep-indexing, done).
Test that overlay disappears when done.
Test bandwidth display formatting.

**Step 5: Commit**

```bash
git add src/components/ColdStartOverlay.tsx src/App.tsx
git commit -m "feat(frontend): cold start overlay with bandwidth progress bar"
```

---

### Task 7: Integration Test — Simulated Large Dataset

**Files:**
- Create: `crates/server/tests/cold_start_bandwidth.rs`

**Step 1: Write integration test**

Create a test that:
1. Creates a temp directory with N fake JSONL session files of known sizes
2. Starts the server pointing at that directory
3. Connects to SSE `/api/indexing/progress`
4. Verifies `bytes_total` matches sum of file sizes
5. Verifies `bytes_processed` increases monotonically
6. Verifies final `done` event has `bytes_processed == bytes_total`

**Step 2: Run test**

Run: `cargo test -p server -- cold_start_bandwidth`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/server/tests/cold_start_bandwidth.rs
git commit -m "test(server): integration test for cold start bandwidth SSE events"
```

---

## Summary

| Task | What | Where |
|------|------|-------|
| 1 | Bandwidth atomics on IndexingState | `crates/server/` |
| 2 | Feed bytes from indexer | `crates/db/` + `crates/server/` |
| 3 | Extend SSE events | `crates/server/` |
| 4 | TUI bandwidth display | `crates/server/` |
| 5 | React SSE hook | `src/hooks/` |
| 6 | Cold start overlay component | `src/components/` + `src/App.tsx` |
| 7 | Integration test | `crates/server/tests/` |

**Dependencies:** Tasks 1 → 2 → 3 → 4 (backend chain), then 5 → 6 (frontend chain). Task 7 after Task 3.

**Not in scope:**
- ETA estimation (unreliable — disk I/O varies, don't show it)
- Cancellation (user can just close the browser)
- Pause/resume (over-engineering)
- Pre-aggregation / DuckDB (deferred — see PROGRESS.md)
