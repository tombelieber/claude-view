---
status: done
date: 2026-02-06
---

# Session Discovery Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship 4 polish items that complete the session discovery feature: collapsible group headers, 500-session safeguard, legacy filter consolidation, and performance benchmarks.

**Architecture:** All changes are frontend-only (React/TypeScript). No Rust backend changes. Task 1-2 are small, independent additions. Task 3 is a deletion-heavy refactor that removes the legacy filter system. Task 4 adds a vitest benchmark suite.

**Tech Stack:** React 18, TypeScript, Vitest, Tailwind CSS, `performance.now()` for benchmarks

---

## Task 1: Collapsible Group Headers

**Why:** Group headers in timeline view are display-only. Users can't collapse groups to focus on specific ones. The data model already has `expanded: boolean` on `SessionGroup` but it's always `true` and never toggled.

**Files:**
- Modify: `src/components/HistoryView.tsx:512-559` (group rendering)
- Modify: `src/utils/group-sessions.ts` (no changes needed — `expanded` already exists)
- Test: `src/utils/group-sessions.test.ts` (existing, add collapse tests)

### Step 1: Write the failing test

Add to `src/utils/group-sessions.test.ts`:

```typescript
describe('expanded state', () => {
  it('all groups start with expanded: true', () => {
    const sessions = [
      makeSession({ id: 's1', gitBranch: 'main' }),
      makeSession({ id: 's2', gitBranch: 'feature/auth' }),
    ];
    const groups = groupSessions(sessions, 'branch');
    expect(groups.every(g => g.expanded)).toBe(true);
  });
});
```

This test already exists (line 333-342) and passes. No new pure-logic tests needed — the collapse is UI state managed by React `useState` inside HistoryView, not in the utility.

### Step 2: Add collapse state and toggle to HistoryView

In `src/components/HistoryView.tsx`, add state and handler **after** the `groups` useMemo (around line 258):

```typescript
// Collapse state for group headers — keyed by group label
const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

const toggleGroup = useCallback((label: string) => {
  setCollapsedGroups(prev => {
    const next = new Set(prev);
    if (next.has(label)) {
      next.delete(label);
    } else {
      next.add(label);
    }
    return next;
  });
}, []);
```

Add `useCallback` to the import at line 1:
```typescript
import { useState, useMemo, useRef, useEffect, useCallback } from 'react'
```

### Step 3: Wire toggle to group headers in timeline view

Replace the group header `<div>` at lines 516-524 with a clickable, togglable version:

**Old (lines 514-558):**
```tsx
{groups.map(group => (
  <div key={group.label}>
    {/* Group header */}
    <div className="sticky top-0 z-10 bg-white/95 dark:bg-gray-950/95 backdrop-blur-sm py-2 flex items-center gap-3">
      <span className="text-[13px] font-semibold text-gray-500 dark:text-gray-400 tracking-tight whitespace-nowrap">
        {group.label}
      </span>
      <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
      <span className="text-[11px] text-gray-400 tabular-nums whitespace-nowrap" aria-label={`${group.sessions.length} sessions`}>
        {group.sessions.length}
      </span>
    </div>

    {/* Cards */}
    <div className="space-y-1.5 pb-3">
      {group.sessions.map((session, idx) => {
        // ... existing card rendering
      })}
    </div>
  </div>
))}
```

**New:**
```tsx
{groups.map(group => {
  const isCollapsed = collapsedGroups.has(group.label);
  return (
    <div key={group.label}>
      {/* Group header — clickable to collapse/expand */}
      <button
        type="button"
        onClick={() => toggleGroup(group.label)}
        className="sticky top-0 z-10 w-full bg-white/95 dark:bg-gray-950/95 backdrop-blur-sm py-2 flex items-center gap-3 cursor-pointer group/header"
        aria-expanded={!isCollapsed}
      >
        <ChevronDown
          className={cn(
            'w-3.5 h-3.5 text-gray-400 transition-transform duration-150',
            isCollapsed && '-rotate-90'
          )}
        />
        <span className="text-[13px] font-semibold text-gray-500 dark:text-gray-400 tracking-tight whitespace-nowrap group-hover/header:text-gray-700 dark:group-hover/header:text-gray-300 transition-colors">
          {group.label}
        </span>
        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
        <span className="text-[11px] text-gray-400 tabular-nums whitespace-nowrap" aria-label={`${group.sessions.length} sessions`}>
          {group.sessions.length}
        </span>
      </button>

      {/* Cards — hidden when collapsed */}
      {!isCollapsed && (
        <div className="space-y-1.5 pb-3">
          {group.sessions.map((session, idx) => {
            // ... existing card rendering (unchanged)
          })}
        </div>
      )}
    </div>
  );
})}
```

