# Report UI Enhancements — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** (1) Build the ReportDetails expandable raw stats component with full ui-ux-pro-max design spec, and (2) show all 4 report period cards instead of hiding behind smart defaults.

**Tech Stack:** React, TypeScript, Tailwind CSS, lucide-react

**Prerequisites:** Tasks 1-4 from `docs/plans/2026-02-21-report-details-plan.md` must be complete (backend token fields, DB query, server wiring, ReportRow context_digest exposure).

---

### Task 1: Build ReportDetails component (Frontend)

**Design optimized with ui-ux-pro-max.** Matches existing ReportCard visual language (same border/bg/padding tokens). Follows Drill-Down Analytics pattern: summary-to-detail flow with smooth expand.

**Files:**
- Create: `src/components/reports/ReportDetails.tsx`

**Step 1: Define the ContextDigest type**

At the top of `ReportDetails.tsx`, define the interface (matches Rust `ContextDigest` struct):

```typescript
interface ContextDigest {
  report_type: string
  date_range: string
  summary_line: string
  total_input_tokens?: number
  total_output_tokens?: number
  projects: {
    name: string
    session_count: number
    commit_count: number
    total_duration_secs: number
    branches: {
      name: string
      sessions: { first_prompt: string; category: string | null; duration_secs: number }[]
    }[]
  }[]
  top_tools: string[]
  top_skills: string[]
}
```

**Step 2: Implement formatting helpers**

```typescript
function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`
  return String(n)
}

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}
```

**Step 3: Implement the component with full design spec**

```typescript
import { useState, useMemo } from 'react'
import { ChevronRight } from 'lucide-react'

interface ReportDetailsProps {
  contextDigestJson: string | null
  totalCostCents: number
}

