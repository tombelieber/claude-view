---
status: approved
date: 2026-02-16
---

# Indicator Simplification — Kill Ring, Kill Card Dot, Inline Countdown

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce session card visual indicators from 3 circles (status dot + spinner + cache ring) to 1 (spinner only), replacing the cache ring with inline countdown text in the SessionSpinner.

**Architecture:** Remove `CacheCountdownRing` component entirely. Remove the header status dot from Board/Grid/Monitor card views (keep in ListView). Add `lastActivityAt` prop to `SessionSpinner` and render countdown text inline with the "Awaiting input" label.

**Tech Stack:** React, TypeScript, Tailwind CSS

---

### Task 1: Add countdown text to SessionSpinner needs_you branch

**Files:**
- Modify: `src/components/spinner/SessionSpinner.tsx:14-26` (props), `src/components/spinner/SessionSpinner.tsx:113-121` (needs_you render)

**Step 1: Add `lastActivityAt` prop to `LiveSpinnerProps`**

```typescript
// src/components/spinner/SessionSpinner.tsx, line 18-26
interface LiveSpinnerProps extends BaseSpinnerProps {
  mode: 'live'
  durationSeconds: number
  inputTokens: number
  outputTokens: number
  isStalled?: boolean
  agentStateGroup?: 'needs_you' | 'autonomous'
  spinnerVerb?: string
  lastActivityAt?: number  // ← ADD: Unix timestamp for cache countdown
}
```

**Step 2: Add countdown helper and modify needs_you branch**

Replace lines 113-121 with:

```typescript
  // needs_you -> amber dot + "Awaiting input" + cache countdown
  if (agentStateGroup === 'needs_you') {
    const CACHE_TTL = 300
    const lastActivity = (props as LiveSpinnerProps).lastActivityAt ?? 0
    const elapsed = lastActivity > 0 ? Math.floor(Date.now() / 1000) - lastActivity : CACHE_TTL
    const remaining = Math.max(0, CACHE_TTL - elapsed)

    const countdownText = remaining > 0
      ? `${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, '0')}`
      : 'cache cold'

    const countdownColor = remaining <= 0
      ? 'text-gray-400'
      : remaining < 60
        ? 'text-red-500'
        : remaining < 180
          ? 'text-amber-500'
          : 'text-green-500'

    return (
      <span className="flex items-center gap-1.5 text-xs">
        <span className="w-3 text-center inline-block text-amber-500">●</span>
        <span className="text-muted-foreground">{(props as LiveSpinnerProps).spinnerVerb ? `${(props as LiveSpinnerProps).spinnerVerb}` : 'Awaiting input'}</span>
        <span className={`font-mono tabular-nums ${countdownColor}`}>
          · {countdownText}
        </span>
      </span>
    )
  }
```

Wait — the countdown needs to tick every second. The SessionSpinner already has a `useEffect` interval for the spinner frame animation (lines 66-78). We need to make the needs_you branch also re-render every second.

**Better approach:** Use the existing `frame` state (already ticks every 120ms for spinner animation) to trigger re-computation of the countdown. The countdown will recalculate on each frame tick. Since the countdown only needs second precision and the frame ticks at 120ms, this is more than sufficient and avoids adding a second interval.

The `frame` state is at line 66-78 and already runs for all live mode renders. The needs_you branch early-returns at line 114 before `frame` is used, but `frame` is still updating via the effect. We just need to reference something that changes — `frame` itself is enough since React will re-render when it changes.

Actually, looking again at the component: the `useEffect` for frame animation (lines 66-78) runs unconditionally for live mode. The `frame` state variable causes re-renders every 120ms. Since the needs_you branch is part of the same component, it will re-render too. The countdown `Date.now()` call happens on each render, so it will naturally update.

But 120ms ticks are wasteful just for a countdown that changes every second. Let's add a dedicated `secondsTick` state that updates every 1000ms only when `agentStateGroup === 'needs_you'`:

**Revised Step 2: Add countdown state and render**

After the existing `useEffect` for frame animation (~line 78), add:

```typescript
  // Countdown tick for needs_you cache TTL display (1s resolution)
  const [, setTick] = useState(0)
  useEffect(() => {
    if (agentStateGroup !== 'needs_you') return
    const interval = setInterval(() => setTick(t => t + 1), 1000)
    return () => clearInterval(interval)
  }, [agentStateGroup])
```

Then replace the needs_you branch (lines 113-121):

```typescript
  // needs_you -> amber dot + status label + cache countdown
  if (agentStateGroup === 'needs_you') {
    const CACHE_TTL = 300
    const lastActivity = (props as LiveSpinnerProps).lastActivityAt ?? 0
    const elapsed = lastActivity > 0 ? Math.floor(Date.now() / 1000) - lastActivity : CACHE_TTL
    const remaining = Math.max(0, CACHE_TTL - elapsed)

    const countdownText = remaining > 0
      ? `${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, '0')}`
      : 'cache cold'

    const countdownColor = remaining <= 0
      ? 'text-gray-400'
      : remaining < 60
        ? 'text-red-500'
        : remaining < 180
          ? 'text-amber-500'
          : 'text-green-500'

    return (
      <span className="flex items-center gap-1.5 text-xs">
        <span className="w-3 text-center inline-block text-amber-500">●</span>
        <span className="text-muted-foreground">Awaiting input</span>
        <span className={`font-mono tabular-nums ${countdownColor}`}>· {countdownText}</span>
      </span>
    )
  }
```

**Note on hooks:** The `useState` + `useEffect` for tick MUST be placed before any conditional returns (before the `if (agentStateGroup === 'needs_you')` check) to satisfy React's rules of hooks. Place it after line 78 (after the frame animation effect), before line 100 (before the live mode destructuring).

**Step 3: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: PASS (no errors)

**Step 4: Commit**

```bash
git add src/components/spinner/SessionSpinner.tsx
git commit -m "feat(spinner): add inline cache countdown to needs_you state"
```

---

### Task 2: Pass `lastActivityAt` to SessionSpinner from SessionCard

**Files:**
- Modify: `src/components/live/SessionCard.tsx:128-139`

**Step 1: Add the prop**

In SessionCard.tsx, the SessionSpinner usage (lines 129-138) — add `lastActivityAt`:

```tsx
<SessionSpinner
  mode="live"
  durationSeconds={elapsedSeconds}
  inputTokens={session.tokens.inputTokens}
  outputTokens={session.tokens.outputTokens}
  model={session.model}
  isStalled={stalledSessions?.has(session.id)}
  agentStateGroup={session.agentState.group}
  spinnerVerb={pickVerb(session.id)}
  lastActivityAt={session.lastActivityAt}
/>
```

**Step 2: Commit**

```bash
git add src/components/live/SessionCard.tsx
git commit -m "feat(card): pass lastActivityAt to SessionSpinner for countdown"
```

---

### Task 3: Remove status dot from SessionCard header

**Files:**
- Modify: `src/components/live/SessionCard.tsx:78-90`

**Step 1: Remove the dot span and CacheCountdownRing**

Replace lines 78-90:

```tsx
      {/* Header: status dot + badges + cost */}
      <div className="flex items-center gap-2 mb-1">
        <span
          className={cn(
            'inline-block h-2.5 w-2.5 rounded-full flex-shrink-0',
            statusConfig.color,
            statusConfig.pulse && 'animate-pulse'
          )}
          title={statusConfig.label}
        />
        {session.agentState.group === 'needs_you' && (
          <CacheCountdownRing lastActivityAt={session.lastActivityAt} />
        )}
        <div className="flex items-center gap-1.5 min-w-0 flex-1 overflow-hidden">
```

With:

```tsx
      {/* Header: badges + cost */}
      <div className="flex items-center gap-2 mb-1">
        <div className="flex items-center gap-1.5 min-w-0 flex-1 overflow-hidden">
```

**Step 2: Remove unused imports**

Remove `CacheCountdownRing` import (line 10). Remove `GROUP_CONFIG` constant (lines 55-58) and `statusConfig` variable (line 62) if no longer used elsewhere in the file.

