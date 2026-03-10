# System Monitor Page — Design Spec

**Date:** 2026-03-11
**Status:** Approved
**Size:** Large feature (new page + backend endpoint + SSE stream)

## Summary

A dedicated `/monitor` page for self-hosted users to monitor their instance's system health. Apple + Swiss Modernism aesthetic with smooth animations. Claude sessions are the primary focus (always fully visible), supported by system-wide gauges (CPU, RAM, disk, network) and a top-5 process list (expandable to all).

**Core principle: Lazy observer.** The entire monitoring subsystem is demand-driven — zero polling, zero memory, zero CPU when nobody has the page open. Backend spawns the `sysinfo` polling task only when an SSE client connects, and kills it the instant all clients disconnect.

## Target User

Self-hosted instance admin who wants to:
- See all active Claude sessions and their resource consumption at a glance
- Understand system health (is the machine overloaded?)
- Identify resource hogs competing with Claude (Chrome, VS Code, Docker, etc.)
- React and adjust usage (kill sessions, close apps)

## Visual Language

**Style:** Swiss Modernism 2.0 + Apple Bento Grid
- Strict 12-column grid, Inter font, 8px base spacing unit
- Generous negative space, rounded cards (16px radius)
- Single accent color per semantic meaning (green=healthy, amber=warning, red=critical)

### Dark Mode Tokens

| Token | Value | Usage |
|-------|-------|-------|
| `--page-bg` | `#0a0a0f` | Page background |
| `--card-bg` | `#141419` | Bento card surface |
| `--card-border` | `rgba(255,255,255,0.06)` | Subtle card edge |
| `--text-primary` | `#f8fafc` | Headings, values |
| `--text-secondary` | `#94a3b8` | Labels, metadata |
| `--accent-green` | `#22c55e` | Working status, healthy gauges |
| `--accent-amber` | `#f59e0b` | Paused, warning thresholds (>70%) |
| `--accent-red` | `#ef4444` | High usage (>90%), errors |
| `--gauge-track` | `rgba(255,255,255,0.06)` | Empty gauge background |

**Light mode:** The app supports both dark and light mode. Light mode tokens follow the existing app pattern — `--card-bg: #ffffff`, `--page-bg: #f8fafc`, `--text-primary: #0f172a`, etc. The dark tokens above are the primary design target; light mode uses standard Tailwind `dark:` variant inversion as the rest of the app does.

## Animation System

All animations respect `prefers-reduced-motion`. Use `ease-out` for entering, `ease-in` for exiting.

| Element | Animation | Duration | Trigger |
|---------|-----------|----------|---------|
| Gauge bars | Width transition (smooth fill) | `300ms ease-out` | Value change from SSE |
| Gauge numbers | Counter tween (old to new via rAF) | `200ms` | Value change |
| Session rows | Fade-in + slide-up | `250ms ease-out` | New session discovered |
| Session removal | Fade-out + height collapse | `200ms ease-in` | Session closed |
| Status dot (Working) | Gentle pulse (opacity 0.4 to 1) | `2s ease-in-out infinite` | Working status only |
| Card hover | `scale(1.01)` + shadow elevation | `200ms ease-out` | Mouse enter |
| "Show all" expand | Height auto-animate + fade children | `300ms ease-out` | Click toggle |
| Page mount | Staggered card reveal (50ms offset each) | `400ms ease-out` | Initial load |
| Live indicator dot | Pulse ring expand + fade | `2s infinite` | SSE connected |
| Threshold color shift | Gauge bar color transition | `500ms ease-out` | Crossing 70% / 90% |

**Key Apple detail:** Number counter-tween. When CPU goes from 62% to 68%, the number smoothly counts through 63, 64, 65... over 200ms using `requestAnimationFrame` interpolation. This is what makes data feel alive.

## Page Layout

```
SystemMonitorPage
├── PageHeader              "System Monitor" + live status dot + connection state
├── SystemGaugeRow          4-column bento grid, compact (~80px)
│   ├── GaugeCard (CPU)     Horizontal bar + percentage + core count
│   ├── GaugeCard (Memory)  Horizontal bar + used/total GB
│   ├── GaugeCard (Disk)    Horizontal bar + used/total GB
│   └── GaugeCard (Network) Up/down speed indicators
├── ClaudeSessionsPanel     PRIMARY ZONE — flex-grow, ALL sessions always visible
│   ├── SectionHeader       "Claude Sessions" + active count badge
│   └── SessionRow[]        One per live session, never collapsed, never paginated
│       ├── StatusDot       Working (green pulse) / Paused (amber) / Done (gray)
│       ├── ProjectName     Bold, primary text
│       ├── ResourceMetrics CPU% bar + RAM value (from sysinfo per-PID)
│       └── SessionMeta     Branch, tokens, cost, tool calls (from LiveSession)
└── TopProcessesPanel       Secondary zone, default top 5, expandable to all
    ├── SectionHeader       "Top Processes" + "Show all" toggle
    └── ProcessRow[]        Grouped by app name, sorted by CPU%
        ├── AppIcon         Lucide icon or generic cube
        ├── AppName         "Google Chrome", "VS Code"
        ├── ProcessCount    "(12 processes)" secondary text
        ├── CpuBar          Thin horizontal bar
        └── RamValue        "2.1 GB"
```