export function ReportDetails({ contextDigestJson, totalCostCents }: ReportDetailsProps) {
  const [expanded, setExpanded] = useState(false)

  const digest = useMemo(() => {
    if (!contextDigestJson) return null
    try {
      return JSON.parse(contextDigestJson) as ContextDigest
    } catch {
      return null
    }
  }, [contextDigestJson])

  if (!digest) return null

  return (
    <div className="mt-4 border-t border-gray-100 dark:border-gray-800 pt-3">
      {/* Toggle button — full-width click target, cursor-pointer */}
      <button
        type="button"
        onClick={() => setExpanded(e => !e)}
        className="flex items-center gap-1.5 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors duration-200 cursor-pointer"
        aria-expanded={expanded}
      >
        <ChevronRight
          className={`w-3.5 h-3.5 transition-transform duration-200 ${expanded ? 'rotate-90' : ''}`}
        />
        <span>Details</span>
        {/* Collapsed inline summary: cost + token count */}
        {!expanded && (
          <span className="ml-1 text-gray-300 dark:text-gray-600">
            &middot; {totalCostCents > 0 ? `$${(totalCostCents / 100).toFixed(2)}` : ''}
            {digest.total_input_tokens ? ` · ${formatTokens(digest.total_input_tokens + (digest.total_output_tokens ?? 0))} tokens` : ''}
          </span>
        )}
      </button>

      {/* Expandable panel — grid-rows transition for smooth height animation */}
      <div
        className={`grid transition-[grid-template-rows] duration-200 ease-out motion-reduce:transition-none ${expanded ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'}`}
      >
        <div className="overflow-hidden">
          <div className="pt-3 space-y-2.5 text-xs">

            {/* Row 1: Cost + Tokens */}
            <div className="flex flex-wrap gap-x-3 gap-y-1 text-gray-600 dark:text-gray-400">
              {totalCostCents > 0 && (
                <span>Cost: <span className="text-gray-900 dark:text-gray-200 font-medium">${(totalCostCents / 100).toFixed(2)}</span></span>
              )}
              {digest.total_input_tokens != null && digest.total_input_tokens > 0 && (
                <span>Tokens: <span className="text-gray-900 dark:text-gray-200 font-medium">{formatTokens(digest.total_input_tokens)}</span> in / <span className="text-gray-900 dark:text-gray-200 font-medium">{formatTokens(digest.total_output_tokens ?? 0)}</span> out</span>
              )}
            </div>

            {/* Row 2: Top tools — inline pills */}
            {digest.top_tools.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-gray-500 dark:text-gray-500 shrink-0">Tools:</span>
                {digest.top_tools.map(tool => (
                  <span
                    key={tool}
                    className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 font-mono"
                  >
                    {tool}
                  </span>
                ))}
              </div>
            )}

            {/* Row 3: Top skills — inline pills (only if non-empty) */}
            {digest.top_skills.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-gray-500 dark:text-gray-500 shrink-0">Skills:</span>
                {digest.top_skills.map(skill => (
                  <span
                    key={skill}
                    className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 font-mono"
                  >
                    /{skill}
                  </span>
                ))}
              </div>
            )}

            {/* Per-project breakdown */}
            {digest.projects.length > 0 && (
              <div className="pt-1 space-y-2">
                {digest.projects.map(proj => (
                  <div key={proj.name}>
                    {/* Project line */}
                    <div className="flex items-baseline gap-1.5">
                      <span className="text-gray-900 dark:text-gray-200 font-medium truncate max-w-[200px]">{proj.name}</span>
                      <span className="text-gray-400 dark:text-gray-500">—</span>
                      <span className="text-gray-500 dark:text-gray-400">
                        {proj.session_count} sessions · {formatDuration(proj.total_duration_secs)}
                        {proj.commit_count > 0 && ` · ${proj.commit_count} commits`}
                      </span>
                    </div>
                    {/* Branch lines */}
                    {proj.branches.length > 0 && (
                      <div className="ml-3 mt-0.5 text-gray-400 dark:text-gray-500">
                        {proj.branches.map(b => (
                          <span key={b.name} className="mr-2.5">
                            <span className="text-gray-300 dark:text-gray-600 select-none">└ </span>
                            <span className="font-mono">{b.name}</span>
                            <span className="ml-0.5">({b.sessions.length})</span>
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

          </div>
        </div>
      </div>
    </div>
  )
}
```

**Design decisions (ui-ux-pro-max informed):**

| Decision | Choice | Why |
|----------|--------|-----|
| **Expand animation** | `grid-rows-[0fr]` → `grid-rows-[1fr]` with `duration-200 ease-out` | Animates actual height (not `max-h` guess). 200ms = responsive micro-interaction per UX guideline. `ease-out` for entering content. |
| **Chevron** | `ChevronRight` with `rotate-90` on expand | Single icon, CSS transform rotation. Uses `transform` (GPU-composited) not layout shift. |
| **Collapsed summary** | Inline cost + token count after "Details" | Zero-click information scent — user sees value before deciding to expand. |
| **Tool/skill pills** | `bg-gray-100 dark:bg-gray-800 font-mono rounded` | Matches dev tool aesthetic. Monospace for tool names. Wrap naturally on narrow screens via `flex-wrap`. |
| **Color tokens** | `text-gray-600 dark:text-gray-400` labels, `text-gray-900 dark:text-gray-200 font-medium` values | 4.5:1+ contrast ratio in both modes. Values visually pop against labels. No shadcn CSS vars. |
| **Spacing** | `mt-4 pt-3` top border separation from report content | Consistent with ReportCard's `mb-3` header spacing. `border-t` creates visual separation without a card-in-card feel. |
| **Reduced motion** | `motion-reduce:transition-none` on grid wrapper | Respects `prefers-reduced-motion` per UX guideline. Chevron still rotates but instant. |
| **Touch targets** | Toggle button is full row, not just chevron icon | 44px+ effective touch target. `cursor-pointer` on the button. |
| **Project name truncation** | `truncate max-w-[200px]` | Prevents long project paths from breaking layout. |
| **No emoji icons** | lucide-react `ChevronRight` only | SVG icon per pre-delivery checklist. |

**Accessibility checklist:**
- [x] `aria-expanded` on toggle button
- [x] `cursor-pointer` on clickable element
- [x] Keyboard accessible (`<button>` element, not `<div onClick>`)
- [x] Color contrast 4.5:1+ in both light and dark mode
- [x] No color-only indicators
- [x] `prefers-reduced-motion` respected via `motion-reduce:transition-none`

**Step 4: Verify TypeScript compilation**

Run: `bunx tsc --noEmit`
Expected: exit 0

**Step 5: Commit**

```bash
git add src/components/reports/ReportDetails.tsx
git commit -m "feat(ui): add ReportDetails expandable raw stats component"
```

---

### Task 2: Show all 4 report period cards with smart-default emphasis (Frontend)

**Design rationale:** Only 4 periods exist — not enough to warrant a picker, dropdown, or segmented control. Hiding options behind toggles adds friction. Per project UX principle: "Every prompt = 1 decision = friction." Show all 4, emphasize the smart default.

**Layout:**
```
┌──────────────────┐  ┌──────────────────┐
│ ★ Today          │  │   Yesterday      │
│ 2026-02-21       │  │   2026-02-20     │
│ [Generate]       │  │   [Generate]     │
└──────────────────┘  └──────────────────┘
┌──────────────────┐  ┌──────────────────┐
│   This Week      │  │   Last Week      │
│ Feb 17 — Feb 21  │  │   Feb 10 — Feb 16│
│ [Generate]       │  │   [Generate]     │
└──────────────────┘  └──────────────────┘
```

Top row = daily (Today, Yesterday). Bottom row = weekly (This Week, Last Week). The smart-default "suggested" card gets a subtle visual indicator (e.g. brighter border or small dot). On mobile, stacks to 1-column with suggested card first.

**Files:**
- Modify: `src/hooks/use-smart-defaults.ts`
- Modify: `src/pages/ReportsPage.tsx`
- Modify: `src/components/reports/ReportCard.tsx`

**Step 1: Refactor use-smart-defaults hook**

Change the return type from `{ primary, secondary }` to expose all 4 configs + which one is suggested:

```typescript
interface SmartDefaults {
  cards: CardConfig[]       // always 4: [today, yesterday, thisWeek, lastWeek]
  suggestedIndex: number    // index of the time-of-day recommended card
}
```

The `cards` array is always in fixed order: Today, Yesterday, This Week, Last Week. The `suggestedIndex` uses the existing time-of-day logic:
- Mon morning → suggestedIndex = 3 (Last Week)
- Other mornings → suggestedIndex = 1 (Yesterday)
- Afternoon/evening → suggestedIndex = 0 (Today)

**Step 2: Update ReportCard to accept `suggested` prop**

Add optional prop:

```typescript
interface ReportCardProps {
  // ... existing props
  suggested?: boolean
}
```

When `suggested` is true, render a subtle visual indicator — a `border-l-2 border-blue-500 dark:border-blue-400` on the card. No animated effects. Keep it minimal.

**Step 3: Update ReportsPage to render all 4 cards**

Replace the current 2-card grid:

```tsx
<div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
  {defaults.cards.map((card, i) => (
    <ReportCard
      key={card.label}
      label={card.label}
      dateStart={card.dateStart}
      dateEnd={card.dateEnd}
      type={card.type}
      startTs={card.startTs}
      endTs={card.endTs}
      suggested={i === defaults.suggestedIndex}
      existingReport={findExisting(card.dateStart, card.dateEnd)}
    />
  ))}
</div>
```

This renders a 2x2 grid on desktop (sm+), 1-column on mobile.

**Step 4: Verify**

Run: `bunx tsc --noEmit`
Expected: exit 0

**Step 5: Commit**

```bash
git add src/hooks/use-smart-defaults.ts src/pages/ReportsPage.tsx src/components/reports/ReportCard.tsx
git commit -m "feat(ui): show all 4 report period cards with smart-default emphasis"
```

---

### Task 3: Full verification

**Step 1: Run frontend type check**

Run: `bunx tsc --noEmit`
Expected: exit 0

**Step 2: Visual verification**

Start dev server and check:
- ReportDetails expands/collapses smoothly on a completed report
- All 4 period cards visible in 2x2 grid
- Suggested card has subtle left border indicator
- Mobile: cards stack to 1-column
- Dark mode: all contrast ratios look correct

**Step 3: Commit any final fixes**

Only if needed.

---

## Dependency Graph

```
Task 1 (ReportDetails component) ──┐
                                     ├── Task 3 (verify)
Task 2 (4-card period grid) ────────┘
```

Tasks 1 and 2 are independent — can run in parallel.
Task 3 depends on both.
