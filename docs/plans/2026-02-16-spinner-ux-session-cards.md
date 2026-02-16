---
status: draft
date: 2026-02-16
---

# Spinner UX for Session Cards

## Context

Claude Code's TUI shows delightful status indicators during agent execution:

```
✶ Effecting…          12m 34s · ↑ 18.3k tokens · thought for 3s
✢ Dilly-dallying…     32s · ↓ 1.2k tokens · thought for 3s
✽ Noodling…           5m 53s · ↑ 26.4k tokens
```

These are NOT exposed via hooks, SSE, JSONL, or any API. The verbs are randomly selected from a list of 155 whimsical words. The unicode symbols are animation frames cycling at 50-200ms. The metadata (tokens, duration, "thought for") IS real and IS available from our data sources.

We want to bring this aesthetic to our session cards — both in Mission Control (live) and the Sessions page (historical). The goal is visual consistency with the Claude Code experience, plus real data our users can't get from the TUI alone (cost, cache status, context usage).

## Design

### Two Modes: Live vs Historical

The spinner component operates in two modes depending on context:

**Live mode** (Mission Control — `LiveSession` data):
```
✶ Noodling…     12m 34s · ↑ 18.3k tokens · claude-opus-4-6
```
- Animated unicode spinner (cycling frames)
- Random verb, fixed for the session lifetime
- Duration ticks up in real-time
- Token count updates on each SSE event
- Stall detection: spinner turns red after 3s of no activity

**Historical mode** (Sessions page — `SessionInfo` data):
```
· Worked     45m · 23.1k tokens · claude-opus-4-6
```
- Static dot (no animation)
- Past-tense verb (from 8-word list) or just "Worked"
- Final duration and token count
- No stall detection (session is over)

### Shared Component: `<SessionSpinner />`

A single component used by both card types, with a discriminated union type. This ensures historical mode callers don't need to pass metrics props (duration, tokens) that are never displayed in that mode:

```tsx
interface BaseSpinnerProps {
  model: string | null           // LiveSession.model or SessionInfo.primaryModel
}

interface LiveSpinnerProps extends BaseSpinnerProps {
  mode: 'live'
  durationSeconds: number        // elapsed seconds (from shared currentTime - startedAt)
  totalTokens: number            // input + output (pre-computed by caller)
  inputTokens: number            // for arrow direction (↑ input-heavy, ↓ output-heavy)
  outputTokens: number           // for arrow direction
  isStalled?: boolean            // no activity for >3s (from useLiveSessions)
  agentStateGroup?: 'needs_you' | 'autonomous' | 'delivered'  // inlined to avoid spinner/ → live/ import coupling
  spinnerVerb?: string           // assigned verb (stable per session)
}

interface HistoricalSpinnerProps extends BaseSpinnerProps {
  mode: 'historical'
  pastTenseVerb?: string         // "Worked", "Baked", etc.
}

type SessionSpinnerProps = LiveSpinnerProps | HistoricalSpinnerProps
```

**Why a discriminated union?** The flat interface required historical mode callers to pass `durationSeconds`, `totalTokens`, `inputTokens`, and `outputTokens` even though historical mode renders only verb + model. The discriminated union makes the type system enforce that only live mode requires metrics props.

**Null handling:** Callers must handle nullable source fields before passing to this component:
- `LiveSession.startedAt` is `number | null` — pass `0` when null (duration shows "0s")
- `SessionInfo.totalInputTokens` is `number | null` — pass `0` when null (only relevant for live mode)
- `SessionInfo.primaryModel` is `string | null` — pass through as-is (component hides model when null)

### Verb Assignment

**Live sessions:** Assign a random verb when the session is first discovered. The verb must be **stable** — same session always shows the same verb for its lifetime. Use session ID as a seed for deterministic selection:

```tsx
function pickVerb(sessionId: string): string {
  // Simple hash of session ID → index into SPINNER_VERBS
  let hash = 0
  for (let i = 0; i < sessionId.length; i++) {
    hash = ((hash << 5) - hash + sessionId.charCodeAt(i)) | 0
  }
  return SPINNER_VERBS[Math.abs(hash) % SPINNER_VERBS.length]
}
```

This means: same session ID = same verb across page refreshes, reconnects, etc. No server-side storage needed.

