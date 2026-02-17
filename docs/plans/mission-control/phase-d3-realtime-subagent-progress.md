---
status: draft
date: 2026-02-17
phase: D3
depends_on: D.2
---

# Phase D3: Kanban Session Side Panel & Card Enhancements

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enhance the Kanban view with a right-side session detail panel (4 tabs: Terminal, Sub-Agents, Timeline, Cost) and visual polish (pulse dot, sub-agent activity pills on cards). Clicking a Kanban card opens the panel instead of navigating away.

**Architecture:** New `KanbanSidePanel` component with tab navigation, rendered alongside KanbanView in a flex layout (40% columns / 60% panel). Reuses existing components from Phase D and D.2: `RichTerminalPane` (extracted to shared file), `SwimLanes` + `SubAgentDrillDown`, `TimelineView`. Card enhancements are CSS/props changes to existing `SessionCard`.

**Tech Stack:** React, Tailwind CSS, Lucide icons. No backend changes.

**Depends on:** Phase D.2 (sub-agent drill-down, `currentActivity`, WebSocket endpoint)

**D.2 Gate:** Phase D.2 is currently `pending` (not started). **Tasks 1-5 and 7-8 can proceed immediately.** Task 6 (KanbanSidePanel) is **partially blocked**: the Terminal, Timeline, and Cost tabs compile fine; only the Sub-Agents tab's drill-down feature requires D.2 components (`SubAgentDrillDown`, `SwimLanes.onDrillDown`). The plan includes stub/placeholder handling for the Sub-Agents tab when D.2 is not yet available.

**Scope constraints:**
- Kanban view only (MonitorView untouched — D2 handles it)
- No mobile-specific layout
- No new keyboard shortcuts (Esc to close panel is sufficient)

---

## Background

### What Exists Today

**KanbanView** (`src/components/live/KanbanView.tsx`):
- 2 columns: Waiting (needs_you) / Running (autonomous) — working perfectly
- Cards rendered via `KanbanColumn` → `SessionCard`
- `SessionCard` is a `<Link>` — clicking navigates to `/session/{id}` (full page)
- `onSelect` callback highlights card with indigo ring, but navigation fires first
- Sub-agents shown only as count text: `(3 sub-agents)`

**MissionControlPage** (`src/pages/MissionControlPage.tsx`):
- SSE connection indicator already exists in header (lines 144-155): green/red dot + "Live"/"Reconnecting..."
- `isConnected`, `selectedId`, `stalledSessions`, `currentTime` all available
- KanbanView receives `onSelect` callback that toggles selection

**RichTerminalPane** (inside `src/components/live/MonitorView.tsx`):
- Local function (~26 lines) wrapping `useTerminalSocket` + `RichPane`
- Not exported — needs extraction for reuse in Kanban side panel

### What D.2 Provides (Dependencies)

- `currentActivity` field on `SubAgentInfo` (populated from progress events)
- `SubAgentDrillDown` component (sub-agent conversation viewer)
- `useSubAgentStream` hook (WebSocket connection to sub-agent JSONL)
- `SwimLanes` with `onDrillDown` prop (clickable sub-agent rows)
- `TimelineView` (Gantt chart of sub-agent execution)
- Sub-agent WebSocket endpoint (`/api/live/sessions/:id/subagents/:agentId/terminal`)

### What D3 Adds

| Feature | Current | After D3 |
|---------|---------|----------|
| Card click in Kanban | Navigates to `/session/{id}` | Opens side panel (no navigation) |
| Session detail | Full page only | Right-side panel with 4 tabs |
| Running card indicator | No visual distinction | Pulsing green dot |
| Sub-agent info on card | Count text `(3 sub-agents)` | SubAgentPills with activity |
| Terminal in Kanban | Not available | Terminal tab in side panel |
| Sub-agent drill-down in Kanban | Not available | Sub-Agents tab → SwimLanes → drill-down |

---

## Design

### Layout: Side Panel Open

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Mission Control · [Grid][List][Kanban][Monitor]    ● Live · Updated 2s │
├──────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────┬────────────────────────────────────────────┐│
│  │ Kanban (compressed 40%) │ Side Panel (60%)                          ││
│  │                         │                                            ││
│  │ ┌──────┬──────┐        │ ← Back   myapp · feature/auth        ✕   ││
│  │ │Wait  │Run   │        │ ● Running · 72% · $2.34 · 12 turns       ││
│  │ │(2)   │(4)   │        ├────────────────────────────────────────────┤│
│  │ │      │      │        │ [Terminal] [Sub-Agents] [Timeline] [Cost] ││
│  │ │┌────┐│┌────┐│        ├────────────────────────────────────────────┤│
│  │ ││docs│││●app││◄─sel   │                                            ││
│  │ │└────┘│└────┘│        │  (Active tab content)                      ││
│  │ │┌────┐│┌────┐│        │                                            ││
│  │ ││fe  │││back││        │  Terminal: live conversation stream        ││
│  │ │└────┘│└────┘│        │  Sub-Agents: SwimLanes + drill-down       ││
│  │ │      │┌────┐│        │  Timeline: Gantt chart                     ││
│  │ │      ││api ││        │  Cost: breakdown table                     ││
│  │ │      │└────┘│        │                                            ││
│  │ └──────┴──────┘        │                                            ││
│  └─────────────────────────┴────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────────┘
```

### Layout: Side Panel Closed (Current, Unchanged)

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Kanban (full width)                                                     │
│  ┌────────────────────────────┬────────────────────────────┐             │
│  │ Waiting (2)                │ Running (4)                │             │
│  │ ┌────────────────────────┐ │ ┌────────────────────────┐ │             │
│  │ │ docs · feature/docs    │ │ │ ● myapp · auth-flow    │ │             │
│  │ │ Fix documentation      │ │ │ Adding auth module     │ │             │
│  │ │ [E ⟳] [C ⟳]           │ │ │ [E ⟳] [C done]        │ │             │
│  │ │ 41% · $0.28            │ │ │ 72% · $2.34            │ │             │
│  │ └────────────────────────┘ │ └────────────────────────┘ │             │
│  └────────────────────────────┴────────────────────────────┘             │
└──────────────────────────────────────────────────────────────────────────┘
```

