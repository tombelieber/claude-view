---
status: pending
date: 2026-02-26
phase: E
depends_on: C
---

# Phase E: Custom Layout with dockview

> Power user feature: VS Code-style docking layout for Monitor mode using dockview. Drag-to-resize, tab groups, floating panels, layout presets, full serialize/deserialize.

**Goal:** Let users customize the Monitor mode layout by dragging pane edges to resize, dragging tabs to reposition, stacking sessions as tab groups, and popping panes out as floating panels. Provide save/load presets so users can switch between layouts instantly.

**Depends on:** Phase C (Monitor Mode) -- Custom Layout is a sub-mode within Monitor view, operating on the same session panes.

**Visual reference:** [`phase-e-mockup-v2.html`](phase-e-mockup-v2.html) -- interactive HTML mockup with real RichPane content, GitHub-dark palette, and switchable pane header styles (A/C toggle).

**Pane header design decision (2026-02-27):**

| Compact toggle | Header style | Description |
| -------------- | ------------ | ----------- |
| OFF | **C: Hybrid** | Dockview tab (status dot + name) + slim info bar (branch, cost, ctx%, turns). Action buttons (pin, maximize, float) in dockview tab bar actions. |
| ON | **A: Rich Tab** | Metrics (cost, ctx%) packed into the dockview tab itself. No info bar. Maximum content space. |

**Never Approach B** (full MonitorPane header below dockview tab) — double header wastes vertical space. The existing Compact toggle in GridControls controls the A/C switch seamlessly.

**Desktop only.** Mobile web is not a concern -- the native Expo app (apps/mobile) handles mobile. The web UI is desktop/web-centric.

---

## Background

Phase C introduces Monitor mode with a responsive CSS Grid that auto-arranges session panes based on count. This works well for casual use, but power users running 4-8 concurrent sessions want precise control over which pane is large (their "focus" session) vs. small (background tasks).

**dockview** is the state-of-the-art docking layout library in 2026:

| Property | Value |
|----------|-------|
| Package | `dockview` + `dockview-react` |
| Version | v5.0.0 (Feb 2026) |
| Size | Zero runtime dependencies |
| Stars | 3,000+ GitHub |
| Adoptors | VS Code-style layouts in production apps |
| Key features | Tab groups, floating panels, popout windows, drag-to-resize, full `toJSON()`/`fromJSON()` serialization |

**Why dockview over react-mosaic (original spec):**
- react-mosaic: binary tree only, no floating panels, no tab groups (only stacking), slowed maintenance
- dockview: VS Code-level UX out of the box, active development (v5.0.0 shipped 9 days ago), zero deps, full serialization API

This phase adds a toggle between "Auto Grid" (Phase C default) and "Custom Layout" (dockview). The toggle lives inside Monitor mode -- it does not affect Grid, List, or Kanban views.

---

## Dependencies to Add

### npm

**File to modify:** `apps/web/package.json`

```json
{
  "dependencies": {
    "dockview": "^5.0.0",
    "dockview-react": "^5.0.0"
  }
}
```

Install:
```bash
cd apps/web && bun add dockview dockview-react
```

### CSS

dockview ships its own CSS for panels, tabs, split bars, and drag handles. Import it in the app's global CSS:

**File to modify:** `apps/web/src/index.css`

Add both imports **immediately after** `@import "tailwindcss"` and **before** `@plugin "@tailwindcss/typography"` (CSS spec requires all `@import` rules to precede non-`@import`/non-`@charset` rules — imports placed after `@plugin` are silently ignored by browsers):

```css
@import 'dockview-react/dist/styles/dockview.css';
@import './styles/dockview-dark.css';
```

Override dockview's default theme with our dark palette. The color values below use the same GitHub-style hex palette as the existing `MonitorPane.tsx` and `MonitorGrid.tsx` components (`#0D1117`, `#161B22`, `#21262D`, `#30363D`, etc.) plus the `status.active` green from `packages/design-tokens`:

**File to create:** `apps/web/src/styles/dockview-dark.css`

```css
/* Override dockview defaults for our dark theme.
   Colors match the existing GitHub-style palette used in MonitorPane.tsx
   and design-tokens/src/colors.ts (status.active = #22c55e). */
.dockview-theme-dark {
  /* Background layers — matches MonitorPane dark:bg-[#161B22] and dark:bg-[#0D1117] */
  --dv-paneview-header-border-color: #30363D;
  --dv-tabs-and-actions-container-background-color: #161B22;
  --dv-activegroup-visiblepanel-tab-background-color: #0D1117;
  --dv-activegroup-hiddenpanel-tab-background-color: #161B22;
  --dv-inactivegroup-visiblepanel-tab-background-color: #0D1117;
  --dv-inactivegroup-hiddenpanel-tab-background-color: #161B22;
  --dv-tab-divider-color: #30363D;
  --dv-activegroup-visiblepanel-tab-color: #F0F6FC;
  --dv-activegroup-hiddenpanel-tab-color: #8B949E;
  --dv-inactivegroup-visiblepanel-tab-color: #C9D1D9;
  --dv-inactivegroup-hiddenpanel-tab-color: #8B949E;

  /* Active tab underline — green accent matching design-tokens status.active */
  --dv-active-tab-border-bottom-color: #22c55e;

  /* Panel body — matches MonitorPane dark:bg-[#0D1117] */
  --dv-group-view-background-color: #0D1117;

  /* Separator (split bar) — matches dark:border-[#21262D] */
  --dv-separator-border: #21262D;
  --dv-separator-handle-color: #30363D;

  /* Drag preview — green accent at low opacity */
  --dv-drag-over-background-color: rgba(34, 197, 94, 0.06);
  --dv-drag-over-border-color: #22c55e;

  /* Floating / popout */
  --dv-floating-box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
}

/* Custom split bar hover */
.dockview-theme-dark .dv-split-view-container > .dv-sash:hover {
  background: #30363D;
}

.dockview-theme-dark .dv-split-view-container > .dv-sash:active {
  background: #22c55e;
}
```

---

## Implementation

### Layout Mode State

**File to create:** `apps/web/src/hooks/use-layout-mode.ts`

