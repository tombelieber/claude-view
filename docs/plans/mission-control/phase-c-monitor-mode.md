---
status: done
date: 2026-02-10
phase: C
depends_on: B
---

# Phase C: Monitor Mode - Live Terminal Grid

> **Architectural update (2026-02-16):** Monitor mode uses RichPane (HTML) exclusively. xterm.js is deferred to Phase F (Interactive Control) where we own the PTY. See `docs/plans/2026-02-16-monitor-rich-only.md` for rationale.

> A "security camera grid" for Claude Code sessions. Users see multiple sessions' real-time output simultaneously, rendered via RichPane (HTML chat view) over WebSocket connections.

## Prerequisites

- **Phase A** (Read-Only Monitoring): Session state machine, JSONL file watching via `notify`, cost calculator, SSE infrastructure for structured data updates.
- **Phase B** (Views & Layout): View switcher infrastructure (Grid/List/Kanban tabs), keyboard shortcuts, mobile-responsive layout shell. Monitor Mode is the fourth view tab.

Phase C assumes the following Phase A/B deliverables exist:

| From Phase | Deliverable | Used By Phase C |
|------------|-------------|-----------------|
| A | `LiveSessionState` struct with status, cost, context% | MonitorPane header displays |
| A | `notify`-based JSONL file watcher (`crates/server/src/watcher.rs`) | Terminal endpoint reuses watcher for new-line streaming |
| A | SSE `/api/live/sessions` endpoint | Grid layout subscribes for session add/remove events |
| B | View switcher component (`src/components/live/ViewSwitcher.tsx`) | Monitor Mode registered as fourth tab |
| B | Responsive layout shell | MonitorGrid inherits breakpoint system |

---

## Architecture Overview

```
                    ┌────────────────────────────────┐
                    │         Browser (React SPA)     │
                    │                                 │
                    │  ┌─────────┐  ┌─────────┐      │
                    │  │MonitorPane│ │MonitorPane│ ... │
                    │  │ xterm.js │  │ xterm.js │      │
                    │  └────┬────┘  └────┬────┘      │
                    │       │ WS          │ WS        │
                    └───────┼─────────────┼───────────┘
                            │             │
                    ┌───────┴─────────────┴───────────┐
                    │     Axum Server (:47892)         │
                    │                                  │
                    │  WS /api/live/sessions/:id/term  │
                    │  ┌──────────────────────────┐    │
                    │  │   TerminalConnection      │   │
                    │  │   - Read last N lines     │   │
                    │  │   - notify watcher        │   │
                    │  │   - Stream new lines       │   │
                    │  └──────────────────────────┘    │
                    │                                  │
                    └──────────────────────────────────┘
                                     │
                            ┌────────┴────────┐
                            │ ~/.claude/       │
                            │ projects/        │
                            │   */*.jsonl      │
                            └─────────────────┘
```

**Key protocol decision (from PROGRESS.md):** SSE for structured data (session status, cost updates), WebSocket for terminal byte streams. WebSocket is required here because xterm.js needs a bidirectional channel (client sends resize events, mode preferences) and binary-friendly streaming.

---

## Step 1: WebSocket Terminal Endpoint (Backend)

### 1.1 Add WebSocket Dependencies

**File: `Cargo.toml` (workspace root)**

Add to `[workspace.dependencies]`:

```toml
axum = { version = "0.8", features = ["macros", "ws"] }  # add "ws" feature
```

The `ws` feature enables `axum::extract::ws::WebSocket` and the upgrade handler. No additional crate needed -- Axum bundles `tokio-tungstenite` internally.

**File: `crates/server/Cargo.toml`**

No change needed -- `axum` is already a workspace dependency. The `ws` feature propagates from the workspace root.

### 1.2 Terminal Connection State

**File: `crates/server/src/terminal_state.rs` (new)**

Manages active WebSocket connections. Tracks which sessions have active terminal viewers to avoid duplicate file watchers.

```rust
pub struct TerminalConnectionManager {
    /// Map of session_id -> count of active WebSocket connections.
    /// When count drops to 0, the file watcher for that session is dropped.
    active: DashMap<String, usize>,
}
```

**Design decisions:**
- Use `DashMap` (add as workspace dependency) for lock-free concurrent access, or use `std::sync::RwLock<HashMap<String, usize>>` to avoid a new dependency. Prefer `RwLock<HashMap>` initially since connection counts are low (~8-20 concurrent).
- Connection count tracks viewers, not sessions. Multiple browser tabs can watch the same session.

### 1.3 WebSocket Route Handler

**File: `crates/server/src/routes/terminal.rs` (new)**

```
WS /api/live/sessions/:id/terminal
```

**Connection lifecycle:**

