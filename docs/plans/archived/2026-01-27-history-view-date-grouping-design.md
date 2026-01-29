---
status: approved
date: 2026-01-27
---

# History View & Date-Grouped Sessions — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add browser-history-style date grouping to session lists, plus a new global History view with a calendar heatmap that doubles as a date range filter.

**Architecture:** Three new components (`DateGroupedList`, `ActivityCalendar`, `HistoryView`) plus a date-grouping utility (`date-groups.ts`, already started). `react-day-picker` v9 provides the calendar grid; we style it with Tailwind for heatmap coloring. `ProjectView` swaps its flat list for `DateGroupedList`. A new `/history` route shows all sessions across all projects with the calendar heatmap + grouped list.

**Tech Stack:** React 19, Tailwind 4, react-day-picker v9, Lucide icons, react-router-dom v7, TanStack Query

---

## Pre-existing work

- `react-day-picker` already installed (v9.13.0)
- `src/lib/date-groups.ts` already created with `groupSessionsByDate`, `countSessionsByDay`, `toDateKey`

---

### Task 1: DateGroupedList component

Renders sessions grouped by recency tiers with sticky date headers. Reusable in both ProjectView and HistoryView.

**Files:**
- Create: `src/components/DateGroupedList.tsx`

**Step 1: Create DateGroupedList component**

```tsx
// src/components/DateGroupedList.tsx
//
// Props:
//   sessions: SessionInfo[] (sorted by modifiedAt DESC)
//   showProjectBadge?: boolean (true in HistoryView, false in ProjectView)
//
// Renders:
//   For each DateGroup from groupSessionsByDate():
//     - Sticky header: "── Today ────────────── 8 sessions ──"
//       Classes: sticky top-0 bg-white/95 backdrop-blur-sm z-10
//       Left: tier label (font-medium text-gray-900 text-sm)
//       Right: session count (text-gray-400 tabular-nums text-xs)
//       Separator: flex-1 border-b border-gray-200 mx-3
//     - For each session in group:
//       <Link> wrapping <SessionCard>
//       If showProjectBadge, render project name pill above preview
//
// Uses: groupSessionsByDate from src/lib/date-groups.ts
// Uses: SessionCard from src/components/SessionCard.tsx
// Uses: Link from react-router-dom
```

**Step 2: Verify it compiles**

Run: `bun run typecheck`
Expected: no errors

**Step 3: Commit**

```bash
git add src/components/DateGroupedList.tsx
git commit -m "feat: add DateGroupedList component with sticky date headers"
```

---

### Task 2: Wire DateGroupedList into ProjectView

Replace the flat session list in ProjectView with the new date-grouped list.

**Files:**
- Modify: `src/components/ProjectView.tsx` (lines 36-49 — the `<div className="space-y-3">` block)

**Step 1: Replace flat list with DateGroupedList**

Replace the `<div className="space-y-3">` block (lines 36-49) with:

```tsx
<DateGroupedList sessions={project.sessions} />
```

Remove the "Load more sessions..." button (lines 51-55) — the date grouping naturally segments content.

Import `DateGroupedList` at top of file.

**Step 2: Verify it compiles**

Run: `bun run typecheck`
Expected: no errors

**Step 3: Commit**

```bash
git add src/components/ProjectView.tsx
git commit -m "feat: replace flat session list with date-grouped list in ProjectView"
```

---

### Task 3: ActivityCalendar heatmap component

Calendar month grid using react-day-picker, styled as a heatmap. Click a day to filter, shift+click for range.

**Files:**
- Create: `src/components/ActivityCalendar.tsx`

**Step 1: Create ActivityCalendar component**