### Information Hierarchy

| Zone | Behavior | Space |
|------|----------|-------|
| System Gauges | Compact single row, sticky when sessions overflow | ~80px fixed |
| Claude Sessions | **Primary — ALL sessions shown, never collapsed, no pagination** | Flex-grow |
| Top Processes | Default top 5, "Show all" expands full list | ~120px default |

### Claude Session Row Detail

Each row shows:
- **Line 1:** Status dot + Project name (bold) + Status label + CPU% mini-bar + RAM
- **Line 2:** Git branch + token count + cost + tool call count (secondary text)

### Process Grouping

All child processes of an app are aggregated into one row:
- Chrome (30 processes) → single row showing total CPU%, total RAM
- Sorted by total CPU% descending
- Default: top 5 visible
- "Show all" toggle reveals the complete list with smooth height animation

## Backend Architecture

### Lazy Observer Pattern

```
Browser opens /monitor
  → EventSource connects to GET /api/monitor/stream
    → Server increments Arc<AtomicUsize> subscriber count
    → If count goes from 0 → 1: spawn polling task
      → sysinfo::System created fresh
      → Poll every 1 second:
        - Global CPU, per-CPU
        - Memory (used, total, swap)
        - Disk: root mount point `/` only (APFS unified volume on macOS)
        - Network: sum all non-loopback interfaces; delta since last poll
          (first poll emits 0 for both rx/tx; no 32-bit counter wrap concern
           since sysinfo uses u64 counters)
        - Process table → group by app bundle name on macOS (not raw exe name,
          so "Google Chrome Helper (Renderer)" groups under "Google Chrome"),
          fall back to exe name on Linux
        - Match Claude PIDs from LiveSessionManager to sysinfo processes;
          sessions with pid=None get cpu_percent=0.0, memory_bytes=0
      → Broadcast ResourceSnapshot via SSE

Browser closes / navigates away
  → EventSource disconnects
    → Server decrements subscriber count
    → If count reaches 0: signal polling task to exit
      → sysinfo::System dropped (frees memory)
      → No background activity
```

### SSE Endpoint

`GET /api/monitor/stream`

Events:
- `monitor_snapshot` — Full ResourceSnapshot (sent every 1s)
- `monitor_connected` — Initial connection ack with system info (core count, total RAM, OS)
- Heartbeat every 15s (reuse existing SSE heartbeat pattern)

### Data Structures

```rust
// Sent on initial connection
struct SystemInfo {
    hostname: String,
    os_name: String,
    os_version: String,
    cpu_brand: String,
    cpu_core_count: u32,
    total_memory_bytes: u64,
    total_disk_bytes: u64,
}

// Sent every 1s while subscribed
struct ResourceSnapshot {
    timestamp_ms: u64,
    cpu_percent: f32,           // global
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    disk_used_bytes: u64,
    disk_total_bytes: u64,
    network_rx_bytes_per_sec: u64,
    network_tx_bytes_per_sec: u64,
    claude_sessions: Vec<SessionResource>,
    top_processes: Vec<ProcessGroup>,
}

// Reuse existing SessionStatus enum from live/state.rs
// (Working | Paused | Done) — generates discriminated union in TS

struct SessionResource {
    session_id: String,
    project_name: String,
    project_path: String,
    git_branch: Option<String>,
    status: SessionStatus,      // Existing enum from live/state.rs, NOT a bare String
    pid: Option<u32>,
    cpu_percent: f32,           // from sysinfo per-PID; 0.0 when pid is None
    memory_bytes: u64,          // from sysinfo per-PID (RSS); 0 when pid is None
    input_tokens: u64,          // from LiveSession accumulator
    output_tokens: u64,         // from LiveSession accumulator
    cache_read_tokens: u64,     // from LiveSession accumulator
    estimated_cost_usd: f64,    // COMPUTED from tokens * model pricing, NOT from JSONL
    tool_call_count: u32,       // from LiveSession accumulator
}
// NOTE on pid: LiveSession may have pid: None if detected via JSONL watcher
// before process scan completes. In this case cpu_percent=0.0, memory_bytes=0,
// and the UI shows "--" for resource columns instead of zero.

struct ProcessGroup {
    name: String,               // "Google Chrome", "Code" (VS Code)
    process_count: u32,
    total_cpu_percent: f32,
    total_memory_bytes: u64,
}
```

