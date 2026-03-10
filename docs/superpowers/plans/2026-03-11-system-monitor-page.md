# System Monitor Page Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `/monitor` page that shows real-time system resource usage (CPU, RAM, disk, network), all active Claude sessions with per-process metrics, and a top-5 expandable process list — using a lazy observer pattern with zero overhead when the page is not open.

**Architecture:** Rust backend polls `sysinfo` only while SSE clients are connected (lazy observer via `Arc<AtomicUsize>` subscriber count). Frontend connects via `EventSource` on mount, disconnects on unmount. Apple + Swiss Modernism aesthetic with animated gauge bars and counter-tweened numbers.

**Tech Stack:** Rust (Axum, sysinfo 0.33, tokio, serde, ts-rs), React (TypeScript, Tailwind CSS, lucide-react), SSE via `EventSource`

**Spec:** `docs/superpowers/specs/2026-03-11-system-monitor-page-design.md`

---

## File Map

### New Files

| File | Responsibility |
|------|---------------|
| `crates/server/src/live/monitor.rs` | Lazy sysinfo polling, subscriber counting, ResourceSnapshot assembly |
| `crates/server/src/routes/monitor.rs` | SSE endpoint `GET /api/monitor/stream`, REST `GET /api/monitor/snapshot` |
| `apps/web/src/pages/SystemMonitorPage.tsx` | Top-level page component |
| `apps/web/src/hooks/use-system-monitor.ts` | SSE connection hook (connect on mount, disconnect on unmount) |
| `apps/web/src/hooks/use-tweened-value.ts` | Generic rAF counter-tween hook |
| `apps/web/src/components/monitor/GaugeCard.tsx` | Bento gauge card (CPU/RAM/Disk/Network) |
| `apps/web/src/components/monitor/SystemGaugeRow.tsx` | 4-gauge grid container |
| `apps/web/src/components/monitor/SessionRow.tsx` | Claude session row with resource metrics |
| `apps/web/src/components/monitor/ClaudeSessionsPanel.tsx` | Primary sessions zone |
| `apps/web/src/components/monitor/ProcessRow.tsx` | System process grouped row |
| `apps/web/src/components/monitor/TopProcessesPanel.tsx` | Expandable top-5 process list |

### Modified Files

| File | Change |
|------|--------|
| `crates/server/src/live/mod.rs` | Add `pub mod monitor;` |
| `crates/server/src/routes/mod.rs` | Add `pub mod monitor;` + register `monitor::router()` |
| `crates/server/src/state.rs` | Add `monitor_tx` broadcast sender + `monitor_subscribers` AtomicUsize |
| `apps/web/src/router.tsx` | Add `/monitor` route |
| `apps/web/src/components/Sidebar.tsx` | Add Monitor nav item (collapsed + expanded) |

---

## Chunk 1: Backend — Data Structures & Lazy Observer

### Task 1: Define monitor data structs with `#[derive(TS)]`

**Files:**

- Create: `crates/server/src/live/monitor.rs`
- Modify: `crates/server/src/live/mod.rs`

- [ ] **Step 1: Create `monitor.rs` with data structures**

```rust
// crates/server/src/live/monitor.rs

use serde::Serialize;
use ts_rs::TS;

use crate::live::state::SessionStatus;

/// Static system info sent on initial SSE connection.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub cpu_brand: String,
    pub cpu_core_count: u32,
    #[ts(type = "number")]
    pub total_memory_bytes: u64,
    #[ts(type = "number")]
    pub total_disk_bytes: u64,
}

/// Snapshot of system resources, sent every ~1s while subscribed.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSnapshot {
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    pub cpu_percent: f32,
    #[ts(type = "number")]
    pub memory_used_bytes: u64,
    #[ts(type = "number")]
    pub memory_total_bytes: u64,
    #[ts(type = "number")]
    pub disk_used_bytes: u64,
    #[ts(type = "number")]
    pub disk_total_bytes: u64,
    #[ts(type = "number")]
    pub network_rx_bytes_per_sec: u64,
    #[ts(type = "number")]
    pub network_tx_bytes_per_sec: u64,
    pub claude_sessions: Vec<SessionResource>,
    pub top_processes: Vec<ProcessGroup>,
}

/// A Claude session with per-PID resource usage merged from sysinfo.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionResource {
    pub session_id: String,
    pub project_name: String,
    pub project_path: String,
    pub git_branch: Option<String>,
    pub status: SessionStatus,
    pub pid: Option<u32>,
    /// CPU% from sysinfo per-PID. 0.0 when pid is None.
    pub cpu_percent: f32,
    /// RSS from sysinfo per-PID. 0 when pid is None.
    #[ts(type = "number")]
    pub memory_bytes: u64,
    #[ts(type = "number")]
    pub input_tokens: u64,
    #[ts(type = "number")]
    pub output_tokens: u64,
    #[ts(type = "number")]
    pub cache_read_tokens: u64,
    /// From existing LiveSession.cost.total_usd (CostBreakdown).
    pub estimated_cost_usd: f64,
    pub turn_count: u32,
}

/// Processes grouped by app name, sorted by total CPU%.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProcessGroup {
    pub name: String,
    pub process_count: u32,
    pub total_cpu_percent: f32,
    #[ts(type = "number")]
    pub total_memory_bytes: u64,
}
```

- [ ] **Step 2: Export the module**

In `crates/server/src/live/mod.rs`, add:

```rust
pub mod monitor;
```

- [ ] **Step 3: Run `cargo test -p claude-view-server` to generate TS types**

Run: `cargo test -p claude-view-server`

Expected: Tests pass. New files appear in `apps/web/src/types/generated/`:
- `SystemInfo.ts`
- `ResourceSnapshot.ts`
- `SessionResource.ts`
- `ProcessGroup.ts`

