---
status: draft
date: 2026-02-17
---

# Unified Session Detail Panel

## Problem

Mission Control has two separate detail views for sessions:

1. **Monitor ExpandedPaneOverlay** — full-screen portal (95vw × 90vh), no tabs, swim lanes + terminal stacked. Triggered by double-click. Missing: cost breakdown, timeline, context gauge, overview.

2. **KanbanSidePanel** — inline 3/5-width panel, 5 tabs. Triggered by single click. Missing: verbose mode toggle in terminal.

They share the same sub-components (`RichTerminalPane`, `SwimLanes`, `SubAgentDrillDown`, `TimelineView`, `CostBreakdown`) but compose them differently. Maintaining two views that do 80% the same thing is a liability — bugs get fixed in one but not the other, features added to one but forgotten in the other.

## Design

Replace both with a single `SessionDetailPanel` — a Notion-style peek drawer that slides in from the right edge.

### Layout

```
┌──────────────────────────────────┬─────────────────────┐
│                                  │  SessionDetailPanel  │
│      Active View                 │  ┌───────────────┐  │
│  (Kanban / List / Monitor)       │  │    Header      │  │
│                                  │  ├───────────────┤  │
│   Stays full-width               │  │  Tab Bar       │  │
│   No backdrop dimming            │  ├───────────────┤  │
│   Panel floats on top            │  │               │  │
│   with drop shadow               │  │  Tab Content   │  │
│                                  │  │               │  │
│                                  │  │               │  │
│                                  │  └───────────────┘  │
└──────────────────────────────────┴─────────────────────┘
```

- **Width**: `w-[480px] max-w-[45vw]`
- **Height**: full viewport (`h-screen`), pinned to right edge
- **No backdrop**: panel floats with `shadow-2xl` / `shadow-black/50`, no dimming overlay
- **Animation**: `translate-x-full → translate-x-0` CSS transition (~200ms ease-out)
- **Z-index**: `z-50`
- **Rendered**: `createPortal` to `document.body` (escapes any parent layout)
- **Close**: X button, ESC key, or clicking the same session card again (deselect)
- **ESC priority**: if sub-agent drill-down is active, ESC exits drill-down first; second ESC closes panel

### Header

Single-row, compact:

```
┌────────────────────────────────────────────────┐
│ 📁 my-project  ⎇ feature/foo  $1.23  Turn 12  ✕│
└────────────────────────────────────────────────┘
```

- Project name (truncated, tooltip for full path)
- Git branch with `GitBranch` icon (truncated)
- Total cost (monospace `$X.XX`)
- Turn count
- Close button (`X` icon)

### Tabs (4 tabs)

| Tab | Icon | Default |
|-----|------|---------|
| **Overview** | `LayoutDashboard` | Yes |
| **Terminal** | `Terminal` | |
| **Sub-Agents** | `Users` | |
| **Cost** | `DollarSign` | |

Tab resets to Overview when `session.id` changes (panel is keyed by session ID).

#### Overview Tab

Scrollable card layout (carried from KanbanSidePanel):

1. **Cost card** (clickable → Cost tab)
   - Large total cost, input/output breakdown, cache savings

2. **Session info card**
   - Status badge (Running/Paused/Done)
   - Model name
   - Turn count
   - Total tokens

3. **Context gauge** (`ContextGauge` component)

4. **Sub-agents compact** (clickable → Sub-Agents tab, only if sub-agents exist)
   - Count heading + `SubAgentPills`

5. **Mini timeline** (clickable → Sub-Agents tab, only if sub-agents + startedAt)
   - Compact `TimelineView`

6. **Last prompt** (only if `lastUserMessage`)
   - 3-line clamp of last user message

#### Terminal Tab

- `RichTerminalPane` filling full tab height
- **Verbose mode toggle button** in tab header area (improvement: currently hardcoded false in KanbanSidePanel)

#### Sub-Agents Tab (merged with Timeline)

**No drill-down active:**
```
┌──────────────────────────────┐
│  SwimLanes                   │
│  (click row → drill down)    │
├──────────────────────────────┤
│  TimelineView (Gantt chart)  │
└──────────────────────────────┘
```

Split: swim lanes take ~50% height, timeline takes ~50%, both scrollable. Empty state: "No sub-agents" message.

**Drill-down active:**
- `SubAgentDrillDown` replaces entire tab content
- Back button / ESC returns to swim lanes + timeline
- Verbose toggle in drill-down header (existing behavior)

#### Cost Tab

- `CostBreakdown` component (total, line items, sub-agent breakdown)

### Trigger: Single Click Everywhere

| View | Current trigger | New trigger |
|------|----------------|-------------|
| Kanban | Single click on SessionCard | Same (unchanged) |
| List | (not implemented yet) | Single click on row |
| Monitor | Double-click / maximize button | **Single click** on pane |

All views call `onSelectSession(sessionId)` → parent manages selection state.

### State Management

`MissionControlPage` owns:
- `selectedSessionId: string | null`
- Passes `onSelectSession` callback to all child views
- Renders `SessionDetailPanel` when `selectedSessionId !== null`
- Resolves the `LiveSession` from the sessions map

`SessionDetailPanel` owns (local state):
- `activeTab` (resets to 'overview' on session change via key)
- `drillDownAgentId` (for sub-agent drill-down)
- `verboseMode` (for terminal tab)

## Files Changed

### Created
- `src/components/live/SessionDetailPanel.tsx` — the unified component

### Modified
- `src/components/live/MissionControlPage.tsx` (or parent) — render `SessionDetailPanel`, remove `KanbanSidePanel` usage
- `src/components/live/MonitorView.tsx` — remove `ExpandedPaneOverlay` composition, add `onSelectSession` prop
- `src/components/live/KanbanView.tsx` (or equivalent) — wire `onSelectSession` instead of inline panel

### Deleted
- `src/components/live/KanbanSidePanel.tsx` — fully replaced
- `src/components/live/KanbanSidePanel.test.tsx` — tests migrate to new component
- `src/components/live/ExpandedPaneOverlay.tsx` — if standalone file (or remove from MonitorView)

### Unchanged (sub-components)
- `RichTerminalPane`, `RichPane`, `SwimLanes`, `SubAgentDrillDown`
- `TimelineView`, `CostBreakdown`, `SubAgentPills`, `ContextGauge`
- `useTerminalSocket`, `useSubAgentStream`

## Migration

1. Build `SessionDetailPanel` with all 4 tabs
2. Wire into `MissionControlPage` with `selectedSessionId` state
3. Update Kanban view to use `onSelectSession` instead of inline panel
4. Update Monitor view to use `onSelectSession` instead of `ExpandedPaneOverlay`
5. Delete `KanbanSidePanel.tsx` and `ExpandedPaneOverlay` usage
6. Update tests