```tsx
import { useCallback, useState } from 'react'
import type { SerializedDockview } from 'dockview-react'

export type LayoutMode = 'auto-grid' | 'custom'

interface UseLayoutModeResult {
  mode: LayoutMode
  setMode: (mode: LayoutMode) => void
  toggleMode: () => void

  /** dockview serialized layout. null when mode is 'auto-grid' or no layout saved. */
  savedLayout: SerializedDockview | null
  setSavedLayout: (layout: SerializedDockview | null) => void

  /** Active preset name, or null if layout has been manually modified. */
  activePreset: string | null
  setActivePreset: (name: string | null) => void
}

const LAYOUT_STORAGE_KEY = 'claude-view:monitor-layout'
const MODE_STORAGE_KEY = 'claude-view:monitor-layout-mode'

export function useLayoutMode(): UseLayoutModeResult {
  const [mode, setModeState] = useState<LayoutMode>(() => {
    const stored = localStorage.getItem(MODE_STORAGE_KEY)
    return stored === 'custom' ? 'custom' : 'auto-grid'
  })

  const [savedLayout, setSavedLayoutState] = useState<SerializedDockview | null>(() => {
    const stored = localStorage.getItem(LAYOUT_STORAGE_KEY)
    if (stored) {
      try { return JSON.parse(stored) } catch { return null }
    }
    return null
  })

  const [activePreset, setActivePreset] = useState<string | null>(null)

  const setMode = useCallback((newMode: LayoutMode) => {
    setModeState(newMode)
    try { localStorage.setItem(MODE_STORAGE_KEY, newMode) } catch { /* QuotaExceeded — state still works in-memory */ }
  }, [])

  const toggleMode = useCallback(() => {
    setMode(mode === 'auto-grid' ? 'custom' : 'auto-grid')
  }, [mode, setMode])

  const setSavedLayout = useCallback((layout: SerializedDockview | null) => {
    setSavedLayoutState(layout)
    setActivePreset(null) // manual change invalidates preset
    try {
      if (layout) {
        localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(layout))
      } else {
        localStorage.removeItem(LAYOUT_STORAGE_KEY)
      }
    } catch { /* QuotaExceeded — layout persisted in-memory for this session */ }
  }, [])

  return { mode, setMode, toggleMode, savedLayout, setSavedLayout, activePreset, setActivePreset }
}
```

### Custom Layout Component (dockview)

**File to create:** `apps/web/src/components/live/DockLayout.tsx`

This is the core component. It wraps dockview-react's `DockviewReact` and renders session panes inside dockview panels.

```tsx
import { useCallback, useEffect, useRef } from 'react'
import {
  DockviewReact,
  type DockviewReadyEvent,
  type DockviewApi,
  type IDockviewPanelProps,
  type SerializedDockview,
} from 'dockview-react'
import type { LiveSession } from './use-live-sessions'
import { RichTerminalPane } from './RichTerminalPane'

interface DockLayoutProps {
  sessions: LiveSession[]
  /** Restore from this layout on mount (from localStorage or preset). */
  initialLayout: SerializedDockview | null
  /** Called whenever the layout changes structurally (resize, move, tab reorder). */
  onLayoutChange: (layout: SerializedDockview) => void
  /** Called once when the dockview API is ready — use to capture the API ref in the parent. */
  onApiReady?: (api: DockviewApi) => void
  compactHeaders: boolean
  verboseMode: boolean
  onSelectSession?: (id: string) => void
}

/**
 * Panel component rendered inside each dockview panel.
 *
 * Params shape: { sessionId: string; verboseMode: boolean }
 * Matches RichTerminalPane props: sessionId, isVisible, verboseMode.
 */
function SessionPanel({ api, containerApi: _containerApi, params }: IDockviewPanelProps<{ sessionId: string; verboseMode: boolean; status: string }>) {
  const sessionId = params.sessionId
  if (!sessionId) return <div className="flex-1 bg-[#0D1117] p-4 text-[#8B949E]">Session ended</div>

  return (
    <div className="flex flex-col h-full bg-[#0D1117]">
      <RichTerminalPane
        sessionId={sessionId}
        isVisible={true}
        verboseMode={params.verboseMode}
      />
    </div>
  )
}

// Component registry + watermark — defined outside the component to avoid
// re-creating on every render (React reconciler uses referential equality).
const components = { session: SessionPanel }

function EmptyWatermark() {
  return (
    <div className="flex items-center justify-center h-full text-[#8B949E] text-sm">
      No sessions. Start a Claude Code session to see it here.
    </div>
  )
}

export function DockLayout({
  sessions,
  initialLayout,
  onLayoutChange,
  onApiReady,
  compactHeaders,
  verboseMode,
  onSelectSession,
}: DockLayoutProps) {
  const apiRef = useRef<DockviewApi | null>(null)
  const sessionsRef = useRef(sessions)
  sessionsRef.current = sessions
  const verboseModeRef = useRef(verboseMode)
  verboseModeRef.current = verboseMode
  const onLayoutChangeRef = useRef(onLayoutChange)
  onLayoutChangeRef.current = onLayoutChange

  // onReady fires ONCE when dockview mounts. All mutable values (sessions,
  // verboseMode) are read via refs so the callback identity is stable and
  // dockview never re-initializes on SSE ticks.
  const onReady = useCallback((event: DockviewReadyEvent) => {
    apiRef.current = event.api
    onApiReady?.(event.api)

    const currentSessions = sessionsRef.current
    const currentVerbose = verboseModeRef.current

    if (initialLayout) {
      // Restore saved layout
      event.api.fromJSON(initialLayout)
      // Update panel params with current verboseMode
      for (const panel of event.api.panels) {
        const session = currentSessions.find((s) => s.id === panel.id)
        if (session) {
          panel.api.updateParameters({ sessionId: session.id, verboseMode: currentVerbose, status: session.status })
        }
      }
    } else {
      // Build initial layout from current sessions
      const ids = currentSessions.map((s) => s.id)
      for (const [i, id] of ids.entries()) {
        const session = currentSessions.find((s) => s.id === id)
        event.api.addPanel({
          id,
          component: 'session',
          title: session?.projectDisplayName ?? id.slice(0, 8),
          params: { sessionId: id, verboseMode: currentVerbose, status: session?.status ?? 'done' },
          // First panel gets its own group, rest stack or split
          position: i === 0 ? undefined : { referencePanel: ids[0], direction: 'right' },
        })
      }
    }

    // Listen for structural layout changes (add/remove/move panels, resize)
    // and persist. Debounce avoids N localStorage.setItem calls during bulk
    // mutations (e.g. preset application that adds 4 panels in quick succession).
    let debounceTimer: ReturnType<typeof setTimeout> | null = null
    const persistLayout = () => {
      if (debounceTimer) clearTimeout(debounceTimer)
      debounceTimer = setTimeout(() => {
        if (apiRef.current) {
          onLayoutChangeRef.current(apiRef.current.toJSON())
        }
      }, 100)
    }
    event.api.onDidAddPanel(persistLayout)
    event.api.onDidRemovePanel(persistLayout)
    event.api.onDidLayoutChange(persistLayout)
  // eslint-disable-next-line react-hooks/exhaustive-deps -- refs are stable; initialLayout is the only true dep
  }, [initialLayout, onApiReady])

  // Sync session data into existing panels when sessions update
  useEffect(() => {
    const api = apiRef.current
    if (!api) return

    // Update existing panels with fresh verboseMode
    for (const panel of api.panels) {
      const session = sessions.find((s) => s.id === panel.id)
      if (session) {
        panel.api.updateParameters({ sessionId: session.id, verboseMode, status: session.status })
      }
    }

    // Add panels for new sessions
    const existingIds = new Set(api.panels.map((p) => p.id))
    for (const session of sessions) {
      if (!existingIds.has(session.id)) {
        api.addPanel({
          id: session.id,
          component: 'session',
          title: session.projectDisplayName ?? session.id.slice(0, 8),
          params: { sessionId: session.id, verboseMode, status: session.status },
        })
      }
    }

    // Remove panels for ended sessions.
    // IMPORTANT: Snapshot the array first — calling removePanel() mutates
    // api.panels in place, which causes iterator invalidation if we iterate
    // the live array directly (same pattern as Array.prototype.filter-then-forEach).
    const currentIds = new Set(sessions.map((s) => s.id))
    const panelsToRemove = api.panels.filter((p) => !currentIds.has(p.id))
    for (const panel of panelsToRemove) {
      api.removePanel(panel)
    }
  }, [sessions, verboseMode])

  return (
    <DockviewReact
      className="dockview-theme-dark"
      components={components}
      tabComponents={{ session: SessionTabRenderer }}
      defaultTabComponent="session"
      onReady={onReady}
      watermarkComponent={EmptyWatermark}
    />
  )
}
```