### Enhanced Card Design

```
┌────────────────────────────────────┐
│ ● myapp    feature/auth    $2.34   │  ← Pulse dot (Running only)
│ Adding auth module                  │
│ [E ⟳] [C done]  2 agents (1 active)│  ← SubAgentPills (if sub-agents)
│ ⟳ Working... 45s · 12k tok · opus  │  ← SessionSpinner (existing)
│ ████████░░░ 72%                     │  ← ContextGauge (existing)
│ 12 turns                            │
└────────────────────────────────────┘
```

### Design System (from UI/UX Pro Max)

**Style:** Dark Mode (OLED) — consistent with existing Mission Control

**Colors (existing palette, no changes):**
- Running dot: `#22C55E` (green-500) with pulse animation
- Panel background: `bg-gray-950` / `bg-gray-900`
- Tab active: `border-indigo-500 text-indigo-400`
- Tab inactive: `text-gray-500 hover:text-gray-400`

**Animations:**
- Pulse dot: `2s cubic-bezier(0.4, 0, 0.6, 1) infinite` with `box-shadow` ring
- Panel slide-in: `200ms ease-out` (transform only, no width/height)
- All animations respect `prefers-reduced-motion: reduce`

**Key UX rules applied:**
- 150-300ms transitions for micro-interactions
- `cursor-pointer` on all clickable elements
- Transform/opacity only for animations (no width/height/top/left)
- Max 1-2 animated elements per view (pulse dot + sub-agent spinner)
- Focus states visible for keyboard navigation

---

## Task 1: Extract RichTerminalPane to Shared File

**Files:**
- Create: `src/components/live/RichTerminalPane.tsx`
- Modify: `src/components/live/MonitorView.tsx` (remove local definition, import from new file)

**Step 1: Create the shared component**

Extract the existing `RichTerminalPane` function from `MonitorView.tsx` (lines 24-50) into its own file:

```tsx
// src/components/live/RichTerminalPane.tsx
import { useState, useCallback } from 'react'
import { RichPane, parseRichMessage, type RichMessage } from './RichPane'
import { useTerminalSocket, type ConnectionState } from '../../hooks/use-terminal-socket'

interface RichTerminalPaneProps {
  sessionId: string
  isVisible: boolean
  verboseMode: boolean
}

/**
 * Wraps useTerminalSocket + RichPane for rich mode.
 * Manages its own WebSocket connection and parses messages into RichMessage[].
 */
export function RichTerminalPane({ sessionId, isVisible, verboseMode }: RichTerminalPaneProps) {
  const [messages, setMessages] = useState<RichMessage[]>([])
  const [bufferDone, setBufferDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
    const parsed = parseRichMessage(data)
    if (parsed) {
      setMessages((prev) => [...prev, parsed])
    }
  }, [])

  const handleConnectionChange = useCallback((state: ConnectionState) => {
    if (state === 'connected') {
      setBufferDone(true)
    }
  }, [])

  useTerminalSocket({
    sessionId,
    mode: 'rich',
    enabled: isVisible,
    onMessage: handleMessage,
    onConnectionChange: handleConnectionChange,
  })

  return <RichPane messages={messages} isVisible={isVisible} verboseMode={verboseMode} bufferDone={bufferDone} />
}
```

**Step 2: Update MonitorView imports and remove local function**

In `MonitorView.tsx`:

1. Add the new import:
```tsx
import { RichTerminalPane } from './RichTerminalPane'
```

2. **Remove** these imports that are ONLY used by the local `RichTerminalPane` (no longer needed):
   - `RichPane`, `parseRichMessage`, `type RichMessage` from `'./RichPane'` (line 7)
   - `type ConnectionState` from `'../../hooks/use-terminal-socket'` (line 12 — keep `useTerminalSocket` if used elsewhere, but check: it is NOT used elsewhere in MonitorView, so remove the entire import line 12)

3. **KEEP** these imports — they are used by MonitorView itself:
   - `useState` (lines 78, 81, 88), `useCallback` (lines 143, 150, 157, 168, 176), `useMemo` (line 99), `useRef` (line 96) from `'react'`

4. Delete the local `RichTerminalPane` function (lines 24-50).

After this step, MonitorView's react import should be:
```tsx
import { useState, useCallback, useMemo, useRef } from 'react'
```
And the `RichPane`/`useTerminalSocket` imports should be fully removed (replaced by the `RichTerminalPane` import).

**Step 3: Verify no regressions**

Run: `bun run vitest run src/components/live/`
Run: `bun run typecheck`
Expected: ALL PASS (MonitorView behavior unchanged)

**Step 4: Commit**

```bash
git add src/components/live/RichTerminalPane.tsx src/components/live/MonitorView.tsx
git commit -m "refactor(live): extract RichTerminalPane to shared file for reuse"
```

---

## Task 2: Pulse Dot on Running Cards

**Files:**
- Modify: `src/components/live/SessionCard.tsx`