Ensure `ChevronDown` is already imported at line 5 (it is — from lucide-react). Ensure `cn` is imported (it is not currently). Add:
```typescript
import { cn } from '../lib/utils'
```

Wait — `cn` is NOT imported in HistoryView.tsx. Check if it exists: it's used in FilterSortBar and SessionToolbar via `import { cn } from '../lib/utils'`. Add this import.

### Step 4: Run tests to verify nothing breaks

```bash
cd /Users/user/dev/@myorg/claude-view/.worktrees/session-discovery
npx vitest run src/utils/group-sessions.test.ts src/components/NullSafety.test.tsx
```

Expected: All existing tests pass.

### Step 5: Commit

```bash
git add src/components/HistoryView.tsx
git commit -m "feat: add collapsible group headers in timeline view

Click group headers to collapse/expand. Chevron rotates on
collapse. aria-expanded attribute for accessibility."
```

---

## Task 2: 500-Session Grouping Safeguard

**Why:** Grouping loads all sessions into memory and creates DOM nodes for every card. With 1000+ sessions across 50+ groups, the UI could freeze. The spec requires disabling grouping when `total > 500` and showing a warning.

**Files:**
- Modify: `src/components/HistoryView.tsx:252-258` (groups useMemo)
- Modify: `src/components/SessionToolbar.tsx` (add `disabled` prop to group-by dropdown)
- Test: `src/utils/group-sessions.test.ts` (pure logic test for safeguard)

### Step 1: Write the failing test

Add a new utility function `shouldDisableGrouping` to `src/utils/group-sessions.ts` and test it.

Add to `src/utils/group-sessions.test.ts`:

```typescript
import { groupSessions, shouldDisableGrouping } from './group-sessions';

describe('shouldDisableGrouping', () => {
  it('returns false for <= 500 sessions', () => {
    expect(shouldDisableGrouping(0)).toBe(false);
    expect(shouldDisableGrouping(250)).toBe(false);
    expect(shouldDisableGrouping(500)).toBe(false);
  });

  it('returns true for > 500 sessions', () => {
    expect(shouldDisableGrouping(501)).toBe(true);
    expect(shouldDisableGrouping(1000)).toBe(true);
  });
});
```

### Step 2: Run test to verify it fails

```bash
npx vitest run src/utils/group-sessions.test.ts
```

Expected: FAIL — `shouldDisableGrouping` is not exported.

### Step 3: Implement `shouldDisableGrouping`

Add to `src/utils/group-sessions.ts` (after the `groupSessions` function, before `getGroupKey`):

```typescript
/** Maximum session count before grouping is disabled for performance */
export const MAX_GROUPABLE_SESSIONS = 500;

/**
 * Check if grouping should be disabled due to session count.
 * When total > 500, client-side grouping can cause UI lag.
 */
export function shouldDisableGrouping(totalSessions: number): boolean {
  return totalSessions > MAX_GROUPABLE_SESSIONS;
}
```

### Step 4: Run test to verify it passes

```bash
npx vitest run src/utils/group-sessions.test.ts
```

Expected: PASS.

### Step 5: Wire safeguard into HistoryView

In `src/components/HistoryView.tsx`, import the new function:

```typescript
import { groupSessions, shouldDisableGrouping, MAX_GROUPABLE_SESSIONS } from '../utils/group-sessions'
```

Modify the `groups` useMemo (lines 252-258) to check the safeguard:

**Old:**
```typescript
const groups = useMemo(() => {
  if (filters.groupBy !== 'none') {
    return groupSessions(filteredSessions, filters.groupBy)
  }
  return sort === 'recent' ? groupSessionsByDate(filteredSessions) : [{ label: SORT_LABELS[sort], sessions: filteredSessions }]
}, [filteredSessions, filters.groupBy, sort])
```

**New:**
```typescript
const tooManyToGroup = shouldDisableGrouping(filteredSessions.length);

const groups = useMemo(() => {
  if (filters.groupBy !== 'none' && !tooManyToGroup) {
    return groupSessions(filteredSessions, filters.groupBy)
  }
  return sort === 'recent' ? groupSessionsByDate(filteredSessions) : [{ label: SORT_LABELS[sort], sessions: filteredSessions }]
}, [filteredSessions, filters.groupBy, sort, tooManyToGroup])
```

### Step 6: Add warning banner when safeguard triggers

In `src/components/HistoryView.tsx`, add a warning banner right before the session list (before line 480 `{/* Session List or Table */}`):

```tsx
{/* Grouping safeguard warning */}
{tooManyToGroup && filters.groupBy !== 'none' && (
  <div className="mt-3 px-3 py-2 bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800 rounded-lg text-xs text-amber-700 dark:text-amber-300">
    Grouping disabled — {filteredSessions.length} sessions exceeds the {MAX_GROUPABLE_SESSIONS} session limit. Use filters to narrow results.
  </div>
)}
```

### Step 7: Disable group-by dropdown when safeguard active

Add `disabled` prop to `SessionToolbar`. In `src/components/SessionToolbar.tsx`:

**Modify the `SessionToolbarProps` interface:**
```typescript
interface SessionToolbarProps {
  filters: SessionFilters;
  onFiltersChange: (filters: SessionFilters) => void;
  onClearFilters: () => void;
  groupByDisabled?: boolean;
  groupByDisabledReason?: string;
}
```

**Modify the component signature:**
```typescript
export function SessionToolbar({ filters, onFiltersChange, onClearFilters, groupByDisabled, groupByDisabledReason }: SessionToolbarProps) {
```

**Modify the Group-by Dropdown (lines 180-187):**

Add `disabled` support to the `Dropdown` component or conditionally pass `onChange`. Simplest approach — wrap the onChange:

```tsx
<Dropdown
  label="Group by"
  icon={<div className="w-3.5 h-3.5 flex items-center justify-center text-xs">⊞</div>}
  value={groupByDisabled ? 'none' : filters.groupBy}
  options={GROUP_BY_OPTIONS}
  onChange={groupByDisabled ? () => {} : handleGroupByChange}
  isActive={!groupByDisabled && filters.groupBy !== 'none'}
/>
```

If `groupByDisabled`, optionally show a tooltip-style title attribute on the dropdown button. (Keep it simple — no tooltip library.)

**In HistoryView.tsx, pass the props:**

```tsx
<SessionToolbar
  filters={filters}
  onFiltersChange={setFilters}
  onClearFilters={() => setFilters(DEFAULT_FILTERS)}
  groupByDisabled={tooManyToGroup}
  groupByDisabledReason={tooManyToGroup ? `Too many sessions (${filteredSessions.length} > ${MAX_GROUPABLE_SESSIONS})` : undefined}
/>
```

### Step 8: Run all related tests

```bash
npx vitest run src/utils/group-sessions.test.ts src/components/SessionToolbar.test.tsx
```

Expected: PASS.

### Step 9: Commit

```bash
git add src/utils/group-sessions.ts src/utils/group-sessions.test.ts src/components/HistoryView.tsx src/components/SessionToolbar.tsx
git commit -m "feat: disable grouping when session count exceeds 500

Shows warning banner and disables group-by dropdown to prevent
UI lag with large datasets."
```

---

## Task 3: Legacy Filter System Consolidation

**Why:** Two filter systems run side-by-side. Both manage `sort`. The legacy system (`useFilterSort` + `FilterSortBar`) duplicates functionality that the new system (`useSessionFilters` + `SessionToolbar`) already provides. This creates sort-param conflicts and UI confusion (two sort dropdowns visible).