Verify: `ls apps/web/src/types/generated/ | grep -E 'SystemInfo|ResourceSnapshot|SessionResource|ProcessGroup'`

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/live/monitor.rs crates/server/src/live/mod.rs apps/web/src/types/generated/SystemInfo.ts apps/web/src/types/generated/ResourceSnapshot.ts apps/web/src/types/generated/SessionResource.ts apps/web/src/types/generated/ProcessGroup.ts
git commit -m "feat(monitor): define monitor data structs with TS type generation"
```

### Task 2: Implement lazy sysinfo polling observer

**Files:**

- Modify: `crates/server/src/live/monitor.rs`
- Modify: `crates/server/src/state.rs`

- [ ] **Step 1: Add monitor fields to AppState**

In `crates/server/src/state.rs`, add to the `AppState` struct:

```rust
/// Broadcast sender for system monitor SSE snapshots.
pub monitor_tx: broadcast::Sender<ResourceSnapshot>,
/// Active monitor SSE subscriber count (lazy polling starts at 1, stops at 0).
pub monitor_subscribers: Arc<std::sync::atomic::AtomicUsize>,
```

Import `ResourceSnapshot` from `crate::live::monitor::ResourceSnapshot`.

In the `AppState::new()` constructor, initialize:

```rust
let (monitor_tx, _) = broadcast::channel(16);
let monitor_subscribers = Arc::new(std::sync::atomic::AtomicUsize::new(0));
```

- [ ] **Step 2: Implement `collect_system_info()` in monitor.rs**

This is called once on SSE connect to send static system info.

```rust
use sysinfo::{Disks, System};

/// Collect static system info (called once per SSE connection).
pub fn collect_system_info() -> SystemInfo {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    let disks = Disks::new_with_refreshed_list();
    let root_disk = disks.iter().find(|d| d.mount_point() == std::path::Path::new("/"));

    SystemInfo {
        hostname: System::host_name().unwrap_or_default(),
        os_name: System::name().unwrap_or_default(),
        os_version: System::os_version().unwrap_or_default(),
        cpu_brand: sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default(),
        cpu_core_count: sys.cpus().len() as u32,
        total_memory_bytes: sys.total_memory(),
        total_disk_bytes: root_disk.map(|d| d.total_space()).unwrap_or(0),
    }
}
```

- [ ] **Step 3: Implement `collect_snapshot()` — the 1-second polling function**

This merges sysinfo data with LiveSession data.

```rust
use std::collections::HashMap;
use sysinfo::{Disks, Networks, Pid, ProcessesToUpdate, System};
use crate::live::state::{LiveSession, SessionStatus};

/// Collect a full resource snapshot.
///
/// `sys` must be a long-lived System instance (reused across polls for delta CPU%).
/// `prev_net_rx`/`prev_net_tx` track network byte counters for delta calculation.
pub fn collect_snapshot(
    sys: &mut System,
    disks: &mut Disks,
    networks: &mut Networks,
    live_sessions: &HashMap<String, LiveSession>,
    prev_net_rx: &mut u64,
    prev_net_tx: &mut u64,
) -> ResourceSnapshot {
    sys.refresh_cpu_all();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    disks.refresh(true);
    networks.refresh(true);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Global CPU
    let cpu_percent = sys.global_cpu_usage();

    // Memory
    let memory_used_bytes = sys.used_memory();
    let memory_total_bytes = sys.total_memory();

    // Disk: root mount point only
    let root_disk = disks.iter().find(|d| d.mount_point() == std::path::Path::new("/"));
    let disk_total_bytes = root_disk.map(|d| d.total_space()).unwrap_or(0);
    let disk_used_bytes = root_disk.map(|d| d.total_space() - d.available_space()).unwrap_or(0);

    // Network: sum all non-loopback, compute delta
    let (total_rx, total_tx) = networks
        .iter()
        .filter(|(name, _)| name.as_str() != "lo" && name.as_str() != "lo0")
        .fold((0u64, 0u64), |(rx, tx), (_, data)| {
            (rx + data.total_received(), tx + data.total_transmitted())
        });
    let rx_per_sec = total_rx.saturating_sub(*prev_net_rx);
    let tx_per_sec = total_tx.saturating_sub(*prev_net_tx);
    *prev_net_rx = total_rx;
    *prev_net_tx = total_tx;

    // Build Claude session resources
    let claude_pids: HashMap<u32, &LiveSession> = live_sessions
        .values()
        .filter_map(|s| s.pid.map(|pid| (pid, s)))
        .collect();

    let claude_sessions: Vec<SessionResource> = live_sessions
        .values()
        .map(|s| {
            let (cpu, mem) = s
                .pid
                .and_then(|pid| sys.process(Pid::from_u32(pid)))
                .map(|p| (p.cpu_usage(), p.memory()))
                .unwrap_or((0.0, 0));

            SessionResource {
                session_id: s.id.clone(),
                project_name: s.project_display_name.clone(),
                project_path: s.project_path.clone(),
                git_branch: s.effective_branch.clone(),
                status: s.status.clone(),
                pid: s.pid,
                cpu_percent: cpu,
                memory_bytes: mem,
                input_tokens: s.tokens.input_tokens,
                output_tokens: s.tokens.output_tokens,
                cache_read_tokens: s.tokens.cache_read_tokens,
                estimated_cost_usd: s.cost.total_usd,
                turn_count: s.turn_count,
            }
        })
        .collect();

    // Build top processes (grouped by app name, excluding Claude PIDs)
    let mut groups: HashMap<String, (u32, f32, u64)> = HashMap::new();
    for (pid, proc) in sys.processes() {
        if claude_pids.contains_key(&pid.as_u32()) {
            continue;
        }
        let name = normalize_process_name(proc.name().to_string_lossy().as_ref());
        let entry = groups.entry(name).or_insert((0, 0.0, 0));
        entry.0 += 1;
        entry.1 += proc.cpu_usage();
        entry.2 += proc.memory();
    }

    let mut top_processes: Vec<ProcessGroup> = groups
        .into_iter()
        .map(|(name, (count, cpu, mem))| ProcessGroup {
            name,
            process_count: count,
            total_cpu_percent: cpu,
            total_memory_bytes: mem,
        })
        .collect();
    top_processes.sort_by(|a, b| b.total_cpu_percent.total_cmp(&a.total_cpu_percent));

    ResourceSnapshot {
        timestamp_ms: now_ms,
        cpu_percent,
        memory_used_bytes,
        memory_total_bytes,
        disk_used_bytes,
        disk_total_bytes,
        network_rx_bytes_per_sec: rx_per_sec,
        network_tx_bytes_per_sec: tx_per_sec,
        claude_sessions,
        top_processes,
    }
}