**Step 1: Write the failing test**

Add to the **existing** `src/components/live/SessionCard.test.tsx` (which already has `createMockSession` helper and `renderCard` function — reuse them, do NOT create a duplicate helper):

```tsx
// Add these describe blocks AFTER the existing 'live SessionCard' describe block:

describe('SessionCard pulse dot', () => {
  it('shows pulse dot for autonomous (running) sessions', () => {
    const session = createMockSession({
      status: 'working',
      agentState: { state: 'tool_use', group: 'autonomous', label: 'Working', confidence: 1.0, source: 'jsonl' },
    })
    renderCard(session)
    expect(screen.getByTestId('pulse-dot')).toBeInTheDocument()
  })

  it('does not show pulse dot for waiting sessions', () => {
    const session = createMockSession({
      agentState: { state: 'awaiting_input', group: 'needs_you', label: 'Waiting', confidence: 0.9, source: 'jsonl' },
    })
    renderCard(session)
    expect(screen.queryByTestId('pulse-dot')).not.toBeInTheDocument()
  })
})
```

**Note:** The existing file already imports `render` but not `screen`. Add `screen` to the `@testing-library/react` import:
```tsx
import { render, screen } from '@testing-library/react'
```
```

**Step 2: Add pulse dot to SessionCard**

In `SessionCard.tsx`, add a pulse dot before the project name badge when `session.agentState.group === 'autonomous'`:

In the header div (line 76), insert before the project name span:

```tsx
{/* Pulse dot for running sessions */}
{session.agentState.group === 'autonomous' && (
  <span
    data-testid="pulse-dot"
    className="inline-block w-2 h-2 rounded-full bg-green-500 flex-shrink-0 motion-safe:animate-pulse"
    aria-hidden="true"
  />
)}
```

**Note:** `motion-safe:animate-pulse` is Tailwind's built-in class that automatically respects `prefers-reduced-motion: reduce`. No custom CSS needed.

**Step 3: Run tests**

Run: `bun run vitest run src/components/live/SessionCard.test.tsx`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/components/live/SessionCard.tsx src/components/live/SessionCard.test.tsx
git commit -m "feat(ui): add pulse dot indicator on running Kanban cards"
```

---

## Task 3: Sub-Agent Activity Pills on Cards

**Files:**
- Modify: `src/components/live/SessionCard.tsx`

**Step 1: Write the failing test**

Add to `SessionCard.test.tsx` (reuse existing `createMockSession` and `renderCard`):

```tsx
describe('SessionCard sub-agent pills', () => {
  it('shows SubAgentPills when session has sub-agents', () => {
    const session = createMockSession({
      subAgents: [
        { toolUseId: 'toolu_01', agentType: 'Explore', description: 'Search', status: 'running', startedAt: 1_700_000_000, currentActivity: 'Read' },
        { toolUseId: 'toolu_02', agentType: 'code-reviewer', description: 'Review', status: 'complete', startedAt: 1_700_000_000, completedAt: 1_700_000_030, durationMs: 30000 },
      ],
    })
    renderCard(session)
    // SubAgentPills renders pill elements with agent initials
    expect(screen.getByText('E')).toBeInTheDocument() // Explore initial
    expect(screen.getByText('2 agents (1 active)')).toBeInTheDocument()
  })

  it('does not show SubAgentPills when no sub-agents', () => {
    const session = createMockSession({ subAgents: undefined })
    renderCard(session)
    expect(screen.queryByText(/agents/)).not.toBeInTheDocument()
  })
})
```

**Step 2: Replace count text with SubAgentPills**

In `SessionCard.tsx`:

1. Add import at top:
```tsx
import { SubAgentPills } from './SubAgentPills'
```

2. Replace the sub-agent count text (lines 100-104) with SubAgentPills. Remove the old count text:
```tsx
// REMOVE this block (lines 100-104):
{session.subAgents && session.subAgents.length > 0 && (
  <span className="text-xs text-gray-400 dark:text-gray-500">
    ({session.subAgents.length} sub-agent{session.subAgents.length !== 1 ? 's' : ''})
  </span>
)}
```

3. Add SubAgentPills after the spinner row and before the context gauge (between lines 134 and 136):
```tsx
{/* Sub-agent pills — onExpand intentionally omitted: no drill-down from card context.
   This suppresses cursor-pointer hover styling on pills, which is correct for cards. */}
{session.subAgents && session.subAgents.length > 0 && (
  <div className="mb-2 -mx-1">
    <SubAgentPills subAgents={session.subAgents} />
  </div>
)}
```

**Step 3: Run tests**