**Strategy:** Delete legacy, move its unique feature (the `filter` dropdown with `has_commits`/`high_reedit`/`long_session`) into the new system. The new `FilterPopover` already covers `hasCommits` and `highReedit`. `long_session` (>30min) maps to `minDuration: 1800`.

**Files:**
- Delete: `src/hooks/use-filter-sort.ts` (35 lines)
- Delete: `src/components/FilterSortBar.tsx` (195 lines)
- Modify: `src/components/HistoryView.tsx` (remove legacy imports, remove legacy dropdown, clean up dual filtering)
- Modify: `src/components/NullSafety.test.tsx:217-268` (remove `FilterSortBar` tests)
- Modify: `src/hooks/use-session-filters.ts` (ensure `sort` is the canonical source)

### Step 1: Verify the mapping — every legacy filter has a new equivalent

| Legacy `filter` value | New system equivalent | Already works? |
|---|---|---|
| `all` | no filter active | Yes |
| `has_commits` | `filters.hasCommits = 'yes'` | Yes (FilterPopover) |
| `high_reedit` | `filters.highReedit = true` | Yes (FilterPopover) |
| `long_session` | `filters.minDuration = 1800` | Yes (FilterPopover) |

| Legacy `sort` value | New system equivalent | Already works? |
|---|---|---|
| `recent` | `filters.sort = 'recent'` | Yes (SessionToolbar) |
| `tokens` | `filters.sort = 'tokens'` | Yes (SessionToolbar) |
| `prompts` | `filters.sort = 'prompts'` | Yes (SessionToolbar) |
| `files_edited` | `filters.sort = 'files_edited'` | Yes (SessionToolbar) |
| `duration` | `filters.sort = 'duration'` | Yes (SessionToolbar) |

**Conclusion:** 100% feature parity. Safe to delete.

### Step 2: Remove legacy imports and state from HistoryView

In `src/components/HistoryView.tsx`:

**Remove these imports (lines 11, 14):**
```typescript
// DELETE: import { FilterSortBar, useFilterSort } from './FilterSortBar'
// DELETE: import type { SessionSort, SessionFilter } from './FilterSortBar'
```

**Add type imports from new system (if not already):**
```typescript
import type { SessionSort } from '../hooks/use-session-filters'
```

Note: `SessionSort` is already defined in `use-session-filters.ts` (line 25). HistoryView uses `SessionSort` for `SORT_LABELS`, `SORT_ICONS`, and `formatSortMetric`. Update the import source.

**Remove legacy hook call (line 90):**
```typescript
// DELETE: const { filter, sort, setFilter, setSort } = useFilterSort(searchParams, setSearchParams)
```

**Replace all `sort` references with `filters.sort`** and all `filter` references with the new equivalent:

- Line 23-37: `SORT_LABELS` and `SORT_ICONS` — keep as-is but they now reference `SessionSort` from the new hook
- Line 56: `formatSortMetric` — keep as-is, it takes a `SessionSort` param
- Line 102-103: `hasDeepLinkSort` / `hasDeepLinkFilter` — replace:
  ```typescript
  const hasDeepLinkSort = filters.sort !== 'recent'
  const hasDeepLinkFilter = filters.hasCommits !== 'any' || filters.hasSkills !== 'any' || filters.highReedit !== null || filters.minDuration !== null
  ```
- Line 226: `if (sort !== 'recent')` → `if (filters.sort !== 'recent')`
- Line 228: `switch (sort)` → `switch (filters.sort)`
- Line 247: dependency array — remove `filter, sort`, keep `filters`
- Line 249: `isFiltered` — remove `filter !== 'all' || sort !== 'recent'`, add `filters.sort !== 'recent' || filters.hasCommits !== 'any'` (etc.)
- Line 257: `sort === 'recent'` → `filters.sort === 'recent'`, `SORT_LABELS[sort]` → `SORT_LABELS[filters.sort]`
- Line 258: dependency array — remove `sort`, it's now inside `filters`
- Line 282-283: `clearAll` — remove `setFilter('all')` and `setSort('recent')`, keep `setFilters(DEFAULT_FILTERS)` which already resets sort