/// Normalize process names to group related processes.
///
/// On macOS, Chrome spawns many helper processes like
/// "Google Chrome Helper (Renderer)", "Google Chrome Helper (GPU)".
/// Group them all under "Google Chrome".
fn normalize_process_name(raw: &str) -> String {
    // Strip common helper suffixes
    if let Some(base) = raw.strip_suffix(" Helper (Renderer)")
        .or_else(|| raw.strip_suffix(" Helper (GPU)"))
        .or_else(|| raw.strip_suffix(" Helper (Plugin)"))
        .or_else(|| raw.strip_suffix(" Helper"))
    {
        return base.to_string();
    }
    raw.to_string()
}
```

- [ ] **Step 4: Implement `start_polling_task()` — the lazy observer**

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::live::manager::LiveSessionMap;

/// Spawn the sysinfo polling loop. Exits when `subscribers` drops to 0.
pub fn start_polling_task(
    subscribers: Arc<AtomicUsize>,
    tx: broadcast::Sender<ResourceSnapshot>,
    live_sessions: LiveSessionMap,
) {
    tokio::task::spawn(async move {
        let mut sys = System::new_all();
        let mut disks = Disks::new_with_refreshed_list();
        let mut networks = Networks::new_with_refreshed_list();
        let mut prev_net_rx: u64 = 0;
        let mut prev_net_tx: u64 = 0;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        tracing::info!("Monitor polling task started");

        loop {
            interval.tick().await;

            if subscribers.load(Ordering::Relaxed) == 0 {
                tracing::info!("Monitor polling task stopping — no subscribers");
                break;
            }

            let sessions = live_sessions.read().await;
            let snapshot = collect_snapshot(
                &mut sys,
                &mut disks,
                &mut networks,
                &sessions,
                &mut prev_net_rx,
                &mut prev_net_tx,
            );
            drop(sessions);

            // Ignore send error (no receivers — will exit next iteration)
            let _ = tx.send(snapshot);
        }
    });
}
```

- [ ] **Step 5: Run `cargo check -p claude-view-server`**

Expected: Compiles (may have unused warnings, that's fine — routes not wired yet).

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/live/monitor.rs crates/server/src/state.rs
git commit -m "feat(monitor): lazy sysinfo polling observer with subscriber counting"
```

### Task 3: SSE and REST endpoints

**Files:**

- Create: `crates/server/src/routes/monitor.rs`
- Modify: `crates/server/src/routes/mod.rs`

- [ ] **Step 1: Create the SSE endpoint**

```rust
// crates/server/src/routes/monitor.rs

//! System monitor endpoints (SSE + REST).
//!
//! - `GET /api/monitor/stream`   -- SSE stream of ResourceSnapshot every ~1s
//! - `GET /api/monitor/snapshot` -- Single ResourceSnapshot (REST fallback)

use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, Sse},
    response::Json,
    routing::get,
    Router,
};

use crate::live::monitor::{
    collect_system_info, collect_snapshot, start_polling_task, ResourceSnapshot,
};
use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/monitor/stream", get(monitor_stream))
        .route("/monitor/snapshot", get(monitor_snapshot))
}