**Key design decisions:**
- Each panel's `id` is the session ID (stable across page reloads)
- Session data is passed via `params` and updated on each SSE tick
- `toJSON()` captures the full layout tree; `fromJSON()` restores it
- New sessions get auto-added; ended sessions get auto-removed
- Watermark component shown when no panels exist

### Layout Presets

**File to create:** `apps/web/src/components/live/LayoutPresets.tsx`

Dropdown for saving, loading, and selecting layout presets.

**Built-in presets:**

| Name | Layout | Use case |
|------|--------|----------|
| "2x2" | 2 columns, 2 rows, equal size | 4 sessions side by side |
| "3+1" | 3 small panes left, 1 large pane right (30/70 split) | Focus on one session, monitor others |
| "Focus" | Single pane fills entire area | Deep dive into one session |

**Props:**
```tsx
interface LayoutPresetsProps {
  /** Current sessions to populate the layout (needs id + projectDisplayName for titles) */
  sessions: LiveSession[]
  /** Dockview API ref to apply presets programmatically */
  dockviewApi: DockviewApi | null
  /** Current active preset name (null if manually modified) */
  activePreset: string | null
  /** Called when user selects a preset */
  onSelectPreset: (presetName: string) => void
  /** Called when user saves current layout as a named preset */
  onSavePreset: (name: string) => void
  /** Called when user deletes a saved preset */
  onDeletePreset: (name: string) => void
}
```

**Preset application strategy:**

Built-in presets programmatically construct a layout via the dockview API:

```tsx
/** Maximum number of positioned slots per built-in preset. */
const PRESET_CAPACITY: Record<string, number> = { '2x2': 4, '3+1': 4, focus: Infinity }

/** Resolve a display title for a session ID.
 *  Uses projectDisplayName from the LiveSession, falls back to first 8 chars of ID. */
function sessionTitle(id: string, sessions: LiveSession[]): string {
  const session = sessions.find((s) => s.id === id)
  return session?.projectDisplayName ?? id.slice(0, 8)
}

function applyPreset(api: DockviewApi, presetName: string, sessions: LiveSession[], verboseMode: boolean) {
  if (sessions.length === 0) return
  const sessionIds = sessions.map((s) => s.id)
  /** Build panel params — required by SessionPanel for rendering. */
  const panelParams = (id: string) => {
    const s = sessions.find((sess) => sess.id === id)
    return { sessionId: id, verboseMode, status: s?.status ?? 'done' }
  }

  // Clear existing layout
  api.clear()

  const capacity = PRESET_CAPACITY[presetName] ?? 4
  const positioned = sessionIds.slice(0, capacity)
  const overflow = sessionIds.slice(capacity)

  switch (presetName) {
    case '2x2': {
      // Grid: sessions[0] top-left, [1] top-right, [2] bottom-left, [3] bottom-right
      for (const [i, id] of positioned.entries()) {
        api.addPanel({
          id,
          component: 'session',
          title: sessionTitle(id, sessions),
          params: panelParams(id),
          position: i === 0 ? undefined :
            i === 1 ? { referencePanel: positioned[0], direction: 'right' } :
            i === 2 ? { referencePanel: positioned[0], direction: 'below' } :
            { referencePanel: positioned[1], direction: 'below' },
        })
      }
      break
    }
    case '3+1': {
      // Left column: 3 stacked, Right: 1 large
      // Right (large) panel first
      api.addPanel({ id: positioned[0], component: 'session', title: sessionTitle(positioned[0], sessions), params: panelParams(positioned[0]) })
      // Left stack
      for (let i = 1; i < positioned.length; i++) {
        api.addPanel({
          id: positioned[i],
          component: 'session',
          title: sessionTitle(positioned[i], sessions),
          params: panelParams(positioned[i]),
          position: i === 1
            ? { referencePanel: positioned[0], direction: 'left' }
            : { referencePanel: positioned[i - 1], direction: 'below' },
        })
      }
      break
    }
    case 'focus': {
      // Single panel, first session — all remaining as tabs in same group
      api.addPanel({ id: sessionIds[0], component: 'session', title: sessionTitle(sessionIds[0], sessions), params: panelParams(sessionIds[0]) })
      for (let i = 1; i < sessionIds.length; i++) {
        api.addPanel({
          id: sessionIds[i],
          component: 'session',
          title: sessionTitle(sessionIds[i], sessions),
          params: panelParams(sessionIds[i]),
          position: { referencePanel: sessionIds[0] }, // same group = tabs
        })
      }
      break
    }
  }

  // Overflow: extra sessions beyond preset capacity are added as tabs
  // in the last positioned group, so no sessions are silently dropped.
  if (overflow.length > 0 && positioned.length > 0) {
    const lastPositionedId = positioned[positioned.length - 1]
    for (const id of overflow) {
      api.addPanel({
        id,
        component: 'session',
        title: sessionTitle(id, sessions),
        params: panelParams(id),
        position: { referencePanel: lastPositionedId }, // same group = tab
      })
    }
  }
}
```