### Step 3: Remove legacy filter logic from `filteredSessions`

**Delete lines 200-208** (the `// OLD Session filter` block):
```typescript
// DELETE THIS BLOCK:
// OLD Session filter (from dropdown) - kept for backwards compatibility
if (filter === 'has_commits' && (s.commitCount ?? 0) === 0) return false
if (filter === 'high_reedit') {
  const filesEdited = s.filesEditedCount ?? 0
  const reeditedFiles = s.reeditedFilesCount ?? 0
  const reeditRate = filesEdited > 0 ? reeditedFiles / filesEdited : 0
  if (reeditRate <= 0.2) return false
}
if (filter === 'long_session' && (s.durationSeconds ?? 0) <= 1800) return false
```

### Step 4: Remove legacy UI from the toolbar

**Delete lines 380-388** (the divider + FilterSortBar):
```tsx
// DELETE:
<div className="w-px h-5 bg-gray-200 dark:bg-gray-700" />

{/* LEGACY: Old FilterSortBar (can be removed later) */}
<FilterSortBar
  filter={filter}
  sort={sort}
  onFilterChange={setFilter}
  onSortChange={setSort}
/>
```

### Step 5: Update deep-link banner

The deep-link banner (lines 308-339) references `hasDeepLinkFilter`, `filter`, `FILTER_LABELS`, `SORT_ICONS[sort]`, `SORT_LABELS[sort]`. Update to use `filters.sort` and remove the `FILTER_LABELS` constant entirely:

**Remove** the `FILTER_LABELS` constant (lines 39-44).

**Update** the banner:
```tsx
{hasDeepLinkSort && (
  <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 text-xs font-medium text-gray-700 dark:text-gray-300">
    {SORT_ICONS[filters.sort]}
    {SORT_LABELS[filters.sort]}
  </span>
)}
```

Remove the `hasDeepLinkFilter` badge entirely (it showed "Has commits" etc. — those now show as active filter pills in the FilterPopover).

### Step 6: Update `clearAll` function

```typescript
function clearAll() {
  setSearchText('')
  setSelectedProjects(new Set())
  setTimeFilter('all')
  setSelectedDate(null)
  setFilters(DEFAULT_FILTERS)
}
```

Remove: `setFilter('all')`, `setSort('recent')` — these no longer exist.

### Step 7: Clean up the `filter` URL param

The legacy system wrote `?filter=has_commits` to the URL. After removal, old bookmarked URLs with `?filter=...` would be ignored (harmless — the param just sits in the URL unused). No migration needed.

Optionally, add a one-time cleanup to `useSessionFilters` that deletes the `filter` param if present:

In `serializeFilters()` in `src/hooks/use-session-filters.ts`, add after the FILTER_KEYS cleanup loop:
```typescript
// Clean up legacy 'filter' param if present
params.delete('filter');
```

### Step 8: Delete legacy files

```bash
git rm src/hooks/use-filter-sort.ts
git rm src/components/FilterSortBar.tsx
```

### Step 9: Update NullSafety.test.tsx

In `src/components/NullSafety.test.tsx`:

**Remove** the `FilterSortBar` import (line 10):
```typescript
// DELETE: import { FilterSortBar } from './FilterSortBar'
```

**Remove** the entire `FilterSortBar component` describe block (lines 217-268).

### Step 10: Run full test suite

```bash
npx vitest run
```

Expected: All tests pass. If any test imports `FilterSortBar` or `useFilterSort`, it will fail and must be updated.

### Step 11: Verify no remaining references

```bash
grep -r "FilterSortBar\|useFilterSort\|use-filter-sort" src/
```

Expected: Zero results.

### Step 12: Commit

```bash
git add -A
git commit -m "refactor: remove legacy filter system (useFilterSort + FilterSortBar)

Consolidates onto useSessionFilters as single source of truth.
Removes duplicate sort dropdown, eliminates sort-param conflicts.
All legacy filter features already exist in FilterPopover."
```

