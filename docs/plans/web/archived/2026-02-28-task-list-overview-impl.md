# Task List in Live Monitor — Implementation Plan

> **Status:** DONE (shipped 2026-02-28, 4 commits, shippable audit 100/100)

**Goal:** Show full task list on card hover and as the first section in the side panel Overview tab.

**Architecture:** Pure frontend change. `progressItems` already flows via SSE from the Rust backend. We wire it through `SessionPanelData`, build a new `TasksOverviewSection` component, and add a Radix Tooltip to the existing `TaskProgressList` card component.

**Tech Stack:** React, Radix UI Tooltip (already installed), Tailwind CSS, Lucide icons

**Design doc:** `docs/plans/2026-02-28-task-list-overview-design.md`

---

### Task 0: Wire `progressItems` through `SessionPanelData`

**Files:**
- Modify: `apps/web/src/components/live/session-panel-data.ts:14-71` (interface + adapter)

**Step 1: Add field to `SessionPanelData` interface**

In `apps/web/src/components/live/session-panel-data.ts`, add the import and field:

```ts
// Add import at top (after line 4):
import type { ProgressItem } from '../../types/generated/ProgressItem'

// Add field to SessionPanelData interface (after line 48, after subAgents):
  // Progress items (live tasks/todos)
  progressItems?: ProgressItem[]
```

**Step 2: Pass through in `liveSessionToPanelData`**

In the same file, add to the return object (after line 88, after `subAgents`):

```ts
    progressItems: session.progressItems,
```

**Step 3: Verify TypeScript compiles**

Run: `cd apps/web && npx tsc --noEmit 2>&1 | head -20`
Expected: No errors (or pre-existing errors only — no new ones from this change)

**Step 4: Commit**

```bash
git add apps/web/src/components/live/session-panel-data.ts
git commit -m "feat(live): wire progressItems through SessionPanelData"
```

---

### Task 1: Add Radix Tooltip hover + export constants from `TaskProgressList`

**Files:**
- Modify: `apps/web/src/components/live/TaskProgressList.tsx`

**Step 1: Replace the entire file contents**

Overwrite `apps/web/src/components/live/TaskProgressList.tsx` with the complete final version below. This adds `export` to `STATUS_ICON`/`STATUS_CLASS` (so Task 2 can import them), adds the Radix Tooltip import + wrapper, and adds tooltip styling constants. Full-file replacement avoids line-number drift.

```tsx
import * as Tooltip from '@radix-ui/react-tooltip'
import type { ProgressItem } from '../../types/generated/ProgressItem'

export const STATUS_ICON: Record<string, string> = {
  pending: '◻',
  in_progress: '◼',
  completed: '✓',
}

export const STATUS_CLASS: Record<string, string> = {
  pending: 'text-gray-400 dark:text-gray-500',
  in_progress: 'text-gray-600 dark:text-gray-300',
  completed: 'text-green-500 dark:text-green-400',
}

const TOOLTIP_CONTENT_CLASS =
  'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-sm text-xs'
const TOOLTIP_ARROW_CLASS = 'fill-gray-200 dark:fill-gray-700'

interface TaskProgressListProps {
  items: ProgressItem[]
}

export function TaskProgressList({ items }: TaskProgressListProps) {
  if (items.length === 0) return null

  const completed = items.filter((i) => i.status === 'completed').length
  const total = items.length

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <div className="mb-2 cursor-default">
            <div className="flex items-center gap-1.5 mb-1">
              <span className="text-[10px] font-medium text-gray-500 dark:text-gray-400">
                Tasks {completed}/{total}
              </span>
              {/* Mini progress bar */}
              <div className="flex-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500 dark:bg-green-400 rounded-full transition-all"
                  style={{ width: `${total > 0 ? (completed / total) * 100 : 0}%` }}
                />
              </div>
            </div>
            <ul className="space-y-0.5">
              {items.slice(0, 5).map((item, idx) => {
                const icon = STATUS_ICON[item.status] ?? '◻'
                const colorClass = STATUS_CLASS[item.status] ?? STATUS_CLASS.pending
                const label =
                  item.status === 'in_progress' && item.activeForm ? item.activeForm : item.title
                return (
                  <li key={item.id ?? idx} className="flex items-start gap-1.5 text-xs leading-tight">
                    <span className={`flex-shrink-0 font-mono ${colorClass}`}>{icon}</span>
                    <span
                      className={`truncate ${item.status === 'completed' ? 'text-gray-400 dark:text-gray-500 line-through' : 'text-gray-600 dark:text-gray-300'}`}
                    >
                      {label}
                    </span>
                  </li>
                )
              })}
              {items.length > 5 && (
                <li className="text-[10px] text-gray-400 dark:text-gray-500 pl-4">
                  +{items.length - 5} more
                </li>
              )}
            </ul>
          </div>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
            <div className="font-medium text-gray-900 dark:text-gray-100 mb-2">
              Tasks {completed}/{total}
            </div>
            <ul className="space-y-1 max-h-64 overflow-y-auto">
              {items.map((item, idx) => {
                const icon = STATUS_ICON[item.status] ?? '◻'
                const colorClass = STATUS_CLASS[item.status] ?? STATUS_CLASS.pending
                const label =
                  item.status === 'in_progress' && item.activeForm ? item.activeForm : item.title
                return (
                  <li key={item.id ?? idx} className="flex items-start gap-1.5 leading-tight">
                    <span className={`flex-shrink-0 font-mono ${colorClass}`}>{icon}</span>
                    <span
                      className={`${item.status === 'completed' ? 'text-gray-400 dark:text-gray-500 line-through' : 'text-gray-700 dark:text-gray-300'}`}
                    >
                      {label}
                    </span>
                  </li>
                )
              })}
            </ul>
            <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
```

