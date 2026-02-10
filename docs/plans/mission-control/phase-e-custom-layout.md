---
status: pending
date: 2026-02-10
phase: E
depends_on: C
---

# Phase E: Custom Layout with react-mosaic

> Power user feature: drag-and-drop pane arrangement for Monitor mode using react-mosaic-component.

**Goal:** Let users customize the Monitor mode layout by dragging pane edges to resize, dragging pane headers to reposition, and stacking panes as tabs. Provide save/load presets so users can switch between layouts instantly.

**Depends on:** Phase C (Monitor Mode) -- Custom Layout is a sub-mode within Monitor view, operating on the same session panes.

---

## Background

Phase C introduces Monitor mode with a responsive CSS Grid that auto-arranges session panes based on count. This works well for casual use, but power users running 4-8 concurrent sessions want precise control over which pane is large (their "focus" session) vs. small (background tasks).

**react-mosaic-component** (used by Palantir's Blueprint) provides:
- Binary tree layout model (each split is H or V)
- Drag-to-resize via split bars
- Drag-to-reposition via header drag
- Drop onto existing pane to create tab stack
- Serializable layout state (JSON-friendly for localStorage)
- ~8KB gzipped, React-native, no jQuery dependency

This phase adds a toggle between "Auto Grid" (Phase C default) and "Custom Layout" (react-mosaic). The toggle lives inside Monitor mode -- it does not affect Grid, List, or Kanban views.

---

## Dependencies to Add

### npm

**File to modify:** `package.json`

```json
{
  "dependencies": {
    "react-mosaic-component": "^6.1.0"
  }
}
```

After modifying `package.json`, regenerate both lockfiles per project conventions:
```bash
bun install          # updates bun.lock (dev)
npm install          # updates package-lock.json (distribution)
```

### CSS

react-mosaic ships its own CSS for split bars, drag handles, and tab styling. Import it in the main app entry point:

**File to modify:** `src/index.css` (or equivalent global CSS import)

```css
@import 'react-mosaic-component/react-mosaic-component.css';
```

Override mosaic's default light theme with dark theme variables to match the OLED design system (Phase A/C):

**File to create:** `src/styles/mosaic-dark.css`

```css
/* Override react-mosaic defaults for dark OLED theme */
.mosaic-root {
  --mosaic-bg: #0a0a0a;
  --mosaic-split-bg: #1a1a1a;
}

.mosaic .mosaic-split {
  background: var(--mosaic-split-bg);
}

.mosaic .mosaic-split:hover {
  background: #2a2a2a;
}

.mosaic .mosaic-window .mosaic-window-toolbar {
  background: #111111;
  border-bottom: 1px solid #222222;
  color: #e0e0e0;
}

.mosaic .mosaic-window .mosaic-window-body {
  background: var(--mosaic-bg);
}

/* Tab stack styling */
.mosaic .mosaic-window .mosaic-window-toolbar .mosaic-tab {
  background: #111111;
  color: #888888;
  border: none;
}

.mosaic .mosaic-window .mosaic-window-toolbar .mosaic-tab.active {
  color: #e0e0e0;
  border-bottom: 2px solid #22c55e; /* green accent matching status colors */
}

/* Drag preview */
.mosaic-blueprint-theme .mosaic-preview {
  border: 2px dashed #22c55e;
  background: rgba(34, 197, 94, 0.05);
}
```

---

## Implementation

### Layout Mode State

**File to create:** `src/hooks/use-layout-mode.ts`

```tsx
import { useState, useCallback } from 'react'
import type { MosaicNode } from 'react-mosaic-component'

export type LayoutMode = 'auto-grid' | 'custom'

interface UseLayoutModeResult {
  mode: LayoutMode
  setMode: (mode: LayoutMode) => void
  toggleMode: () => void

  /** react-mosaic layout tree. null when mode is 'auto-grid'. */
  mosaicLayout: MosaicNode<string> | null
  setMosaicLayout: (layout: MosaicNode<string> | null) => void

  /** Active preset name, or null if layout has been manually modified. */
  activePreset: string | null
}

const LAYOUT_STORAGE_KEY = 'claude-view:monitor-layout'
const MODE_STORAGE_KEY = 'claude-view:monitor-layout-mode'

export function useLayoutMode(): UseLayoutModeResult {
  // Restore mode from localStorage, default to 'auto-grid'
  const [mode, setModeState] = useState<LayoutMode>(() => {
    const stored = localStorage.getItem(MODE_STORAGE_KEY)
    return stored === 'custom' ? 'custom' : 'auto-grid'
  })

  // Restore mosaic layout from localStorage
  const [mosaicLayout, setMosaicLayoutState] = useState<MosaicNode<string> | null>(() => {
    const stored = localStorage.getItem(LAYOUT_STORAGE_KEY)
    if (stored) {
      try { return JSON.parse(stored) } catch { return null }
    }
    return null
  })

  const [activePreset, setActivePreset] = useState<string | null>(null)

  const setMode = useCallback((newMode: LayoutMode) => {
    setModeState(newMode)
    localStorage.setItem(MODE_STORAGE_KEY, newMode)
  }, [])

  const toggleMode = useCallback(() => {
    setMode(mode === 'auto-grid' ? 'custom' : 'auto-grid')
  }, [mode, setMode])

  const setMosaicLayout = useCallback((layout: MosaicNode<string> | null) => {
    setMosaicLayoutState(layout)
    setActivePreset(null) // manual change invalidates preset
    if (layout) {
      localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(layout))
    } else {
      localStorage.removeItem(LAYOUT_STORAGE_KEY)
    }
  }, [])

  return { mode, setMode, toggleMode, mosaicLayout, setMosaicLayout, activePreset }
}
```

### Custom Layout Component

**File to create:** `src/components/live/CustomLayout.tsx`

```tsx
import { useCallback } from 'react'
import { Mosaic, MosaicWindow, MosaicNode } from 'react-mosaic-component'
import type { LiveSession } from '@/types/generated/LiveSession'

interface CustomLayoutProps {
  /** Map of session ID to session data */
  sessions: Map<string, LiveSession>
  /** Current mosaic layout tree */
  layout: MosaicNode<string> | null
  /** Called when layout changes (resize, reposition, tab stack) */
  onLayoutChange: (layout: MosaicNode<string> | null) => void
  /** Render function for a session pane (reuses Phase C's MonitorPane) */
  renderPane: (sessionId: string) => React.ReactNode
}

export function CustomLayout({ sessions, layout, onLayoutChange, renderPane }: CustomLayoutProps) {
  const renderTile = useCallback(
    (id: string, path: MosaicBranch[]) => (
      <MosaicWindow<string>
        path={path}
        title={sessions.get(id)?.project_name ?? id}
        // Toolbar shows session project name + status dot
        toolbarControls={[/* close button, expand button */]}
      >
        {renderPane(id)}
      </MosaicWindow>
    ),
    [sessions, renderPane]
  )

  if (!layout) {
    // Build initial layout from current sessions
    const ids = Array.from(sessions.keys())
    const initial = buildBalancedLayout(ids)
    onLayoutChange(initial)
    return null // re-render with layout set
  }

  return (
    <Mosaic<string>
      renderTile={renderTile}
      value={layout}
      onChange={onLayoutChange}
      className="mosaic-blueprint-theme" // for CSS targeting
    />
  )
}
```

**Helper: `buildBalancedLayout`**

Converts a list of session IDs into a balanced binary tree for react-mosaic:

```tsx
function buildBalancedLayout(ids: string[]): MosaicNode<string> | null {
  if (ids.length === 0) return null
  if (ids.length === 1) return ids[0]
  if (ids.length === 2) {
    return { direction: 'row', first: ids[0], second: ids[1], splitPercentage: 50 }
  }
  // Split into two halves, recurse
  const mid = Math.ceil(ids.length / 2)
  const left = buildBalancedLayout(ids.slice(0, mid))
  const right = buildBalancedLayout(ids.slice(mid))
  if (!left) return right
  if (!right) return left
  return { direction: 'column', first: left, second: right, splitPercentage: 50 }
}
```

### Layout Presets

**File to create:** `src/components/live/LayoutPresets.tsx`

Dropdown for saving, loading, and selecting layout presets.

**Built-in presets:**

| Name | Layout | Use case |
|------|--------|----------|
| "2x2" | 2 columns, 2 rows, equal size | 4 sessions side by side |
| "3+1" | 3 small panes left, 1 large pane right (70/30 split) | Focus on one session, monitor others |
| "Focus" | Single pane fills entire area | Deep dive into one session |

**Props:**
```tsx
interface LayoutPresetsProps {
  /** Current session IDs to populate the layout */
  sessionIds: string[]
  /** Current active preset name (null if manually modified) */
  activePreset: string | null
  /** Called when user selects a preset */
  onSelectPreset: (layout: MosaicNode<string>, presetName: string) => void
  /** Called when user saves current layout as a named preset */
  onSavePreset: (name: string) => void
  /** Called when user deletes a saved preset */
  onDeletePreset: (name: string) => void
}
```

**Saved presets storage:**

Custom presets are stored in localStorage under key `claude-view:monitor-presets`:

```json
{
  "My Dev Setup": { "direction": "row", "first": "...", "second": "...", "splitPercentage": 70 },
  "Review Mode": { "direction": "column", "first": "...", "second": "..." }
}
```

**Behavior:**
- Dropdown shows built-in presets first (separator), then user-saved presets
- Selecting a preset applies it immediately and highlights it as active
- "Save Current Layout" option opens a small inline text input for naming
- User presets have a delete button (built-in presets do not)
- Presets store the tree structure only, not session IDs. When applying a preset, session IDs are mapped positionally (first session goes to first leaf, etc.)

**File to create:** `src/hooks/use-layout-presets.ts`

```tsx
import { useState, useCallback } from 'react'
import type { MosaicNode } from 'react-mosaic-component'

const PRESETS_STORAGE_KEY = 'claude-view:monitor-presets'

interface Preset {
  name: string
  layout: MosaicNode<string>
  builtIn: boolean
}

export function useLayoutPresets() {
  const [customPresets, setCustomPresets] = useState<Record<string, MosaicNode<string>>>(() => {
    const stored = localStorage.getItem(PRESETS_STORAGE_KEY)
    if (stored) {
      try { return JSON.parse(stored) } catch { return {} }
    }
    return {}
  })

  const savePreset = useCallback((name: string, layout: MosaicNode<string>) => {
    setCustomPresets(prev => {
      const next = { ...prev, [name]: layout }
      localStorage.setItem(PRESETS_STORAGE_KEY, JSON.stringify(next))
      return next
    })
  }, [])

  const deletePreset = useCallback((name: string) => {
    setCustomPresets(prev => {
      const next = { ...prev }
      delete next[name]
      localStorage.setItem(PRESETS_STORAGE_KEY, JSON.stringify(next))
      return next
    })
  }, [])

  // Built-in presets are generated dynamically based on session count
  // Custom presets are persisted in localStorage
  return { customPresets, savePreset, deletePreset }
}
```

### Monitor View Integration

**File to modify:** The Monitor view component from Phase C.

Add a layout mode toggle to the Monitor view toolbar:

```tsx
// Inside Monitor view component
const { mode, toggleMode, mosaicLayout, setMosaicLayout } = useLayoutMode()

return (
  <div>
    <MonitorToolbar>
      {/* Existing controls from Phase C */}
      <LayoutModeToggle mode={mode} onToggle={toggleMode} />
      {mode === 'custom' && (
        <>
          <LayoutPresets
            sessionIds={sessionIds}
            activePreset={activePreset}
            onSelectPreset={handleSelectPreset}
            onSavePreset={handleSavePreset}
            onDeletePreset={handleDeletePreset}
          />
          <ResetLayoutButton onClick={handleResetLayout} />
        </>
      )}
    </MonitorToolbar>

    {mode === 'auto-grid' ? (
      <AutoGrid sessions={sessions} />  {/* Phase C's existing grid */}
    ) : (
      <CustomLayout
        sessions={sessions}
        layout={mosaicLayout}
        onLayoutChange={setMosaicLayout}
        renderPane={renderMonitorPane}  {/* Reuse Phase C's pane renderer */}
      />
    )}
  </div>
)
```

### Layout Mode Toggle

**File to create:** `src/components/live/LayoutModeToggle.tsx`

A simple segmented control with two options:

```
[ Auto Grid | Custom Layout ]
```

**Props:**
```tsx
interface LayoutModeToggleProps {
  mode: 'auto-grid' | 'custom'
  onToggle: () => void
}
```

**Styling:**
- Segmented button matching the existing view switcher style (Phase B)
- Active segment highlighted with green accent
- Icons: grid icon for Auto Grid, layout icon for Custom Layout
- Tooltip: "Auto Grid: responsive layout" / "Custom Layout: drag to arrange"

### Reset Layout Button

**File to create:** `src/components/live/ResetLayoutButton.tsx`

A button that clears the custom layout and switches back to Auto Grid mode.

**Behavior:**
- Clears `mosaicLayout` from state and localStorage
- Switches mode back to `'auto-grid'`
- Shows confirmation tooltip on hover: "Return to automatic grid layout"

### Keyboard Shortcuts

Integrate with the existing keyboard shortcut system (Phase B):

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+G` | Switch to Auto Grid |
| `Ctrl+Shift+L` | Switch to Custom Layout |
| `1`-`9` (in Custom Layout) | Focus pane N (brings to front if tabbed) |

---

## Session Add/Remove Handling

When sessions start or stop while in Custom Layout mode, the layout tree must be updated:

### New Session Appears
- Add new session as a leaf node split from the last node in the tree
- Direction alternates (row, column, row, ...) for balanced splits
- New pane gets 30% of the split (existing content keeps 70%)

### Session Ends
- Remove the session's leaf from the tree
- Its sibling "inherits" the parent's space (standard react-mosaic behavior)
- If the removed session was the last one, switch to empty state

### Session ID Stability
- Session IDs from Phase A are stable (based on JSONL file path hash)
- react-mosaic nodes reference session IDs as string keys
- Layout tree survives page reload as long as the same sessions are still active

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `src/components/live/CustomLayout.tsx` | react-mosaic wrapper with pane rendering |
| `src/components/live/LayoutPresets.tsx` | Preset dropdown with save/load/delete |
| `src/components/live/LayoutModeToggle.tsx` | Auto Grid / Custom Layout segmented control |
| `src/components/live/ResetLayoutButton.tsx` | Button to reset to Auto Grid |
| `src/hooks/use-layout-mode.ts` | Layout mode + mosaic state with localStorage persistence |
| `src/hooks/use-layout-presets.ts` | Preset save/load with localStorage |
| `src/styles/mosaic-dark.css` | Dark theme overrides for react-mosaic |
| `src/components/live/CustomLayout.test.tsx` | Tests for custom layout |
| `src/components/live/LayoutPresets.test.tsx` | Tests for presets |
| `src/hooks/use-layout-mode.test.ts` | Tests for layout mode hook |
| `src/hooks/use-layout-presets.test.ts` | Tests for presets hook |

### Modified Files

| File | Change |
|------|--------|
| `package.json` | Add `react-mosaic-component` dependency |
| `bun.lock` | Regenerated after `bun install` |
| `package-lock.json` | Regenerated after `npm install` |
| `src/index.css` | Import react-mosaic base CSS |
| Monitor view component (Phase C) | Add layout mode toggle + conditional rendering |
| Keyboard shortcuts config (Phase B) | Add `Ctrl+Shift+G`, `Ctrl+Shift+L`, `1`-`9` bindings |

### Dependencies Added

| Package | Version | Size | Why |
|---------|---------|------|-----|
| `react-mosaic-component` | ^6.1.0 | ~8KB gzip | Drag-and-drop pane layout |

No new Rust dependencies. This phase is frontend-only.

---

## Testing Strategy

### Unit Tests

1. **`buildBalancedLayout`:**
   - 0 IDs returns null
   - 1 ID returns the ID string (leaf node)
   - 2 IDs returns a row split at 50%
   - 4 IDs returns a balanced 2x2 tree
   - 7 IDs returns a balanced tree with correct depth

2. **`useLayoutMode` hook:**
   - Defaults to `'auto-grid'` when localStorage is empty
   - Restores `'custom'` from localStorage
   - `toggleMode` switches between modes
   - `setMosaicLayout` persists to localStorage
   - `setMosaicLayout(null)` removes from localStorage

3. **`useLayoutPresets` hook:**
   - Loads custom presets from localStorage on mount
   - `savePreset` persists to localStorage
   - `deletePreset` removes from localStorage
   - Built-in presets are not deletable

4. **`CustomLayout` component:**
   - Renders mosaic with correct session panes
   - Calls `onLayoutChange` when user drags
   - Builds initial layout when `layout` is null

5. **`LayoutPresets` component:**
   - Shows built-in presets
   - Shows custom presets after separator
   - Save flow: click "Save" -> input name -> confirm
   - Delete button only on custom presets

### Integration Tests

1. **Toggle between modes:**
   - Start in Auto Grid, switch to Custom Layout, verify mosaic renders
   - Switch back to Auto Grid, verify CSS Grid renders
   - Layout persists after toggle round-trip

2. **Session add/remove in Custom Layout:**
   - Start with 2 sessions in custom layout
   - New session appears -> pane added to tree
   - Session ends -> pane removed, sibling fills space

3. **Preset flow:**
   - Apply "2x2" preset, verify 4-pane layout
   - Manually resize a pane, verify activePreset becomes null
   - Save as "My Layout", verify it appears in dropdown
   - Reload page, verify "My Layout" still in dropdown

### Performance Tests

- Drag resize with 8 panes: should maintain 60fps (no layout thrashing)
- Switching modes with 8 sessions: should complete in < 100ms
- localStorage serialization of layout tree: should be < 1ms for 8-pane tree

---

## Acceptance Criteria

- [ ] Toggle between Auto Grid and Custom Layout works without losing session state
- [ ] Drag-to-resize panes works smoothly (60fps, no jank)
- [ ] Drag-to-reposition creates new horizontal/vertical splits
- [ ] Tab stacking works (drop pane header onto another pane's header area)
- [ ] Layout persists across page reloads via localStorage
- [ ] Save named presets: save current layout with a custom name
- [ ] Load named presets: selecting a preset applies it immediately
- [ ] Delete named presets: remove user-created presets (built-in presets cannot be deleted)
- [ ] Built-in presets work: "2x2", "3+1", "Focus" apply correctly
- [ ] "Reset Layout" returns to Auto Grid mode and clears saved custom layout
- [ ] Performance: no jank during drag operations with up to 8 panes
- [ ] Works with 2 panes (minimum useful count)
- [ ] Works with 8 panes (maximum practical count)
- [ ] New sessions added while in Custom Layout get a pane automatically
- [ ] Ended sessions removed from Custom Layout without breaking the tree
- [ ] Dark theme styling matches the existing OLED design system
- [ ] Keyboard shortcuts (`Ctrl+Shift+G`, `Ctrl+Shift+L`) switch modes
- [ ] All new components and hooks have passing tests