1. **Upgrade**: Client sends HTTP upgrade request. Axum's `WebSocketUpgrade` extractor handles the handshake.
2. **Handshake message**: Client sends first WS text frame with mode preference:
   ```json
   { "mode": "raw" | "rich", "scrollback": 100 }
   ```
   - `raw`: Lines are sent as UTF-8 text with ANSI escape codes preserved. xterm.js renders them directly.
   - `rich`: Lines are parsed as JSON and sent as structured message objects. Frontend renders React cards.
   - `scrollback`: Number of historical lines to send as initial buffer (default: 100, max: 500).
3. **Initial buffer**: Server reads the last `scrollback` lines from the session's JSONL file and sends them as the first batch.
4. **Live streaming**: Server registers a `notify` watcher on the JSONL file. On each `modify` event, reads new bytes appended since last position, splits by newline, sends each line.
5. **Disconnect**: Client closes WS or connection drops. Server drops the watcher, decrements connection count.

**Message format (server -> client):**

Raw mode:
```json
{ "type": "line", "data": "assistant: Here is the fix for..." }
{ "type": "line", "data": "[tool_use] Read file: src/main.rs" }
{ "type": "buffer_end" }  // Marks end of initial scrollback
{ "type": "line", "data": "..." }  // Live lines after this
```

Rich mode:
```json
{ "type": "message", "role": "assistant", "content": "Here is the fix...", "ts": 1707580000 }
{ "type": "tool_use", "name": "Read", "input": {"path": "src/main.rs"}, "ts": 1707580001 }
{ "type": "buffer_end" }
```

**Message format (client -> server):**
```json
{ "type": "mode", "mode": "raw" | "rich" }  // Switch mode mid-stream
{ "type": "resize", "cols": 120, "rows": 30 }  // Inform server of terminal size (for future use)
{ "type": "ping" }  // Keepalive
```

**Error handling:**
- Session JSONL file not found: Send `{ "type": "error", "message": "Session not found" }` then close with 4004 code.
- File watcher fails: Send `{ "type": "error", "message": "Watch failed" }` then close with 4500 code.
- Malformed client message: Ignore silently (log at debug level).

**Implementation constraints (from CLAUDE.md):**
- Use `memmem::Finder` to pre-filter JSONL lines before JSON parse in rich mode (SIMD pre-filter rule).
- Read last N lines via seeking to end and scanning backwards for newlines -- do NOT read the entire file.
- The `notify` watcher MUST be dropped when the WebSocket closes. Use `tokio::select!` with a cancellation token or simply drop the watcher handle in the task's cleanup.

### 1.4 JSONL Tail Reader

**File: `crates/core/src/tail.rs` (new)**

Utility for reading the last N lines of a JSONL file efficiently. This is used by the terminal endpoint for initial scrollback and could be reused elsewhere.

```rust
/// Read the last `n` lines from a file without loading the entire file.
///
/// Strategy: seek to EOF, read backwards in 8KB chunks, find newlines.
/// Returns lines in chronological order (oldest first).
pub async fn tail_lines(path: &Path, n: usize) -> io::Result<Vec<String>>
```

**Why in `crates/core/` not `crates/server/`:** The tail reader is a file utility, not HTTP-specific. It can be tested independently and reused by future CLI tools.

### 1.5 File Position Tracker

**File: `crates/server/src/file_tracker.rs` (new)**

Tracks the byte offset of each watched file so that on `notify::Modify` events, only new bytes are read.

```rust
pub struct FilePositionTracker {
    /// Current read position (byte offset from start of file).
    position: u64,
    path: PathBuf,
}

impl FilePositionTracker {
    /// Read bytes from current position to EOF, update position.
    /// Returns the new lines as a Vec<String>.
    pub async fn read_new_lines(&mut self) -> io::Result<Vec<String>>
}
```

**Important:** If the file is truncated (position > file size), reset position to 0 and re-read. This handles the edge case where Claude Code rotates/rewrites a session file (unlikely but defensive).

### 1.6 Register Route

**File: `crates/server/src/routes/mod.rs`**

Add `pub mod terminal;` and nest the router:

```rust
.nest("/api/live", terminal::router())
```

Use the `/api/live` prefix to namespace all real-time endpoints (SSE and WebSocket) under a single path, distinct from the existing `/api` REST routes.

### 1.7 AppState Extension

**File: `crates/server/src/state.rs`**

Add the `TerminalConnectionManager` to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    /// Tracks active WebSocket terminal connections per session.
    pub terminal_connections: TerminalConnectionManager,
}
```

### 1.8 Vite Proxy for WebSocket

**File: `vite.config.ts`**

Add WebSocket proxy rule so dev mode works:

```ts
proxy: {
  '/api/live': {
    target: 'http://localhost:47892',
    ws: true,  // Enable WebSocket proxying
  },
  '/api': 'http://localhost:47892',
}
```

**Note:** Vite's `http-proxy` handles WebSocket upgrades correctly (unlike SSE). The `/api/live` rule must come before `/api` so it matches first. However, per CLAUDE.md's SSE bypass pattern, if we discover buffering issues in practice, the frontend `wsUrl()` helper (Step 2.3) already supports direct connection bypass.

### 1.9 Tests

**File: `crates/core/src/tail.rs` (inline `#[cfg(test)]`)**

