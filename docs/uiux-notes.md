# UI/UX Mistakes & Rules

Checklist of UI/UX patterns that have caused bugs. **Review this before any UI work.**

## 1. Interactive elements must show active/selected state

When an item can be clicked to filter, navigate, or toggle — it **must** visually indicate the active state.

**Common misses:**
- Sidebar branch list: clicked branch had no highlight, user couldn't tell which branch was active
- Filter chips/buttons: selected option must look visually distinct from unselected

**Rule:** For every clickable item that changes app state, ask: "Can the user see which one is currently active?" If not, add `isActive` styling.

## 2. URL param names must be consistent across all consumers

When multiple components read/write the same URL param, they must use the **exact same key name**.

**What happened:** Sidebar set `branches` (plural), ProjectView read `branch` (singular) — filter silently did nothing.

**Rule:** When adding a URL param, `grep` for all consumers of that param name. When renaming, update **every** file.

## 3. Every view that displays filtered data must apply the filters

Adding a new filter system? **ALL views** that show the filtered data must implement the filter logic.

**What happened:** HistoryView had full client-side filtering for all `useSessionFilters` properties. ProjectView had none — it showed raw `page.sessions` without any filtering.

**Rule:** When adding a filter, search for every component that renders the filtered data type and add filtering there too.

## 4. Clickable items should support toggle (deselect)

If clicking an item selects it, clicking it again should deselect it.

**Example:** Sidebar branch click — clicking active branch should clear the filter and show all sessions.

## 5. Hooks must be called before any early return

React hooks (useState, useMemo, useEffect, useCallback) must be called in the same order every render. Never place a hook after an `if (...) return`.

**What happened:** `useMemo` for client-side filtering was placed after an early return guard in ProjectView, causing "Rendered fewer hooks than expected" crash.

**Rule:** All hooks go at the top of the component, before any conditional returns.

## 6. Draft state in popovers must only reset on open transition

Popovers with "Apply" buttons hold draft state. This draft must only reset when the popover **opens**, not on every parent re-render.

**What happened:** FilterPopover's `useEffect([isOpen, filters])` reset draft selections whenever the parent re-rendered (React Query refetch), wiping the user's in-progress choices.

**Fix pattern:**
```tsx
const prevIsOpenRef = useRef(false);
useEffect(() => {
  if (isOpen && !prevIsOpenRef.current) {
    setDraftState(currentState);
  }
  prevIsOpenRef.current = isOpen;
}, [isOpen, currentState]);
```

## 7. Loading/empty states need context

When showing "No results" or "Not found", include enough context for the user to understand **why** and **what to do**.

**Examples:**
- "No sessions found" → Add "Try adjusting your filters" with a clear-filters button
- "Project not found" → Add breadcrumb back to project list

## 8. Count badges must update with filters

If the UI shows a count (e.g. "230 sessions"), it must update when filters are applied to reflect the filtered count.

**Rule:** Use `filteredSessions.length` not `page.total` for display counts when filters are active.

## Pre-PR UI/UX Checklist

Before submitting UI changes, verify:

- [ ] Every clickable/selectable item has visible active state
- [ ] URL params are consistent (grep all consumers)
- [ ] All views that show filtered data apply the filters
- [ ] Toggle behavior works (select + deselect)
- [ ] No hooks after early returns
- [ ] Popover draft state only resets on open transition
- [ ] Empty states have helpful context + clear action
- [ ] Count badges reflect current filter state
