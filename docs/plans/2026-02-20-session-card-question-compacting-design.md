# Session Card: AskUserQuestion & Compacting UI

**Date:** 2026-02-20
**Status:** Approved

## Summary

Add two dedicated UI elements to the Mission Control session card:
1. **Inline question preview** when Claude asks the user a question (`AskUserQuestion`)
2. **Context gauge compacting overlay** when the session is compacting context (`PreCompact`)

Both leverage existing backend data — no Rust changes needed.

## Motivation

- `AskUserQuestion`: The card already goes amber with "Asked you a question", but the operator has no idea *what* is being asked. Showing the question text enables triage without context-switching to the terminal. Future-proofs for web-controlled sessions where answers can be submitted from the UI.
- `Compacting`: Explains why the context gauge value drops and why the agent appears "thinking" but not doing useful work. Currently indistinguishable from normal thinking state.

## Design

### 1. AskUserQuestion — Inline Question Card

**Trigger:** `agentState.state === 'awaiting_input'` AND `agentState.context?.questions` exists.

**Placement:** Between spinner row (4) and task progress (5) in the session card layout — highest-priority actionable item.

```
┌─────────────────────────────────────────────┐
│ [my-project] [main]                  $0.42  │
│ Add authentication to the app               │
│ ⟳ 12s · 4.2k tokens · Sonnet 4.6           │
│                                             │
│ ┌─ ◉ Question ─────────────────────────┐    │
│ │ Which auth method should we use?     │    │
│ │                                      │    │
│ │  ○ JWT tokens        ○ OAuth 2.0     │    │
│ │  ○ Session cookies                   │    │
│ │                                      │    │
│ │  ⌨ Answer in terminal               │    │
│ └──────────────────────────────────────┘    │
│                                             │
│ Tasks 3/5  ████████░░░░                     │
│ [E ⟳] [C ✓]   2 agents (1 active)         │
│ ▓▓▓▓▓▓░░░░░░░░░░░ 42% context   12 turns   │
└─────────────────────────────────────────────┘
```

**Visual details:**
- Amber left-border accent: `border-l-4 border-amber-400 dark:border-amber-500`
- Background: `bg-amber-50/50 dark:bg-amber-900/10`
- Rounded corners, `p-3`, `mb-2`
- Header: `◉ Question` in amber, small badge
- Question text: `text-sm font-medium text-gray-900 dark:text-gray-100`, `line-clamp-2`
- Options: Read-only radio-style chips, `text-xs text-gray-600 dark:text-gray-400`, 2-column grid, max 4 shown
- Footer: `⌨ Answer in terminal` in `text-[10px] text-gray-400 dark:text-gray-500` — placeholder for future interactive controls
- Multi-question: Show `"1 of N questions"` badge when `questions.length > 1`

**Data source:** `agentState.context` already carries the full `AskUserQuestion` tool input:
```typescript
context.questions: Array<{
  question: string
  header: string
  options: Array<{ label: string; description: string }>
  multiSelect: boolean
}>
```

### 2. Session Compacting — Context Gauge Overlay

**Trigger:** `agentState.state === 'thinking'` AND `agentState.label` contains "compacting" (case-insensitive).

**Placement:** Integrated into the existing `ContextGauge` component — not a separate element.

```
During:   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░ 89% ← ◊ compacting...
After:    ▓▓▓▓▓▓▓░░░░░░░░░░ 38% ↓ compacted         (fades out after 5s)
```

**Visual details:**
- During compacting:
  - Append label after percentage: `◊ compacting...`
  - Icon: `Minimize2` from Lucide, `motion-safe:animate-pulse`
  - Color: `text-blue-500 dark:text-blue-400`
  - Gauge bar: subtle `animate-pulse` to indicate value is changing
- After compacting (state transitions away):
  - Brief `↓ compacted` label in `text-green-500 dark:text-green-400`
  - Fade-out after 5s via CSS `animation-delay` + `animate-out fade-out-0`
  - Track "just compacted" via a `useRef` that stores previous label

**Data source:** `agentState.label` already contains "Compacting context..." or "Auto-compacting context..." — pure string check, no backend changes.

### 3. Updated Card Layout Order

```
1. Header: [project] [branch]        [$cost]
2. Title: latest user prompt
3. Last message (if different)
4. Spinner row (duration, tokens, model)
5. ★ AskUserQuestion card (conditional)
6. Task progress list
7. Sub-agent pills
8. Context gauge (★ with compacting overlay)
9. Footer: turn count
```

## Implementation Scope

### Frontend only — no backend changes

| Component | Action |
|-----------|--------|
| `QuestionCard.tsx` | New component — parses `agentState.context.questions`, renders inline card |
| `SessionCard.tsx` | Import `QuestionCard`, render between spinner and task progress |
| `ContextGauge.tsx` | Add compacting state detection, label overlay, post-compact fade |

### Files touched

- `src/components/live/QuestionCard.tsx` — NEW
- `src/components/live/SessionCard.tsx` — add QuestionCard slot
- `src/components/live/ContextGauge.tsx` — add compacting overlay

## Future Evolution

When web-controlled sessions ship, the `QuestionCard` options become clickable. The layout is already designed for this:
- Radio chips become actual `<button>` elements
- "Answer in terminal" footer becomes a submit action
- The card sends the selected option back via API