**Historical sessions:** Use the shorter past-tense list, also seeded by session ID:

```tsx
const PAST_TENSE_VERBS = [
  'Baked', 'Brewed', 'Churned', 'Cogitated',
  'Cooked', 'Crunched', 'Sauteed', 'Worked'
]
```

### Animation

The spinner animation cycles through 6 unicode frames in a forward-reverse pattern:

```
Frames: · ✢ ✳ ✶ ✻ ✽ ✻ ✶ ✳ ✢ · ✢ ...
```

Speed depends on agent state:
- `autonomous` group → 200ms per frame (normal work)
- `needs_you` group → no animation (static amber dot)
- `delivered` group → no animation (static checkmark)

When `isStalled` is true (no SSE update for >3s), the spinner character turns red with increasing intensity (matches Claude Code's behavior).

Respects `prefers-reduced-motion`: shows a static blinking dot instead of cycling frames.

### Stall Detection

Stall tracking is added **inside** `useLiveSessions` (not a separate hook) because it needs access to SSE event timestamps. Creating a separate hook would require either a second `EventSource` connection (wasteful) or complex state sharing.

**Changes to `src/components/live/use-live-sessions.ts`:**

```tsx
// New state: track last SSE event time per session
const lastEventTimes = useRef<Map<string, number>>(new Map())
const [stalledSessions, setStalledSessions] = useState<Set<string>>(new Set())

// In each SSE event handler (session_discovered, session_updated), add:
lastEventTimes.current.set(session.id, Date.now())

// In session_completed handler, add:
lastEventTimes.current.delete(sessionId)

// New 1s interval to compute stall status
// (Step 6 merges currentTime tick into this same interval — see Step 6 for final version):
useEffect(() => {
  const interval = setInterval(() => {
    const now = Date.now()
    const stalled = new Set<string>()
    for (const [id, lastTime] of lastEventTimes.current.entries()) {
      if (now - lastTime > 3000) stalled.add(id)
    }
    setStalledSessions(stalled)
    setCurrentTime(Math.floor(now / 1000))  // shared clock tick (Step 6)
  }, 1000)
  return () => clearInterval(interval)
}, [])

// Updated return type:
return { sessions: sessionList, summary, isConnected, lastUpdate, stalledSessions, currentTime }
```

This is purely frontend — no backend changes needed. The stall indicator tells the operator "we haven't heard from this session in a while" which could mean the agent is doing extended thinking, a long tool execution, or the process died.

**Note:** The backend process detector polls every 5 seconds, but JSONL file changes arrive in real-time via `notify` watcher. So the 3-second stall threshold is meaningful — it catches pauses in JSONL activity, not backend polling gaps.

### Metrics Format

Token display uses the Claude Code TUI format (lowercase `k`, 1 decimal):

```tsx
function formatTokensCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return `${n}`
}

// Arrow direction: ↑ for input-heavy (tokens going up to Claude), ↓ for output-heavy
const arrow = inputTokens > outputTokens ? '↑' : '↓'
```

**Note:** The existing `formatNumber()` in `src/lib/format-utils.ts` uses uppercase `K` (e.g., `18.3K`). The spinner intentionally uses lowercase `k` to match Claude Code's TUI aesthetic. Both are valid — they serve different contexts.

**Duration format:** No shared `formatDuration()` utility exists — there are 12+ local implementations with different signatures. The spinner will use a new `formatDurationCompact(seconds)` function in `src/components/spinner/constants.ts` that outputs Claude Code TUI format (`12m`, `2h 15m`).

**Model name** is truncated to the short variant. A new `formatModelShort()` function handles this:

```tsx
// "claude-opus-4-6" → "opus-4.6"
// "claude-sonnet-4-5-20250929" → "sonnet-4.5"
// "claude-sonnet-4-20250514" → "sonnet-4"
// null → hidden (component omits model segment)
function formatModelShort(model: string | null): string | null {
  if (!model) return null
  // Modern: claude-{family}-{major}[-{minor}][-{date}]
  const m = model.match(/^claude-([a-z]+)-(\d+)(?:-(\d{1,2}))?(?:-\d{8})?$/)
  if (m) {
    const [, family, major, minor] = m
    return minor ? `${family}-${major}.${minor}` : `${family}-${major}`
  }
  // Legacy: claude-{major}[-{minor}]-{family}[-{date}]
  const l = model.match(/^claude-(\d+)(?:-(\d{1,2}))?-([a-z]+)(?:-\d{8})?$/)
  if (l) {
    const [, major, minor, family] = l
    return minor ? `${family}-${major}.${minor}` : `${family}-${major}`
  }
  return model  // non-Claude models pass through
}
```

**Note:** The existing `formatModelName()` in `src/lib/format-model.ts` does the opposite (expands to "Claude Opus 4.6"). The regex patterns here mirror the ones in that file to stay consistent.

### Integration Points

**Mission Control `SessionCard` (`src/components/live/SessionCard.tsx`):**

Replace the **conditional state badge block** (lines 121-134) with `<SessionSpinner />`.

The current code renders conditionally based on `agentState.group`:
```tsx
// CURRENT (lines 121-134) — to be replaced:
{session.agentState.group === 'needs_you' ? (
  <div className="mb-2"><StateBadge agentState={session.agentState} /></div>
) : session.currentActivity ? (
  <div className="text-xs text-green-600 truncate mb-2">{session.currentActivity}</div>
) : session.agentState.group === 'delivered' ? (
  <div className="mb-2"><StateBadge agentState={session.agentState} /></div>
) : null}
```

Replace with:
```tsx
// NEW — spinner replaces the entire conditional block:
<SessionSpinner
  mode="live"
  durationSeconds={elapsedSeconds}
  totalTokens={session.tokens.totalTokens}
  inputTokens={session.tokens.inputTokens}
  outputTokens={session.tokens.outputTokens}
  model={session.model}
  isStalled={stalledSessions.has(session.id)}
  agentStateGroup={session.agentState.group}
  spinnerVerb={pickVerb(session.id)}
/>
```

**What happens to StateBadge?** The spinner component itself adapts to the agent state group:
- `autonomous` → animated spinner + verb ("✶ Noodling…")
- `needs_you` → static amber dot + state label ("● Awaiting input")
- `delivered` → static checkmark + "Done"

This means `StateBadge` is no longer rendered in the card body. The status dot in the header (line 77-84) is unchanged.

```
Before (actual current layout):
┌────────────────────────────────────────┐
│ 🟢 my-project  feature/auth     $0.42 │
│ "Add authentication to the API"        │
│ [StateBadge] or [currentActivity text] │  ← conditional, sometimes empty
│ ████████░░ 65% context                 │
│ 5 turns  45m                           │
└────────────────────────────────────────┘

After:
┌────────────────────────────────────────┐
│ 🟢 my-project  feature/auth     $0.42 │
│ "Add authentication to the API"        │
│ ✶ Noodling…   45m · ↑18.3k · opus-4.6 │  ← always visible spinner row
│ ████████░░ 65% context                 │
│ 5 turns                                │  ← duration moves to spinner
└────────────────────────────────────────┘
```

**Footer change:** Remove `{duration && <span>{duration}</span>}` from the footer (line 146) since duration now lives in the spinner row. Keep "X turns" only.

**Sessions page `SessionCard` (`src/components/SessionCard.tsx`):**

Add a spinner row between the preview text and the metrics row.

**Important: avoid duplicate data.** The header already shows duration (line 217: `formatDuration(durationSeconds)`) and the metrics row already shows tokens (line 249: `formatNumber(totalTokens) tokens`). The spinner row adds **only** the verb + model — data not shown elsewhere:

```
Before (actual current layout):
┌──────────────────────────────────────────────────────┐
│ 📁 my-project  🌿 main   Today 2:30 PM -> 3:15 PM | 45 min │
│ "Add authentication to the API"                      │
│ 5 prompts · 23.1K tokens · 3 files · 1 re-edit      │
│ +142 / -28                                           │
│ 📝 auth.ts · middleware.ts  +2 more                  │
│ [CategoryBadge] [2 commits]                          │
└──────────────────────────────────────────────────────┘

After:
┌──────────────────────────────────────────────────────┐
│ 📁 my-project  🌿 main   Today 2:30 PM -> 3:15 PM | 45 min │
│ "Add authentication to the API"                      │
│ · Brewed · opus-4.6                                  │  ← spinner row (verb + model only)
│ 5 prompts · 23.1K tokens · 3 files · 1 re-edit      │
│ +142 / -28                                           │
│ 📝 auth.ts · middleware.ts  +2 more                  │
│ [CategoryBadge] [2 commits]                          │
└──────────────────────────────────────────────────────┘
```

The spinner row is a one-liner that adds personality (whimsical verb) + context (which model was used). Duration and tokens are NOT repeated since they're already visible. When `primaryModel` is null, the entire spinner row is hidden (no value in showing just `· Brewed` with no model).

---

## Implementation Steps

### Step 1: Create SPINNER_VERBS constant and utility functions

**New file:** `src/components/spinner/constants.ts`

- Export `SPINNER_VERBS`: array of 155 verbs (extracted from Claude Code)
- Export `PAST_TENSE_VERBS`: array of 8 past-tense verbs
- Export `SPINNER_FRAMES`: `['·', '✢', '✳', '✶', '✻', '✽']`
- Export `pickVerb(sessionId: string): string` — deterministic verb selection
- Export `pickPastVerb(sessionId: string): string` — deterministic past-tense selection
- Export `formatTokensCompact(n: number): string` — "18.3k" format (lowercase k, 1 decimal — matches Claude Code TUI, intentionally different from `formatNumber()` in `src/lib/format-utils.ts` which uses uppercase K)
- Export `formatModelShort(model: string | null): string | null` — "opus-4.6" format (new function, mirrors regex patterns from `src/lib/format-model.ts` but outputs short form instead of expanded)
- Export `formatDurationCompact(seconds: number): string` — "12m", "2h 15m" format (new canonical function — 12+ inconsistent local implementations exist across components)

### Step 2: Create `<SessionSpinner />` component

**New file:** `src/components/spinner/SessionSpinner.tsx`

- Renders: `[animated-frame] [verb]…    [duration] · [arrow][tokens] · [model]`
- **Live mode:** `✶ Noodling…   45m · ↑18.3k · opus-4.6`
- **Historical mode:** `· Brewed · opus-4.6` (no duration/tokens — already shown elsewhere in the card)
- Props: `SessionSpinnerProps` (see Design section above)
- Uses `useRef` + `requestAnimationFrame` for frame animation (cleanup on unmount). Codebase convention uses RAF over `setInterval` — see `TerminalPane.tsx` and `BottomSheet.tsx` for patterns.
- **RAF throttling pattern** — RAF fires at 60fps (~16ms), not the 200ms we need. Throttle by tracking `lastFrameTime` and only advancing the frame index when 200ms has elapsed:
  ```tsx
  const lastFrameTime = useRef(0)
  const frameIndex = useRef(0)
  const BOUNCE_SEQUENCE = [0, 1, 2, 3, 4, 5, 4, 3, 2, 1] // indices into SPINNER_FRAMES

  useEffect(() => {
    if (mode !== 'live' || agentStateGroup !== 'autonomous') return
    let rafId: number

    function tick(now: number) {
      if (now - lastFrameTime.current >= 200) {
        lastFrameTime.current = now
        frameIndex.current = (frameIndex.current + 1) % BOUNCE_SEQUENCE.length
        setFrame(SPINNER_FRAMES[BOUNCE_SEQUENCE[frameIndex.current]])
      }
      rafId = requestAnimationFrame(tick)
    }
    rafId = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(rafId)
  }, [mode, agentStateGroup])
  ```
  This keeps the RAF loop running but only triggers a state update (and re-render) every 200ms. The `BOUNCE_SEQUENCE` array encodes the forward-reverse pattern from the Animation section.
- Respects `prefers-reduced-motion` via existing `useMediaQuery` hook from `src/hooks/use-media-query.ts`
- Stall state: spinner character transitions to red (CSS transition, 2s ramp)
- Agent state group drives animation on/off
- Tailwind classes for layout: `flex gap-2 text-xs`, `font-mono tabular-nums` for metrics (see `CostTooltip.tsx` line 75 for precedent)

### Step 3: Add stall tracking to `useLiveSessions`

**Modify:** `src/components/live/use-live-sessions.ts`

- Add `lastEventTimes` ref (`useRef<Map<string, number>>`) — updated in SSE event handlers
- Add `stalledSessions` state (`useState<Set<string>>`) — computed by 1s interval
- Update `session_discovered` and `session_updated` handlers to record `Date.now()` per session ID
- Update `session_completed` handler to clean up the entry
- Add new `useEffect` with 1s `setInterval` that computes stall set (threshold: 3 seconds)
- **Update `UseLiveSessionsResult` interface** (lines 49-54):
  ```tsx
  export interface UseLiveSessionsResult {
    sessions: LiveSession[]
    summary: LiveSummary | null
    isConnected: boolean
    lastUpdate: Date | null
    stalledSessions: Set<string>  // ← NEW (Step 3)
    currentTime: number           // ← NEW (Step 6) — unix seconds, ticks every 1s
  }
  ```
- Add `stalledSessions` and `currentTime` to the return statement
- **No new file** — stall detection lives where the SSE events live

### Step 4: Integrate into Mission Control `SessionCard`

**Modify:** `src/components/live/SessionCard.tsx`

- Import `SessionSpinner`, `pickVerb` from `'../spinner'`
- **Delete** the entire conditional state badge block (lines 121-134):
  ```tsx
  // DELETE this block:
  {session.agentState.group === 'needs_you' ? (
    <div className="mb-2"><StateBadge agentState={session.agentState} /></div>
  ) : session.currentActivity ? (
    <div className="text-xs text-green-600 dark:text-green-400 truncate mb-2">
      {session.currentActivity}
    </div>
  ) : session.agentState.group === 'delivered' ? (
    <div className="mb-2"><StateBadge agentState={session.agentState} /></div>
  ) : null}
  ```
- **Replace with:**
  ```tsx
  <SessionSpinner
    mode="live"
    durationSeconds={elapsedSeconds}
    totalTokens={session.tokens.totalTokens}
    inputTokens={session.tokens.inputTokens}
    outputTokens={session.tokens.outputTokens}
    model={session.model}
    isStalled={stalledSessions.has(session.id)}
    agentStateGroup={session.agentState.group}
    spinnerVerb={pickVerb(session.id)}
  />
  ```
- Compute elapsed duration inline: `const elapsedSeconds = currentTime - (session.startedAt ?? currentTime)` — no hook needed, uses `currentTime` from `useLiveSessions`
- **Add `stalledSessions` and `currentTime` to `SessionCardProps`:**
  ```tsx
  interface SessionCardProps {
    session: LiveSession
    stalledSessions?: Set<string>  // from useLiveSessions()
    currentTime: number            // from useLiveSessions() — unix seconds, ticks every 1s
  }
  ```
- **Remove** `{duration && <span>{duration}</span>}` from footer (line 146) — duration now in spinner
- **Remove** the local `formatDuration()` function (lines 152-165) — replaced by spinner's `formatDurationCompact`
- Keep status dot color driven by `session.agentState.group` (unchanged)
- **Do NOT remove `StateBadge` or its `export`** — `ListView.tsx` imports it at line 8 (`import { StateBadge } from './SessionCard'`) and uses it at line 199

### Step 5: Integrate into Sessions page `SessionCard`

**Modify:** `src/components/SessionCard.tsx`

- Import `SessionSpinner`, `pickPastVerb` from `'./spinner'`
- Add `<SessionSpinner mode="historical" />` row between the preview text `<div>` (line 226-237) and the metrics row `<div>` (line 239)
- **Historical mode renders only verb + model** (no duration/tokens — already visible in header and metrics)
- **Drive-by fix (bigint type confusion at line 152):** The existing code does:
  ```tsx
  // CURRENT (line 152) — BROKEN: `as bigint` is a TS assertion, NOT a runtime conversion.
  // If totalInputTokens is a non-null number, `number + bigint` throws TypeError at runtime.
  const totalTokens = ((session?.totalInputTokens ?? 0n) as bigint) + ((session?.totalOutputTokens ?? 0n) as bigint)
  const hasTokens = totalTokens > 0n
  ```
  Replace with safe `number` arithmetic:
  ```tsx
  // FIX: use number arithmetic, not bigint. SessionInfo fields are number | null, never bigint.
  const totalTokens = (session?.totalInputTokens ?? 0) + (session?.totalOutputTokens ?? 0)
  const hasTokens = totalTokens > 0
  ```
  This must be fixed as a drive-by in the same PR since we're modifying this file anyway. The `as bigint` assertion silently passes TypeScript but throws at runtime whenever `totalInputTokens` is a non-null `number` (which is the common case).
- Pass props (wrapped in visibility guard, using the discriminated union — historical mode needs only `model` and `pastTenseVerb`):
  ```tsx
  {session.primaryModel && (
    <SessionSpinner
      mode="historical"
      model={session.primaryModel}
      pastTenseVerb={pickPastVerb(session.id)}
    />
  )}
  ```
- **Note:** With the discriminated union type, historical mode no longer requires `durationSeconds`, `totalTokens`, `inputTokens`, or `outputTokens` props.
- **Note:** `totalInputTokens` and `totalOutputTokens` are `number | null` on `SessionInfo`. The `?? 0` fallback on the `totalTokens` computation above handles this.
- **Visibility rule:** Only render the spinner row when `session.primaryModel` is truthy. A spinner showing just `· Brewed` with no model adds no value.
- No stall detection (historical mode)

### Step 6: Add `currentTime` to `useLiveSessions` for duration computation (replaces per-card timers)

**Modify:** `src/components/live/use-live-sessions.ts` (same file as Step 3)

Instead of creating a `useLiveDuration` hook that spawns one `setInterval` per `SessionCard` (N timers for N sessions), piggyback duration computation onto the **same 1-second interval** already added in Step 3 for stall detection.

- Add `currentTime` state (`useState<number>(() => Math.floor(Date.now() / 1000))`)
- In the existing 1s `setInterval` callback (from Step 3's stall detection), add: `setCurrentTime(Math.floor(Date.now() / 1000))`
- Add `currentTime` to the `UseLiveSessionsResult` interface:
  ```tsx
  export interface UseLiveSessionsResult {
    sessions: LiveSession[]
    summary: LiveSummary | null
    isConnected: boolean
    lastUpdate: Date | null
    stalledSessions: Set<string>  // from Step 3
    currentTime: number           // ← NEW: unix seconds, ticks every 1s
  }
  ```
- Each `SessionCard` computes elapsed duration inline: `currentTime - (session.startedAt ?? currentTime)` — zero additional timers
- **Updated interval callback** (merges stall detection + clock tick):
  ```tsx
  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now()
      // Stall detection (from Step 3)
      const stalled = new Set<string>()
      for (const [id, lastTime] of lastEventTimes.current.entries()) {
        if (now - lastTime > 3000) stalled.add(id)
      }
      setStalledSessions(stalled)
      // Clock tick for duration computation (replaces per-card timers)
      setCurrentTime(Math.floor(now / 1000))
    }, 1000)
    return () => clearInterval(interval)
  }, [])
  ```

**Why this is better than `useLiveDuration`:**
- **1 timer total** instead of N timers (one per visible SessionCard)
- **1 state update** propagates to all cards via React context/props, instead of N independent `setState` calls each causing isolated re-renders
- Duration computation is a trivial inline subtraction — no hook needed
- The 1s interval already exists for stall detection, so this is zero marginal cost

**No new file created.** The `src/components/spinner/use-live-duration.ts` file is no longer needed.

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/components/spinner/constants.ts` | Verb lists, frames, `pickVerb`, `pickPastVerb`, `formatTokensCompact`, `formatModelShort`, `formatDurationCompact` |
| `src/components/spinner/SessionSpinner.tsx` | Shared spinner component (live + historical modes) |
| `src/components/spinner/index.ts` | Barrel export |

## Files to Modify

| File | Change |
|------|--------|
| `src/components/live/use-live-sessions.ts` | Add stall tracking (`lastEventTimes` ref, `stalledSessions` state). Add `currentTime` state for shared clock tick. Merge both into the single 1s interval (Step 3 + Step 6). Update `UseLiveSessionsResult` interface with `stalledSessions` and `currentTime`. |
| `src/components/live/SessionCard.tsx` | Add `stalledSessions?: Set<string>` and `currentTime: number` to `SessionCardProps`. Replace conditional state badge block (lines 121-134) with `<SessionSpinner mode="live" />`. Compute `elapsedSeconds` inline from `currentTime`. Remove duration from footer. Remove local `formatDuration`. **Keep `StateBadge` exported** (used by `ListView.tsx`). |
| `src/pages/MissionControlPage.tsx` | Destructure `stalledSessions` and `currentTime` from `useLiveSessions()`. Pass both to `<SessionCard>` in grid view (line 187) and `<KanbanView>` (line 198). |
| `src/components/live/KanbanView.tsx` | Add `stalledSessions?: Set<string>` and `currentTime: number` to `KanbanViewProps`. Pass through to `<KanbanColumn>`. |
| `src/components/live/KanbanColumn.tsx` | Add `stalledSessions?: Set<string>` and `currentTime: number` to `KanbanColumnProps`. Pass through to `<SessionCard>`. |
| `src/components/SessionCard.tsx` | Add `<SessionSpinner mode="historical" />` row between preview and metrics (guarded by `session.primaryModel`). Fix `bigint` type confusion at line 152 (drive-by). |

## No Backend Changes

This is purely frontend. All data is already available:
- `LiveSession.tokens.{inputTokens,outputTokens,totalTokens}`, `.model`, `.startedAt`, `.agentState.group` (Mission Control)
- `SessionInfo.durationSeconds`, `.totalInputTokens`, `.totalOutputTokens`, `.primaryModel` (Sessions page — all nullable, guard with `?? 0`/`?? null`)

---

## What This Plan Does NOT Do

| Deferred | Why |
|----------|-----|
| Capture actual Claude Code TUI status | Not exposed via any API — would require screen scraping |
| Sync verb with Claude Code's chosen verb | Claude Code picks randomly at session start, doesn't persist it |
| Show "thought for Xs" | Extended thinking duration not reliably available in JSONL yet. Neither `LiveSession` nor `SessionInfo` has a `thinkingSeconds` field. |
| Custom verb settings | Claude Code supports `spinnerVerbs` in settings — we could read that later |
| Spinner in CompactSessionTable | Table view is data-dense, spinner adds clutter — defer |
| Consolidate `formatDuration` implementations | 12+ local implementations exist across components. This plan adds one more (`formatDurationCompact`) rather than refactoring all existing ones. A separate cleanup task should consolidate them. |
| Audit other `bigint` / `as` assertions in SessionCard.tsx | The `as bigint` bug at line 152 is fixed as a drive-by in Step 5, but other `as` type assertions in the file may hide similar runtime mismatches. A separate audit should check all `as` casts. |

## Success Criteria

- Mission Control session cards show animated spinner with random verb + real metrics
- Sessions page cards show static past-tense verb + final metrics
- Same session ID always shows the same verb (deterministic)
- Spinner respects `prefers-reduced-motion`
- Spinner turns red when session hasn't sent an update in >3s
- No backend changes, no new API calls
- All existing tests still pass

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Live SessionCard has conditional state badge (`needs_you`→StateBadge, `autonomous`+activity→green text, etc.), not a simple "State:" row as plan described | **Blocker** | Rewrote Integration Points section with actual code (lines 121-134) and explicit replacement strategy. Added "Before" diagram matching actual layout. |
| 2 | Historical SessionCard already shows duration in header (line 217) and tokens in metrics row (line 249) — spinner row would duplicate both | **Blocker** | Changed historical mode to render only verb + model (`· Brewed · opus-4.6`). Duration/tokens omitted. Visibility gated on `primaryModel` being truthy. |
| 3 | Plan referenced `session.tokens` without specifying actual field names. Actual: `.tokens.inputTokens`, `.tokens.outputTokens`, `.tokens.totalTokens` | **Blocker** | Added `inputTokens` and `outputTokens` to `SessionSpinnerProps`. Updated all code blocks with correct field paths. |
| 4 | Plan said "Duration format matches existing `formatDuration()` utility" but no shared utility exists — 12+ local implementations with different signatures (some take seconds, some take two timestamps) | **Blocker** | Added `formatDurationCompact(seconds)` to Step 1 exports. Documented the fragmentation. Added cleanup to "Deferred" table. |
| 5 | `useSpinnerStall` as separate hook would need its own SSE connection or complex state sharing with `useLiveSessions` | **Blocker** | Eliminated separate hook. Step 3 now modifies `useLiveSessions` directly. Added `stalledSessions` to its return type. |
| 6 | `formatModelShort()` doesn't exist. Existing `formatModelName()` in `src/lib/format-model.ts` does the opposite (expands to "Claude Opus 4.6") | **Blocker** | Added full `formatModelShort()` implementation in Metrics Format section with regex patterns mirroring `format-model.ts`. |
| 7 | `SessionInfo.totalInputTokens`, `.totalOutputTokens`, `.primaryModel` are all `number | null` / `string | null`. Plan didn't handle nulls. | **Warning** | Added null handling notes to interface section. Updated Step 5 with `?? 0` / `?? null` fallbacks. |
| 8 | Token format inconsistency: plan uses `18.3k` (lowercase), existing `formatNumber()` uses `18.3K` (uppercase) | **Warning** | Added explicit note in Metrics Format section documenting intentional difference for Claude Code TUI aesthetic. |
| 9 | `LiveSession.startedAt` is `number | null`, plan didn't specify fallback | **Warning** | Added to null handling notes. Duration shows "0s" when null. |
| 10 | `thinkingSeconds` doesn't exist on either `LiveSession` or `SessionInfo` | **Warning** | Removed from `SessionSpinnerProps` interface. Updated "Deferred" table with explanation. |
| 11 | Plan used `setInterval` for animation but codebase convention uses `requestAnimationFrame` (see `TerminalPane.tsx`, `BottomSheet.tsx`) | **Minor** | Updated Step 2 to use RAF. Added codebase pattern references. |
| 12 | Step 4 said "Pass token/model data from `session.tokens` and `session.model`" without explicit field mapping | **Minor** | Rewrote Step 4 with full JSX code block showing exact prop mapping. |
| 13 | `stalledSessions` data flow unresolved — returned from `useLiveSessions` but never threaded to `SessionCard` via Grid/Kanban views | **Blocker** | Added `stalledSessions` to `SessionCardProps`. Added `MissionControlPage.tsx`, `KanbanView.tsx`, `KanbanColumn.tsx` to Files to Modify with explicit prop threading. |
| 14 | `StateBadge` removal would break `ListView.tsx` line 8 import | **Blocker** | Changed guidance to "Do NOT remove `StateBadge` or its export". |
| 15 | `UseLiveSessionsResult` interface update not shown | **Warning** | Added full interface diff to Step 3 showing the new `stalledSessions: Set<string>` field. |
| 16 | `AgentStateGroup` import from `spinner/` to `live/types` creates undocumented cross-boundary coupling | **Warning** | Inlined the type as `'needs_you' | 'autonomous' | 'delivered'` in `SessionSpinnerProps`. |
| 17 | Step 5 JSX code block omits the `{session.primaryModel && (...)}` conditional wrapper | **Warning** | Wrapped the JSX code block in the visibility guard. |
| 18 | One `setInterval` per SessionCard from `useLiveDuration` — N timers for N sessions, each causing independent re-renders | **Minor** | Eliminated `useLiveDuration` hook entirely. Step 6 rewritten to piggyback a `currentTime` state onto the existing 1s stall-detection interval in `useLiveSessions`. Cards compute `currentTime - startedAt` inline — 1 timer total, zero additional hooks. Removed `use-live-duration.ts` from Files to Create. |
| 19 | Historical mode receives unused props (`durationSeconds`, `totalTokens`, `inputTokens`, `outputTokens`) that are never displayed | **Minor** | Replaced flat `SessionSpinnerProps` interface with a discriminated union: `LiveSpinnerProps | HistoricalSpinnerProps`. Historical mode requires only `model` and `pastTenseVerb`. Updated Step 5 JSX to pass only the required historical props. |
| 20 | Pre-existing `bigint` type confusion at `SessionCard.tsx:152` — `as bigint` assertion does NOT convert `number` to `bigint` at runtime, causing `TypeError` on `number + bigint` | **Warning** | Added drive-by fix in Step 5: replace `bigint` arithmetic with `number` arithmetic. Added audit of other `as` casts to Deferred table. |
| 21 | Step 2 says "uses RAF" but doesn't show how to throttle 60fps RAF to 200ms frame intervals | **Minor** | Added full throttled RAF code pattern to Step 2 using `lastFrameTime` ref with 200ms threshold and `BOUNCE_SEQUENCE` array for forward-reverse animation. |