| Test | What it verifies |
|------|-----------------|
| `tail_0_lines_returns_empty` | Edge case: n=0 |
| `tail_fewer_than_n` | File has 3 lines, request 100, get 3 |
| `tail_exact` | File has 100 lines, request 100, get 100 |
| `tail_last_5` | File has 1000 lines, request 5, get last 5 in order |
| `tail_empty_file` | Empty file returns empty vec |
| `tail_large_file` | 10MB file, request last 10 lines, completes in <10ms |

**File: `crates/server/src/routes/terminal.rs` (inline `#[cfg(test)]`)**

| Test | What it verifies |
|------|-----------------|
| `ws_upgrade_returns_101` | HTTP upgrade handshake works |
| `ws_unknown_session_returns_error` | 4004 close code for missing session |
| `ws_initial_buffer_sent` | First messages are historical lines, ends with `buffer_end` |
| `ws_live_lines_streamed` | Append to JSONL file -> WS client receives new line |
| `ws_disconnect_drops_watcher` | After client disconnect, no more file watching |
| `ws_mode_switch` | Client sends mode change, subsequent lines use new format |

---

## Step 2: xterm.js Integration (Frontend) -- DEFERRED TO PHASE F

> **Deferred (2026-02-16):** xterm.js is deferred to Phase F (Interactive Control) where we own the PTY via Agent SDK. Monitor mode reads JSONL (structured data), so HTML rendering via RichPane is strictly better. The WebSocket hook and infrastructure from Step 1 remain and are used by RichPane. See `docs/plans/2026-02-16-monitor-rich-only.md` for rationale.

The TerminalPane component, xterm.js dependencies, and WebGL renderer are not needed for Monitor mode. When Phase F adds interactive control (Agent SDK spawns a process we own), xterm.js will be introduced for bidirectional terminal I/O.

### 2.3 WebSocket URL Helper (RETAINED)

**File: `src/lib/ws-url.ts` (new)**

Mirrors the `sseUrl()` bypass pattern from CLAUDE.md:

```ts
/**
 * Construct WebSocket URL, bypassing Vite proxy in dev mode.
 * Vite proxies WS correctly (unlike SSE), but we keep the bypass
 * option for consistency and as a fallback.
 */
export function wsUrl(path: string): string {
  const loc = window.location;
  // Dev mode: Vite runs on 5173, proxy to Rust on 47892
  if (loc.port === '5173') {
    return `ws://localhost:47892${path}`;
  }
  // Production: same origin, upgrade protocol
  const protocol = loc.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${loc.host}${path}`;
}
```

### 2.4 useTerminalSocket Hook (RETAINED)

**File: `src/hooks/use-terminal-socket.ts` (new)**

Encapsulates WebSocket connection logic with auto-reconnect. Used by RichPane to receive structured messages.

```ts
interface UseTerminalSocketOptions {
  sessionId: string;
  scrollback?: number;
  enabled: boolean;  // false when pane not visible
  onMessage: (data: string) => void;
  onConnectionChange?: (state: ConnectionState) => void;
}

type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error';
```

**Reconnect strategy:**
- On unexpected disconnect: wait 1s, then reconnect with exponential backoff (1s, 2s, 4s, max 30s).
- On intentional disconnect (`enabled` set to `false`): no reconnect.
- On `buffer_end` event: update connection state to `'connected'` (marks initial sync complete).
- Max reconnect attempts: 10. After that, stay in `'error'` state until user manually retries.

**Rules from CLAUDE.md:**
- No hooks after early returns. All hooks declared at top of component/hook.
- `useMemo` any parsed objects used in `useEffect` deps.

---

## Step 3: Responsive Grid Layout

### 3.1 MonitorGrid Component

**File: `src/components/live/MonitorGrid.tsx` (new)**

CSS Grid container that arranges MonitorPane children responsively.

```tsx
interface MonitorGridProps {
  sessions: LiveSession[];
  /** User override for grid dimensions. null = auto-responsive */
  gridOverride: { cols: number; rows: number } | null;
}
```

**Responsive breakpoints (auto mode):**

| Breakpoint | Screen | Grid | Rationale |
|------------|--------|------|-----------|
| < 640px | Mobile | 1x1 | One pane + swipe navigation |
| 640-1023px | Tablet | 1x2 | Two panes stacked |
| 1024-1439px | Laptop | 2x2 | Four-pane quad view |
| 1440-2559px | Desktop | 2x3 | Six panes, common monitor |
| >= 2560px | Ultrawide | 2x4 | Eight panes, 4K/ultrawide |

**CSS implementation:**

Use CSS Grid with `auto-fill` and `minmax` for fluid responsiveness:

```css
.monitor-grid {
  display: grid;
  gap: 4px;                              /* Tight gap for max terminal space */
  grid-template-columns: repeat(auto-fill, minmax(480px, 1fr));
  grid-auto-rows: minmax(300px, 1fr);    /* Minimum useful terminal height */
  height: 100%;
  overflow: hidden;
}
```

When `gridOverride` is set, use explicit template:

```css
grid-template-columns: repeat(var(--cols), 1fr);
grid-template-rows: repeat(var(--rows), 1fr);
```

**Mobile swipe (< 640px):**

When only 1 pane is visible, render all panes in a horizontal scroll-snap container:

```css
.monitor-grid--mobile {
  display: flex;
  overflow-x: auto;
  scroll-snap-type: x mandatory;
}
.monitor-grid--mobile > * {
  scroll-snap-align: start;
  min-width: 100%;
  flex-shrink: 0;
}
```

Show dot indicators at bottom for current position (like iOS home screen).

### 3.2 Grid Controls Bar

**File: `src/components/live/GridControls.tsx` (new)**

Toolbar above the grid with:

| Control | Type | Behavior |
|---------|------|----------|
| Rows x Cols slider | Range input pair | Override auto-responsive. Range: 1-4 rows, 1-4 cols. |
| "Auto" button | Toggle | Reset to auto-responsive mode |
| "Compact headers" toggle | Checkbox | Collapse pane headers to single line |
| Active pane count | Badge | "6 of 12 sessions" |

**localStorage persistence:**

```ts
const GRID_PREFS_KEY = 'claude-view:monitor-grid-prefs';

interface GridPrefs {
  override: { cols: number; rows: number } | null;
  compactHeaders: boolean;
}
```

Read on mount, write on change. Use `zustand` with `persist` middleware (already in dependencies) for consistency with other app state.

### 3.3 Zustand Store

**File: `src/stores/monitor-store.ts` (new)**

```ts
interface MonitorStore {
  // Grid layout
  gridOverride: { cols: number; rows: number } | null;
  compactHeaders: boolean;
  setGridOverride: (override: { cols: number; rows: number } | null) => void;
  setCompactHeaders: (compact: boolean) => void;

  // Pane state
  selectedPaneId: string | null;
  expandedPaneId: string | null;
  pinnedPaneIds: Set<string>;
  hiddenPaneIds: Set<string>;
  paneMode: Record<string, 'raw' | 'rich'>;  // per-pane mode

  // Actions
  selectPane: (id: string | null) => void;
  expandPane: (id: string | null) => void;
  pinPane: (id: string) => void;
  unpinPane: (id: string) => void;
  hidePane: (id: string) => void;
  showPane: (id: string) => void;
  setPaneMode: (id: string, mode: 'raw' | 'rich') => void;
}
```

Persist `gridOverride`, `compactHeaders`, `pinnedPaneIds`, and `paneMode` to localStorage. Do NOT persist `selectedPaneId` or `expandedPaneId` (ephemeral UI state).

---

## Step 4: Monitor Pane Component

### 4.1 MonitorPane

**File: `src/components/live/MonitorPane.tsx` (new)**

Wraps `TerminalPane` (or `RichPane`) with a chrome header/footer.

```tsx
interface MonitorPaneProps {
  session: LiveSession;
  isSelected: boolean;
  isExpanded: boolean;
  isPinned: boolean;
  mode: 'raw' | 'rich';
  compactHeader: boolean;
  isVisible: boolean;
  onSelect: () => void;
  onExpand: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}
```

**Layout (default header):**

```
┌─[●] my-project / feature-branch ──── $2.34 │ 67% ctx │ ⏸ ─── [↕][×]─┐
│                                                                         │
│  (TerminalPane or RichPane content area)                                │
│                                                                         │
├─ Writing tests... │ Turn 14 │ [sub-agent placeholder] ──────────────────┤
└─────────────────────────────────────────────────────────────────────────┘
```

**Compact header (when `compactHeaders` is true):**

```
┌─● my-project $2.34 67% T14 ── [↕]──────────────────────────────────────┐
```

**Header elements:**