**Saved presets storage:**

Custom presets are stored in localStorage under key `claude-view:monitor-presets`. They store the full `SerializedDockview` JSON from `api.toJSON()`:

```json
{
  "My Dev Setup": { "grid": { ... }, "panels": { ... }, "activeGroup": "..." },
  "Review Mode": { "grid": { ... }, "panels": { ... }, "activeGroup": "..." }
}
```

**Behavior:**
- Dropdown shows built-in presets first (separator), then user-saved presets
- Selecting a preset applies it immediately and highlights it as active
- "Save Current Layout" option opens a small inline text input for naming
- User presets have a delete button (built-in presets do not)
- Built-in presets construct layouts programmatically (work with any session count)
- User-saved presets store the full dockview JSON and restore via `fromJSON()`

**Session count vs preset mismatch:** Built-in presets adapt to any session count. The `applyPreset()` function splits sessions into `positioned` (up to preset capacity) and `overflow` (the rest). Overflow sessions are added as tabs in the last positioned group via `position: { referencePanel: lastPositionedId }`. If fewer sessions than preset slots, the remaining slots are simply skipped. The "Focus" preset has infinite capacity (all sessions become tabs).

### Layout Presets Hook

**File to create:** `apps/web/src/hooks/use-layout-presets.ts`

```tsx
import { useState, useCallback } from 'react'
import type { SerializedDockview } from 'dockview-react'

const PRESETS_STORAGE_KEY = 'claude-view:monitor-presets'

export function useLayoutPresets() {
  const [customPresets, setCustomPresets] = useState<Record<string, SerializedDockview>>(() => {
    const stored = localStorage.getItem(PRESETS_STORAGE_KEY)
    if (stored) {
      try { return JSON.parse(stored) } catch { return {} }
    }
    return {}
  })

  const savePreset = useCallback((name: string, layout: SerializedDockview) => {
    setCustomPresets(prev => {
      const next = { ...prev, [name]: layout }
      try { localStorage.setItem(PRESETS_STORAGE_KEY, JSON.stringify(next)) } catch { /* QuotaExceeded */ }
      return next
    })
  }, [])

  const deletePreset = useCallback((name: string) => {
    setCustomPresets(prev => {
      const next = { ...prev }
      delete next[name]
      try { localStorage.setItem(PRESETS_STORAGE_KEY, JSON.stringify(next)) } catch { /* QuotaExceeded */ }
      return next
    })
  }, [])

  return { customPresets, savePreset, deletePreset }
}
```

### Monitor View Integration

**File to modify:** `apps/web/src/components/live/MonitorView.tsx`

Add a layout mode toggle to the Monitor view toolbar. The existing `MonitorGrid` renders when mode is `auto-grid`; the new `DockLayout` renders when mode is `custom`.