Run: `bun run vitest run src/components/live/SessionCard.test.tsx`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/components/live/SessionCard.tsx src/components/live/SessionCard.test.tsx
git commit -m "feat(ui): show SubAgentPills with activity on Kanban cards"
```

---

## Task 4: SessionCard Click Override for Kanban

**Files:**
- Modify: `src/components/live/SessionCard.tsx`
- Modify: `src/components/live/KanbanColumn.tsx`

**Problem:** SessionCard is a `<Link>` that navigates on click. In Kanban view, clicking should open the side panel instead. We need SessionCard to support both behaviors.

**Step 1: Add `onClickOverride` prop to SessionCard**

In `SessionCard.tsx`, add optional prop:

```tsx
interface SessionCardProps {
  session: LiveSession
  stalledSessions?: Set<string>
  currentTime: number
  /** When provided, renders as a div instead of Link. Used by Kanban for side panel. */
  onClickOverride?: () => void
}
```

**Step 2: Conditionally render div or Link**

Replace the outer `<Link>` (line 70-73) with conditional rendering:

```tsx
export function SessionCard({ session, stalledSessions, currentTime, onClickOverride }: SessionCardProps) {
  const [searchParams] = useSearchParams()
  // ... existing logic ...

  const cardClassName = "group block rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4 hover:bg-gray-50 dark:hover:bg-gray-800/70 cursor-pointer transition-colors"

  const cardContent = (
    <>
      {/* ... all existing card content (lines 76-146) ... */}
    </>
  )

  if (onClickOverride) {
    return (
      <div
        onClick={(e) => { e.stopPropagation(); onClickOverride() }}
        className={cardClassName}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onClickOverride() } }}
      >
        {cardContent}
      </div>
    )
  }

  return (
    <Link to={buildSessionUrl(session.id, searchParams)} className={cardClassName} style={{ cursor: 'pointer' }}>
      {cardContent}
    </Link>
  )
}
```

**Step 3: Pass onClickOverride from KanbanColumn**

In `KanbanColumn.tsx`, add `onCardClick` prop:

```tsx
interface KanbanColumnProps {
  // ... existing props ...
  /** When provided, cards render as div instead of Link */
  onCardClick?: (sessionId: string) => void
}
```

**Important:** KanbanColumn wraps each SessionCard in a `<div>` with `onClick={() => onSelect(session.id)}`. When `onCardClick` is provided, the SessionCard renders as a `<div>` with `e.stopPropagation()` to prevent the wrapper's `onSelect` from also firing. The wrapper div's `onSelect` still handles the selection ring highlight. This is intentional: clicking a card in Kanban both selects it (ring) AND opens the side panel.

Pass `onClickOverride` to SessionCard:

```tsx
<SessionCard
  session={session}
  stalledSessions={stalledSessions}
  currentTime={currentTime}
  onClickOverride={onCardClick ? () => onCardClick(session.id) : undefined}
/>
```

**Step 4: Pass from KanbanView**

In `KanbanView.tsx`, add `onCardClick` prop:

```tsx
interface KanbanViewProps {
  // ... existing props ...
  onCardClick?: (sessionId: string) => void
}
```

Pass through to KanbanColumn:

```tsx
<KanbanColumn
  key={col.group}
  // ... existing props ...
  onCardClick={onCardClick}
/>
```

**Step 5: Type check**

Run: `bun run typecheck`
Expected: No errors (prop is optional, so existing callers unaffected)

**Step 6: Commit**

```bash
git add src/components/live/SessionCard.tsx src/components/live/KanbanColumn.tsx src/components/live/KanbanView.tsx
git commit -m "feat(ui): add onClickOverride to SessionCard for Kanban side panel"
```

---

## Task 5: Cost Breakdown Component

**Files:**
- Create: `src/components/live/CostBreakdown.tsx`
- Test: `src/components/live/CostBreakdown.test.tsx`

**Step 1: Write the test**

```tsx
// src/components/live/CostBreakdown.test.tsx
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { CostBreakdown } from './CostBreakdown'
import type { LiveSession } from './use-live-sessions'

describe('CostBreakdown', () => {
  it('renders total cost', () => {
    const cost = { totalUsd: 2.34, inputCostUsd: 1.50, outputCostUsd: 0.84, cacheReadCostUsd: 0.10, cacheCreationCostUsd: 0.05, cacheSavingsUsd: 0.50 }
    render(<CostBreakdown cost={cost} subAgents={[]} />)
    expect(screen.getByText('$2.34')).toBeInTheDocument()
  })

  it('renders sub-agent costs when present', () => {
    const cost = { totalUsd: 5.00, inputCostUsd: 3.00, outputCostUsd: 2.00, cacheReadCostUsd: 0, cacheCreationCostUsd: 0, cacheSavingsUsd: 0 }
    const subAgents = [
      { toolUseId: 'toolu_01', agentType: 'Explore', description: 'Search', status: 'complete' as const, startedAt: 0, costUsd: 0.50 },
      { toolUseId: 'toolu_02', agentType: 'code-reviewer', description: 'Review', status: 'complete' as const, startedAt: 0, costUsd: 0.30 },
    ]
    render(<CostBreakdown cost={cost} subAgents={subAgents} />)
    expect(screen.getByText('$0.50')).toBeInTheDocument()
    expect(screen.getByText('$0.30')).toBeInTheDocument()
  })
})
```

**Step 2: Implement the component**

```tsx
// src/components/live/CostBreakdown.tsx
import type { LiveSession } from './use-live-sessions'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface CostBreakdownProps {
  cost: LiveSession['cost']
  subAgents?: SubAgentInfo[]
}

export function CostBreakdown({ cost, subAgents }: CostBreakdownProps) {
  const subAgentTotal = subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  const mainAgentCost = cost.totalUsd - subAgentTotal

  return (
    <div className="space-y-4 p-4">
      {/* Total */}
      <div className="flex items-baseline justify-between">
        <span className="text-sm text-gray-400">Total Cost</span>
        <span className="text-2xl font-mono font-semibold text-gray-100">${cost.totalUsd.toFixed(2)}</span>
      </div>

      {/* Breakdown table */}
      <div className="space-y-2">
        <CostRow label="Input tokens" value={cost.inputCostUsd} />
        <CostRow label="Output tokens" value={cost.outputCostUsd} />
        {cost.cacheReadCostUsd > 0 && <CostRow label="Cache reads" value={cost.cacheReadCostUsd} />}
        {cost.cacheCreationCostUsd > 0 && <CostRow label="Cache creation" value={cost.cacheCreationCostUsd} />}
        {cost.cacheSavingsUsd > 0 && (
          <CostRow label="Cache savings" value={-cost.cacheSavingsUsd} className="text-green-400" />
        )}
      </div>

      {/* Sub-agent breakdown */}
      {subAgents && subAgents.length > 0 && (
        <div className="border-t border-gray-800 pt-3 space-y-2">
          <h4 className="text-xs font-medium text-gray-500 uppercase tracking-wide">Sub-Agent Costs</h4>
          <CostRow label="Main agent" value={mainAgentCost} />
          {subAgents
            .filter((a) => a.costUsd != null && a.costUsd > 0)
            .map((a) => (
              <CostRow key={a.toolUseId} label={`${a.agentType}: ${a.description}`} value={a.costUsd!} />
            ))}
        </div>
      )}
    </div>
  )
}

