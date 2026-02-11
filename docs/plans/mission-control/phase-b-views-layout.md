---
status: pending
date: 2026-02-10
phase: B
depends_on: A
---

# Phase B: Views & Layout

> Alternative view modes, keyboard-driven navigation, command palette, mobile responsive design, and session filtering for the Mission Control dashboard.

## Prerequisites

Phase A must be complete. Phase B depends on:
- `LiveSession` type with status, project, branch, cost, turns, context usage, last active timestamp
- `GET /api/live/sessions` REST endpoint returning all active sessions
- `GET /api/live/sessions/stream` SSE endpoint for real-time updates
- Grid view with `LiveSessionCard` rendering session cards
- Summary bar showing aggregate counts
- In-memory session state managed by `SessionWatcher`

## Overview

Phase A delivers a read-only Grid view of live sessions. Phase B adds three more view modes (List, Kanban, Monitor placeholder), a view mode switcher, keyboard shortcuts, a Mission Control command palette, mobile responsive layout, and session filtering with search.

### Design System Reference

| Token | Value | Usage |
|-------|-------|-------|
| Background | `#020617` (slate-950) | Page background |
| Surface | `#0F172A` (slate-900) | Card backgrounds |
| Border | `#1E293B` (slate-800) | Card borders, dividers |
| Text primary | `#F8FAFC` (slate-50) | Headings, primary text |
| Text secondary | `#94A3B8` (slate-400) | Labels, metadata |
| Text muted | `#475569` (slate-600) | Disabled, placeholder |
| Status: working | `#22C55E` (green-500) | Active session dot + glow |
| Status: waiting | `#F59E0B` (amber-500) | Waiting for user input |
| Status: idle | `#6B7280` (gray-500) | No recent activity |
| Status: done | `#3B82F6` (blue-500) | Session completed |
| Accent | `#6366F1` (indigo-500) | Selected state, active tab |

---

## Step 1: View Mode Switcher

### File: `src/components/live/ViewModeSwitcher.tsx`

A tab bar component that switches between the four Mission Control view modes.

#### Type Definitions

```typescript
// src/types/live.ts (add to existing Phase A types)
export type LiveViewMode = 'grid' | 'list' | 'kanban' | 'monitor'

export const LIVE_VIEW_MODES: { id: LiveViewMode; label: string; icon: string; shortcut: string }[] = [
  { id: 'grid', label: 'Grid', icon: 'LayoutGrid', shortcut: '1' },
  { id: 'list', label: 'List', icon: 'List', shortcut: '2' },
  { id: 'kanban', label: 'Board', icon: 'Columns3', shortcut: '3' },
  { id: 'monitor', label: 'Monitor', icon: 'Monitor', shortcut: '4' },
]
```

#### Component Design

```typescript
interface ViewModeSwitcherProps {
  mode: LiveViewMode
  onChange: (mode: LiveViewMode) => void
}
```

- Render a horizontal tab bar with icon + label for each mode.
- Use Lucide icons: `LayoutGrid`, `List`, `Columns3`, `Monitor`.
- Active tab: `bg-indigo-500/10 text-indigo-400 border-b-2 border-indigo-500`.
- Inactive tab: `text-slate-400 hover:text-slate-200 hover:bg-slate-800/50`.
- Show keyboard shortcut hint as a small `<kbd>` badge to the right of the label (visible on `md+` breakpoints, hidden on mobile).
- Monitor tab: render normally but when selected, the view content shows a "Coming in Phase C" placeholder card.

#### Persistence

Two layers of persistence, with URL taking priority:

1. **URL param `?view=grid|list|kanban|monitor`**: Shareable, bookmarkable. Parsed on mount.
2. **localStorage key `claude-view:live-view-mode`**: Fallback when no URL param is present.

Logic in the parent `MissionControlPage`:
```
const urlView = searchParams.get('view') as LiveViewMode | null
const storedView = localStorage.getItem('claude-view:live-view-mode') as LiveViewMode | null
const initialView = urlView ?? storedView ?? 'grid'
```

When the user changes view mode:
1. Update URL param (`?view=X`) via `setSearchParams` (copy-then-modify pattern per CLAUDE.md rules).
2. Write to localStorage.

#### Integration Point

The `MissionControlPage` (from Phase A) currently renders only `LiveGridView`. After this step, it conditionally renders the active view:

```tsx
{mode === 'grid' && <LiveGridView sessions={sessions} />}
{mode === 'list' && <ListView sessions={sessions} />}
{mode === 'kanban' && <KanbanView sessions={sessions} />}
{mode === 'monitor' && <MonitorPlaceholder />}
```