### REST Fallback

`GET /api/monitor/snapshot` — Single snapshot for clients that don't support SSE. Same `ResourceSnapshot` payload.

## Frontend Architecture

### Hook: `useSystemMonitor()`

```typescript
// Connects to SSE only when component is mounted (page is open)
// Disconnects on unmount (navigating away)
// Stores latest raw snapshot in state
// Does NOT tween — individual GaugeCards call useTweenedValue themselves

interface UseSystemMonitorReturn {
    connected: boolean;
    systemInfo: SystemInfo | null;         // static, set on initial connect
    snapshot: ResourceSnapshot | null;     // latest raw values from SSE
}
```

### Counter Tween Hook: `useTweenedValue(target: number)`

Generic reusable hook — each `GaugeCard` and `SessionRow` calls this independently for its own values:

- Uses `requestAnimationFrame` to interpolate from previous to target
- 200ms duration, ease-out curve
- Respects `prefers-reduced-motion` (instant jump if enabled)
- Composable: `const tweenedCpu = useTweenedValue(snapshot?.cpu_percent ?? 0)`

### Route & Navigation

| Item | Value |
|------|-------|
| Route | `/monitor` |
| Sidebar icon | `Activity` (lucide-react) |
| Sidebar label | "Monitor" |
| Position | Below "Plugins", above "Settings" |

## Responsive Breakpoints

| Breakpoint | Gauge Row | Session Rows | Process Panel |
|------------|-----------|-------------|---------------|
| `>=1024px` | 4-col grid | Metrics inline | 2-col grid |
| `768-1023px` | 2x2 grid | Meta wraps below | 1-col list |
| `<768px` | 1-col stack | Card layout | 1-col list |

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| No Claude sessions | Empty state: "No active Claude sessions" centered, gauges still work |
| SSE disconnects | Amber "Reconnecting..." banner, auto-retry with exponential backoff |
| macOS sandbox blocks process info | Fall back to `lsof` for CWD (existing pattern in `process.rs`) |
| 20+ Claude sessions | All visible, page scrolls. Gauge row stays sticky at top |
| Process name ambiguity | Group by executable name, show "(N processes)" count |
| `sysinfo` first poll slow | Show skeleton cards on mount, replace with data on first snapshot |
| Reduced motion preference | All animations instant, no pulse, no tween — values jump directly |

## What This Is NOT

- NOT a historical monitoring dashboard (no time-series storage, no graphs over time)
- NOT a full process manager (no kill-all-chrome button)
- NOT always-running (zero cost when page is closed)
- NOT replacing the existing Live Monitor page (that's for session content; this is for system resources)

## Files To Create/Modify

### New Files
- `crates/server/src/live/monitor.rs` — sysinfo polling, lazy observer, ResourceSnapshot
- `crates/server/src/routes/monitor.rs` — SSE endpoint, REST fallback
- `apps/web/src/pages/SystemMonitorPage.tsx` — Page component
- `apps/web/src/hooks/use-system-monitor.ts` — SSE hook
- `apps/web/src/hooks/use-tweened-value.ts` — rAF counter tween
- `apps/web/src/components/monitor/GaugeCard.tsx` — System gauge bento card
- `apps/web/src/components/monitor/SessionRow.tsx` — Claude session row
- `apps/web/src/components/monitor/ProcessRow.tsx` — System process row
- `apps/web/src/components/monitor/SystemGaugeRow.tsx` — 4-gauge grid container
- `apps/web/src/components/monitor/ClaudeSessionsPanel.tsx` — Primary sessions zone
- `apps/web/src/components/monitor/TopProcessesPanel.tsx` — Expandable process list

### Modified Files
- `crates/server/src/routes/mod.rs` — Register monitor routes
- `crates/server/src/live/mod.rs` — Export monitor module
- `apps/web/src/router.tsx` — Add `/monitor` route
- `apps/web/src/components/Sidebar.tsx` — Add Monitor nav item

### Generated Types
- `ResourceSnapshot`, `SessionResource`, `ProcessGroup`, `SystemInfo` — Add `#[derive(TS)]` to Rust structs, auto-generate into `apps/web/src/types/generated/`