function CostRow({ label, value, className }: { label: string; value: number; className?: string }) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-gray-500 truncate mr-4">{label}</span>
      <span className={`font-mono tabular-nums ${className ?? 'text-gray-300'}`}>
        ${Math.abs(value).toFixed(2)}
      </span>
    </div>
  )
}
```

**Step 3: Run tests**

Run: `bun run vitest run src/components/live/CostBreakdown.test.tsx`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/components/live/CostBreakdown.tsx src/components/live/CostBreakdown.test.tsx
git commit -m "feat(ui): add CostBreakdown component for session cost tab"
```

---

## Task 6: Kanban Side Panel Component

**Files:**
- Create: `src/components/live/KanbanSidePanel.tsx`
- Test: `src/components/live/KanbanSidePanel.test.tsx`

**Step 1: Write the test**

```tsx
// src/components/live/KanbanSidePanel.test.tsx
import { describe, it, expect, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { KanbanSidePanel } from './KanbanSidePanel'
import type { LiveSession } from './use-live-sessions'

// Mock terminal pane (creates WebSocket)
vi.mock('./RichTerminalPane', () => ({
  RichTerminalPane: ({ sessionId }: { sessionId: string }) => (
    <div data-testid="terminal-pane">Terminal: {sessionId}</div>
  ),
}))

// Mock SwimLanes (display-only until D.2 adds onDrillDown)
vi.mock('./SwimLanes', () => ({
  SwimLanes: ({ subAgents }: { subAgents: unknown[] }) => (
    <div data-testid="swim-lanes">{subAgents.length} sub-agents</div>
  ),
}))

// D.2 GATE: Uncomment when SubAgentDrillDown exists:
// vi.mock('./SubAgentDrillDown', () => ({
//   SubAgentDrillDown: ({ agentId }: { agentId: string }) => (
//     <div data-testid="drill-down">DrillDown: {agentId}</div>
//   ),
// }))

function makeSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'test-1',
    project: 'test-project',
    projectDisplayName: 'test-project',
    projectPath: '/path',
    filePath: '/path/to/file.jsonl',
    status: 'working',
    agentState: { state: 'tool_use', group: 'autonomous', label: 'Working', confidence: 1.0, source: 'jsonl' },
    gitBranch: 'feature/auth',
    pid: null,
    title: 'Test session',
    lastUserMessage: 'Add auth module',
    currentActivity: '',
    turnCount: 12,
    startedAt: 1_700_000_000,
    lastActivityAt: 1_700_000_120,
    model: 'claude-sonnet-4-5-20250929',
    tokens: { inputTokens: 10000, outputTokens: 5000, cacheReadTokens: 0, cacheCreationTokens: 0, totalTokens: 15000 },
    contextWindowTokens: 100000,
    cost: { totalUsd: 2.34, inputCostUsd: 1.50, outputCostUsd: 0.84, cacheReadCostUsd: 0, cacheCreationCostUsd: 0, cacheSavingsUsd: 0 },
    cacheStatus: 'warm',
    ...overrides,
  }
}

describe('KanbanSidePanel', () => {
  it('renders session header with project and branch', () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    expect(screen.getByText('test-project')).toBeInTheDocument()
    expect(screen.getByText('feature/auth')).toBeInTheDocument()
  })

  it('renders 4 tab buttons', () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    expect(screen.getByRole('tab', { name: /terminal/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /sub-agents/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /timeline/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /cost/i })).toBeInTheDocument()
  })

  it('shows Terminal tab by default', () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    expect(screen.getByTestId('terminal-pane')).toBeInTheDocument()
  })

  it('switches to Cost tab on click', async () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('tab', { name: /cost/i }))
    expect(screen.getByText('Total Cost')).toBeInTheDocument()
  })

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn()
    render(<KanbanSidePanel session={makeSession()} onClose={onClose} />)
    await userEvent.click(screen.getByRole('button', { name: /close/i }))
    expect(onClose).toHaveBeenCalled()
  })

  it('calls onClose when Escape pressed', async () => {
    const onClose = vi.fn()
    render(<KanbanSidePanel session={makeSession()} onClose={onClose} />)
    await userEvent.keyboard('{Escape}')
    expect(onClose).toHaveBeenCalled()
  })

  it('defaults to Sub-Agents tab when session has sub-agents', () => {
    const session = makeSession({
      subAgents: [
        { toolUseId: 'toolu_01', agentType: 'Explore', description: 'Search', status: 'running', startedAt: 1_700_000_000 },
      ],
    })
    render(<KanbanSidePanel session={session} onClose={vi.fn()} />)
    // Sub-Agents tab should be active (not Terminal)
    const subAgentsTab = screen.getByRole('tab', { name: /sub-agents/i })
    expect(subAgentsTab).toHaveAttribute('aria-selected', 'true')
  })
})
```