/// GET /api/monitor/stream — SSE stream of system resource snapshots.
///
/// Lazy observer: increments subscriber count on connect, decrements on disconnect.
/// Polling task starts when first subscriber connects, stops when last disconnects.
pub async fn monitor_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let prev_count = state
        .monitor_subscribers
        .fetch_add(1, Ordering::SeqCst);

    // If we're the first subscriber, start the polling task
    if prev_count == 0 {
        start_polling_task(
            state.monitor_subscribers.clone(),
            state.monitor_tx.clone(),
            state.live_sessions.clone(),
        );
    }

    let mut rx = state.monitor_tx.subscribe();
    let subscribers = state.monitor_subscribers.clone();
    let mut shutdown = state.shutdown.clone();

    let stream = async_stream::stream! {
        // 1. Send static system info on connect
        let sys_info = collect_system_info();
        match serde_json::to_string(&sys_info) {
            Ok(data) => yield Ok(Event::default().event("monitor_connected").data(data)),
            Err(e) => tracing::error!("Failed to serialize SystemInfo: {e}"),
        }

        // 2. Stream snapshots with heartbeat
        let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                snapshot = rx.recv() => {
                    match snapshot {
                        Ok(snap) => {
                            match serde_json::to_string(&snap) {
                                Ok(data) => yield Ok(Event::default().event("monitor_snapshot").data(data)),
                                Err(e) => tracing::error!("Failed to serialize ResourceSnapshot: {e}"),
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Monitor SSE client lagged by {n} events");
                            // Next snapshot will be a full refresh anyway
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat.tick() => {
                    yield Ok(Event::default().event("heartbeat").data("{}"));
                }
                _ = shutdown.changed() => {
                    break;
                }
            }
        }

        // Decrement subscriber count on disconnect
        subscribers.fetch_sub(1, Ordering::SeqCst);
        tracing::debug!("Monitor SSE client disconnected");
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("{}"),
    )
}

/// GET /api/monitor/snapshot — Single resource snapshot (REST fallback).
pub async fn monitor_snapshot(
    State(state): State<Arc<AppState>>,
) -> Json<ResourceSnapshot> {
    use sysinfo::{Disks, Networks, System};

    let mut sys = System::new_all();
    let mut disks = Disks::new_with_refreshed_list();
    let mut networks = Networks::new_with_refreshed_list();
    let mut prev_rx = 0u64;
    let mut prev_tx = 0u64;

    let sessions = state.live_sessions.read().await;
    let snapshot = collect_snapshot(
        &mut sys, &mut disks, &mut networks, &sessions, &mut prev_rx, &mut prev_tx,
    );
    Json(snapshot)
}
```

- [ ] **Step 2: Register the monitor router**

In `crates/server/src/routes/mod.rs`:

Add `pub mod monitor;` after the existing module declarations.

In the `api_routes()` function, add before the metrics line:

```rust
.nest("/api", monitor::router())
```

- [ ] **Step 3: Run `cargo check -p claude-view-server`**

Expected: Compiles cleanly.

- [ ] **Step 4: Run `cargo test -p claude-view-server routes::tests`**

Expected: The existing `test_api_routes_creation` test should still pass (it exercises all route registration).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/routes/monitor.rs crates/server/src/routes/mod.rs crates/server/src/state.rs
git commit -m "feat(monitor): SSE and REST endpoints for system resource streaming"
```

---

## Chunk 2: Frontend — Hook, Tween, and Page Skeleton

### Task 4: Create the `useTweenedValue` hook

**Files:**

- Create: `apps/web/src/hooks/use-tweened-value.ts`

- [ ] **Step 1: Implement the rAF tween hook**

```typescript
// apps/web/src/hooks/use-tweened-value.ts

import { useEffect, useRef, useState } from 'react'

const DURATION_MS = 200

/**
 * Smoothly interpolate a number from its previous value to the target.
 * Uses requestAnimationFrame for 60fps animation.
 * Respects prefers-reduced-motion (instant jump when enabled).
 */
export function useTweenedValue(target: number): number {
  const [value, setValue] = useState(target)
  const prevRef = useRef(target)
  const rafRef = useRef<number | null>(null)
  const startTimeRef = useRef(0)
  const startValueRef = useRef(target)

  useEffect(() => {
    // Respect reduced motion preference
    const prefersReduced =
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches

    if (prefersReduced) {
      setValue(target)
      prevRef.current = target
      return
    }

    // Cancel any in-progress animation
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current)
    }

    startValueRef.current = prevRef.current
    startTimeRef.current = performance.now()

    function animate(now: number) {
      const elapsed = now - startTimeRef.current
      const progress = Math.min(elapsed / DURATION_MS, 1)
      // ease-out: 1 - (1 - t)^2
      const eased = 1 - (1 - progress) * (1 - progress)
      const current = startValueRef.current + (target - startValueRef.current) * eased

      setValue(current)

      if (progress < 1) {
        rafRef.current = requestAnimationFrame(animate)
      } else {
        prevRef.current = target
        rafRef.current = null
      }
    }

    rafRef.current = requestAnimationFrame(animate)

    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current)
      }
    }
  }, [target])

  return value
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/hooks/use-tweened-value.ts
git commit -m "feat(monitor): useTweenedValue hook for smooth number animation"
```

### Task 5: Create the `useSystemMonitor` SSE hook

**Files:**

- Create: `apps/web/src/hooks/use-system-monitor.ts`

- [ ] **Step 1: Implement the SSE hook**

```typescript
// apps/web/src/hooks/use-system-monitor.ts

import { useEffect, useRef, useState } from 'react'
import { sseUrl } from '../lib/sse-url'
import type { ResourceSnapshot } from '../types/generated/ResourceSnapshot'
import type { SystemInfo } from '../types/generated/SystemInfo'

export interface UseSystemMonitorReturn {
  connected: boolean
  systemInfo: SystemInfo | null
  snapshot: ResourceSnapshot | null
}

/**
 * Connect to the system monitor SSE stream.
 * Only active while the component is mounted (lazy observer).
 * Automatically reconnects with exponential backoff on disconnect.
 */
export function useSystemMonitor(): UseSystemMonitorReturn {
  const [connected, setConnected] = useState(false)
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null)
  const [snapshot, setSnapshot] = useState<ResourceSnapshot | null>(null)
  const esRef = useRef<EventSource | null>(null)

  useEffect(() => {
    let retryDelay = 1000
    let unmounted = false
    let retryTimer: ReturnType<typeof setTimeout> | null = null

    function connect() {
      if (unmounted) return

      const url = sseUrl('/api/monitor/stream')
      const es = new EventSource(url)
      esRef.current = es

      es.onopen = () => {
        if (!unmounted) {
          setConnected(true)
          retryDelay = 1000
        }
      }

      es.addEventListener('monitor_connected', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data) as SystemInfo
          setSystemInfo(data)
        } catch {
          /* ignore malformed */
        }
      })

      es.addEventListener('monitor_snapshot', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data) as ResourceSnapshot
          setSnapshot(data)
        } catch {
          /* ignore malformed */
        }
      })

      es.onerror = () => {
        if (unmounted) return
        setConnected(false)
        es.close()
        esRef.current = null
        // Reconnect with exponential backoff (max 30s)
        retryTimer = setTimeout(() => {
          retryDelay = Math.min(retryDelay * 2, 30000)
          connect()
        }, retryDelay)
      }
    }

    connect()

    return () => {
      unmounted = true
      if (retryTimer) clearTimeout(retryTimer)
      if (esRef.current) {
        esRef.current.close()
        esRef.current = null
      }
      setConnected(false)
    }
  }, [])

  return { connected, systemInfo, snapshot }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/hooks/use-system-monitor.ts
git commit -m "feat(monitor): useSystemMonitor SSE hook with auto-reconnect"
```

### Task 6: Create the page skeleton and route wiring

**Files:**

- Create: `apps/web/src/pages/SystemMonitorPage.tsx`
- Modify: `apps/web/src/router.tsx`
- Modify: `apps/web/src/components/Sidebar.tsx`

- [ ] **Step 1: Create the page skeleton**

```tsx
// apps/web/src/pages/SystemMonitorPage.tsx

import { Activity, WifiOff } from 'lucide-react'
import { useSystemMonitor } from '../hooks/use-system-monitor'

export function SystemMonitorPage() {
  const { connected, systemInfo, snapshot } = useSystemMonitor()

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      {/* Page Header */}
      <div className="px-6 pt-6 pb-4">
        <div className="flex items-center gap-3">
          <Activity className="w-5 h-5 text-gray-400" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            System Monitor
          </h1>
          <div className="flex items-center gap-1.5">
            {connected ? (
              <>
                <span className="relative flex h-2 w-2">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75" />
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-green-500" />
                </span>
                <span className="text-xs text-green-600 dark:text-green-400">Live</span>
              </>
            ) : (
              <>
                <WifiOff className="w-3 h-3 text-amber-500" />
                <span className="text-xs text-amber-600 dark:text-amber-400">Reconnecting...</span>
              </>
            )}
          </div>
          {systemInfo && (
            <span className="ml-auto text-xs text-gray-400">
              {systemInfo.hostname} · {systemInfo.osName} {systemInfo.osVersion} · {systemInfo.cpuCoreCount} cores
            </span>
          )}
        </div>
      </div>

      {/* Content — placeholder until UI components are built */}
      <div className="flex-1 px-6 pb-6">
        {!snapshot ? (
          <div className="grid grid-cols-4 gap-4">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="h-20 rounded-2xl bg-gray-100 dark:bg-gray-800 animate-pulse" />
            ))}
          </div>
        ) : (
          <pre className="text-xs text-gray-500 overflow-auto">
            {JSON.stringify(snapshot, null, 2)}
          </pre>
        )}
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Add the route**

In `apps/web/src/router.tsx`, add import at top:

```typescript
import { SystemMonitorPage } from './pages/SystemMonitorPage'
```

Add route after the `plugins` route (before `settings`):

```typescript
{ path: 'monitor', element: <SystemMonitorPage /> },
```

- [ ] **Step 3: Add Sidebar nav item — collapsed mode**

In `apps/web/src/components/Sidebar.tsx`, in the collapsed sidebar section, add a Monitor link after the Plugins link (before `<div className="flex-1" />`):

```tsx
<Link
  to="/monitor"
  className={cn(
    'p-2 rounded-md transition-colors',
    'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
    location.pathname === '/monitor'
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70',
  )}
  title="Monitor"
>
  <Activity className="w-5 h-5" />
</Link>
```

Add `Activity` to the lucide-react import at the top of the file.

- [ ] **Step 4: Add Sidebar nav item — expanded mode**

In the expanded sidebar section, add a Monitor link after the Plugins `</Link>` and before the "Agent SDK Studio" `<span>`:

```tsx
<Link
  to={`/monitor${paramString ? `?${paramString}` : ''}`}
  className={cn(
    'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
    location.pathname === '/monitor'
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70',
  )}
>
  <Activity className="w-4 h-4" />
  <span className="font-medium">Monitor</span>
</Link>
```

- [ ] **Step 5: Build and verify**

Run: `bun run build`

Expected: Build succeeds. Navigate to `http://localhost:47892/monitor` — should show the page header with live indicator and raw JSON snapshot data.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/pages/SystemMonitorPage.tsx apps/web/src/router.tsx apps/web/src/components/Sidebar.tsx
git commit -m "feat(monitor): page skeleton with SSE wiring and sidebar nav"
```

---

## Chunk 3: Frontend — UI Components (Apple + Swiss)

### Task 7: GaugeCard and SystemGaugeRow

**Files:**

- Create: `apps/web/src/components/monitor/GaugeCard.tsx`
- Create: `apps/web/src/components/monitor/SystemGaugeRow.tsx`

- [ ] **Step 1: Create GaugeCard component**

```tsx
// apps/web/src/components/monitor/GaugeCard.tsx

import { useTweenedValue } from '../../hooks/use-tweened-value'

interface GaugeCardProps {
  label: string
  value: number
  max: number
  unit: string
  /** Secondary info line (e.g. "12 cores", "8/16 GB") */
  detail?: string
  /** Custom format for the value display */
  formatValue?: (value: number) => string
}

function gaugeColor(percent: number): string {
  if (percent >= 90) return 'bg-red-500'
  if (percent >= 70) return 'bg-amber-500'
  return 'bg-green-500'
}

function gaugeColorTransition(percent: number): string {
  if (percent >= 90) return 'from-red-500 to-red-600'
  if (percent >= 70) return 'from-amber-400 to-amber-500'
  return 'from-green-400 to-green-500'
}

export function GaugeCard({ label, value, max, unit, detail, formatValue }: GaugeCardProps) {
  const percent = max > 0 ? (value / max) * 100 : 0
  const tweenedPercent = useTweenedValue(percent)
  const tweenedValue = useTweenedValue(value)

  const displayValue = formatValue ? formatValue(tweenedValue) : `${Math.round(tweenedPercent)}${unit}`

  return (
    <div className="rounded-2xl border border-gray-200/50 dark:border-white/[0.06] bg-white dark:bg-[#141419] p-4 transition-all duration-200 hover:scale-[1.01] hover:shadow-md cursor-default">
      <div className="flex items-baseline justify-between mb-2">
        <span className="text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">
          {label}
        </span>
        <span className="text-lg font-semibold tabular-nums text-gray-900 dark:text-gray-100">
          {displayValue}
        </span>
      </div>
      {/* Gauge bar */}
      <div className="h-1.5 rounded-full bg-gray-100 dark:bg-white/[0.06] overflow-hidden">
        <div
          className={`h-full rounded-full bg-gradient-to-r ${gaugeColorTransition(tweenedPercent)} transition-colors duration-500`}
          style={{ width: `${Math.min(tweenedPercent, 100)}%`, transition: 'width 300ms ease-out' }}
        />
      </div>
      {detail && (
        <p className="mt-1.5 text-[11px] text-gray-400 dark:text-gray-500">{detail}</p>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Create SystemGaugeRow container**

```tsx
// apps/web/src/components/monitor/SystemGaugeRow.tsx

import type { ResourceSnapshot } from '../../types/generated/ResourceSnapshot'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { GaugeCard } from './GaugeCard'

interface SystemGaugeRowProps {
  snapshot: ResourceSnapshot
  systemInfo: SystemInfo | null
}

function formatBytes(bytes: number): string {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1)} TB`
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(0)} KB`
  return `${bytes} B`
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec >= 1e6) return `${(bytesPerSec / 1e6).toFixed(1)} MB/s`
  if (bytesPerSec >= 1e3) return `${(bytesPerSec / 1e3).toFixed(0)} KB/s`
  return `${bytesPerSec} B/s`
}

export function SystemGaugeRow({ snapshot, systemInfo }: SystemGaugeRowProps) {
  const memUsed = snapshot.memoryUsedBytes
  const memTotal = snapshot.memoryTotalBytes
  const diskUsed = snapshot.diskUsedBytes
  const diskTotal = snapshot.diskTotalBytes

  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
      <GaugeCard
        label="CPU"
        value={snapshot.cpuPercent}
        max={100}
        unit="%"
        detail={systemInfo ? `${systemInfo.cpuCoreCount} cores` : undefined}
      />
      <GaugeCard
        label="Memory"
        value={memUsed}
        max={memTotal}
        unit=""
        formatValue={(v) => `${formatBytes(v)} / ${formatBytes(memTotal)}`}
        detail={`${Math.round((memUsed / memTotal) * 100)}% used`}
      />
      <GaugeCard
        label="Disk"
        value={diskUsed}
        max={diskTotal}
        unit=""
        formatValue={(v) => `${formatBytes(v)} / ${formatBytes(diskTotal)}`}
        detail={`${formatBytes(diskTotal - diskUsed)} free`}
      />
      <GaugeCard
        label="Network"
        value={0}
        max={1}
        unit=""
        formatValue={() =>
          `↑ ${formatSpeed(snapshot.networkTxBytesPerSec)}  ↓ ${formatSpeed(snapshot.networkRxBytesPerSec)}`
        }
        detail="throughput"
      />
    </div>
  )
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/monitor/GaugeCard.tsx apps/web/src/components/monitor/SystemGaugeRow.tsx
git commit -m "feat(monitor): GaugeCard and SystemGaugeRow with animated bars"
```

### Task 8: ClaudeSessionsPanel and SessionRow

**Files:**

- Create: `apps/web/src/components/monitor/SessionRow.tsx`
- Create: `apps/web/src/components/monitor/ClaudeSessionsPanel.tsx`

- [ ] **Step 1: Create SessionRow**

```tsx
// apps/web/src/components/monitor/SessionRow.tsx

import type { SessionResource } from '../../types/generated/SessionResource'
import { useTweenedValue } from '../../hooks/use-tweened-value'

function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(0)} KB`
  return `${bytes} B`
}

function formatTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return `${n}`
}

function statusColor(status: string): string {
  switch (status) {
    case 'Working':
      return 'bg-green-500'
    case 'paused':
      return 'bg-amber-500'
    default:
      return 'bg-gray-400'
  }
}

export function SessionRow({ session }: { session: SessionResource }) {
  const tweenedCpu = useTweenedValue(session.cpuPercent)
  const hasPid = session.pid !== null && session.pid !== undefined

  return (
    <div className="group flex flex-col gap-1 px-4 py-3 rounded-xl border border-gray-200/50 dark:border-white/[0.06] bg-white dark:bg-[#141419] transition-all duration-200 hover:scale-[1.01] hover:shadow-md">
      {/* Line 1: Status + Project + CPU/RAM */}
      <div className="flex items-center gap-3">
        {/* Status dot */}
        <span className="relative flex h-2.5 w-2.5 shrink-0">
          {session.status === 'working' && (
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-40" />
          )}
          <span className={`relative inline-flex rounded-full h-2.5 w-2.5 ${statusColor(session.status)}`} />
        </span>

        {/* Project name */}
        <span className="font-medium text-sm text-gray-900 dark:text-gray-100 truncate">
          {session.projectName}
        </span>

        {/* Status label */}
        <span className="text-xs text-gray-400 dark:text-gray-500 shrink-0">
          {session.status}
        </span>

        <div className="flex-1" />

        {/* CPU bar */}
        <div className="flex items-center gap-2 shrink-0">
          <div className="w-16 h-1.5 rounded-full bg-gray-100 dark:bg-white/[0.06] overflow-hidden">
            <div
              className="h-full rounded-full bg-blue-500 transition-all duration-300"
              style={{ width: `${Math.min(tweenedCpu, 100)}%` }}
            />
          </div>
          <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 w-10 text-right">
            {hasPid ? `${Math.round(tweenedCpu)}%` : '--'}
          </span>
        </div>

        {/* RAM */}
        <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 w-16 text-right shrink-0">
          {hasPid ? formatBytes(session.memoryBytes) : '--'}
        </span>
      </div>

      {/* Line 2: Branch + tokens + cost + tools */}
      <div className="flex items-center gap-3 pl-5">
        {session.gitBranch && (
          <span className="text-[11px] text-gray-400 dark:text-gray-500 truncate max-w-[120px]">
            {session.gitBranch}
          </span>
        )}
        <span className="text-[11px] text-gray-400 dark:text-gray-500">
          {formatTokens(session.inputTokens + session.outputTokens)} tokens
        </span>
        <span className="text-[11px] text-gray-400 dark:text-gray-500">
          ${session.estimatedCostUsd.toFixed(2)}
        </span>
        <span className="text-[11px] text-gray-400 dark:text-gray-500">
          {session.turnCount} turns
        </span>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Create ClaudeSessionsPanel**

```tsx
// apps/web/src/components/monitor/ClaudeSessionsPanel.tsx

import type { SessionResource } from '../../types/generated/SessionResource'
import { SessionRow } from './SessionRow'

interface ClaudeSessionsPanelProps {
  sessions: SessionResource[]
}

export function ClaudeSessionsPanel({ sessions }: ClaudeSessionsPanelProps) {
  const activeCount = sessions.filter((s) => s.status === 'working').length

  return (
    <div className="flex flex-col gap-3">
      {/* Section header */}
      <div className="flex items-center gap-2">
        <h2 className="text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">
          Claude Sessions
        </h2>
        {activeCount > 0 && (
          <span className="text-[10px] font-semibold bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-400 px-1.5 py-0.5 rounded-full tabular-nums">
            {activeCount} active
          </span>
        )}
      </div>

      {/* Session list — ALL visible, never collapsed */}
      {sessions.length === 0 ? (
        <div className="flex items-center justify-center py-12 rounded-2xl border border-dashed border-gray-200 dark:border-gray-700">
          <p className="text-sm text-gray-400 dark:text-gray-500">
            No active Claude sessions
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {sessions.map((session) => (
            <SessionRow key={session.sessionId} session={session} />
          ))}
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/monitor/SessionRow.tsx apps/web/src/components/monitor/ClaudeSessionsPanel.tsx
git commit -m "feat(monitor): ClaudeSessionsPanel with SessionRow — always-visible sessions"
```

### Task 9: TopProcessesPanel with expand/collapse

**Files:**

- Create: `apps/web/src/components/monitor/ProcessRow.tsx`
- Create: `apps/web/src/components/monitor/TopProcessesPanel.tsx`

- [ ] **Step 1: Create ProcessRow**

```tsx
// apps/web/src/components/monitor/ProcessRow.tsx

import { Box } from 'lucide-react'
import type { ProcessGroup } from '../../types/generated/ProcessGroup'
import { useTweenedValue } from '../../hooks/use-tweened-value'

function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`
  return `${(bytes / 1e3).toFixed(0)} KB`
}

export function ProcessRow({ process }: { process: ProcessGroup }) {
  const tweenedCpu = useTweenedValue(process.totalCpuPercent)

  return (
    <div className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-gray-50 dark:hover:bg-white/[0.03] transition-colors">
      <Box className="w-4 h-4 text-gray-400 shrink-0" />
      <span className="text-sm text-gray-900 dark:text-gray-100 truncate flex-1">
        {process.name}
      </span>
      {process.processCount > 1 && (
        <span className="text-[10px] text-gray-400 dark:text-gray-500 shrink-0">
          ({process.processCount})
        </span>
      )}
      <div className="w-14 h-1.5 rounded-full bg-gray-100 dark:bg-white/[0.06] overflow-hidden shrink-0">
        <div
          className="h-full rounded-full bg-blue-400 transition-all duration-300"
          style={{ width: `${Math.min(tweenedCpu, 100)}%` }}
        />
      </div>
      <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 w-10 text-right shrink-0">
        {Math.round(tweenedCpu)}%
      </span>
      <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 w-16 text-right shrink-0">
        {formatBytes(process.totalMemoryBytes)}
      </span>
    </div>
  )
}
```

- [ ] **Step 2: Create TopProcessesPanel**

```tsx
// apps/web/src/components/monitor/TopProcessesPanel.tsx

import { ChevronDown, ChevronUp } from 'lucide-react'
import { useState } from 'react'
import type { ProcessGroup } from '../../types/generated/ProcessGroup'
import { ProcessRow } from './ProcessRow'

const DEFAULT_VISIBLE = 5

interface TopProcessesPanelProps {
  processes: ProcessGroup[]
}

export function TopProcessesPanel({ processes }: TopProcessesPanelProps) {
  const [expanded, setExpanded] = useState(false)
  const visible = expanded ? processes : processes.slice(0, DEFAULT_VISIBLE)
  const hasMore = processes.length > DEFAULT_VISIBLE

  return (
    <div className="flex flex-col gap-2">
      {/* Section header */}
      <div className="flex items-center justify-between">
        <h2 className="text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">
          Top Processes
        </h2>
        {hasMore && (
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="flex items-center gap-1 text-xs text-blue-500 hover:text-blue-400 transition-colors cursor-pointer"
          >
            {expanded ? (
              <>
                Show less <ChevronUp className="w-3 h-3" />
              </>
            ) : (
              <>
                Show all ({processes.length}) <ChevronDown className="w-3 h-3" />
              </>
            )}
          </button>
        )}
      </div>

      {/* Process list */}
      <div className="rounded-2xl border border-gray-200/50 dark:border-white/[0.06] bg-white dark:bg-[#141419] overflow-hidden">
        <div
          className="transition-all duration-300 ease-out overflow-hidden"
          style={{
            maxHeight: expanded ? `${processes.length * 44}px` : `${DEFAULT_VISIBLE * 44}px`,
          }}
        >
          {visible.map((proc) => (
            <ProcessRow key={proc.name} process={proc} />
          ))}
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/monitor/ProcessRow.tsx apps/web/src/components/monitor/TopProcessesPanel.tsx
git commit -m "feat(monitor): TopProcessesPanel with expand/collapse animation"
```

### Task 10: Assemble the full SystemMonitorPage

**Files:**

- Modify: `apps/web/src/pages/SystemMonitorPage.tsx`

- [ ] **Step 1: Replace the skeleton with real components**

Replace the entire content of `SystemMonitorPage.tsx`:

```tsx
// apps/web/src/pages/SystemMonitorPage.tsx

import { Activity, WifiOff } from 'lucide-react'
import { ClaudeSessionsPanel } from '../components/monitor/ClaudeSessionsPanel'
import { SystemGaugeRow } from '../components/monitor/SystemGaugeRow'
import { TopProcessesPanel } from '../components/monitor/TopProcessesPanel'
import { useSystemMonitor } from '../hooks/use-system-monitor'

export function SystemMonitorPage() {
  const { connected, systemInfo, snapshot } = useSystemMonitor()

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      {/* Page Header */}
      <div className="px-6 pt-6 pb-4">
        <div className="flex items-center gap-3">
          <Activity className="w-5 h-5 text-gray-400" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            System Monitor
          </h1>
          <div className="flex items-center gap-1.5">
            {connected ? (
              <>
                <span className="relative flex h-2 w-2">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75" />
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-green-500" />
                </span>
                <span className="text-xs text-green-600 dark:text-green-400">Live</span>
              </>
            ) : (
              <>
                <WifiOff className="w-3 h-3 text-amber-500" />
                <span className="text-xs text-amber-600 dark:text-amber-400">
                  Reconnecting...
                </span>
              </>
            )}
          </div>
          {systemInfo && (
            <span className="ml-auto text-xs text-gray-400">
              {systemInfo.hostname} · {systemInfo.osName} {systemInfo.osVersion} ·{' '}
              {systemInfo.cpuCoreCount} cores
            </span>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 px-6 pb-6 flex flex-col gap-6">
        {/* System Gauges — sticky when content overflows */}
        {snapshot ? (
          <div className="sticky top-0 z-10 bg-gray-50/80 dark:bg-[#0a0a0f]/80 backdrop-blur-sm -mx-6 px-6 py-3">
            <SystemGaugeRow snapshot={snapshot} systemInfo={systemInfo} />
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={i}
                className="h-20 rounded-2xl bg-gray-100 dark:bg-gray-800 animate-pulse"
              />
            ))}
          </div>
        )}

        {/* Claude Sessions — PRIMARY ZONE */}
        {snapshot ? (
          <ClaudeSessionsPanel sessions={snapshot.claudeSessions} />
        ) : (
          <div className="flex flex-col gap-2">
            {Array.from({ length: 2 }).map((_, i) => (
              <div
                key={i}
                className="h-16 rounded-xl bg-gray-100 dark:bg-gray-800 animate-pulse"
              />
            ))}
          </div>
        )}

        {/* Top Processes */}
        {snapshot && <TopProcessesPanel processes={snapshot.topProcesses} />}
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Build and test end-to-end**

Run: `bun run build`

Then start the server: `cargo run -p claude-view-server`

Navigate to `http://localhost:47892/monitor`

Verify:

1. System gauges show live CPU/RAM/Disk/Network
2. Claude sessions are listed (start a Claude session in another terminal to test)
3. Top processes show Chrome, VS Code, etc.
4. Numbers animate smoothly (not jumping)
5. "Show all" expands the process list
6. Navigating away and back reconnects SSE
7. Check server logs: "Monitor polling task started" on page open, "Monitor polling task stopping" on navigate away

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/pages/SystemMonitorPage.tsx
git commit -m "feat(monitor): assemble full SystemMonitorPage with Apple + Swiss design"
```

---

## Chunk 4: Polish and Stagger Animations

### Task 11: Page mount stagger animation

**Files:**

- Modify: `apps/web/src/pages/SystemMonitorPage.tsx`

- [ ] **Step 1: Add staggered reveal on initial data load**

Wrap each section in a utility that applies `opacity-0 translate-y-2 → opacity-100 translate-y-0` with staggered delay. Use a simple CSS approach with inline `style={{ animationDelay }}`:

Add to the page a `<style>` block or use Tailwind's `animate-` classes. The simplest approach: add a utility CSS class in the component:

```tsx
// Add at top of SystemMonitorPage.tsx, inside the component:

const staggerClass = (index: number) =>
  `animate-in fade-in slide-in-from-bottom-2 duration-400 fill-mode-both`

const staggerStyle = (index: number) => ({
  animationDelay: `${index * 50}ms`,
})
```

Apply to each section wrapper:

```tsx
<div className={staggerClass(0)} style={staggerStyle(0)}>
  <SystemGaugeRow ... />
</div>
<div className={staggerClass(1)} style={staggerStyle(1)}>
  <ClaudeSessionsPanel ... />
</div>
<div className={staggerClass(2)} style={staggerStyle(2)}>
  <TopProcessesPanel ... />
</div>
```

Note: Check if the project uses `tailwindcss-animate` plugin (which provides `animate-in`, `fade-in`, `slide-in-from-bottom-*`). If not, use plain CSS keyframes:

```css
@keyframes monitor-reveal {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
```

And apply: `style={{ animation: 'monitor-reveal 400ms ease-out both', animationDelay: '${index * 50}ms' }}`

- [ ] **Step 2: Build and verify animations**

Run: `bun run build`

Refresh the monitor page — sections should fade in sequentially with slight upward slide.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/pages/SystemMonitorPage.tsx
git commit -m "feat(monitor): staggered page mount animation"
```

### Task 12: Remove `/system` redirect (now goes to `/monitor`)

**Files:**

- Modify: `apps/web/src/router.tsx`

- [ ] **Step 1: Update the redirect**

Change the existing `/system` redirect from `/settings` to `/monitor`:

```typescript
// Before:
{ path: 'system', element: <Navigate to="/settings" replace /> },
// After:
{ path: 'system', element: <Navigate to="/monitor" replace /> },
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/router.tsx
git commit -m "feat(monitor): redirect /system to /monitor instead of /settings"
```

### Task 14: Final build verification

- [ ] **Step 1: Full build**

Run: `bun run build`

Expected: Zero errors.

- [ ] **Step 2: Typecheck**

Run: `bunx turbo typecheck`

Expected: Zero errors.

- [ ] **Step 3: Run Rust tests**

Run: `cargo test -p claude-view-server`

Expected: All tests pass including the route registration test.

- [ ] **Step 4: Manual end-to-end verification**

Start server: `cargo run -p claude-view-server`

Open browser to `http://localhost:47892/monitor`

Checklist:

1. [ ] Page header shows "System Monitor" with live green dot
2. [ ] System hostname and core count displayed
3. [ ] 4 gauge cards show CPU%, Memory, Disk, Network
4. [ ] Gauge bars animate smoothly on value changes
5. [ ] Numbers tween (don't jump)
6. [ ] Claude Sessions section shows all active sessions
7. [ ] Session rows show status dot (green pulse for Working)
8. [ ] Session rows show CPU%, RAM, tokens, cost, tool count
9. [ ] Sessions with no PID show "--" for CPU/RAM
10. [ ] Top Processes shows top 5 grouped by app name
11. [ ] "Show all" expands to full list with smooth animation
12. [ ] Navigate away → server logs "polling task stopping"
13. [ ] Navigate back → reconnects, data resumes
14. [ ] Sidebar shows Monitor icon in both collapsed and expanded modes
15. [ ] Light/dark mode both look correct
16. [ ] Page mount has staggered fade-in animation