---

## Task 4: Performance Benchmarks

**Why:** The spec defined 5 performance targets (AC-13) but none were measured. Adding benchmarks catches regressions and validates that grouping/filtering stays fast.

**Strategy:** Use Vitest's `bench` mode for client-side JS benchmarks. Measure `groupSessions()`, `filteredSessions` filter logic, and `performance.mark`/`performance.measure` patterns for runtime use.

**Files:**
- Create: `src/utils/group-sessions.bench.ts` (vitest bench file)
- Create: `src/utils/filter-sessions.bench.ts` (vitest bench file)
- Modify: `package.json` (add `bench` script)

### Step 1: Add bench script to package.json

Check current scripts in `package.json` and add:

```json
"bench": "vitest bench"
```

### Step 2: Create grouping benchmark

Create `src/utils/group-sessions.bench.ts`:

```typescript
import { bench, describe } from 'vitest';
import { groupSessions } from './group-sessions';
import type { SessionInfo } from '../types/generated/SessionInfo';
import type { ToolCounts } from '../types/generated/ToolCounts';

function makeSession(i: number, overrides: Partial<SessionInfo> = {}): SessionInfo {
  const defaultToolCounts: ToolCounts = { bash: 0, edit: 0, read: 0, write: 0 };
  const branches = ['main', 'feature/auth', 'feature/ui', 'fix/bug-123', 'dev', 'staging'];
  const models = ['claude-opus-4', 'claude-sonnet-4', 'claude-haiku-4'];
  const baseTime = Math.floor(Date.now() / 1000) - i * 3600; // 1 hour apart

  return {
    id: `session-${i}`,
    project: `project-${i % 5}`,
    projectPath: `/test/project-${i % 5}`,
    filePath: `/test/file-${i}.jsonl`,
    modifiedAt: BigInt(baseTime),
    sizeBytes: BigInt(1024 * (i + 1)),
    preview: `Session preview ${i}`,
    lastMessage: `Last message ${i}`,
    filesTouched: [`file${i}.ts`, `file${i + 1}.ts`],
    skillsUsed: i % 3 === 0 ? ['skill-a'] : [],
    toolCounts: defaultToolCounts,
    messageCount: 10 + i,
    turnCount: 5 + i,
    isSidechain: false,
    deepIndexed: true,
    userPromptCount: 5 + (i % 20),
    apiCallCount: 10 + i,
    toolCallCount: 20 + i,
    filesRead: [`file${i}.ts`],
    filesEdited: [`file${i + 1}.ts`],
    filesReadCount: 1,
    filesEditedCount: 1 + (i % 5),
    reeditedFilesCount: i % 4 === 0 ? 1 : 0,
    durationSeconds: 300 + i * 60,
    commitCount: i % 2,
    gitBranch: branches[i % branches.length],
    primaryModel: models[i % models.length],
    totalInputTokens: BigInt(5000 + i * 100),
    totalOutputTokens: BigInt(2000 + i * 50),
    thinkingBlockCount: 0,
    apiErrorCount: 0,
    compactionCount: 0,
    agentSpawnCount: 0,
    bashProgressCount: 0,
    hookProgressCount: 0,
    mcpProgressCount: 0,
    parseVersion: 1,
    ...overrides,
  };
}

function generateSessions(count: number): SessionInfo[] {
  return Array.from({ length: count }, (_, i) => makeSession(i));
}

describe('groupSessions performance', () => {
  const sessions100 = generateSessions(100);
  const sessions500 = generateSessions(500);

  bench('group 100 sessions by branch', () => {
    groupSessions(sessions100, 'branch');
  });

  bench('group 500 sessions by branch', () => {
    groupSessions(sessions500, 'branch');
  });

  bench('group 100 sessions by model', () => {
    groupSessions(sessions100, 'model');
  });

  bench('group 500 sessions by model', () => {
    groupSessions(sessions500, 'model');
  });

  bench('group 100 sessions by week', () => {
    groupSessions(sessions100, 'week');
  });

  bench('group 500 sessions by week', () => {
    groupSessions(sessions500, 'week');
  });

  bench('group 500 sessions by month', () => {
    groupSessions(sessions500, 'month');
  });
});
```