**Step 2: Implement the component**

**D.2 Gate:** This component conditionally imports `SubAgentDrillDown` (D.2 Task 8). If D.2 is not yet complete, implement WITHOUT the drill-down import and render SwimLanes in display-only mode (no `onDrillDown`). The code below shows the FULL version (with D.2). If D.2 is incomplete, remove the `SubAgentDrillDown` import, `drillDownAgent` state, and `handleDrillDown` callback, and render SwimLanes without `onDrillDown`.

```tsx
// src/components/live/KanbanSidePanel.tsx
import { useState, useEffect, useCallback } from 'react'
import { X, Terminal, Users, BarChart3, DollarSign, GitBranch } from 'lucide-react'
import type { LiveSession } from './use-live-sessions'
import { RichTerminalPane } from './RichTerminalPane'
import { SwimLanes } from './SwimLanes'
import { TimelineView } from './TimelineView'
import { CostBreakdown } from './CostBreakdown'
// D.2 GATE: Uncomment when Phase D.2 Task 8 is complete:
// import { SubAgentDrillDown } from './SubAgentDrillDown'
import { cn } from '../../lib/utils'

type TabId = 'terminal' | 'sub-agents' | 'timeline' | 'cost'

const TABS: { id: TabId; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { id: 'terminal', label: 'Terminal', icon: Terminal },
  { id: 'sub-agents', label: 'Sub-Agents', icon: Users },
  { id: 'timeline', label: 'Timeline', icon: BarChart3 },
  { id: 'cost', label: 'Cost', icon: DollarSign },
]

interface KanbanSidePanelProps {
  session: LiveSession
  onClose: () => void
}

export function KanbanSidePanel({ session, onClose }: KanbanSidePanelProps) {
  const hasSubAgents = session.subAgents && session.subAgents.length > 0
  const [activeTab, setActiveTab] = useState<TabId>(hasSubAgents ? 'sub-agents' : 'terminal')
  const [verboseMode, setVerboseMode] = useState(false)

  // D.2 GATE: Uncomment when SubAgentDrillDown is available:
  // const [drillDownAgent, setDrillDownAgent] = useState<{
  //   agentId: string; agentType: string; description: string
  // } | null>(null)

  // Reset tab when session changes
  useEffect(() => {
    setActiveTab(hasSubAgents ? 'sub-agents' : 'terminal')
    // D.2 GATE: Also setDrillDownAgent(null) when drill-down is enabled
  }, [session.id, hasSubAgents])

  // Escape to close
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        // D.2 GATE: Check drillDownAgent first, setDrillDownAgent(null) if active
        onClose()
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [onClose])

  // D.2 GATE: Uncomment when SwimLanes has onDrillDown prop:
  // const handleDrillDown = useCallback((agentId: string, agentType: string, description: string) => {
  //   setDrillDownAgent({ agentId, agentType, description })
  // }, [])

  return (
    <div className="flex flex-col h-full bg-gray-950 border border-gray-800 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-800 bg-gray-900">
        <span className="text-sm font-medium text-gray-100 truncate">{session.projectDisplayName || session.project}</span>
        {session.gitBranch && (
          <span className="inline-flex items-center gap-1 text-xs font-mono text-gray-500 truncate max-w-[160px]">
            <GitBranch className="w-3 h-3 flex-shrink-0" />
            {session.gitBranch}
          </span>
        )}
        <div className="flex-1" />
        <span className="text-xs font-mono text-gray-400 tabular-nums">${session.cost.totalUsd.toFixed(2)}</span>
        <span className="text-xs text-gray-500">{session.turnCount} turns</span>
        <button
          onClick={onClose}
          aria-label="Close side panel"
          className="text-gray-500 hover:text-gray-300 transition-colors p-1"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-gray-800" role="tablist">
        {TABS.map((tab) => {
          const Icon = tab.icon
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={activeTab === tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                'flex items-center gap-1.5 px-4 py-2.5 text-xs font-medium transition-colors border-b-2',
                activeTab === tab.id
                  ? 'border-indigo-500 text-indigo-400'
                  : 'border-transparent text-gray-500 hover:text-gray-400',
              )}
            >
              <Icon className="w-3.5 h-3.5" />
              {tab.label}
            </button>
          )
        })}
      </div>

      {/* Tab content */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {activeTab === 'terminal' && (
          <RichTerminalPane
            sessionId={session.id}
            isVisible={true}
            verboseMode={verboseMode}
          />
        )}

        {activeTab === 'sub-agents' && (
          <div className="p-4 overflow-y-auto h-full">
            {/* D.2 GATE: When D.2 Task 8 is complete, add drill-down support:
                if (drillDownAgent) render <SubAgentDrillDown ... />
                else render SwimLanes with onDrillDown={handleDrillDown} */}
            {hasSubAgents ? (
              <SwimLanes
                subAgents={session.subAgents!}
                sessionActive={session.status === 'working'}
                /* D.2 GATE: Add onDrillDown={handleDrillDown} when D.2 SwimLanes has the prop */
              />
            ) : (
              <p className="text-sm text-gray-500 text-center py-8">No sub-agents in this session</p>
            )}
          </div>
        )}

        {activeTab === 'timeline' && (
          <div className="p-4 overflow-y-auto h-full">
            {hasSubAgents && session.startedAt ? (
              <TimelineView
                subAgents={session.subAgents!}
                sessionStartedAt={session.startedAt}
                sessionDurationMs={
                  session.status === 'done'
                    ? (session.lastActivityAt - session.startedAt) * 1000
                    : Date.now() - session.startedAt * 1000
                }
              />
            ) : (
              <p className="text-sm text-gray-500 text-center py-8">No timeline data available</p>
            )}
          </div>
        )}

        {activeTab === 'cost' && (
          <CostBreakdown cost={session.cost} subAgents={session.subAgents} />
        )}
      </div>
    </div>
  )
}
```