#### Acceptance Criteria

- [ ] All 4 tabs render with correct icons and labels
- [ ] Active tab has visible selected state (indigo accent)
- [ ] Clicking a tab switches the view
- [ ] URL param `?view=` updates on tab click
- [ ] View mode persists to localStorage
- [ ] On page load, URL param takes priority over localStorage
- [ ] Keyboard shortcut badges visible on desktop, hidden on mobile
- [ ] No hooks after early returns

---

## Step 2: List View

### File: `src/components/live/ListView.tsx`

A sortable, compact table optimized for monitoring 20+ sessions at a glance.

#### Column Definition

| Column | Width | Content | Sortable | Default Sort |
|--------|-------|---------|----------|-------------|
| Status | 40px | Colored dot (green/amber/gray/blue) | Yes (1st) | Working > Waiting > Idle > Done |
| Project | 140px | Project display name, truncated | Yes | Alpha |
| Branch | 120px | Git branch badge (mono font), truncated | Yes | Alpha |
| Activity | flex | Last message preview, single line, truncated | No | -- |
| Turns | 60px | User prompt count | Yes | Desc |
| Cost | 70px | Estimated cost in USD (e.g. `$0.42`) | Yes | Desc |
| Context% | 65px | Context window usage bar + percentage | Yes | Desc |
| Last Active | 90px | Relative time ("2m ago", "1h ago") | Yes (2nd) | Most recent first |

#### Sorting Behavior

- Click a column header to sort by that column.
- Click again to toggle direction (asc/desc).
- Default sort: status (custom order: Working=0, Waiting=1, Idle=2, Done=3) ascending, then last active descending as tiebreaker.
- Sort state stored in URL params: `?sort=status&dir=asc` (copy-then-modify pattern).
- Use `@tanstack/react-table` (already in `package.json`) for column management and sorting, following the pattern established in `CompactSessionTable.tsx`.

#### Component Interface

```typescript
interface ListViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
}
```

#### Row Interaction