### Step 3: Create filtering benchmark

Create `src/utils/filter-sessions.bench.ts`:

```typescript
import { bench, describe } from 'vitest';
import type { SessionInfo } from '../types/generated/SessionInfo';
import type { ToolCounts } from '../types/generated/ToolCounts';
import type { SessionFilters } from '../hooks/use-session-filters';
import { DEFAULT_FILTERS } from '../hooks/use-session-filters';

function makeSession(i: number): SessionInfo {
  const defaultToolCounts: ToolCounts = { bash: 0, edit: 0, read: 0, write: 0 };
  const branches = ['main', 'feature/auth', 'feature/ui', 'fix/bug-123', 'dev', 'staging'];
  const models = ['claude-opus-4', 'claude-sonnet-4', 'claude-haiku-4'];
  const baseTime = Math.floor(Date.now() / 1000) - i * 3600;

  return {
    id: `session-${i}`,
    project: `project-${i % 5}`,
    projectPath: `/test/project-${i % 5}`,
    filePath: `/test/file-${i}.jsonl`,
    modifiedAt: BigInt(baseTime),
    sizeBytes: BigInt(1024 * (i + 1)),
    preview: `Session preview ${i}`,
    lastMessage: `Last message ${i}`,
    filesTouched: [`file${i}.ts`, `file${i + 1}.ts`],
    skillsUsed: i % 3 === 0 ? ['skill-a'] : [],
    toolCounts: defaultToolCounts,
    messageCount: 10 + i,
    turnCount: 5 + i,
    isSidechain: false,
    deepIndexed: true,
    userPromptCount: 5 + (i % 20),
    apiCallCount: 10 + i,
    toolCallCount: 20 + i,
    filesRead: [`file${i}.ts`],
    filesEdited: [`file${i + 1}.ts`],
    filesReadCount: 1,
    filesEditedCount: 1 + (i % 5),
    reeditedFilesCount: i % 4 === 0 ? 1 : 0,
    durationSeconds: 300 + i * 60,
    commitCount: i % 2,
    gitBranch: branches[i % branches.length],
    primaryModel: models[i % models.length],
    totalInputTokens: BigInt(5000 + i * 100),
    totalOutputTokens: BigInt(2000 + i * 50),
    thinkingBlockCount: 0,
    apiErrorCount: 0,
    compactionCount: 0,
    agentSpawnCount: 0,
    bashProgressCount: 0,
    hookProgressCount: 0,
    mcpProgressCount: 0,
    parseVersion: 1,
  };
}

/**
 * Simulate the client-side filtering logic from HistoryView.
 * Extracted as a pure function for benchmarking.
 */
function filterSessions(sessions: SessionInfo[], filters: SessionFilters): SessionInfo[] {
  return sessions.filter(s => {
    if (filters.branches.length > 0) {
      if (!s.gitBranch || !filters.branches.includes(s.gitBranch)) return false;
    }
    if (filters.models.length > 0) {
      if (!s.primaryModel || !filters.models.includes(s.primaryModel)) return false;
    }
    if (filters.hasCommits === 'yes' && (s.commitCount ?? 0) === 0) return false;
    if (filters.hasCommits === 'no' && (s.commitCount ?? 0) > 0) return false;
    if (filters.hasSkills === 'yes' && (s.skillsUsed ?? []).length === 0) return false;
    if (filters.hasSkills === 'no' && (s.skillsUsed ?? []).length > 0) return false;
    if (filters.minDuration !== null && (s.durationSeconds ?? 0) < filters.minDuration) return false;
    if (filters.minFiles !== null && (s.filesEditedCount ?? 0) < filters.minFiles) return false;
    if (filters.minTokens !== null) {
      const totalTokens = Number((s.totalInputTokens ?? 0n) + (s.totalOutputTokens ?? 0n));
      if (totalTokens < filters.minTokens) return false;
    }
    if (filters.highReedit === true) {
      const filesEdited = s.filesEditedCount ?? 0;
      const reeditedFiles = s.reeditedFilesCount ?? 0;
      const reeditRate = filesEdited > 0 ? reeditedFiles / filesEdited : 0;
      if (reeditRate <= 0.2) return false;
    }
    return true;
  });
}

function generateSessions(count: number): SessionInfo[] {
  return Array.from({ length: count }, (_, i) => makeSession(i));
}

describe('filterSessions performance', () => {
  const sessions500 = generateSessions(500);
  const sessions1000 = generateSessions(1000);

  const noFilters = DEFAULT_FILTERS;

  const branchFilter: SessionFilters = {
    ...DEFAULT_FILTERS,
    branches: ['main', 'feature/auth'],
  };

  const heavyFilter: SessionFilters = {
    ...DEFAULT_FILTERS,
    branches: ['main'],
    models: ['claude-opus-4'],
    hasCommits: 'yes',
    minDuration: 1800,
    minTokens: 50000,
    highReedit: true,
  };

  bench('filter 500 sessions — no filters', () => {
    filterSessions(sessions500, noFilters);
  });

  bench('filter 500 sessions — branch filter', () => {
    filterSessions(sessions500, branchFilter);
  });

  bench('filter 500 sessions — all filters active', () => {
    filterSessions(sessions500, heavyFilter);
  });

  bench('filter 1000 sessions — no filters', () => {
    filterSessions(sessions1000, noFilters);
  });

  bench('filter 1000 sessions — all filters active', () => {
    filterSessions(sessions1000, heavyFilter);
  });
});
```