```tsx
// New imports (add to existing import block).
// NOTE: `useRef` is already imported in MonitorView.tsx — do NOT re-import it.
import type { DockviewApi } from 'dockview-react'
import { DockLayout } from './DockLayout'
import { LayoutModeToggle } from './LayoutModeToggle'
import { LayoutPresets } from './LayoutPresets'
import { useLayoutMode } from '../../hooks/use-layout-mode'
import { useLayoutPresets } from '../../hooks/use-layout-presets'

// Inside MonitorView component, after existing store state hooks:
const { mode, setMode, toggleMode, savedLayout, setSavedLayout, activePreset, setActivePreset } = useLayoutMode()
const { customPresets, savePreset, deletePreset } = useLayoutPresets()
const dockviewApiRef = useRef<DockviewApi | null>(null)

// Handlers for preset operations
const handleSelectPreset = useCallback((presetName: string) => {
  setActivePreset(presetName)
}, [setActivePreset])

const handleSavePreset = useCallback((name: string) => {
  if (dockviewApiRef.current) {
    savePreset(name, dockviewApiRef.current.toJSON())
  }
}, [savePreset])

const handleDeletePreset = useCallback((name: string) => {
  deletePreset(name)
}, [deletePreset])

const handleResetLayout = useCallback(() => {
  setSavedLayout(null)
  setMode('auto-grid') // Explicit target — toggleMode() is fragile (depends on current state)
}, [setSavedLayout, setMode])

// In the existing return JSX, replace the grid content area with:
return (
  <div className="flex flex-col h-full gap-2">
    {/* Grid controls toolbar — existing Phase C (shown in both modes) */}
    <GridControls
      gridOverride={gridOverride}
      compactHeaders={compactHeaders}
      verboseMode={verboseMode}
      sessionCount={sessions.length}
      visibleCount={visibleSessions.length}
      onGridOverrideChange={setGridOverride}
      onCompactHeadersChange={setCompactHeaders}
      onVerboseModeChange={toggleVerbose}
    />

    {/* Layout mode toggle + preset controls (NEW Phase E) */}
    <div className="flex items-center gap-3 px-3">
      <LayoutModeToggle mode={mode} onToggle={toggleMode} />

      {mode === 'custom' && (
        <>
          <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
          <LayoutPresets
            sessions={visibleSessions}
            dockviewApi={dockviewApiRef.current}
            activePreset={activePreset}
            onSelectPreset={handleSelectPreset}
            onSavePreset={handleSavePreset}
            onDeletePreset={handleDeletePreset}
          />
          <button
            onClick={handleResetLayout}
            className="text-xs text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 px-2 py-1 rounded border border-transparent hover:border-gray-300 dark:hover:border-gray-700 transition-colors"
          >
            Reset
          </button>
        </>
      )}
    </div>

    {/* Content area */}
    <div className="flex-1 min-h-0">
      {mode === 'auto-grid' ? (
        <MonitorGrid sessions={visibleSessions} gridOverride={gridOverride} compactHeaders={compactHeaders} onVisibilityChange={setVisiblePanes}>
          {/* Existing Phase C pane rendering (unchanged) */}
        </MonitorGrid>
      ) : (
        <DockLayout
          sessions={visibleSessions}
          initialLayout={savedLayout}
          onLayoutChange={setSavedLayout}
          onApiReady={(api) => { dockviewApiRef.current = api }}
          compactHeaders={compactHeaders}
          verboseMode={verboseMode}
          onSelectSession={onSelectSession}
        />
      )}
    </div>
  </div>
)

// Update the existing useMonitorKeyboardShortcuts call to pass new Phase E options:
useMonitorKeyboardShortcuts({
  enabled: true,
  sessions: visibleSessions,
  onLayoutModeChange: setMode,        // NEW Phase E
  layoutMode: mode,                    // NEW Phase E
  dockviewApi: dockviewApiRef.current, // NEW Phase E
})
```

### Layout Mode Toggle

**File to create:** `apps/web/src/components/live/LayoutModeToggle.tsx`

A segmented control with two options:

```
[ Auto | Custom ]
```

**Props:**
```tsx
interface LayoutModeToggleProps {
  mode: 'auto-grid' | 'custom'
  onToggle: () => void
}
```

**Styling** (matches `ViewModeSwitcher.tsx` pattern from Phase B):
- Segmented button: `rounded-lg bg-gray-100/50 dark:bg-gray-900/50 border border-gray-200 dark:border-gray-800`
- Active segment: `bg-indigo-500/10 text-indigo-400 border-b-2 border-indigo-500`
- Inactive segment: `text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200`
- Icons: grid icon for Auto, layout icon for Custom (matching mockup SVGs)
- Tooltip: "Auto: responsive grid" / "Custom: drag to arrange"

### Keyboard Shortcuts

**File to modify:** `apps/web/src/components/live/useMonitorKeyboardShortcuts.ts`

Integrate with the existing keyboard shortcut system. **Critical:** The existing handler at line 65 has `if (e.ctrlKey || e.altKey || e.metaKey) return` which rejects ALL modifier combos. The new `Ctrl+Shift` shortcuts must be handled **before** this guard.

**New import required** at the top of this file:
```ts
import type { DockviewApi } from 'dockview-react'
```

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+G` | Switch to Auto Grid |
| `Ctrl+Shift+L` | Switch to Custom Layout |
| `1`-`9` (in Custom Layout) | Focus pane N (activates the Nth panel's group) |

**Required code change** — insert this block **before** the existing `if (e.ctrlKey || e.altKey || e.metaKey) return` guard:

```ts
// Ctrl+Shift combos for layout mode switching (must come BEFORE the modifier guard)
if (e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey) {
  if (e.key === 'G' || e.key === 'g') {
    // Ctrl+Shift+G → Auto Grid
    opts.onLayoutModeChange?.('auto-grid')
    e.preventDefault()
    return
  }
  if (e.key === 'L' || e.key === 'l') {
    // Ctrl+Shift+L → Custom Layout
    opts.onLayoutModeChange?.('custom')
    e.preventDefault()
    return
  }
}
```

The hook's options interface needs three new optional fields. The `1`-`9` shortcut also needs **mode-aware logic** — in Auto Grid mode the existing handler calls `store.selectPane(id)`, but in Custom Layout mode it must call `dockviewApi.getPanel(id)?.focus()` to activate the panel within dockview. Here is the **complete** updated interface and handler:

```ts
interface UseMonitorKeyboardShortcutsOptions {
  enabled: boolean
  sessions: LiveSession[]
  onLayoutModeChange?: (mode: 'auto-grid' | 'custom') => void  // NEW Phase E
  layoutMode?: 'auto-grid' | 'custom'  // NEW Phase E
  dockviewApi?: DockviewApi | null      // NEW Phase E
}