**Step 3: Run tests**

Run: `bun run vitest run src/components/live/KanbanSidePanel.test.tsx`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/components/live/KanbanSidePanel.tsx src/components/live/KanbanSidePanel.test.tsx
git commit -m "feat(ui): add KanbanSidePanel with Terminal, Sub-Agents, Timeline, Cost tabs"
```

---

## Task 7: Wire Side Panel into MissionControlPage

**Files:**
- Modify: `src/pages/MissionControlPage.tsx`
- Modify: `src/components/live/KanbanView.tsx`

This is the **critical wiring task**. All the components are built — now connect them.

**Step 1: Update MissionControlPage Kanban section**

In `MissionControlPage.tsx`, add these imports at the top:

```tsx
import { KanbanSidePanel } from '../components/live/KanbanSidePanel'
import { cn } from '../lib/utils'  // NOT already imported — must add
```

Replace the Kanban view rendering block (lines 195-197):

```tsx
// BEFORE:
{viewMode === 'kanban' && (
  <KanbanView sessions={filteredSessions} selectedId={selectedId} onSelect={handleSelectSession} stalledSessions={stalledSessions} currentTime={currentTime} />
)}

// AFTER:
{viewMode === 'kanban' && (
  <div className="flex gap-4">
    <div className={cn(
      'transition-all duration-200',
      selectedId ? 'w-2/5 min-w-[300px]' : 'w-full',
    )}>
      <KanbanView
        sessions={filteredSessions}
        selectedId={selectedId}
        onSelect={handleSelectSession}
        onCardClick={handleSelectSession}
        stalledSessions={stalledSessions}
        currentTime={currentTime}
      />
    </div>
    {selectedId && (() => {
      const session = filteredSessions.find(s => s.id === selectedId)
      if (!session) return null
      return (
        <div className="w-3/5 min-w-[400px] h-[calc(100vh-220px)]">
          <KanbanSidePanel
            key={session.id}
            session={session}
            onClose={() => setSelectedId(null)}
          />
        </div>
      )
    })()}
  </div>
)}
```

**Step 2: Pass onCardClick through KanbanView**

In `KanbanView.tsx`, add `onCardClick` to props (done in Task 4) and pass to KanbanColumn:

```tsx
<KanbanColumn
  key={col.group}
  // ... existing props ...
  onCardClick={onCardClick}
/>
```

**Step 3: Type check**

Run: `bun run typecheck`
Expected: No errors

**Step 4: Manual verification**

1. Start dev server: `bun dev`
2. Open Mission Control → Kanban view
3. Click a session card → side panel opens on the right, Kanban compresses to left
4. Verify tabs switch correctly
5. Click a different card → panel updates to new session
6. Click close (✕) or press Escape → panel closes, Kanban returns to full width
7. Verify: clicking a card does NOT navigate to `/session/{id}`

**Step 5: Commit**

```bash
git add src/pages/MissionControlPage.tsx src/components/live/KanbanView.tsx
git commit -m "feat(ui): wire KanbanSidePanel into MissionControlPage with split layout"
```

---

## Task 8: Tests & Verification

**Files:** None (verification only)

**Step 1: Run all frontend tests**

```bash
bun run vitest run src/components/live/
bun run vitest run src/pages/
```

Expected: ALL PASS

**Step 2: Type check**

```bash
bun run typecheck
```

Expected: No errors

**Step 3: Verify MonitorView is unaffected**

1. Switch to Monitor view → verify expanded pane + terminal streaming still work
2. If D2 is implemented: verify sub-agent drill-down works in Monitor view

**Step 4: Verify all card views**

1. Grid view: cards still navigate on click (Link behavior preserved)
2. List view: cards still navigate on click
3. Kanban view: cards open side panel (div behavior)
4. Monitor view: unchanged

**Step 5: Final commit (if fixes needed)**

```bash
git add -A
git commit -m "fix(live): address Phase D3 verification issues"
```

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `src/components/live/RichTerminalPane.tsx` | Shared terminal stream component (extracted from MonitorView) |
| `src/components/live/CostBreakdown.tsx` | Cost breakdown table for Cost tab |
| `src/components/live/CostBreakdown.test.tsx` | Tests for CostBreakdown |
| `src/components/live/KanbanSidePanel.tsx` | 4-tab side panel for Kanban session detail |
| `src/components/live/KanbanSidePanel.test.tsx` | Tests for KanbanSidePanel |
| `src/components/live/SessionCard.test.tsx` | Tests for pulse dot + sub-agent pills |

### Modified Files

| File | Change |
|------|--------|
| `src/components/live/MonitorView.tsx` | Remove local RichTerminalPane, import from shared file |
| `src/components/live/SessionCard.tsx` | Add pulse dot, SubAgentPills, `onClickOverride` prop |
| `src/components/live/KanbanColumn.tsx` | Add `onCardClick` prop, pass to SessionCard |
| `src/components/live/KanbanView.tsx` | Add `onCardClick` prop, pass to KanbanColumn |
| `src/pages/MissionControlPage.tsx` | Kanban flex layout with KanbanSidePanel |

### Dependencies

No new npm dependencies. All components reuse existing libraries (Lucide, Tailwind).

---

## Task Dependency Chain

```
Task 1 (extract RichTerminalPane) ─┐
Task 2 (pulse dot)                 │─── independent, can run in parallel
Task 3 (sub-agent pills)          │
Task 5 (CostBreakdown)            ─┘
                                    │