Check: `statusConfig` is only used at lines 81-86 (the dot we just removed). So remove:
- Line 10: `import { CacheCountdownRing } from './CacheCountdownRing'`
- Lines 55-58: `const GROUP_CONFIG = { ... }`
- Line 62: `const statusConfig = GROUP_CONFIG[session.agentState.group] || GROUP_CONFIG.autonomous`

**Step 3: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 4: Commit**

```bash
git add src/components/live/SessionCard.tsx
git commit -m "refactor(card): remove status dot and cache ring from card header"
```

---

### Task 4: Remove status dot + cache ring from MonitorPane headers

**Files:**
- Modify: `src/components/live/MonitorPane.tsx:13` (import), `209-222` (FullHeader), `328-340` (CompactHeader)

**Step 1: Remove from FullHeader (lines 209-222)**

Remove lines 209-222 (the `{/* Status dot */}` span and `{/* Cache countdown ring */}` block).

**Step 2: Remove from CompactHeader (lines 328-340)**

Remove lines 328-340 (same pattern — dot span and ring block).

**Step 3: Remove CacheCountdownRing import (line 13)**

**Step 4: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 5: Commit**

```bash
git add src/components/live/MonitorPane.tsx
git commit -m "refactor(monitor): remove status dot and cache ring from pane headers"
```

---

### Task 5: Remove status dot + cache ring from ExpandedPaneOverlay

**Files:**
- Modify: `src/components/live/ExpandedPaneOverlay.tsx:13` (import), `131-144` (header)

**Step 1: Remove status dot and cache ring from header (lines 131-144)**

Remove the `{/* Status dot */}` span (lines 131-139) and `{/* Cache countdown ring */}` block (lines 141-144).

**Step 2: Remove CacheCountdownRing import (line 13)**

**Step 3: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 4: Commit**

```bash
git add src/components/live/ExpandedPaneOverlay.tsx
git commit -m "refactor(overlay): remove status dot and cache ring from expanded overlay"
```

---

### Task 6: Delete CacheCountdownRing component

**Files:**
- Delete: `src/components/live/CacheCountdownRing.tsx`

**Step 1: Verify no remaining imports**

Run: `grep -r "CacheCountdownRing" src/`
Expected: 0 matches (all imports removed in Tasks 3-5)

**Step 2: Delete the file**

```bash
rm src/components/live/CacheCountdownRing.tsx
```

**Step 3: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor(live): delete CacheCountdownRing component — replaced by inline countdown"
```

---

### Task 7: Verify and cleanup

**Step 1: Verify no status dots remain on card views**

```bash
# Should only find StatusDot in ListView.tsx and StatusDot.tsx itself
grep -r "StatusDot" src/
grep -r "groupDotColor" src/
```

The `groupDotColor` function in MonitorPane.tsx and ExpandedPaneOverlay.tsx may now be unused if the dot was the only consumer. If so, remove it.

**Step 2: Full TypeScript check**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 3: Commit any cleanup**

```bash
git add -A
git commit -m "chore: remove unused groupDotColor and imports after indicator simplification"
```

---

## File Change Summary

| Task | Files | Change |
|------|-------|--------|
| 1 | `SessionSpinner.tsx` | Add countdown to needs_you branch |
| 2 | `SessionCard.tsx` | Pass lastActivityAt prop |
| 3 | `SessionCard.tsx` | Remove dot + ring from header |
| 4 | `MonitorPane.tsx` | Remove dot + ring from pane headers |
| 5 | `ExpandedPaneOverlay.tsx` | Remove dot + ring from overlay |
| 6 | `CacheCountdownRing.tsx` | Delete entirely |
| 7 | Various | Cleanup unused code |
| **Total** | **5 files modified, 1 deleted** | |

## Design Rationale

- **3 circles → 1:** Status dot + cache ring removed from cards, only spinner remains
- **Countdown as text:** Unambiguous — "2:34" can only mean time, unlike a circle which Claude Code / Cursor users associate with context windows
- **ListView keeps dot:** Table cells need compact indicators, a colored dot is the right density
- **SessionSpinner is the single source:** It already renders status text for needs_you — adding countdown inline keeps info co-located