// In the 1-9 handler (existing lines 70-78), wrap the existing selectPane call:
const idx = Number(e.key) - 1
if (idx >= 0 && idx < opts.sessions.length) {
  const session = opts.sessions[idx]
  if (opts.layoutMode === 'custom' && opts.dockviewApi) {
    // Focus the panel inside dockview
    const panel = opts.dockviewApi.getPanel(session.id)
    if (panel) {
      panel.focus()
    }
  } else {
    // Auto Grid: existing behavior
    store.selectPane(session.id)
  }
  e.preventDefault()
}
```

---

## Session Add/Remove Handling

dockview's API handles dynamic panel add/remove cleanly:

### New Session Appears
- Call `api.addPanel()` with a new panel ID
- Panel is added to the layout (defaults to a new group at the right edge)
- User can drag it to their preferred position

### Session Ends
- Call `api.removePanel(panel)` for the ended session
- dockview automatically collapses the group if it was the last panel
- Adjacent panels expand to fill the space

### Session ID Stability
- Session IDs from Phase A are stable (based on JSONL file path hash)
- dockview panel IDs are session IDs (strings)
- Layout tree survives page reload as long as the same sessions are still active
- On restore via `fromJSON()`, panels whose session IDs no longer exist are skipped

---

## Floating Panels

dockview supports floating panels out of the box. Users can:

1. **Pop out a panel:** Right-click tab > "Float" or click the float icon in the tab bar actions
2. **Drag floating panel:** Grab the tab bar of the floating panel
3. **Resize floating panel:** Drag the resize handle (bottom-right corner)
4. **Dock back:** Drag the floating panel's tab onto the main layout area

Floating panels are included in `toJSON()` serialization, so they persist across page reloads.

**Custom float button in tab actions:** Add a "Float" icon button alongside the existing "Maximize" button in each panel's tab bar. Implementation uses dockview's `api.addFloatingGroup()`:

```tsx
// In the tab actions renderer
function handleFloat(panelId: string) {
  const panel = dockviewApi.getPanel(panelId)
  if (panel) {
    dockviewApi.addFloatingGroup(panel.group, {
      width: 400,
      height: 300,
      position: { right: 24, bottom: 24 },
    })
  }
}
```

---

## Tab Group Features

dockview supports tab groups natively:

- **Create tab group:** Drag a panel tab onto another panel's tab bar area
- **Reorder tabs:** Drag tabs within a tab bar
- **Move tab to new group:** Drag a tab out of the tab bar into a drop zone
- **Tab styling:** Active tab gets green underline (`--dv-active-tab-border-bottom-color: #22c55e`)
- **Status dot in tab:** Custom tab renderer shows status dot next to the session name. Colors map `LiveSession.status` to `design-tokens/colors.ts status` values: `working` → green (`#22c55e` = `status.active`), `paused` → amber (`#f59e0b` = `status.waiting`), `done` → gray (`#6b7280` = `status.done`).

**Custom tab renderer:**

```tsx
import { type IDockviewPanelHeaderProps } from 'dockview-react'
import type { LiveSession } from './use-live-sessions'

/**
 * Maps LiveSession.status to design-token status colors.
 * Values from packages/design-tokens/src/colors.ts → status.
 */
function statusToColor(status: LiveSession['status']): string {
  switch (status) {
    case 'working': return '#22c55e'  // status.active
    case 'paused':  return '#f59e0b'  // status.waiting
    case 'done':    return '#6b7280'  // status.done
    default:        return '#6b7280'
  }
}

function SessionTabRenderer({ api, containerApi, params }: IDockviewPanelHeaderProps) {
  const status = params.status as LiveSession['status'] | undefined
  const statusColor = status ? statusToColor(status) : '#6b7280'

  return (
    <div className="flex items-center gap-1.5 px-3 h-full text-xs">
      <div
        className="w-1.5 h-1.5 rounded-full flex-shrink-0"
        style={{ backgroundColor: statusColor }}
      />
      <span className="truncate">{api.title}</span>
    </div>
  )
}
```

**Note:** Panel params must include `status` when calling `updateParameters()`. Update the session sync `useEffect` in `DockLayout.tsx` to pass `status`:

```tsx
panel.api.updateParameters({ sessionId: session.id, verboseMode, status: session.status })
```

Register the custom tab renderer in `DockviewReact` (update the return in `DockLayout.tsx`):

```tsx
<DockviewReact
  className="dockview-theme-dark"
  components={components}
  tabComponents={{ session: SessionTabRenderer }}
  defaultTabComponent="session"
  onReady={onReady}
  watermarkComponent={EmptyWatermark}
/>
```

**Note on dockview v5 theming:** dockview v5 added a `theme` prop as an alternative to `className`. The `className="dockview-theme-dark"` approach with CSS custom properties still works in v5 and is the more established pattern. If a future dockview update deprecates `className` theming, migrate to the `theme` prop object with the same color values.

---

## Error & Fallback States

| Scenario | Behavior |
|----------|----------|
| `fromJSON()` fails (corrupt localStorage) | Log warning, fall back to auto-build from current sessions |
| Saved layout references sessions that no longer exist | Skip those panels, keep valid ones |
| Saved layout has fewer panels than current sessions | Add new sessions as panels in default positions |
| Zero sessions in Custom Layout mode | Show watermark ("No sessions. Start a Claude Code session to see it here.") |
| dockview import fails (CDN/bundle issue) | Fall back to Auto Grid mode, log error |

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `apps/web/src/components/live/DockLayout.tsx` | dockview wrapper with session pane rendering |
| `apps/web/src/components/live/LayoutPresets.tsx` | Preset dropdown with save/load/delete |
| `apps/web/src/components/live/LayoutModeToggle.tsx` | Auto Grid / Custom Layout segmented control |
| `apps/web/src/hooks/use-layout-mode.ts` | Layout mode + dockview state with localStorage persistence |
| `apps/web/src/hooks/use-layout-presets.ts` | Preset save/load with localStorage |
| `apps/web/src/styles/dockview-dark.css` | Dark theme overrides for dockview |
| `apps/web/src/components/live/DockLayout.test.tsx` | Tests for custom layout |
| `apps/web/src/components/live/LayoutPresets.test.tsx` | Tests for presets |
| `apps/web/src/hooks/use-layout-mode.test.ts` | Tests for layout mode hook |
| `apps/web/src/hooks/use-layout-presets.test.ts` | Tests for presets hook |

### Modified Files

| File | Change |
|------|--------|
| `apps/web/package.json` | Add `dockview`, `dockview-react` dependencies |
| `bun.lock` | Regenerated after `bun add` |
| `apps/web/src/index.css` | Import dockview base CSS + dark theme override |
| `apps/web/src/components/live/MonitorView.tsx` | Add layout mode toggle + conditional rendering |
| `apps/web/src/components/live/useMonitorKeyboardShortcuts.ts` | Add `Ctrl+Shift+G`, `Ctrl+Shift+L`, `1`-`9` bindings |

### Dependencies Added

| Package | Version | Size | Why |
|---------|---------|------|-----|
| `dockview` | ^5.0.0 | Zero deps, ~40KB gzip | Core docking layout engine |
| `dockview-react` | ^5.0.0 | React bindings | React wrapper components |