Task 4 (SessionCard click override) ─── independent, but needed by Task 7
                                    │
Task 6 (KanbanSidePanel) ──────────┘── depends on Tasks 1, 5 (imports them)
                                    │   Sub-Agents drill-down gated on D.2
                                    │
Task 7 (wiring) ────────────────────── depends on Tasks 4, 6 (plugs everything in)
                                    │
Task 8 (verification) ─────────────── depends on all
```

**Parallel opportunities:** Tasks 1-5 can all be built in parallel. Task 6 depends on 1 and 5. Task 7 depends on 4 and 6.

**D.2 Gate:** Task 6 can be built immediately with display-only SwimLanes (no drill-down). When D.2 ships Tasks 7-8 (`useSubAgentStream` + `SubAgentDrillDown`), a follow-up PR adds: (1) `SubAgentDrillDown` import, (2) `drillDownAgent` state, (3) `onDrillDown` callback to SwimLanes.

---

## Acceptance Criteria

- [ ] Clicking a Kanban card opens side panel (no navigation)
- [ ] Side panel shows 4 tabs: Terminal, Sub-Agents, Timeline, Cost
- [ ] Terminal tab streams live conversation via WebSocket
- [ ] Sub-Agents tab shows SwimLanes with click-to-drill-down (D2 components)
- [ ] Timeline tab shows Gantt chart (D TimelineView)
- [ ] Cost tab shows breakdown (main agent vs sub-agents)
- [ ] Default tab: Sub-Agents if session has sub-agents, else Terminal
- [ ] Escape key closes panel (or exits drill-down first)
- [ ] Panel closes when clicking ✕ button
- [ ] Selecting different card updates panel (no stale state)
- [ ] Running cards show pulsing green dot (respects `prefers-reduced-motion`)
- [ ] SubAgentPills displayed inline on cards (with activity from D2)
- [ ] Kanban compresses to ~40% width when panel is open
- [ ] Grid/List/Monitor views unaffected (no regression)
- [ ] RichTerminalPane extracted and working in both MonitorView and side panel
- [ ] All existing frontend tests pass

---

## Rollback

All changes are additive. No existing behavior is modified (SessionCard's Link rendering is default, override is opt-in).

**To roll back completely:**
1. Revert all commits from this phase
2. `RichTerminalPane.tsx` deletion requires restoring the local function in `MonitorView.tsx`
3. `SessionCard.tsx` changes are backward-compatible — removing `onClickOverride` prop just removes the div path

**To roll back partially (keep card enhancements, revert side panel):**
1. Keep Tasks 2-3 (pulse dot, SubAgentPills on cards)
2. Revert Tasks 4, 6-7 (click override, side panel, wiring)
3. Task 1 (extract RichTerminalPane) and Task 5 (CostBreakdown) are standalone

---

## Changelog

| Date | Change | Author |
|------|--------|--------|
| 2026-02-17 | Initial draft (D3 v1): 2-part solution with Session Detail View | Claude (Opus 4.6) |
| 2026-02-17 | Redesigned (D3 v2): Kanban-focused side panel, dropped mobile/shortcuts, updated dependency to D.2 | Claude (Opus 4.6) |
| 2026-02-17 | Audit fixes (D3 v3): 3 Blockers, 2 Warnings, 3 Minor issues fixed | Claude (Opus 4.6) |

## Changelog of Fixes Applied (Audit -> Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `agentState` missing required `confidence` and `source` fields in all test `makeSession()`/`createMockSession()` helpers | Blocker | Added `confidence: 1.0, source: 'jsonl'` to all `agentState` literals in Task 2, 3, and 6 test code |
| 2 | `SubAgentDrillDown` does not exist (D.2 pending) — import fails at build | Blocker | Added D.2 Gate pattern: commented-out import, Sub-Agents tab renders SwimLanes in display-only mode without drill-down |
| 3 | `SwimLanes` has no `onDrillDown` prop (D.2 pending) — TypeScript error | Blocker | Removed `onDrillDown` from SwimLanes call; added D.2 GATE comment documenting when to add it |
| 4 | Double-click: KanbanColumn wrapper `onClick` + SessionCard `onClickOverride` both fire | Warning | Added `e.stopPropagation()` to SessionCard's div `onClick` when using `onClickOverride`; documented interaction with wrapper div |
| 5 | `makeSession` name conflicts with existing `createMockSession` in SessionCard.test.tsx | Warning | Changed Task 2/3 to reuse existing `createMockSession` and `renderCard` helpers; removed duplicate helper definition |
| 6 | `cn` not imported in MissionControlPage | Minor | Already noted in plan; flagged for visibility |
| 7 | `pid: 1234` vs project convention `pid: null` | Minor | Changed to `pid: null` in Task 6 makeSession |
| 8 | `onExpand` omission on SubAgentPills not documented | Minor | Added comment explaining intentional omission (no drill-down from card context) |
| 9 | Task 1: Ambiguous import removal guidance — `useState`/`useCallback` must be KEPT; `RichPane`/`parseRichMessage`/`ConnectionState` must be REMOVED | Blocker | Explicitly listed which imports to keep (4) and which to remove (4) with line numbers |
| 10 | Task 7: `cn` import missing from code block — buried in footnote | Warning | Moved `cn` import into the Task 7 Step 1 code block alongside `KanbanSidePanel` import |