- Row click: select the session (highlight row with `bg-indigo-500/10 border-l-2 border-indigo-500`).
- Double-click or Enter on selected row: navigate to session detail (Phase A's session detail panel or route).
- Hover: `bg-slate-800/50` subtle highlight.
- Selected row tracks with `selectedId` prop managed by parent.

#### Status Dot Component

Reusable across List and Kanban views:

```typescript
// src/components/live/StatusDot.tsx
interface StatusDotProps {
  status: 'working' | 'waiting' | 'idle' | 'done'
  size?: 'sm' | 'md'  // sm=8px, md=10px
  pulse?: boolean       // animated pulse for 'working'
}
```

- `working`: green-500 dot with optional CSS pulse animation.
- `waiting`: amber-500 dot.
- `idle`: gray-500 dot.
- `done`: blue-500 dot.

#### Context Bar Component

Tiny inline progress bar showing context window usage:

```typescript
// src/components/live/ContextBar.tsx
interface ContextBarProps {
  percent: number  // 0-100
}
```

- Bar fill color: green (0-60%), amber (60-85%), red (85-100%).
- Width: 40px fixed, height: 4px.
- Percentage text to the right: `text-[10px] tabular-nums`.

#### Empty State

When no sessions match current filters, show:
- Icon: `List` from lucide-react
- Title: "No sessions to display"
- Subtitle: "Active Claude Code sessions will appear here"
- If filters are active: "Try adjusting your filters" with a "Clear filters" button

#### Acceptance Criteria

- [ ] All 8 columns render with correct data
- [ ] Clicking any sortable column header sorts the table
- [ ] Clicking sorted column toggles direction (visual arrow indicator)
- [ ] Default sort is status then last active
- [ ] Rows are clickable with visible hover and selected states
- [ ] Status dots use correct colors per session status
- [ ] Context% bar changes color at thresholds
- [ ] Table handles 0, 1, 20, 50+ sessions without layout issues
- [ ] Activity column truncates cleanly with ellipsis
- [ ] Branch column shows monospace badge matching existing `CompactSessionTable` style

---

## Step 3: Kanban View

### File: `src/components/live/KanbanView.tsx`

Sessions organized into columns by status, providing a board-style overview.

#### Column Layout

| Column | Status | Header Color | Empty State |
|--------|--------|-------------|-------------|
| Working | `working` | green-500 accent | "No active sessions" |
| Waiting | `waiting` | amber-500 accent | "No sessions waiting" |
| Idle | `idle` | gray-500 accent | "All sessions active" |
| Done | `done` | blue-500 accent | "No completed sessions" |

#### Component Interface

```typescript
interface KanbanViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
}
```

#### Column Component

```typescript
// src/components/live/KanbanColumn.tsx
interface KanbanColumnProps {
  title: string
  status: LiveSessionStatus
  sessions: LiveSession[]
  accentColor: string  // Tailwind color class
  selectedId: string | null
  onSelect: (id: string) => void
  emptyMessage: string
}
```

Each column:
- Header: status name + count badge (e.g. "Working (3)").
- Top border or left accent stripe in the status color.
- Cards: reuse `LiveSessionCard` from Phase A (the Grid view card component).
- Cards sorted within column: by last active timestamp, most recent first.
- Column width: `min-w-[280px] w-[320px]` so columns have consistent sizing.
- Column background: `bg-slate-900/50` with `rounded-lg` border.

#### Scroll Behavior

- Columns arranged in a horizontal flex container.
- On screens wider than all 4 columns (>1280px): all columns visible, evenly distributed.
- On narrower screens: horizontal scroll with `overflow-x-auto`, subtle scroll indicators.
- Each column internally scrolls vertically if it has many cards: `max-h-[calc(100vh-220px)] overflow-y-auto`.

#### No Drag-and-Drop

Sessions auto-sort by status. Status is determined by the backend file watcher, not by user arrangement. Drag-and-drop adds complexity with no benefit for read-only monitoring. Explicit non-goal.

#### Animation

When a session changes status (e.g. working -> waiting), animate the card moving between columns using CSS transitions. Implementation approach:
- Use `key={session.id}` on cards so React preserves identity.
- Add `transition-all duration-300` on the card wrapper.
- The column re-render naturally moves the card; CSS handles the fade.

#### Empty State (all columns empty)

If there are zero sessions total (not just filtered to zero):
- Full-width centered message: "No active sessions detected"
- Subtitle: "Start a Claude Code session in your terminal"

#### Acceptance Criteria

- [ ] 4 columns render with correct status headers
- [ ] Session cards appear in the correct column based on status
- [ ] Column count badges update in real-time via SSE
- [ ] Cards within each column sorted by last active (most recent first)
- [ ] Horizontal scroll works on narrower screens
- [ ] Vertical scroll works within columns with many sessions
- [ ] Empty columns show status-appropriate empty messages
- [ ] Selected card has visible selected state (matching Grid view selection style)
- [ ] Cards reuse `LiveSessionCard` from Phase A (no duplicate card component)

---

## Step 4: Keyboard Shortcuts

### File: `src/hooks/use-keyboard-shortcuts.ts`

A global keyboard shortcut handler for the Mission Control page.

#### Shortcut Map

| Key | Action | Context |
|-----|--------|---------|
| `1` | Switch to Grid view | Global |
| `2` | Switch to List view | Global |
| `3` | Switch to Kanban view | Global |
| `4` | Switch to Monitor view | Global |
| `j` | Select next session | Global |
| `k` | Select previous session | Global |
| `Enter` | Expand/navigate to selected session | When session selected |
| `r` | Resume selected session (Phase F placeholder) | When session selected |
| `Escape` | Deselect / collapse / back | Context-dependent |
| `/` | Focus search input | Global |
| `Cmd+K` | Open command palette | Global (already exists in App.tsx) |
| `g` then `m` | Go to Monitor view | Sequence shortcut |
| `g` then `k` | Go to Kanban view | Sequence shortcut |
| `g` then `l` | Go to List view | Sequence shortcut |
| `g` then `g` | Go to Grid view | Sequence shortcut |
| `?` | Show keyboard shortcut help overlay | Global |

#### Implementation Design

```typescript
interface UseKeyboardShortcutsOptions {
  // View mode
  viewMode: LiveViewMode
  onViewModeChange: (mode: LiveViewMode) => void

  // Session navigation
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string | null) => void
  onExpand: (id: string) => void

  // Search
  onFocusSearch: () => void

  // Command palette (reuse existing App.tsx Cmd+K)
  // Not needed here — already handled at App level

  // Enabled flag (disable when modal/input is focused)
  enabled: boolean
}
```

#### Guard: Skip When Input Focused

All shortcuts (except Escape) must be suppressed when the active element is an `<input>`, `<textarea>`, `<select>`, or any element with `contenteditable`. Check via:

```typescript
function isInputFocused(): boolean {
  const el = document.activeElement
  if (!el) return false
  const tag = el.tagName.toLowerCase()
  return tag === 'input' || tag === 'textarea' || tag === 'select' || el.hasAttribute('contenteditable')
}
```

#### Sequence Shortcuts (`g` then `X`)

Use a timeout-based prefix system:
1. On `g` keypress, set a `pendingPrefix = 'g'` state with a 1-second timeout.
2. If the next keypress within 1 second is `m`, `k`, `l`, or `g`, execute the corresponding navigation.
3. If timeout expires or a non-matching key is pressed, clear the prefix.

Store the prefix state in a `useRef` to avoid re-renders.

#### `j`/`k` Navigation

Navigate through the flat list of sessions:
- In Grid view: sessions are in grid order (left-to-right, top-to-bottom).
- In List view: sessions are in table row order (respecting current sort).
- In Kanban view: navigate within the focused column, or across columns if at the top/bottom of a column.
- `j` = next (down/right), `k` = previous (up/left).
- If no session is selected, `j` selects the first session.
- Ensure the selected session is scrolled into view (`element.scrollIntoView({ block: 'nearest' })`).

#### Help Overlay

`?` opens a modal listing all shortcuts. Simple overlay with a 2-column layout:
- Left column: key combo (styled as `<kbd>` elements)
- Right column: description
- Close with `Escape` or clicking outside

### File: `src/components/live/KeyboardShortcutHelp.tsx`

```typescript
interface KeyboardShortcutHelpProps {
  isOpen: boolean
  onClose: () => void
}
```

- Modal with `bg-slate-900 border border-slate-700` styling.
- Grouped sections: "Navigation", "Views", "Actions".
- `<kbd>` styling: `px-1.5 py-0.5 bg-slate-800 border border-slate-600 rounded text-[11px] font-mono text-slate-300`.

#### Acceptance Criteria

- [ ] Number keys 1-4 switch view modes
- [ ] j/k navigate between sessions with visible selection
- [ ] Enter expands/navigates to the selected session
- [ ] Escape deselects the current session
- [ ] / focuses the search input (prevents typing "/" into search)
- [ ] `g` then `m` navigates to Monitor view (within 1s window)
- [ ] `?` opens the shortcut help overlay
- [ ] ALL shortcuts suppressed when an input/textarea is focused
- [ ] Selected session scrolls into view on j/k navigation
- [ ] No shortcuts fire when command palette is open

---

## Step 5: Command Palette (Mission Control Extension)

### File: `src/components/live/LiveCommandPalette.tsx`

Extend the existing `CommandPalette` pattern (see `src/components/CommandPalette.tsx`) with Mission Control-specific actions. This is a **separate component** from the existing search-focused palette, triggered by the same `Cmd+K` shortcut but context-aware based on the current route.

#### Architecture Decision

Two options were considered:
1. **Extend existing `CommandPalette`** with conditional sections based on route.
2. **Separate `LiveCommandPalette`** for the Mission Control page.

Choice: **Option 2** — separate component. Rationale:
- The existing `CommandPalette` is search-oriented (project search, filter hints).
- Mission Control needs action-oriented commands (switch view, filter by status, resume session).
- Mixing both in one component creates a confusing UX and complex conditional rendering.
- The Mission Control page registers its own `Cmd+K` handler that takes priority over the global one (via `e.stopPropagation()` in the capture phase).

#### Action Types

```typescript
type CommandAction =
  | { type: 'switch-view'; mode: LiveViewMode }
  | { type: 'filter-status'; status: LiveSessionStatus }
  | { type: 'sort-by'; field: string }
  | { type: 'select-session'; sessionId: string }
  | { type: 'resume-session'; sessionId: string }  // Phase F placeholder
  | { type: 'clear-filters' }
  | { type: 'toggle-help' }

interface CommandItem {
  id: string
  label: string
  description?: string
  icon: LucideIcon
  action: CommandAction
  keywords: string[]  // for fuzzy matching
  shortcut?: string   // display shortcut hint
}
```

#### Command Registry

Build the command list dynamically:

1. **View modes** (always available):
   - "Switch to Grid view" / "Switch to List view" / "Switch to Board view" / "Switch to Monitor view"
   - Keywords: `['view', 'grid', 'list', 'board', 'kanban', 'monitor']`

2. **Session search** (dynamic, from current sessions):
   - Each active session as a selectable item
   - Label: `"[project] branch — last message preview"`
   - Keywords: `[project, branch, sessionId]`

3. **Filter actions** (always available):
   - "Show working sessions" / "Show waiting sessions" / "Show idle sessions"
   - "Clear all filters"
   - "Sort by last active" / "Sort by cost" / "Sort by turns"

4. **Recent actions** (persisted in Zustand store):
   - Last 5 command palette actions, shown at the top when query is empty.

#### Fuzzy Search

Use a simple substring + word-boundary matching approach (no external dependency):

```typescript
function fuzzyMatch(query: string, target: string, keywords: string[]): number {
  const q = query.toLowerCase()
  const allText = [target, ...keywords].join(' ').toLowerCase()

  // Exact substring match (highest score)
  if (allText.includes(q)) return 100

  // Word start match
  const words = q.split(/\s+/)
  const matchCount = words.filter(w => allText.includes(w)).length
  return (matchCount / words.length) * 80
}
```

Filter items with score > 0, sort by score descending.

#### Component Design

Follow the existing `CommandPalette` visual pattern:
- Fixed overlay at `pt-[12vh]` with backdrop blur.
- Input with search icon, X close button.
- Results list with keyboard navigation (arrow keys + Enter).
- Show item icon, label, description, and shortcut hint.
- Maximum 10 visible results (scrollable if more).
- Footer with keyboard navigation hints.

#### Zustand Store Extension

Add to `app-store.ts`:

```typescript
// In AppState interface:
recentLiveCommands: string[]  // last 5 command IDs
addRecentLiveCommand: (id: string) => void

// In persist partialize:
recentLiveCommands: state.recentLiveCommands,
```

#### Acceptance Criteria

- [ ] Cmd+K on Mission Control page opens `LiveCommandPalette` (not the search palette)
- [ ] Typing filters commands by fuzzy match
- [ ] Arrow keys navigate results, Enter executes
- [ ] View mode commands switch the view immediately
- [ ] Session commands select/navigate to the session
- [ ] Filter commands apply the filter
- [ ] Recent commands shown when query is empty
- [ ] Escape or backdrop click closes palette
- [ ] No flash/flicker when opening (portal rendering)

---

## Step 6: Mobile Responsive Design

### Breakpoints

| Name | Width | Layout |
|------|-------|--------|
| `xs` | <375px | Not supported (show "Use larger screen" message) |
| `sm` | 375px-767px | Mobile: single column, bottom tab bar |
| `md` | 768px-1023px | Tablet: two-column grid, sidebar collapsed |
| `lg` | 1024px-1439px | Desktop: full layout, sidebar visible |
| `xl` | 1440px+ | Wide desktop: larger grid, more columns |

### Mobile Layout Changes (sm: 375px-767px)

#### Bottom Tab Bar

**File: `src/components/live/MobileTabBar.tsx`**

```typescript
interface MobileTabBarProps {
  activeTab: 'monitor' | 'board' | 'more'
  onTabChange: (tab: 'monitor' | 'board' | 'more') => void
}
```

- Fixed to bottom of viewport: `fixed bottom-0 inset-x-0 z-40`.
- 3 tabs: Monitor (Grid view on mobile), Board (Kanban), More (List + settings).
- Each tab: icon + label, 44px minimum touch target.
- Active tab: indigo-500 icon + text color.
- Background: `bg-slate-950/95 backdrop-blur-md border-t border-slate-800`.
- Safe area padding for iOS notch: `pb-safe` (use `env(safe-area-inset-bottom)`).

The mobile tab bar **replaces** the `ViewModeSwitcher` on mobile. The desktop `ViewModeSwitcher` is hidden on `sm` breakpoint:

```
ViewModeSwitcher: hidden sm:flex
MobileTabBar: flex sm:hidden
```

#### Card Stack with Horizontal Swipe

On mobile Grid view, sessions display as a vertical card stack (single column). Horizontal swipe is reserved for Kanban column navigation, not individual card actions.

- Cards: full-width, stacked vertically with 8px gap.
- Cards show condensed info: status dot, project, branch, last active, cost.
- Tap card: open bottom sheet with full session detail.

#### Bottom Sheet

**File: `src/components/live/BottomSheet.tsx`**

```typescript
interface BottomSheetProps {
  isOpen: boolean
  onClose: () => void
  title: string
  children: React.ReactNode
}
```

- Slides up from bottom with spring animation.
- Drag handle at top (centered 32x4px rounded bar).
- Drag down to dismiss (threshold: 100px).
- Backdrop: `bg-black/50 backdrop-blur-sm`.
- Content area: `max-h-[80vh] overflow-y-auto`.
- Close on backdrop tap or drag-down.

Used for:
- Session detail (tap a card)
- Session actions (Resume, View History) - action buttons inside the bottom sheet
- Filter panel (mobile version of FilterPopover)

#### Touch Targets

All interactive elements must meet 44x44px minimum:
- Tab bar buttons: `min-h-[44px] min-w-[44px]`
- Card tap area: full card width
- Action buttons in bottom sheet: `min-h-[44px] px-4`
- Filter chips: `min-h-[44px] px-3`

#### Mobile Kanban

On mobile, Kanban columns display as horizontally swipeable pages:
- One column visible at a time (full width).
- Swipe left/right to navigate between columns.
- Dot indicators at top showing current column position.
- Column header sticky at top.
- Use CSS `scroll-snap-type: x mandatory` with `scroll-snap-align: start` for smooth snapping.

### Tablet Layout (md: 768px-1023px)

- Grid view: 2-column grid.
- List view: table with horizontal scroll for wider columns.
- Kanban: 2 columns visible, horizontal scroll for remaining.
- Sidebar hidden by default, toggle with hamburger menu.
- ViewModeSwitcher visible (not bottom tab bar).

### Desktop Layout (lg: 1024px+)

- Grid view: 3-column grid (4 columns on xl).
- List view: all columns visible without horizontal scroll.
- Kanban: all 4 columns visible side by side.
- Sidebar always visible.

### Responsive Utilities

No new CSS files. Use Tailwind responsive prefixes throughout:
```
grid grid-cols-1 sm:grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4
```

### Acceptance Criteria

- [ ] Mobile: bottom tab bar visible, ViewModeSwitcher hidden
- [ ] Desktop: ViewModeSwitcher visible, bottom tab bar hidden
- [ ] Mobile cards are full-width, single column
- [ ] Bottom sheet opens on card tap, closes on drag-down or backdrop tap
- [ ] All touch targets >= 44x44px (verify with browser dev tools)
- [ ] Mobile Kanban: swipe between columns with snap
- [ ] Tablet: 2-column grid, sidebar toggleable
- [ ] No horizontal scroll on mobile except Kanban
- [ ] Safe area insets respected on iOS
- [ ] Layout is usable at all 4 breakpoints (375/768/1024/1440)

---

## Step 7: Session Filtering & Search

### File: `src/hooks/use-live-session-filters.ts`

A dedicated filter hook for Mission Control live sessions, separate from the existing `use-session-filters.ts` (which manages historical session filters).

#### Filter Types

```typescript
export type LiveSessionStatus = 'working' | 'waiting' | 'idle' | 'done'
export type LiveSortField = 'status' | 'last_active' | 'cost' | 'turns' | 'context' | 'project'
export type LiveSortDirection = 'asc' | 'desc'

export interface LiveSessionFilters {
  // Status filter (multi-select)
  statuses: LiveSessionStatus[]

  // Project filter (multi-select, project display names)
  projects: string[]

  // Branch filter (multi-select)
  branches: string[]

  // Text search
  search: string

  // Sort
  sort: LiveSortField
  sortDir: LiveSortDirection
}

export const DEFAULT_LIVE_FILTERS: LiveSessionFilters = {
  statuses: [],
  projects: [],
  branches: [],
  search: '',
  sort: 'status',
  sortDir: 'asc',
}
```

#### URL Param Mapping

| Filter | URL Param | Example |
|--------|-----------|---------|
| statuses | `?status=working,waiting` | Multi-value comma-separated |
| projects | `?project=claude-view,api-server` | Multi-value comma-separated |
| branches | `?branch=main,feature/x` | Multi-value comma-separated |
| search | `?q=fix+bug` | URL-encoded search string |
| sort | `?sort=cost` | Single value |
| sortDir | `?dir=desc` | Single value |
| view (from Step 1) | `?view=list` | Single value |

All filters use the copy-then-modify `URLSearchParams` pattern to preserve other hooks' params (per CLAUDE.md rules).

#### Hook Interface

```typescript
export function useLiveSessionFilters(
  searchParams: URLSearchParams,
  setSearchParams: (params: URLSearchParams, opts?: { replace?: boolean }) => void
): [LiveSessionFilters, {
  setFilters: (filters: LiveSessionFilters) => void
  setStatus: (statuses: LiveSessionStatus[]) => void
  setSearch: (query: string) => void
  setSort: (field: LiveSortField) => void
  clearAll: () => void
  activeCount: number
}]
```

The `filters` object is memoized on `searchParams.toString()` (same stable-reference pattern as `use-session-filters.ts`).

#### Client-Side Filtering

Filtering is performed client-side since Phase A's in-memory state already holds all live sessions (typically <50). No backend endpoint changes needed.

```typescript
// src/lib/live-filter.ts
export function filterLiveSessions(
  sessions: LiveSession[],
  filters: LiveSessionFilters
): LiveSession[] {
  let result = sessions

  // Status filter
  if (filters.statuses.length > 0) {
    result = result.filter(s => filters.statuses.includes(s.status))
  }

  // Project filter
  if (filters.projects.length > 0) {
    result = result.filter(s => filters.projects.includes(s.project))
  }

  // Branch filter
  if (filters.branches.length > 0) {
    result = result.filter(s => filters.branches.includes(s.branch))
  }

  // Text search (fuzzy match on project name + last message)
  if (filters.search.trim()) {
    const q = filters.search.toLowerCase()
    result = result.filter(s =>
      s.project.toLowerCase().includes(q) ||
      s.branch.toLowerCase().includes(q) ||
      (s.lastMessage ?? '').toLowerCase().includes(q)
    )
  }

  // Sort
  result = sortLiveSessions(result, filters.sort, filters.sortDir)

  return result
}
```

#### Filter Pills UI

**File: `src/components/live/LiveFilterBar.tsx`**

Rendered below the summary bar and above the view content.

```typescript
interface LiveFilterBarProps {
  filters: LiveSessionFilters
  onChange: (filters: LiveSessionFilters) => void
  onClear: () => void
  activeCount: number
  // Available options derived from current sessions
  availableStatuses: LiveSessionStatus[]
  availableProjects: string[]
  availableBranches: string[]
}
```

Layout:
- Left side: search input (`/` shortcut focuses this).
- Right side: filter dropdown buttons (Status, Project, Branch) + "Clear all" button.
- Below: active filter pills showing current filters, each with an `X` to remove.

Filter pill styling:
```
px-2 py-1 text-xs rounded-full bg-indigo-500/10 text-indigo-400 border border-indigo-500/30
```

Remove button on each pill: `ml-1 hover:text-red-400`.

"Clear all" button: only visible when `activeCount > 0`. Styled as `text-xs text-red-400 hover:text-red-300`.

#### Search Input

- Rendered inline in the filter bar.
- Debounced: 200ms delay before updating URL params (avoid excessive URL updates while typing).
- `placeholder="Search sessions..."`.
- `/` keyboard shortcut focuses this input.
- Clear button (X icon) when non-empty.
- Styling matches existing app inputs: `bg-slate-900 border-slate-700 text-slate-100 placeholder-slate-500`.

#### Cross-View Consistency

Filters apply to ALL view modes. The filtering happens in the parent `MissionControlPage`, and filtered sessions are passed to whichever view is active:

```tsx
const filteredSessions = useMemo(
  () => filterLiveSessions(sessions, filters),
  [sessions, filters]
)

// Then render the active view with filteredSessions
{mode === 'grid' && <LiveGridView sessions={filteredSessions} />}
{mode === 'list' && <ListView sessions={filteredSessions} />}
{mode === 'kanban' && <KanbanView sessions={filteredSessions} />}
```

The summary bar (from Phase A) also updates to reflect filtered counts.

#### Acceptance Criteria

- [ ] Status, project, and branch filters work correctly
- [ ] Search matches against project name, branch, and last message
- [ ] Filter pills appear for each active filter
- [ ] Clicking X on a pill removes that filter
- [ ] "Clear all" resets all filters
- [ ] Filters persist in URL params (shareable/bookmarkable)
- [ ] Filters apply across all view modes (Grid, List, Kanban)
- [ ] Summary bar counts update to reflect filtered sessions
- [ ] Search input debounces at 200ms
- [ ] `/` shortcut focuses search input
- [ ] Empty state messages are contextual (filtered vs. no sessions)
- [ ] Count badges in filter buttons show active filter count
- [ ] No stale URL params when clearing filters (per FILTER_KEYS cleanup pattern)

---

## New Files Summary

| File | Type | Purpose |
|------|------|---------|
| `src/components/live/ViewModeSwitcher.tsx` | Component | Tab bar for Grid/List/Kanban/Monitor |
| `src/components/live/ListView.tsx` | Component | Sortable table view |
| `src/components/live/KanbanView.tsx` | Component | Status-column board view |
| `src/components/live/KanbanColumn.tsx` | Component | Single Kanban column |
| `src/components/live/StatusDot.tsx` | Component | Reusable status indicator dot |
| `src/components/live/ContextBar.tsx` | Component | Inline context window usage bar |
| `src/components/live/LiveCommandPalette.tsx` | Component | Mission Control command palette |
| `src/components/live/LiveFilterBar.tsx` | Component | Search + filter pills bar |
| `src/components/live/MobileTabBar.tsx` | Component | Mobile bottom navigation |
| `src/components/live/BottomSheet.tsx` | Component | Mobile bottom sheet overlay |
| `src/components/live/KeyboardShortcutHelp.tsx` | Component | Shortcut reference overlay |
| `src/components/live/MonitorPlaceholder.tsx` | Component | "Coming in Phase C" placeholder |
| `src/hooks/use-keyboard-shortcuts.ts` | Hook | Global keyboard shortcut handler |
| `src/hooks/use-live-session-filters.ts` | Hook | Live session filter state + URL sync |
| `src/lib/live-filter.ts` | Utility | Client-side filter + sort logic |
| `src/types/live.ts` | Types | Extend Phase A types with view mode types |

## Modified Files

| File | Change |
|------|--------|
| `src/store/app-store.ts` | Add `recentLiveCommands` + `liveViewMode` to persisted state |
| `src/components/live/MissionControlPage.tsx` | Wire up view switcher, filters, keyboard shortcuts, conditional view rendering |
| `src/components/live/LiveGridView.tsx` | Accept `selectedId`/`onSelect` props for keyboard navigation |
| `src/components/live/LiveSessionCard.tsx` | Accept `isSelected` prop for keyboard navigation highlight |
| `src/components/live/SummaryBar.tsx` | Accept filtered session count, display filtered vs total |

## Implementation Order

Steps should be implemented in order (1 through 7) because:

1. **Step 1 (View Switcher)** establishes the routing and view-switching infrastructure that all subsequent views plug into.
2. **Step 2 (List View)** and **Step 3 (Kanban View)** are the views themselves. They can be built in parallel once Step 1 is done.
3. **Step 4 (Keyboard Shortcuts)** depends on view switching (Step 1) and session selection (Steps 2-3) being in place.
4. **Step 5 (Command Palette)** depends on all view modes and actions being available to search over.
5. **Step 6 (Mobile)** should come after the desktop layout is stable, to avoid reworking responsive code.
6. **Step 7 (Filtering)** is listed last but can begin as early as after Step 1 — it is independent of specific view implementations. However, testing requires all views to be in place, so final integration is last.

Parallel track opportunity: Steps 2+3 can be built simultaneously by different developers (or in the same session, alternating). Steps 6+7 can also overlap once Steps 1-3 are stable.

## Estimated Effort

| Step | Effort | Notes |
|------|--------|-------|
| 1. View Switcher | 2-3 hours | Small component + URL/localStorage wiring |
| 2. List View | 4-6 hours | Table with sorting, reuse @tanstack/react-table patterns |
| 3. Kanban View | 4-6 hours | Column layout, card reuse, scroll behavior |
| 4. Keyboard Shortcuts | 3-4 hours | Hook + sequence shortcuts + help overlay |
| 5. Command Palette | 4-5 hours | Fuzzy search, action registry, store extension |
| 6. Mobile Responsive | 6-8 hours | Bottom tab bar, bottom sheet, Kanban swipe, breakpoint testing |
| 7. Filtering & Search | 4-5 hours | Filter hook, URL sync, filter bar, pill UI |
| **Total** | **27-37 hours** | ~4-5 days of focused implementation |

## Testing Strategy

### Unit Tests

- `src/lib/live-filter.test.ts`: Test `filterLiveSessions` with all filter combinations.
- `src/hooks/use-keyboard-shortcuts.test.ts`: Test shortcut dispatch, input suppression, sequence timeouts.
- `src/hooks/use-live-session-filters.test.ts`: Test URL serialization/deserialization round-trips.

### Component Tests (Vitest + Testing Library)

- `ViewModeSwitcher`: Tab click updates mode, active tab styling.
- `ListView`: Column sort, row selection, empty state.
- `KanbanView`: Sessions in correct columns, count badges.
- `LiveFilterBar`: Filter pill rendering, clear all.
- `BottomSheet`: Open/close, drag dismiss.
- `KeyboardShortcutHelp`: Renders shortcut list.

### Manual Testing Checklist

- [ ] Open Mission Control with 0, 1, 5, 20, 50+ sessions
- [ ] Switch between all 4 view modes via tabs, keyboard, and command palette
- [ ] Verify view mode persists across page reload
- [ ] Verify filters apply to all views and clear correctly
- [ ] Test j/k navigation in all view modes
- [ ] Test on mobile viewport (375px), tablet (768px), desktop (1024px+)
- [ ] Test keyboard shortcuts are suppressed in input fields
- [ ] Verify Monitor tab shows Phase C placeholder
- [ ] Test bottom sheet on mobile: open, scroll, drag-dismiss
- [ ] Verify no console errors or React warnings

### E2E Tests (Playwright)

- `e2e/mission-control-views.spec.ts`: Switch views, verify URL params, verify persistence.
- `e2e/mission-control-keyboard.spec.ts`: Keyboard navigation, shortcut help overlay.
- `e2e/mission-control-filters.spec.ts`: Apply filters, verify visible sessions, clear filters.
- `e2e/mission-control-mobile.spec.ts`: Mobile viewport, bottom tab bar, bottom sheet.