| Element | Source | Style |
|---------|--------|-------|
| Status dot | `session.status` | Green (#22C55E) = working, Amber (#F59E0B) = waiting, Gray (#6B7280) = idle |
| Project name | `session.projectName` | Truncate with ellipsis at 20ch |
| Branch | `session.branch` | Truncate at 15ch, gray text |
| Cost | `session.totalCost` | Format: `$X.XX` |
| Context % | `session.contextPercent` | Color: green <50%, amber 50-80%, red >80% |
| Status icon | `session.status` | Spinner = working, pause = waiting, check = done |
| Expand button | Click handler | `[↕]` icon, toggles full-screen |
| Close button | Click handler | `[x]` icon, hides pane |

**Footer elements:**

| Element | Source | Style |
|---------|--------|-------|
| Current activity | `session.lastActivity` | Truncate at 40ch |
| Turn count | `session.turnCount` | "Turn 14" |
| Sub-agent pills | Placeholder for Phase D | Gray pill: "2 sub-agents" (non-functional until Phase D) |

**Interactions:**

| Action | Trigger | Effect |
|--------|---------|--------|
| Select | Single-click header | Blue border (#3B82F6, 2px), `onSelect()` |
| Deselect | Click header of selected pane | Remove border, `onSelect(null)` |
| Expand | Double-click anywhere, or click expand button | Pane goes full-screen overlay, ESC to close |
| Context menu | Right-click | Opens context menu (Step 6) |
| Hover | Mouse enter | Show subtle border highlight (gray-700) |

**Active state rule (from uiux-notes.md):** Selected pane MUST have visually distinct border. Use `ring-2 ring-blue-500` (Tailwind).

### 4.2 Expanded Pane Overlay

**File: `src/components/live/ExpandedPaneOverlay.tsx` (new)**

Full-screen overlay for an expanded pane. Renders over the grid with a semi-transparent backdrop.

```tsx
interface ExpandedPaneOverlayProps {
  session: LiveSession;
  mode: 'raw' | 'rich';
  onClose: () => void;
}
```

- Backdrop: `bg-black/80` (80% opaque black)
- Pane fills 95vw x 90vh, centered
- Close: ESC key, click backdrop, or close button
- xterm.js re-fits to the larger container via `fitAddon.fit()`
- The expanded pane gets its own WebSocket connection (or reuses the existing one if already connected)

---

## Step 5: Verbose Toggle

> **Updated (2026-02-16):** Replaces the original "Rich vs Raw Toggle" section. Monitor mode is RichPane-only (no xterm.js). The toggle now controls verbosity level: chat-only (default) vs. full details.

### 5.1 RichPane Component

**File: `src/components/live/RichPane.tsx` (new)**

The sole rendering component for Monitor mode panes. Renders structured messages as compact React cards, using a virtualized list for performance.

```tsx
interface RichPaneProps {
  messages: RichMessage[];
  isVisible: boolean;
  verbose: boolean;
}
```

Uses `react-virtuoso` (already in dependencies) for virtualized scrolling. Each message renders as a compact card:

| Message type | Chat mode (default) | Verbose mode |
|--------------|-------------------|-------------|
| User prompt | Blue-left-border card, truncated to 2 lines | Same |
| Assistant text | White text, truncated to 3 lines with "..." expand | Same |
| Tool use | Hidden | Orange pill: `Read src/main.rs`, `Edit 5 files`, etc. |
| Tool result | Hidden | Gray, collapsed by default (click to expand) |
| Thinking | Hidden | Italic gray, collapsed |
| Error | Red-left-border card | Same |

**Auto-scroll:** Pin to bottom when new messages arrive. If user has scrolled up, show "New messages" pill at bottom to jump back.

### 5.2 Verbose Toggle Button

Located in the pane header. Simple icon toggle:

- Chat mode icon: `MessageSquare` (from lucide-react, already in dependencies)
- Verbose mode icon: `List` (from lucide-react)

Toggle is client-side only -- all message types are always delivered over the WebSocket. The frontend filters what to display based on the verbose flag.

**Default mode:** Chat (non-verbose) for new panes. Rationale: in a small grid pane, showing only user prompts and assistant responses provides the best scanability. Users who need to see tool calls, thinking, and results toggle to verbose.

**Per-pane persistence:** Stored in `MonitorStore.verboseMode` (zustand). Persisted to localStorage so mode preferences survive page reloads.

---

## Step 6: Pane Selection & Actions

### 6.1 Context Menu

**File: `src/components/live/PaneContextMenu.tsx` (new)**

Right-click context menu for panes. Uses a simple custom dropdown (no external library needed).

| Action | Icon | Behavior |
|--------|------|----------|
| Pin | `Pin` | Keeps pane visible even if session becomes idle. Pinned panes show a pin icon in header. |
| Unpin | `PinOff` | Removes pin (only shown for pinned panes) |
| Hide | `EyeOff` | Removes pane from grid. Session still tracked, can restore from session list sidebar. |
| Move to front | `ArrowUpToLine` | Reorder pane to first position in grid |
| Expand | `Maximize2` | Same as double-click: full-screen overlay |
| Switch to Raw/Rich | `Terminal`/`MessageSquare` | Toggle mode (label reflects current mode's opposite) |

**Implementation:** Render a `<div>` positioned at the right-click coordinates with `position: fixed`. Close on click outside, ESC, or scroll. Use `useEffect` cleanup to remove event listeners.

### 6.2 Auto-Fill Behavior

When a new session becomes active (detected via Phase A's SSE `/api/live/sessions` stream) and the grid has empty slots:

1. Check if the session is in `hiddenPaneIds` -- if so, do NOT auto-fill.
2. If grid has fewer panes than `rows * cols`, add the new session to the next empty slot.
3. If grid is full, do nothing (new session appears in the session list sidebar but not the grid).

When a session becomes idle and is NOT pinned:
1. After 60 seconds of idle, fade the pane's border to gray.
2. After 5 minutes of idle, if there are active sessions waiting for a slot, swap the idle pane out.
3. Pinned panes are NEVER auto-removed.

### 6.3 Keyboard Shortcuts

Integrate with Phase B's keyboard shortcut system.

| Key | Action |
|-----|--------|
| `1-9` | Select pane by position (1 = top-left, 2 = next, etc.) |
| `Enter` | Expand selected pane |
| `ESC` | Close expanded pane, or deselect if no pane expanded |
| `P` | Toggle pin on selected pane |
| `H` | Hide selected pane |
| `M` | Toggle raw/rich mode on selected pane |
| `+` / `-` | Increase/decrease grid columns |

---

## Step 7: Performance Optimization

### 7.1 Visibility-Based Connection Management

**Only connect WebSocket for VISIBLE panes.**

Use `IntersectionObserver` to track which panes are in the viewport:

```tsx
// In MonitorGrid.tsx
const observerRef = useRef<IntersectionObserver | null>(null);
const [visiblePanes, setVisiblePanes] = useState<Set<string>>(new Set());

useEffect(() => {
  observerRef.current = new IntersectionObserver(
    (entries) => {
      setVisiblePanes(prev => {
        const next = new Set(prev);
        for (const entry of entries) {
          const id = entry.target.getAttribute('data-session-id');
          if (!id) continue;
          if (entry.isIntersecting) next.add(id);
          else next.delete(id);
        }
        return next;
      });
    },
    { threshold: 0.1 }  // 10% visible = connect
  );
  return () => observerRef.current?.disconnect();
}, []);
```

Pass `isVisible={visiblePanes.has(session.id)}` to each `MonitorPane`. The `TerminalPane` connects/disconnects its WebSocket based on this prop.

**Debounce:** Add 500ms debounce before disconnecting on visibility loss. This prevents rapid connect/disconnect during resize or scroll.

### 7.2 xterm.js Resource Limits

| Setting | Value | Rationale |
|---------|-------|-----------|
| `scrollback` | 1000 lines | Each line ~200 bytes, 1000 lines ~200KB per pane. 8 panes = ~1.6MB. |
| WebGL renderer | Enabled | GPU-accelerated, no CPU overhead for rendering |
| `fastScrollModifier` | `'alt'` | Alt+scroll for fast scrolling |
| `cursorBlink` | `false` | No animation overhead |
| `disableStdin` | `true` | No input processing overhead |

### 7.3 Write Throttling

As detailed in Step 2.2, buffer writes and flush at 60fps via `requestAnimationFrame`. This prevents xterm.js from being overwhelmed by rapid line output (e.g., a build command printing hundreds of lines per second).

### 7.4 Backend Connection Limits

In `crates/server/src/routes/terminal.rs`:

```rust
/// Maximum concurrent WebSocket connections across all sessions.
const MAX_WS_CONNECTIONS: usize = 32;

/// Maximum concurrent viewers per session.
const MAX_VIEWERS_PER_SESSION: usize = 4;
```

If limits are exceeded, reject the WebSocket upgrade with HTTP 429 and a JSON error body.

### 7.5 Memory Budget

Target: **< 500MB total with 8 active panes.**

| Component | Per-pane | 8 panes |
|-----------|----------|---------|
| xterm.js buffer (1000 lines) | ~200KB | ~1.6MB |
| WebGL context | ~20MB | ~160MB |
| WebSocket buffers | ~50KB | ~400KB |
| React component tree | ~5KB | ~40KB |
| **Total frontend** | | **~162MB** |
| Rust file watchers | ~1KB | ~8KB |
| Rust WS connections | ~16KB | ~128KB |
| **Total backend** | | **~136KB** |

Well within the 500MB budget. The WebGL contexts are the largest cost. If a system lacks GPU memory, the canvas fallback uses ~2MB per pane instead (total ~16MB for 8 panes).

### 7.6 Virtualization for > 8 Panes

If the user configures a grid with > 8 panes (e.g., 3x4 = 12), only render the DOM nodes for panes currently in the viewport. Use the `IntersectionObserver` from 7.1 -- panes outside the viewport render as a lightweight placeholder (session name + status dot only, no terminal).

---

## New & Modified Files Summary

### New Files

| File | Crate/Dir | Purpose |
|------|-----------|---------|
| `crates/core/src/tail.rs` | core | Efficient last-N-lines reader for JSONL files |
| `crates/server/src/terminal_state.rs` | server | WebSocket connection manager |
| `crates/server/src/file_tracker.rs` | server | Byte-offset tracker for incremental file reads |
| `crates/server/src/routes/terminal.rs` | server | WebSocket `/api/live/sessions/:id/terminal` handler |
| `src/components/live/MonitorGrid.tsx` | frontend | Responsive CSS Grid container |
| `src/components/live/MonitorPane.tsx` | frontend | Pane chrome (header, footer, border) |
| `src/components/live/TerminalPane.tsx` | frontend | xterm.js wrapper |
| `src/components/live/RichPane.tsx` | frontend | Rich mode message cards |
| `src/components/live/GridControls.tsx` | frontend | Grid size controls toolbar |
| `src/components/live/ExpandedPaneOverlay.tsx` | frontend | Full-screen expanded pane |
| `src/components/live/PaneContextMenu.tsx` | frontend | Right-click context menu |
| `src/hooks/use-terminal-socket.ts` | frontend | WebSocket connection hook with reconnect |
| `src/lib/ws-url.ts` | frontend | WebSocket URL builder (dev/prod) |
| `src/stores/monitor-store.ts` | frontend | Zustand store for monitor state |

### Modified Files

| File | Change |
|------|--------|
| `Cargo.toml` | Add `ws` feature to axum |
| `crates/core/src/lib.rs` | Add `pub mod tail;` |
| `crates/server/src/routes/mod.rs` | Add `pub mod terminal;` and nest router under `/api/live` |
| `crates/server/src/state.rs` | Add `TerminalConnectionManager` field |
| `crates/server/src/lib.rs` | Add `pub mod terminal_state;` and `pub mod file_tracker;` |
| `vite.config.ts` | Add `/api/live` WebSocket proxy rule |
| `package.json` | Add `@xterm/xterm`, `@xterm/addon-fit`, `@xterm/addon-webgl` dependencies |
| Phase B's `ViewSwitcher.tsx` | Register "Monitor" as fourth view tab |

---

## Implementation Order

Execute these steps sequentially. Each step builds on the previous.

| Step | Deliverable | Estimated Effort | Can Test Independently? |
|------|-------------|-----------------|------------------------|
| 1a | `crates/core/src/tail.rs` (tail reader) | 2h | Yes -- unit tests with temp files |
| 1b | `crates/server/src/terminal_state.rs` + `file_tracker.rs` | 2h | Yes -- unit tests |
| 1c | `crates/server/src/routes/terminal.rs` (WS endpoint) | 4h | Yes -- `websocat` CLI or integration test |
| 2a | `@xterm/xterm` deps + `TerminalPane.tsx` | 3h | Yes -- render in isolation with mock WS |
| 2b | `use-terminal-socket.ts` hook | 2h | Yes -- connect to Step 1c endpoint |
| 3 | `MonitorGrid.tsx` + `GridControls.tsx` + `monitor-store.ts` | 3h | Yes -- render with mock panes |
| 4 | `MonitorPane.tsx` + `ExpandedPaneOverlay.tsx` | 3h | Yes -- render with mock data |
| 5 | `RichPane.tsx` + mode toggle | 2h | Yes -- render with mock messages |
| 6 | `PaneContextMenu.tsx` + auto-fill + keyboard shortcuts | 2h | Yes -- interaction testing |
| 7 | Performance tuning (IntersectionObserver, throttling, limits) | 2h | Measure with 8+ panes |

**Total estimated effort: ~25 hours**

---

## Acceptance Criteria

- [ ] **AC-1: Monitor view accessible.** Monitor Mode appears as a tab in the view switcher (from Phase B). Clicking it shows the grid.
- [ ] **AC-2: Responsive grid.** Grid adapts to screen size per breakpoint table (1x1 at 375px through 2x4 at 3440px). Verify at each breakpoint.
- [ ] **AC-3: rows x cols slider.** User can override auto-responsive with manual grid dimensions. Override persists across page reloads (localStorage).
- [ ] **AC-4: RichPane renders markdown correctly.** Tables, bold, code blocks, and inline code render properly in panes. No raw markdown artifacts.
- [ ] **AC-5: WebSocket connects.** Opening Monitor Mode connects WebSocket per visible pane. Verify in DevTools Network tab: WS connection established, messages flowing.
- [ ] **AC-6: WebSocket disconnects.** Closing a pane, hiding it, or navigating away cleanly closes the WebSocket. No orphan connections (verify in DevTools and server logs).
- [ ] **AC-7: Initial scrollback.** On connect, pane shows last 100 lines of session history before live lines begin streaming.
- [ ] **AC-8: Live streaming.** When Claude Code writes to a JSONL file, the new content appears in the corresponding pane within 100ms. Measure: write a line to the file, timestamp when it appears on screen.
- [ ] **AC-9: Double-click expand.** Double-clicking a pane opens it as a full-screen overlay. ESC closes it. Terminal re-fits to the larger size.
- [ ] **AC-10: Verbose toggle.** Per-pane toggle shows/hides tool calls, thinking, and results. Default is chat-only. Toggle persists per pane.
- [ ] **AC-11: Pane selection.** Single-click header selects pane (blue border). Click again deselects. Only one pane selected at a time.
- [ ] **AC-12: Context menu.** Right-click shows Pin/Hide/Move/Expand options. Each action works correctly.
- [ ] **AC-13: Auto-reconnect.** Kill the Rust server, restart it. WebSocket reconnects automatically within 5 seconds. Pane shows "Reconnecting..." during gap.
- [ ] **AC-14: Memory budget.** With 8 active panes streaming output, browser memory stays under 500MB (measure via Chrome DevTools Performance Monitor).
- [ ] **AC-15: Visibility optimization.** Panes scrolled out of view disconnect their WebSocket. Scrolling them back reconnects. Verify via server logs showing connect/disconnect events.
- [ ] **AC-16: Mobile swipe.** At < 640px viewport, panes are swipeable horizontally with scroll-snap. Dot indicators show current position.
- [ ] **AC-17: Keyboard shortcuts.** `1-9` selects pane, `Enter` expands, `ESC` closes, `P` pins, `H` hides, `M` toggles mode.
- [ ] **AC-18: Pin behavior.** Pinned panes are never auto-removed when idle. Pin icon visible in header. Pin state persists across reloads.
- [ ] **AC-19: No epoch-zero dates.** Any timestamp display in pane headers guards against `ts <= 0` per CLAUDE.md rule.
- [ ] **AC-20: Sub-agent placeholder.** Footer shows grayed-out "sub-agents" pill area. Non-functional until Phase D. No errors if Phase D data is absent.

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| WebGL not available on some machines | Low | Medium | Canvas fallback is automatic in xterm.js. Test on non-GPU CI. |
| `notify` watcher misses events on NFS/network drives | Low | Low | Claude Code writes to local `~/.claude/`. Not a concern for MVP. |
| Large JSONL files (>100MB) slow initial tail read | Medium | Medium | `tail_lines` seeks from EOF, reads ~200KB max. Profile with 100MB test file. |
| Browser tab crash with 8+ WebGL contexts | Low | High | Chrome supports 16+ WebGL contexts. If crash observed, fall back to canvas for panes >4. |
| Vite proxy buffers WebSocket (like it does SSE) | Very low | Medium | Vite's http-proxy handles WS upgrade correctly. Fallback: `wsUrl()` bypass in dev mode. |
| Race condition: file watcher fires before write completes | Low | Low | Read to EOF on each event. Partial last line is discarded (no terminating newline). Next event picks it up. |

---

## Testing Strategy

### Unit Tests (Rust)

Run with `cargo test -p claude-view-core -- tail` and `cargo test -p claude-view-server -- routes::terminal`.

| Test | Location |
|------|----------|
| Tail reader: edge cases (0 lines, empty file, huge file) | `crates/core/src/tail.rs` |
| File position tracker: read new, handle truncation | `crates/server/src/file_tracker.rs` |
| WebSocket handshake and message format | `crates/server/src/routes/terminal.rs` |
| Connection manager: count, limit, cleanup | `crates/server/src/terminal_state.rs` |

### Component Tests (Frontend)

Run with `bun run test:client`.

| Test | Location |
|------|----------|
| TerminalPane renders without crash | `src/components/live/TerminalPane.test.tsx` |
| MonitorGrid responsive layout | `src/components/live/MonitorGrid.test.tsx` |
| MonitorPane header displays session data | `src/components/live/MonitorPane.test.tsx` |
| Mode toggle switches between Raw/Rich | `src/components/live/MonitorPane.test.tsx` |
| Context menu actions fire callbacks | `src/components/live/PaneContextMenu.test.tsx` |
| GridControls slider updates store | `src/components/live/GridControls.test.tsx` |

### Integration / E2E Tests

Run with `bun run test:e2e`.

| Test | What it verifies |
|------|-----------------|
| Open Monitor tab, see grid of panes | Full flow: API -> WS -> xterm.js |
| Write to JSONL file, see output in pane | End-to-end latency < 100ms |
| Resize browser window, grid adapts | Responsive breakpoints |
| Expand/collapse pane | Overlay + re-fit |
| Kill server, restart, panes reconnect | Auto-reconnect |

---

## Open Questions (to resolve during implementation)

1. **Should the WebSocket endpoint require the JSONL file path or just the session ID?** If session ID, the server must resolve it to a path via the database or in-memory session registry from Phase A. If file path, the client must know it (available from Phase A's session list API). **Recommendation:** Session ID only. Server resolves path. Cleaner API, no path leakage to client.

2. **Should rich mode parse JSON on the server or client?** Server-side parsing means less bandwidth (send structured JSON, not raw JSONL lines). Client-side parsing means simpler server. **Recommendation:** Server-side for rich mode (leverage the existing `claude-view-core` parser). Raw mode sends lines verbatim.

3. **How to handle JSONL files that are being actively compacted by Claude Code?** Claude Code may rewrite session files during auto-compaction. The file watcher will see a modify event, and the position tracker may be invalid. **Recommendation:** Detect file truncation (new size < tracked position), reset position to 0, and re-send the last N lines as a fresh buffer with a `{ "type": "reset" }` event so the client clears its terminal.