No new Rust dependencies. This phase is frontend-only.

---

## Testing Strategy

### Unit Tests

1. **`useLayoutMode` hook:**
   - Defaults to `'auto-grid'` when localStorage is empty
   - Restores `'custom'` from localStorage
   - `toggleMode` switches between modes
   - `setSavedLayout` persists `SerializedDockview` to localStorage
   - `setSavedLayout(null)` removes from localStorage
   - `setSavedLayout` clears `activePreset`

2. **`useLayoutPresets` hook:**
   - Loads custom presets from localStorage on mount
   - `savePreset` persists to localStorage
   - `deletePreset` removes from localStorage

3. **`DockLayout` component:**
   - Renders dockview with correct session panels
   - Calls `onLayoutChange` when layout changes
   - Builds initial layout when `initialLayout` is null
   - Adds new panels when sessions appear
   - Removes panels when sessions end
   - Restores layout from `initialLayout` via `fromJSON()`

4. **`LayoutPresets` component:**
   - Shows built-in presets (2x2, 3+1, Focus)
   - Shows custom presets after separator
   - Save flow: click "Save" -> input name -> confirm
   - Delete button only on custom presets
   - Built-in presets adapt to session count

5. **`LayoutModeToggle` component:**
   - Renders both mode buttons
   - Active mode is visually highlighted
   - Calls `onToggle` on click

### Integration Tests

1. **Toggle between modes:**
   - Start in Auto Grid, switch to Custom Layout, verify dockview renders
   - Switch back to Auto Grid, verify CSS Grid renders
   - Layout persists after toggle round-trip

2. **Session add/remove in Custom Layout:**
   - Start with 2 sessions in custom layout
   - New session appears -> panel added
   - Session ends -> panel removed, layout adjusts

3. **Preset flow:**
   - Apply "2x2" preset, verify 4-panel layout
   - Manually resize a panel, verify activePreset becomes null
   - Save as "My Layout", verify it appears in dropdown
   - Reload page, verify "My Layout" still in dropdown

4. **Layout serialization:**
   - Arrange panels manually
   - Call `toJSON()`, verify serialized form
   - Reload, call `fromJSON()`, verify layout restored
   - Corrupt localStorage, verify fallback to auto-build

### Performance Tests

- Drag resize with 8 panels: should maintain 60fps (no layout thrashing)
- Switching modes with 8 sessions: should complete in < 100ms
- `toJSON()`/`fromJSON()` round-trip: should be < 5ms for 8-panel layout

---

## Implementation Sequence

| Step | Task | Estimated effort |
|------|------|-----------------|
| 1 | Install dockview + dockview-react, import CSS, create dark theme override | Small |
| 2 | Create `useLayoutMode` hook with localStorage persistence | Small |
| 3 | Create `DockLayout` component with `SessionPanel` inner component | Medium |
| 4 | Create `LayoutModeToggle` component | Small |
| 5 | Integrate into `MonitorView.tsx` (conditional render, toolbar additions) | Medium |
| 6 | Create `LayoutPresets` component with built-in preset logic | Medium |
| 7 | Create `useLayoutPresets` hook | Small |
| 8 | Add custom tab renderer with status dots | Small |
| 9 | Add floating panel support (float button in tab actions) | Small |
| 10 | Add keyboard shortcuts | Small |
| 11 | Write tests (unit + integration) | Medium |
| 12 | End-to-end verification with live sessions | Small |

---

## Acceptance Criteria