```tsx
// src/components/ActivityCalendar.tsx
//
// Props:
//   sessions: SessionInfo[]
//   selectedRange: DateRange | undefined  (from react-day-picker)
//   onRangeChange: (range: DateRange | undefined) => void
//
// Implementation:
//   1. useMemo: countSessionsByDay(sessions) → Map<string, number>
//   2. Compute maxCount from the map values
//   3. Render <DayPicker> from react-day-picker with:
//      - mode="range" for range selection
//      - selected={selectedRange}
//      - onSelect={onRangeChange}
//      - modifiers: for each intensity level (empty, low, mid, high, hot)
//      - modifiersClassNames: map intensity levels to Tailwind classes
//      - showOutsideDays={false}
//      - Custom DayButton component that:
//        - Looks up session count for that day via toDateKey()
//        - Applies heatmap color class based on count/maxCount ratio:
//          0     → bg-gray-100
//          1-2   → bg-emerald-100 text-emerald-900
//          3-5   → bg-emerald-300 text-emerald-900
//          6-10  → bg-emerald-500 text-white
//          11+   → bg-emerald-700 text-white
//        - Shows tooltip on hover: "Jan 15: 8 sessions"
//        - hover: ring-2 ring-emerald-400
//
//   4. Below calendar grid: summary line
//      "◉ 342 sessions · 7 projects · since Oct 2025"
//
// Styling:
//   Override react-day-picker default styles with Tailwind via className prop
//   and custom CSS in index.css (minimal — just grid gap and cell sizing)
//
// Navigation:
//   ‹ › arrows for month nav (built into react-day-picker)
//   Today's cell gets a small pulsing dot indicator
```

**Step 2: Add minimal react-day-picker CSS overrides to index.css**

```css
/* in src/index.css — after @import 'tailwindcss' */

/* react-day-picker calendar heatmap overrides */
.rdp {
  --rdp-accent-color: #059669; /* emerald-600 */
  --rdp-background-color: transparent;
}
.rdp-month_caption {
  font-weight: 600;
  font-size: 0.875rem;
  color: #111827; /* gray-900 */
}
.rdp-day {
  width: 2.25rem;
  height: 2.25rem;
  border-radius: 0.375rem;
  font-size: 0.75rem;
  transition: all 150ms;
}
```

**Step 3: Verify it compiles**

Run: `bun run typecheck`
Expected: no errors

**Step 4: Commit**

```bash
git add src/components/ActivityCalendar.tsx src/index.css
git commit -m "feat: add ActivityCalendar heatmap component with react-day-picker"
```

---

### Task 4: HistoryView page

New `/history` route combining ActivityCalendar + DateGroupedList showing all sessions across all projects.

**Files:**
- Create: `src/components/HistoryView.tsx`

**Step 1: Create HistoryView component**

```tsx
// src/components/HistoryView.tsx
//
// Gets projects from useOutletContext (same pattern as ProjectView/StatsDashboard)
//
// State:
//   selectedRange: DateRange | undefined (react-day-picker type)
//
// Computation:
//   1. Flatten all sessions: projects.flatMap(p => p.sessions)
//   2. Sort by modifiedAt DESC
//   3. If selectedRange set, filter sessions to those within range
//   4. Pass filtered sessions to DateGroupedList with showProjectBadge={true}
//
// Layout (top to bottom, max-w-3xl mx-auto):
//   1. Page header: Clock icon + "History" title
//   2. ActivityCalendar (the heatmap hero)
//      - If a range is selected, show a "Clear filter" chip button
//   3. DateGroupedList with filtered sessions
//   4. If filtered and no results: empty state "No sessions in selected range"
//
// Edge cases:
//   - No sessions at all → show empty state
//   - Range selected but no sessions in range → show "No sessions" + clear button
```

**Step 2: Verify it compiles**

Run: `bun run typecheck`
Expected: no errors

**Step 3: Commit**

```bash
git add src/components/HistoryView.tsx
git commit -m "feat: add HistoryView page with calendar heatmap and date-grouped sessions"
```

---

### Task 5: Add /history route and sidebar nav

Wire HistoryView into the router and add a sidebar link.

**Files:**
- Modify: `src/router.tsx` (add route)
- Modify: `src/components/Sidebar.tsx` (add nav link)
- Modify: `src/components/Header.tsx` (add breadcrumb for /history)

**Step 1: Add route to router.tsx**

Add after the index route (line 13):

```tsx
{ path: 'history', element: <HistoryView /> },
```

Import `HistoryView` at top.

**Step 2: Add History link to Sidebar**

Above the project list (`<div className="flex-1 overflow-y-auto py-2">`), add a nav section:

```tsx
<div className="px-3 py-2 border-b border-gray-200">
  <Link
    to="/history"
    className={cn(
      'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors',
      location.pathname === '/history'
        ? 'bg-blue-500 text-white'
        : 'text-gray-600 hover:bg-gray-200/70'
    )}
  >
    <Clock className="w-4 h-4" />
    <span className="font-medium">History</span>
  </Link>
</div>
```

Import `Clock` from lucide-react. Import `useLocation` (already imported).

**Step 3: Add breadcrumb for /history in Header.tsx**

After the `/search` breadcrumb block (line 33-35), add:

```tsx
if (location.pathname === '/history') {
  crumbs.push({ label: 'History', path: '/history' })
}
```

**Step 4: Verify it compiles**

Run: `bun run typecheck`
Expected: no errors

**Step 5: Commit**

```bash
git add src/router.tsx src/components/Sidebar.tsx src/components/Header.tsx
git commit -m "feat: add /history route with sidebar nav link and breadcrumb"
```

---

### Task 6: SessionCard project badge (for HistoryView)

Add an optional project name badge to SessionCard when displayed in the global History view.

**Files:**
- Modify: `src/components/SessionCard.tsx`

**Step 1: Add optional projectDisplayName prop**

Add to `SessionCardProps`:

```tsx
projectDisplayName?: string  // Shown in History view
```

Render it at the top of the card, before the preview text:

```tsx
{projectDisplayName && (
  <span className="inline-block px-1.5 py-0.5 text-[11px] font-medium bg-blue-50 text-blue-600 rounded mb-1.5">
    {projectDisplayName}
  </span>
)}
```

**Step 2: Update DateGroupedList to pass projectDisplayName when showProjectBadge is true**

In `DateGroupedList.tsx`, when rendering SessionCard:

```tsx
<SessionCard
  session={session}
  isSelected={false}
  onClick={() => {}}
  projectDisplayName={showProjectBadge ? session.project : undefined}
/>
```

Note: `session.project` contains the project name. If `displayName` is needed, the parent can provide a lookup map — but `project` (folder name) is sufficient for the badge.

**Step 3: Verify it compiles**

Run: `bun run typecheck`
Expected: no errors

**Step 4: Commit**

```bash
git add src/components/SessionCard.tsx src/components/DateGroupedList.tsx
git commit -m "feat: add project badge to SessionCard for History view"
```

---

### Task 7: Visual polish and smoke test

Final styling pass and manual verification.

**Files:**
- Possibly tweak: `src/index.css`, any component files

**Step 1: Run dev server**

```bash
bun run dev
```

**Step 2: Manual verification checklist**

- [ ] `/` (dashboard) — still works, no regressions
- [ ] `/project/:id` — sessions now grouped by date with sticky headers
- [ ] `/history` — shows calendar heatmap + all sessions across projects
- [ ] Calendar cells colored by session density
- [ ] Click a day → list filters to that day's sessions
- [ ] Shift+click two days → range selection, list filters to range
- [ ] Click "Clear filter" → removes filter, shows all sessions
- [ ] Session cards in History show project badge
- [ ] Sticky headers pin while scrolling
- [ ] Month navigation ‹ › works
- [ ] Sidebar "History" link highlights when active

**Step 3: Fix any visual issues found**

**Step 4: Commit any polish fixes**

```bash
git add -A
git commit -m "fix: visual polish for history view and calendar heatmap"
```

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | DateGroupedList component | Create `DateGroupedList.tsx` |
| 2 | Wire into ProjectView | Modify `ProjectView.tsx` |
| 3 | ActivityCalendar heatmap | Create `ActivityCalendar.tsx`, modify `index.css` |
| 4 | HistoryView page | Create `HistoryView.tsx` |
| 5 | Route + sidebar nav | Modify `router.tsx`, `Sidebar.tsx`, `Header.tsx` |
| 6 | Project badge on SessionCard | Modify `SessionCard.tsx`, `DateGroupedList.tsx` |
| 7 | Visual polish + smoke test | Various |

**Pre-existing:** `react-day-picker` installed, `src/lib/date-groups.ts` created with grouping + counting utilities.