**Step 2: Verify TypeScript compiles**

Run: `cd apps/web && npx tsc --noEmit 2>&1 | head -20`
Expected: No errors

**Step 3: Verify visually**

Run: `cd apps/web && bun run dev`
Open browser → navigate to live monitor → hover over a session card's task list area.
Expected: Tooltip appears showing ALL tasks with status icons.

**Step 4: Commit**

```bash
git add apps/web/src/components/live/TaskProgressList.tsx
git commit -m "feat(live): add hover tooltip showing all tasks on session card

Also exports STATUS_ICON and STATUS_CLASS for reuse by TasksOverviewSection."
```

---

### Task 2: Create `TasksOverviewSection` component

**Files:**
- Create: `apps/web/src/components/live/TasksOverviewSection.tsx`

**Step 1: Create the component file**

Create `apps/web/src/components/live/TasksOverviewSection.tsx`:

```tsx
import { ListChecks } from 'lucide-react'
import type { ProgressItem } from '../../types/generated/ProgressItem'
import { STATUS_CLASS, STATUS_ICON } from './TaskProgressList'

interface TasksOverviewSectionProps {
  items: ProgressItem[]
}

export function TasksOverviewSection({ items }: TasksOverviewSectionProps) {
  const completed = items.filter((i) => i.status === 'completed').length
  const total = items.length

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
      {/* Header */}
      <div className="flex items-center gap-1.5 mb-2">
        <ListChecks className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
        <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
          Tasks
        </span>
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 tabular-nums ml-auto">
          {completed}/{total}
        </span>
      </div>

      {/* Progress bar */}
      <div className="h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden mb-3">
        <div
          className="h-full bg-green-500 dark:bg-green-400 rounded-full transition-all"
          style={{ width: `${total > 0 ? (completed / total) * 100 : 0}%` }}
        />
      </div>

      {/* Task list */}
      <ul className="space-y-1.5 max-h-[400px] overflow-y-auto">
        {items.map((item, idx) => {
          const icon = STATUS_ICON[item.status] ?? '◻'
          const colorClass = STATUS_CLASS[item.status] ?? STATUS_CLASS.pending
          return (
            <li key={item.id ?? idx} className="flex items-start gap-2 text-xs leading-relaxed">
              <span className={`flex-shrink-0 font-mono mt-0.5 ${colorClass}`}>{icon}</span>
              <div className="min-w-0">
                <span
                  className={
                    item.status === 'completed'
                      ? 'text-gray-400 dark:text-gray-500 line-through'
                      : 'text-gray-700 dark:text-gray-300'
                  }
                >
                  {item.title}
                </span>
                {item.status === 'in_progress' && item.activeForm && (
                  <div className="flex items-center gap-1.5 mt-0.5 text-blue-600 dark:text-blue-400 text-[11px]">
                    <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-500 dark:bg-blue-400 animate-pulse flex-shrink-0" />
                    {item.activeForm}
                  </div>
                )}
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
```

**Step 2: Verify TypeScript compiles**

Run: `cd apps/web && npx tsc --noEmit 2>&1 | head -20`
Expected: No errors

**Step 3: Commit**

```bash
git add apps/web/src/components/live/TasksOverviewSection.tsx
git commit -m "feat(live): add TasksOverviewSection component for side panel"
```

---

### Task 3: Wire `TasksOverviewSection` into `SessionDetailPanel`