- [ ] Toggle between Auto Grid and Custom Layout works without losing session state
- [ ] Drag-to-resize panels works smoothly (60fps, no jank)
- [ ] Drag-to-reposition creates new splits (drop zones: top/bottom/left/right/center)
- [ ] Tab grouping works (drop panel tab onto another panel's tab bar)
- [ ] Floating panels work (pop out, drag, resize, dock back)
- [ ] Layout persists across page reloads via localStorage (`toJSON`/`fromJSON`)
- [ ] Save named presets: save current layout with a custom name
- [ ] Load named presets: selecting a preset applies it immediately
- [ ] Delete named presets: remove user-created presets (built-in presets cannot be deleted)
- [ ] Built-in presets work: "2x2", "3+1", "Focus" apply correctly and adapt to session count
- [ ] "Reset" returns to Auto Grid mode and clears saved custom layout
- [ ] Session count vs preset mismatch handled gracefully (extra sessions as tabs, fewer slots skipped)
- [ ] New sessions added while in Custom Layout get a panel automatically
- [ ] Ended sessions removed from Custom Layout without breaking the layout
- [ ] Dark theme styling matches the existing design system (green accent, OLED blacks)
- [ ] Custom tab renderer shows status dots with correct colors
- [ ] Keyboard shortcuts (`Ctrl+Shift+G`, `Ctrl+Shift+L`, `1`-`9`) work
- [ ] Watermark shown when no sessions exist
- [ ] Corrupt localStorage fallback works (auto-build from current sessions)
- [ ] All new components and hooks have passing tests
- [ ] Performance: no jank during drag operations with up to 8 panels

---

## Changelog of Fixes Applied (Audit → Final Plan)

| #  | Issue | Severity | Fix Applied |
|----|-------|----------|-------------|
| 1  | `session.session_id` doesn't exist on `LiveSession` | Blocker | Replaced all `session_id` → `id` throughout DockLayout.tsx and preset code |
| 2  | `session.project_name` doesn't exist on `LiveSession` | Blocker | Replaced all `project_name` → `projectDisplayName` |
| 3  | `sortedSessions` doesn't exist in MonitorView | Blocker | Replaced all `sortedSessions` → `visibleSessions` (actual variable name) |
| 4  | `RichTerminalPane` props wrong (missing `isVisible`, wrong `verbose`, extra `session`) | Blocker | Rewrote `SessionPanel` to pass `sessionId`, `isVisible={true}`, `verboseMode` — matching actual `RichTerminalPaneProps` interface |
| 5  | `addFloatingGroup` signature wrong (`width`/`height` inside `position`) | Blocker | Fixed to `{ width: 400, height: 300, position: { right: 24, bottom: 24 } }` |
| 6  | Keyboard shortcut guard blocks all modifier combos including `Ctrl+Shift` | Blocker | Added `Ctrl+Shift` handler block before the `if (e.ctrlKey \|\| ...) return` guard, plus `onLayoutModeChange` callback in options interface |
| 7  | `IDockviewPanelProps` destructuring missing `containerApi` | Warning | Added `containerApi` to `SessionPanel` destructuring |
| 8  | `IDockviewPanelHeaderProps` destructuring missing `containerApi` | Warning | Added `containerApi` to `SessionTabRenderer` destructuring |
| 9  | `className` may be deprecated for theming in dockview v5 | Warning | Kept `className` (still works in v5), added migration note about `theme` prop |
| 10 | Status vocabulary mismatch (plan used `active/waiting/idle/done`, actual is `working/paused/done`) | Warning | Defined `statusToColor()` function mapping `LiveSession.status` values to design-token colors; pass `status` via panel params |
| 11 | Slate palette not in design tokens | Warning | Replaced all slate hex values with GitHub-style palette (`#0D1117`, `#161B22`, `#21262D`, `#30363D`) matching existing `MonitorPane.tsx` |
| 12 | Missing `@import` for `dockview-dark.css` | Warning | Added `@import './styles/dockview-dark.css'` to index.css instructions |
| 13 | Preset overflow: `slice(0, 4)` silently drops extra sessions | Warning | Implemented `positioned`/`overflow` split; overflow sessions added as tabs in last group |
| 14 | Unused `MonitorPane` import in DockLayout.tsx | Warning | Removed import |
| 15 | CSS import path: `dockview-react/dist/styles/dockview.css` vs `dockview/dist/styles/dockview.css` | Minor | Kept `dockview-react` path (both work, this is canonical for React wrapper) |
| 16 | Stars count "2,300+" outdated | Minor | Updated to "3,000+" |
| 17 | MonitorView integration missing `dockviewApiRef`, handler definitions, toolbar color classes | Added | Added `useRef<DockviewApi>`, all handler implementations, replaced `bg-slate-800` with `dark:` variants matching codebase |
| 18 | `onDidLayoutChange` fires per mutation (no debounce) | Added | Added 100ms debounce timer in `onDidLayoutChange` handler |
| 19 | `LayoutModeToggle` styling used slate classes | Added | Updated to match `ViewModeSwitcher.tsx` pattern (indigo active, gray inactive, `dark:` variants) |
| 20 | CSS `@import` must precede `@plugin` (CSS spec) | Blocker | Added explicit instruction: imports go after `@import "tailwindcss"` and before `@plugin` |
| 21 | Mixed import sources (`dockview` vs `dockview-react`) | Blocker | Standardized all imports to `'dockview-react'` throughout |
| 22 | Unused `containerApi` destructuring (lint warning) | Warning | Changed to `_containerApi` underscore prefix in SessionPanel |
| 23 | `dockviewApiRef` never populated (missing `onApiReady` prop) | Blocker | Added `onApiReady` to `DockLayoutProps`, call in `onReady`, pass callback in MonitorView JSX |
| 24 | `api.removePanel()` during live iteration (iterator invalidation) | Blocker | Snapshot via `.filter()` before iterating; removed panels from snapshot array |
| 25 | `handleResetLayout` uses `toggleMode()` (fragile, depends on current state) | Warning | Changed to `setMode('auto-grid')` — explicit target, no toggle ambiguity |
| 26 | 1-9 shortcuts broken in Custom Layout mode (`selectPane` doesn't interact with dockview) | Warning | Added mode-aware logic: `dockviewApi.getPanel(id)?.focus()` in custom mode, existing `selectPane` in auto-grid |
| 27 | Preset titles use raw `id.slice(0, 8)` instead of display names | Warning | Added `sessionTitle()` helper; changed `applyPreset` to accept `LiveSession[]`; `LayoutPresetsProps.sessions` replaces `.sessionIds` |
| 28 | `onDidLayoutChange` fires on tab focus (unnecessary persists) | Warning | Added `onDidAddPanel` + `onDidRemovePanel` listeners alongside debounced `onDidLayoutChange` catch-all |
| 29 | localStorage missing `QuotaExceededError` handling | Warning | Wrapped all `localStorage.setItem()` calls in try/catch in `useLayoutMode` and `useLayoutPresets` |
| 30 | `onLayoutModeChange` never passed at keyboard shortcuts call site | Blocker | Added updated `useMonitorKeyboardShortcuts()` call in MonitorView with `onLayoutModeChange`, `layoutMode`, `dockviewApi` |
| 31 | `onApiReady` missing from `onReady` useCallback deps | Warning | Added `onApiReady` to dependency array |
| 32 | Watermark component created inline (new ref each render) | Minor | Moved `EmptyWatermark` + `components` registry outside component body |
| 33 | `components` registry defined inside render (referential instability) | Minor | Moved to module scope alongside `EmptyWatermark` |
| 34 | `applyPreset` omits `params` in all `addPanel` calls (panels render "Session ended") | Blocker | Added `panelParams()` helper; every `addPanel` in `applyPreset` now passes `params: panelParams(id)` with `sessionId`, `verboseMode`, `status` |
| 35 | Inline watermark arrow in Tab Group section contradicts module-level `EmptyWatermark` | Warning | Replaced inline `() => (...)` with `EmptyWatermark` reference in Tab Group section's `DockviewReact` JSX |
| 36 | `onReady` useCallback deps include `sessions`/`verboseMode` — dockview re-inits on every SSE tick | Blocker | Moved `sessions`, `verboseMode`, `onLayoutChange` to refs; `onReady` deps reduced to `[initialLayout, onApiReady]` with eslint-disable comment |
| 37 | Missing `import type { DockviewApi }` in `useMonitorKeyboardShortcuts.ts` | Blocker | Added explicit import instruction in the keyboard shortcuts section |
| 38 | Duplicate `useRef` import in MonitorView new imports section | Warning | Removed — noted `useRef` is already imported in existing file |
| 39 | Options interface defined twice (partial then full) | Warning | Collapsed into single complete definition with all 3 new fields |