### Step 4: Run benchmarks

```bash
npx vitest bench
```

Expected output (example):
```
 ✓ src/utils/group-sessions.bench.ts
   ✓ groupSessions performance
     name                              hz        min        max       mean
     group 500 sessions by branch  12,345     0.05ms     0.15ms     0.08ms
     group 500 sessions by week     9,876     0.06ms     0.20ms     0.10ms
     ...

 ✓ src/utils/filter-sessions.bench.ts
   ✓ filterSessions performance
     name                                   hz        min        max       mean
     filter 500 sessions — no filters   45,678     0.01ms     0.05ms     0.02ms
     filter 1000 sessions — all active  23,456     0.02ms     0.08ms     0.04ms
```

**AC-13 targets:**
- Client-side grouping < 50ms for 500 sessions → expect ~0.1ms (well under)
- Client-side filtering < 50ms for 1000 sessions → expect ~0.04ms (well under)

### Step 5: Run existing tests to verify nothing broke

```bash
npx vitest run
```

Expected: All tests pass. Bench files are separate from test files — vitest only runs `.bench.ts` with `vitest bench`.

### Step 6: Commit

```bash
git add src/utils/group-sessions.bench.ts src/utils/filter-sessions.bench.ts package.json
git commit -m "perf: add vitest benchmarks for grouping and filtering

Measures groupSessions() and filterSessions() at 100/500/1000
session counts. Validates AC-13 performance targets."
```

---

## Execution Order & Dependencies

```
Task 1 (Collapsible headers)  ──┐
                                 ├──► Task 3 (Legacy removal) ──► Task 4 (Benchmarks)
Task 2 (500-session safeguard) ─┘
```

- **Tasks 1 and 2** are independent — can be done in parallel.
- **Task 3** should run after 1 and 2. It modifies HistoryView heavily, so doing it last avoids merge conflicts with the header collapse and safeguard changes.
- **Task 4** should run last — it benchmarks the final state of the code after all refactoring.

## Verification Checklist

After all 4 tasks:

- [ ] `npx vitest run` — all tests pass
- [ ] `npx vitest bench` — benchmarks run and print results
- [ ] `grep -r "FilterSortBar\|useFilterSort\|use-filter-sort" src/` — zero results
- [ ] Open app in browser: timeline view groups collapse/expand on header click
- [ ] Open app with 500+ sessions: grouping disabled, warning banner visible
- [ ] Only one sort dropdown visible in toolbar (no duplicate)
- [ ] All filter features still work via FilterPopover