**Files:**
- Modify: `apps/web/src/components/live/SessionDetailPanel.tsx`

**Step 1: Add import**

In `SessionDetailPanel.tsx`, add after line 39 (after the last import `import type { LiveSession } from './use-live-sessions'`):

```ts
import { TasksOverviewSection } from './TasksOverviewSection'
```

**Step 2: Insert section into Overview tab**

In the Overview tab content, after the "Quick status" div (after line 397 — the closing `</div>` of the status strip) and BEFORE the "Cost + Cache" grid, add:

```tsx
            {/* ── Tasks (first section — primary monitoring concern) ── */}
            {data.progressItems && data.progressItems.length > 0 && (
              <TasksOverviewSection items={data.progressItems} />
            )}
```

The insertion point is between the Quick status strip and the Cost+Cache grid. In the current code, that's between line 397 (closing `</div>` of quick status) and line 399 (`{/* ── Cost + Cache ...`).

**Step 3: Verify TypeScript compiles**

Run: `cd apps/web && npx tsc --noEmit 2>&1 | head -20`
Expected: No errors

**Step 4: Build frontend**

Run: `cd apps/web && bun run build`
Expected: Build succeeds (remember: `cargo run` serves the dist/ bundle, frontend changes need `bun run build`)

**Step 5: Verify visually — end-to-end**

1. Start the dev server: `bun dev` (from repo root — runs Rust + Vite)
2. Open browser → live monitor page
3. Find a session card with tasks → verify hover tooltip shows all tasks
4. Click the session card → verify side panel Overview shows Tasks section at the top
5. Check: progress bar matches card, all tasks visible, in_progress items show blue spinner text

**Step 6: Commit**

```bash
git add apps/web/src/components/live/SessionDetailPanel.tsx
git commit -m "feat(live): show task list as first section in side panel Overview"
```

---

### Task 4: Final verification

**Step 1: Run full TypeScript check**

Run: `cd apps/web && npx tsc --noEmit`
Expected: No errors

**Step 2: Run web frontend tests**

Run: `cd apps/web && bunx vitest run`
Expected: All pass (no existing tests for these components — if any fail, they're pre-existing)

**Step 3: Build**

Run: `cd apps/web && bun run build`
Expected: Success

**Step 4: Visual QA checklist**

- [ ] Card hover: tooltip appears with all tasks, scrollable if 15+
- [ ] Card hover: tooltip dismisses on mouse-out
- [ ] Side panel: Tasks section appears as FIRST section (above Cost)
- [ ] Side panel: progress bar shows correct ratio
- [ ] Side panel: completed items have green ✓ + strikethrough
- [ ] Side panel: in_progress items have blue pulse dot + activeForm text
- [ ] Side panel: pending items have gray ◻
- [ ] Side panel: task list scrolls if 12+ items
- [ ] Dark mode: all text/borders/backgrounds render correctly
- [ ] No horizontal scroll, no layout shift

**Step 5: If any fixes were needed, commit them**

If Steps 1-4 required code changes, commit those fixes:

```bash
git add -p  # review each hunk
git commit -m "fix(live): address QA issues from task list implementation"
```

If no fixes were needed, skip this step — all work was already committed in Tasks 0-4.

---

### Rollback

All changes are pure frontend, committed in 4 small commits (Tasks 0-3). Task 4 is verification only — it only commits if QA fixes are needed.

To revert all changes:

```bash
git log --oneline -5  # identify the commits from Tasks 0-3 (and Task 4 if it committed)
git revert HEAD~4..HEAD --no-commit  # adjust count: 4 if Task 4 didn't commit, 5 if it did
git commit -m "revert: remove task list in Overview"
```

Or to undo a single task, revert its specific commit.

---

### Changelog of Fixes Applied (Audit -> Final Plan)

All references below use the numbering from the draft where the issue was found. The final plan has 5 tasks (0-4).

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `side="right"` on Tooltip.Content causes inconsistent positioning | Warning | Removed `side` prop — defaults to `"top"`, matching SubAgentPills |
| 2 | Incremental edits in Tooltip task had line-number drift between steps | Blocker | Replaced with complete final file (now Task 1) — zero line references |
| 3 | Import insertion "after line 34" placed mid-import-block | Warning | Changed to "after line 39 (after the last import)" (now Task 3) |
| 4 | "Export constants" task was redundant after full-file replacement | Minor | Merged into Tooltip task (now Task 1), renumbered remaining tasks |
| 5 | Final verification task had no commit step | Blocker | Added conditional commit Step 5 (now Task 4) |
| 6 | No rollback instructions | Minor | Added Rollback section |
