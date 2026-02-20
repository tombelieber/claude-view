---
status: pending
date: 2026-02-10
---

# Unify Time Range Filters Across All Pages

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Align the time range filter UI, state management, and type system across the Fluency (Dashboard), Sessions, and Contributions pages so they share one component, one hook, and one type — while preserving the DateRangePicker calendar UI.

**Architecture:** Extend `useTimeRange()` with a `'today'` preset + legacy URL param migration, make it the single source of truth for all three pages. Keep the contributions API accepting named strings (backend unchanged) but map from `TimeRangePreset` via a thin `presetToApiRange()` function. Replace all bespoke time filter UIs with `TimeRangeSelector` + `DateRangePicker`.

**Tech Stack:** React, react-router-dom (URL search params), TypeScript, Axum (Rust backend)

---

## Deliberate UX Changes

These are intentional behavioral changes, not bugs:

1. **Unified default:** All pages default to `'30d'` (was `'all'` on Sessions, `'week'` on Contributions)
2. **Shared time context:** Changing the time range on any page persists via URL + localStorage. Navigating between pages preserves your selection. Previously each page had independent state.
3. **Custom date range everywhere:** Sessions and Contributions gain the DateRangePicker calendar (previously Dashboard-only).

---

## Blast Radius

### Files to modify

| File | Change |
|------|--------|
| `src/hooks/use-time-range.ts` | Add `'today'` preset, add legacy URL param migration (`week`→`7d`, etc.) |
| `src/hooks/use-time-range.test.tsx` | Add tests for `'today'` preset and legacy param migration |
| `src/hooks/use-contributions.ts` | New `ContributionsTimeRange` interface + `presetToApiRange` mapper; delete `TimeRange` type |
| `src/components/StatsDashboard.tsx` | Add `'today'` option to preset list |
| `src/components/HistoryView.tsx` | Replace local `TimeFilter` + inline buttons with `useTimeRange()` + `TimeRangeSelector` + `DateRangePicker`; update `isFiltered` check |
| `src/pages/ContributionsPage.tsx` | Replace local `range` state with `useTimeRange()` + `TimeRangeSelector` + `DateRangePicker` |
| `src/components/contributions/ContributionsHeader.tsx` | Accept `TimeRangePreset` + callbacks; render `TimeRangeSelector` + `DateRangePicker` |
| `src/components/contributions/ContributionsEmptyState.tsx` | Accept `TimeRangePreset` instead of `TimeRange`; update labels |
| `src/components/contributions/BranchCard.tsx` | Accept `ContributionsTimeRange` instead of `TimeRange` string; update default `= 'week'` → `= { preset: '7d' }` |
| `src/components/contributions/BranchList.tsx` | Accept `ContributionsTimeRange` instead of `TimeRange` string; update default `= 'week'` → `= { preset: '7d' }` |
| `src/components/contributions/index.ts` | Remove `TimeRangeFilter` re-export |
| `src/components/ContributionSummaryCard.tsx` | Remove `mapPresetToRange` bridge; add `'today'` to `titleLabel` switch |
| `e2e/dashboard-time-range.spec.ts` | Update option counts from 5→6, update nth indices for `'Today'` at position 0 |

### Files to delete

| File | Reason |
|------|--------|
| `src/components/contributions/TimeRangeFilter.tsx` | Replaced by shared `TimeRangeSelector` + `DateRangePicker` |

### Types to delete

| Type | Location | Replaced by |
|------|----------|-------------|
| `TimeRange` | `src/hooks/use-contributions.ts` | `TimeRangePreset` from `use-time-range.ts` (for presets) + `ContributionsTimeRange` (for API calls) |
| `TimeFilter` | `src/components/HistoryView.tsx` (local) | `TimeRangePreset` from `use-time-range.ts` |

### Backend — no changes needed

The contributions API accepts named range strings (`today`, `week`, `month`, `90days`, `all`, `custom`) with optional `from`/`to` as **YYYY-MM-DD date strings**. The dashboard stats API accepts `from`/`to` as **Unix timestamp integers**. These are different formats for the same param names — the frontend mapping layer handles the conversion.

> **API format warning:** `GET /api/contributions?from=YYYY-MM-DD` vs `GET /api/stats/dashboard?from=<unix_seconds>`. Never swap them.

---

## Task 1: Add `'today'` preset + legacy URL migration to `useTimeRange`

**Files:**
- Modify: `src/hooks/use-time-range.ts`
- Modify: `src/hooks/use-time-range.test.tsx`

### Step 1: Update the `TimeRangePreset` type

```ts
export type TimeRangePreset = 'today' | '7d' | '30d' | '90d' | 'all' | 'custom'
```

### Step 2: Add legacy URL param migration map

Add near the top of the file, after the type definitions:

```ts
/** Map old Contributions-page URL params to new unified presets.
 *  Keeps bookmarked /contributions?range=week URLs working after migration. */
const LEGACY_RANGE_MAP: Record<string, TimeRangePreset> = {
  week: '7d',
  month: '30d',
  '90days': '90d',
}
```

### Step 3: Update `getTimestampsFromPreset` to handle `'today'`

Use "since midnight today" semantics (not "last 24 hours") to match the backend's `TimeRange::Today`:

```ts
function getTimestampsFromPreset(preset: TimeRangePreset): { from: number | null; to: number | null } {
  if (preset === 'all') {
    return { from: null, to: null }
  }

  const now = Math.floor(Date.now() / 1000)

  if (preset === 'today') {
    // Since midnight today (matches backend's TimeRange::Today semantics)
    const midnight = new Date()
    midnight.setHours(0, 0, 0, 0)
    return { from: Math.floor(midnight.getTime() / 1000), to: now }
  }

  const days = preset === '7d' ? 7 : preset === '30d' ? 30 : 90
  const from = now - days * 86400
  return { from, to: now }
}
```

### Step 4: Update the validation arrays + apply legacy migration

Update the preset initializer (line 94-118) to apply the legacy map:

```ts
const [preset, setPresetState] = useState<TimeRangePreset>(() => {
  // Check URL params first
  const rangeParam = searchParams.get('range')
  if (rangeParam) {
    // Apply legacy migration (e.g. ?range=week → 7d)
    const migrated = LEGACY_RANGE_MAP[rangeParam]
    if (migrated) return migrated
    if (['today', '7d', '30d', '90d', 'all', 'custom'].includes(rangeParam)) {
      return rangeParam as TimeRangePreset
    }
  }
  // Check if custom timestamps are in URL
  if (searchParams.get('from') && searchParams.get('to')) {
    return 'custom'
  }
  // Fall back to localStorage
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored) {
      const parsed = JSON.parse(stored)
      if (parsed.preset && ['today', '7d', '30d', '90d', 'all', 'custom'].includes(parsed.preset)) {
        return parsed.preset as TimeRangePreset
      }
    }
  } catch (e) {
    console.warn('Failed to read time range from localStorage:', e)
  }
  // Default to 30d
  return '30d'
})
```

### Step 5: Update the label computation for `'today'`

In the `label` useMemo, add before the `preset === 'all'` check:

```ts
if (preset === 'today') {
  return 'Today'
}
```

### Step 6: Update the `comparisonLabel` for `'today'`

In the `comparisonLabel` useMemo, add before the `preset === 'all'` check:

```ts
if (preset === 'today') {
  return 'vs yesterday'
}
```

### Step 7: Add tests for `'today'` preset and legacy migration

Add to `src/hooks/use-time-range.test.tsx`:

```tsx
it('updates preset to today with correct timestamps', () => {
  const { result } = renderHook(() => useTimeRange(), { wrapper })

  act(() => {
    result.current.setPreset('today')
  })

  expect(result.current.state.preset).toBe('today')
  expect(result.current.state.fromTimestamp).not.toBeNull()
  expect(result.current.state.toTimestamp).not.toBeNull()
  // fromTimestamp should be since midnight
  const midnight = new Date()
  midnight.setHours(0, 0, 0, 0)
  expect(result.current.state.fromTimestamp).toBe(Math.floor(midnight.getTime() / 1000))
})

it('returns "Today" label for today preset', () => {
  const { result } = renderHook(() => useTimeRange(), { wrapper })

  act(() => {
    result.current.setPreset('today')
  })

  expect(result.current.label).toBe('Today')
  expect(result.current.comparisonLabel).toBe('vs yesterday')
})
```

Add a test for legacy migration using `MemoryRouter` with `initialEntries`:

```tsx
it('migrates legacy ?range=week to 7d', () => {
  function legacyWrapper({ children }: { children: React.ReactNode }) {
    return <MemoryRouter initialEntries={['/?range=week']}>{children}</MemoryRouter>
  }
  const { result } = renderHook(() => useTimeRange(), { wrapper: legacyWrapper })
  expect(result.current.state.preset).toBe('7d')
})

it('migrates legacy ?range=90days to 90d', () => {
  function legacyWrapper({ children }: { children: React.ReactNode }) {
    return <MemoryRouter initialEntries={['/?range=90days']}>{children}</MemoryRouter>
  }
  const { result } = renderHook(() => useTimeRange(), { wrapper: legacyWrapper })
  expect(result.current.state.preset).toBe('90d')
})
```

### Step 8: Run tests

```bash
bun test src/hooks/use-time-range.test.tsx
```

### Step 9: Commit

```
feat(time-range): add 'today' preset and legacy URL param migration to useTimeRange
```

---

## Task 2: Refactor `use-contributions` to accept `ContributionsTimeRange`

The contributions API expects named strings (`today`, `week`, `month`, `90days`, `all`). We need a thin mapper from `TimeRangePreset` to the backend's expected format.

**Files:**
- Modify: `src/hooks/use-contributions.ts`

### Step 1: Replace the `TimeRange` type with mapping function + new interface

Remove the exported `TimeRange` type. Add:

```ts
import type { TimeRangePreset } from './use-time-range'

/**
 * Map frontend presets to the contributions API's expected range strings.
 * NOTE: Contributions API expects YYYY-MM-DD for from/to (NOT Unix timestamps).
 * This differs from the dashboard stats API which uses Unix seconds.
 */
function presetToApiRange(preset: TimeRangePreset): string {
  switch (preset) {
    case 'today': return 'today'
    case '7d': return 'week'
    case '30d': return 'month'
    case '90d': return '90days'
    case 'all': return 'all'
    case 'custom': return 'custom'
  }
}

export interface ContributionsTimeRange {
  preset: TimeRangePreset
  from?: number | null  // unix seconds (converted to YYYY-MM-DD before sending)
  to?: number | null    // unix seconds (converted to YYYY-MM-DD before sending)
}
```

### Step 2: Update `fetchContributions` to accept the new params

```ts
async function fetchContributions(
  time: ContributionsTimeRange,
  projectId?: string,
  branch?: string
): Promise<ContributionsResponse> {
  const apiRange = presetToApiRange(time.preset)
  let url = `/api/contributions?range=${encodeURIComponent(apiRange)}`
  if (time.preset === 'custom' && time.from != null && time.to != null) {
    // Convert unix seconds to YYYY-MM-DD (contributions API format, NOT unix timestamps)
    const fromDate = new Date(time.from * 1000).toISOString().split('T')[0]
    const toDate = new Date(time.to * 1000).toISOString().split('T')[0]
    url += `&from=${fromDate}&to=${toDate}`
  }
  if (projectId) {
    url += `&projectId=${encodeURIComponent(projectId)}`
  }
  if (branch) {
    url += `&branch=${encodeURIComponent(branch)}`
  }
  const response = await fetch(url)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch contributions: ${errorText}`)
  }
  return response.json()
}
```

### Step 3: Update `useContributions` hook signature

```ts
export function useContributions(time: ContributionsTimeRange, projectId?: string, branch?: string) {
  return useQuery({
    queryKey: ['contributions', time.preset, time.from, time.to, projectId, branch],
    queryFn: () => fetchContributions(time, projectId, branch),
    staleTime: getStaleTime(time.preset),
    gcTime: 30 * 60 * 1000,
  })
}
```

### Step 4: Update `getStaleTime` to use `TimeRangePreset`

```ts
function getStaleTime(preset: TimeRangePreset): number {
  switch (preset) {
    case 'today': return 60 * 1000
    case '7d': return 5 * 60 * 1000
    case '30d': return 15 * 60 * 1000
    default: return 30 * 60 * 1000
  }
}
```

### Step 5: Update `fetchBranchSessions` and `useBranchSessions` similarly

```ts
async function fetchBranchSessions(
  branch: string,
  time: ContributionsTimeRange,
  projectId?: string
): Promise<BranchSessionsResponse> {
  const apiRange = presetToApiRange(time.preset)
  let url = `/api/contributions/branches/${encodeURIComponent(branch)}/sessions?range=${encodeURIComponent(apiRange)}`
  if (time.preset === 'custom' && time.from != null && time.to != null) {
    const fromDate = new Date(time.from * 1000).toISOString().split('T')[0]
    const toDate = new Date(time.to * 1000).toISOString().split('T')[0]
    url += `&from=${fromDate}&to=${toDate}`
  }
  if (projectId) {
    url += `&projectId=${encodeURIComponent(projectId)}`
  }
  const response = await fetch(url)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch branch sessions: ${errorText}`)
  }
  return response.json()
}

export function useBranchSessions(
  branch: string | null,
  time: ContributionsTimeRange = { preset: '7d' },
  enabled: boolean = true,
  projectId?: string
) {
  return useQuery({
    queryKey: ['branch-sessions', branch, time.preset, time.from, time.to, projectId],
    queryFn: () => fetchBranchSessions(branch!, time, projectId),
    enabled: enabled && !!branch,
    staleTime: 5 * 60 * 1000,
  })
}
```

### Step 6: Remove the old `TimeRange` type export

Delete line `export type TimeRange = 'today' | 'week' | 'month' | '90days' | 'all'` and update the re-export list at the bottom (keep all the response types, just remove `TimeRange`).

### Step 7: Commit

```
refactor(contributions): accept ContributionsTimeRange instead of named range strings
```

---

## Task 3: Update all Contributions page consumers

Now that `useContributions` and `useBranchSessions` accept `ContributionsTimeRange`, update every file that imported `TimeRange` from `use-contributions`.

**Files:**
- Modify: `src/pages/ContributionsPage.tsx`
- Modify: `src/components/contributions/ContributionsHeader.tsx`
- Modify: `src/components/contributions/ContributionsEmptyState.tsx`
- Modify: `src/components/contributions/BranchCard.tsx`
- Modify: `src/components/contributions/BranchList.tsx`
- Modify: `src/components/contributions/index.ts`
- Modify: `src/components/ContributionSummaryCard.tsx`
- Delete: `src/components/contributions/TimeRangeFilter.tsx`
- Verify: `src/components/contributions/SessionDrillDown.tsx` (imports `useSessionContribution` — signature unchanged, but verify re-exports still resolve)

### Step 1: Update `ContributionsPage.tsx`

Replace the local `range`/`useState` with `useTimeRange()`:

```tsx
import { useTimeRange } from '../hooks/use-time-range'
import { useContributions, type ContributionsTimeRange } from '../hooks/use-contributions'
import { TimeRangeSelector, DateRangePicker } from '../components/ui'
import { useIsMobile } from '../hooks/use-media-query'

// In the component:
const { state: timeRange, setPreset, setCustomRange } = useTimeRange()
const isMobile = useIsMobile()

const contribTime: ContributionsTimeRange = {
  preset: timeRange.preset,
  from: timeRange.fromTimestamp,
  to: timeRange.toTimestamp,
}

const { data, isLoading, error, refetch } = useContributions(contribTime, projectId ?? undefined, branchFilter)
```

Remove the old `handleRangeChange`, `range` state, and `initialRange` logic.

Pass time range props to `ContributionsHeader`:

```tsx
<ContributionsHeader
  preset={timeRange.preset}
  customRange={timeRange.customRange}
  onPresetChange={setPreset}
  onCustomRangeChange={setCustomRange}
  sessionCount={sessionCount}
  // ... other props unchanged
/>
```

Pass `contribTime` to `BranchList`:

```tsx
<BranchList timeRange={contribTime} /* ... */ />
```

Pass `timeRange.preset` to `ContributionsEmptyState`:

```tsx
<ContributionsEmptyState preset={timeRange.preset} onPresetChange={setPreset} />
```

### Step 2: Update `ContributionsHeader.tsx`

Change props and render `TimeRangeSelector` + `DateRangePicker`:

```tsx
import type { TimeRangePreset, CustomDateRange } from '../../hooks/use-time-range'
import { TimeRangeSelector, DateRangePicker } from '../ui'
import { useIsMobile } from '../../hooks/use-media-query'

interface ContributionsHeaderProps {
  preset: TimeRangePreset
  customRange: CustomDateRange | null
  onPresetChange: (preset: TimeRangePreset) => void
  onCustomRangeChange: (range: CustomDateRange | null) => void
  sessionCount: number
  projectFilter?: string | null
  onClearProjectFilter?: () => void
  branchFilter?: string | null
  onClearBranchFilter?: () => void
}
```

Replace the `<TimeRangeFilter ... />` at line 83 with:

```tsx
<div className="flex items-center gap-2">
  <TimeRangeSelector
    value={preset}
    onChange={onPresetChange}
    options={[
      { value: 'today', label: isMobile ? 'Today' : 'Today' },
      { value: '7d', label: isMobile ? '7 days' : '7d' },
      { value: '30d', label: isMobile ? '30 days' : '30d' },
      { value: '90d', label: isMobile ? '90 days' : '90d' },
      { value: 'all', label: isMobile ? 'All time' : 'All' },
      { value: 'custom', label: 'Custom' },
    ]}
  />
  {preset === 'custom' && (
    <DateRangePicker
      value={customRange}
      onChange={onCustomRangeChange}
    />
  )}
</div>
```

Remove the `TimeRangeFilter` import.

### Step 3: Update `ContributionsEmptyState.tsx`

```tsx
import type { TimeRangePreset } from '../../hooks/use-time-range'

interface ContributionsEmptyStateProps {
  preset: TimeRangePreset
  onPresetChange: (preset: TimeRangePreset) => void
}

const RANGE_LABELS: Record<TimeRangePreset, string> = {
  today: 'today',
  '7d': 'this week',
  '30d': 'this month',
  '90d': 'the last 90 days',
  all: 'all time',
  custom: 'the selected range',
}
```

Update the component to use `preset` and `onPresetChange` props instead of `range` and `onRangeChange`. The `isFiltered` check becomes `preset !== 'all'`.

### Step 4: Update `BranchList.tsx` and `BranchCard.tsx`

Change `timeRange: TimeRange` prop to `timeRange: ContributionsTimeRange` **and update default values**:

```tsx
import type { ContributionsTimeRange } from '../../hooks/use-contributions'

// Props:
timeRange?: ContributionsTimeRange
```

**Critical:** Both files have `timeRange = 'week'` as the default parameter value. This must change to `timeRange = { preset: '7d' }`:

- `BranchCard.tsx:30` — `timeRange = 'week'` → `timeRange = { preset: '7d' }`
- `BranchList.tsx:34` — `timeRange = 'week'` → `timeRange = { preset: '7d' }`

Update `useBranchSessions` calls to pass `ContributionsTimeRange` instead of the old string.

### Step 5: Update `ContributionSummaryCard.tsx`

Remove the `mapPresetToRange` bridge function. Add `'today'` to `titleLabel` switch:

```tsx
import type { TimeRangePreset } from '../hooks/use-time-range'
import type { ContributionsTimeRange } from '../hooks/use-contributions'

export function ContributionSummaryCard({ className, timeRange, project, branch }: ContributionSummaryCardProps) {
  const contribTime: ContributionsTimeRange = {
    preset: (timeRange?.preset as TimeRangePreset) || '30d',
    from: timeRange?.fromTimestamp,
    to: timeRange?.toTimestamp,
  }
  const { data, isLoading, error } = useContributions(contribTime, project, branch)

  // ...

  const titleLabel = (() => {
    switch (timeRange?.preset) {
      case 'today': return 'AI Contribution Today'
      case '7d': return 'AI Contribution This Week'
      case '30d': return 'AI Contribution This Month'
      case '90d': return 'AI Contribution (90 Days)'
      case 'all': return 'AI Contribution (All Time)'
      case 'custom': return 'AI Contribution (Custom Range)'
      default: return 'AI Contribution This Month'
    }
  })()
```

### Step 6: Update `contributions/index.ts` barrel export

Remove the `TimeRangeFilter` re-export:

```ts
// Theme 3: Contributions components
export { ContributionsHeader } from './ContributionsHeader'
// REMOVED: export { TimeRangeFilter } from './TimeRangeFilter'
export { OverviewCards } from './OverviewCards'
export { TrendChart } from './TrendChart'
export { InsightLine, InsightLineCompact } from './InsightLine'
export { ContributionsEmptyState } from './ContributionsEmptyState'
```

### Step 7: Delete `src/components/contributions/TimeRangeFilter.tsx`

### Step 8: Verify `SessionDrillDown.tsx`

This file imports `useSessionContribution` from `use-contributions`. Its signature is unchanged, but verify the import still resolves after removing `TimeRange` from the module's exports.

### Step 9: Run type check + tests

```bash
bunx tsc --noEmit && bun test
```

### Step 10: Commit

```
refactor(contributions): use shared TimeRangeSelector + useTimeRange across contributions page
```

---

## Task 4: Migrate Sessions page to shared `useTimeRange`

**Files:**
- Modify: `src/components/HistoryView.tsx`

**Cross-hook URL coordination note:** After this change, `useTimeRange()` and `useSessionFilters()` will both write to URL search params on the Sessions page. Both use copy-then-modify (`new URLSearchParams(existing)`), so they preserve each other's keys. They write to non-overlapping param names (`range`/`from`/`to` vs `sort`/`groupBy`/`viewMode`/etc.), so no collision. The `useTimeRange` effect fires on state change; `useSessionFilters` fires on user action. No race condition.

### Step 1: Replace local time filter state with `useTimeRange()`

Remove:
```tsx
type TimeFilter = 'all' | 'today' | '7d' | '30d'
// ...
const [timeFilter, setTimeFilter] = useState<TimeFilter>('all')
```

Add:
```tsx
import { useTimeRange } from '../hooks/use-time-range'
import { TimeRangeSelector, DateRangePicker } from './ui'
import { useIsMobile } from '../hooks/use-media-query'

// In the component:
const { state: timeRange, setPreset, setCustomRange } = useTimeRange()
const isMobile = useIsMobile()
```

### Step 2: Update the client-side filtering logic

Replace the cutoff calculation in `filteredSessions` useMemo:

```tsx
// Replace:
const cutoffs: Record<TimeFilter, number> = { ... }
const cutoff = cutoffs[timeFilter]

// With:
const cutoff = timeRange.fromTimestamp ?? 0
```

This works because `useTimeRange` already computes `fromTimestamp` for each preset (including `'today'`). For `'all'`, `fromTimestamp` is `null`, and `?? 0` gives 0 (no cutoff).

### Step 3: Replace inline time filter buttons with `TimeRangeSelector` + `DateRangePicker`

Replace the `<div className="flex items-center gap-0.5 p-0.5 bg-gray-100 ...">` block (lines 415-429) with:

```tsx
<TimeRangeSelector
  value={timeRange.preset}
  onChange={setPreset}
  options={[
    { value: 'today', label: isMobile ? 'Today' : 'Today' },
    { value: '7d', label: isMobile ? '7 days' : '7d' },
    { value: '30d', label: isMobile ? '30 days' : '30d' },
    { value: '90d', label: isMobile ? '90 days' : '90d' },
    { value: 'all', label: isMobile ? 'All time' : 'All' },
    { value: 'custom', label: 'Custom' },
  ]}
/>
{timeRange.preset === 'custom' && (
  <DateRangePicker
    value={timeRange.customRange}
    onChange={setCustomRange}
  />
)}
```

### Step 4: Update `clearAll` to reset time range

```tsx
function clearAll() {
  setSearchText('')
  setPreset('all')
  setSelectedDate(null)
  setFilters(DEFAULT_FILTERS)
}
```

### Step 5: Remove the old `timeOptions` array definition (lines 287-292)

### Step 6: Update the `filteredSessions` dependency array

Replace `timeFilter` with `timeRange.fromTimestamp` in the useMemo deps.

### Step 7: Update the `isFiltered` check (line 241)

Replace `timeFilter !== 'all'` with `timeRange.preset !== '30d'` (the new default):

```tsx
// Before:
const isFiltered = searchText || ... || timeFilter !== 'all' || selectedDate || ...

// After:
const isFiltered = searchText || ... || timeRange.preset !== '30d' || selectedDate || ...
```

### Step 8: Run type check + tests

```bash
bunx tsc --noEmit && bun test
```

### Step 9: Commit

```
refactor(sessions): use shared useTimeRange + TimeRangeSelector
```

---

## Task 5: Add `'today'` to Dashboard preset options

**Files:**
- Modify: `src/components/StatsDashboard.tsx`

### Step 1: Add `'today'` to the options array

```tsx
<TimeRangeSelector
  value={timeRange.preset}
  onChange={setPreset}
  options={[
    { value: 'today', label: isMobile ? 'Today' : 'Today' },
    { value: '7d', label: isMobile ? '7 days' : '7d' },
    { value: '30d', label: isMobile ? '30 days' : '30d' },
    { value: '90d', label: isMobile ? '90 days' : '90d' },
    { value: 'all', label: isMobile ? 'All time' : 'All' },
    { value: 'custom', label: 'Custom' },
  ]}
/>
```

### Step 2: Commit

```
feat(dashboard): add 'today' preset to time range selector
```

---

## Task 6: Update E2E tests

**Files:**
- Modify: `e2e/dashboard-time-range.spec.ts`

### Step 1: Update TC-2A-01 (desktop segmented control)

Change option count from 5 to 6 and update label assertions:

```ts
// Verify all 6 options exist as radio buttons
const radioButtons = segmentedControl.locator('button[role="radio"]')
await expect(radioButtons).toHaveCount(6)

// Verify the labels (Today is now first)
await expect(radioButtons.nth(0)).toHaveText('Today')
await expect(radioButtons.nth(1)).toHaveText('7d')
await expect(radioButtons.nth(2)).toHaveText('30d')
await expect(radioButtons.nth(3)).toHaveText('90d')
await expect(radioButtons.nth(4)).toHaveText('All')
await expect(radioButtons.nth(5)).toHaveText('Custom')

// Verify "30d" is selected by default (aria-checked="true")
await expect(radioButtons.nth(2)).toHaveAttribute('aria-checked', 'true')

// Verify others are not selected
await expect(radioButtons.nth(0)).toHaveAttribute('aria-checked', 'false')
await expect(radioButtons.nth(1)).toHaveAttribute('aria-checked', 'false')
await expect(radioButtons.nth(3)).toHaveAttribute('aria-checked', 'false')
await expect(radioButtons.nth(4)).toHaveAttribute('aria-checked', 'false')
await expect(radioButtons.nth(5)).toHaveAttribute('aria-checked', 'false')
```

### Step 2: Update TC-2A-02 (mobile dropdown)

Change option count from 5 to 6 and update label/index assertions:

```ts
const options = dropdown.locator('option')
await expect(options).toHaveCount(6)

await expect(options.nth(0)).toHaveText('Today')
await expect(options.nth(1)).toHaveText('7 days')
await expect(options.nth(2)).toHaveText('30 days')
await expect(options.nth(3)).toHaveText('90 days')
await expect(options.nth(4)).toHaveText('All time')
await expect(options.nth(5)).toHaveText('Custom')
```

### Step 3: Update TC-2A-03 and other tests that locate buttons by text

These tests use `{ hasText: '7d' }` which still works regardless of position. Verify the `.first()` selector on `btn7d` is still needed (it is, since "7d" might match inside "7 days" — but `hasText` is substring-match, so `.first()` is correct).

### Step 4: Add a test for `?range=today` URL persistence (in TC-2A-05)

```ts
// --- Navigate with ?range=today ---
await page.goto('/?range=today')
await page.waitForLoadState('domcontentloaded')
await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

const btnToday = segmentedControl.locator('button[role="radio"]', { hasText: 'Today' })
await expect(btnToday).toHaveAttribute('aria-checked', 'true')
```

### Step 5: Commit

```
test(e2e): update dashboard time range tests for 'today' preset (6 options)
```

---

## Task 7: Final verification and cleanup

### Step 1: Grep for any remaining references to the old `TimeRange` type from `use-contributions`

```bash
grep -rn "type TimeRange" src/ --include="*.ts" --include="*.tsx"
```

Should only find `TimeRangePreset` in `use-time-range.ts` and `ContributionsTimeRange` in `use-contributions.ts`.

### Step 2: Grep for any remaining `TimeRangeFilter` import

```bash
grep -rn "TimeRangeFilter" src/ --include="*.ts" --include="*.tsx"
```

Should return nothing.

### Step 3: Run TypeScript type check

```bash
bunx tsc --noEmit
```

### Step 4: Run unit tests

```bash
bun test
```

### Step 5: Manual verification checklist

- [ ] Same `TimeRangeSelector` segmented control appears on all three pages
- [ ] All pages show 6 options: Today, 7d, 30d, 90d, All, Custom
- [ ] Selecting "Custom" shows the `DateRangePicker` calendar popover on all pages
- [ ] URL persists the range (`?range=7d`, `?range=today`, or `?from=...&to=...`)
- [ ] Navigating between pages preserves the time range selection
- [ ] All presets produce correct data on each page
- [ ] Legacy URL `/contributions?range=week` correctly resolves to 7d
- [ ] Legacy URL `/contributions?range=90days` correctly resolves to 90d
- [ ] `ContributionSummaryCard` on Dashboard shows "AI Contribution Today" when today is selected
- [ ] `SessionDrillDown` modal still works (imports from `use-contributions` still resolve)

### Step 6: Commit

```
chore: verify unified time range filters across all pages
```

---

## Summary

| Before | After |
|--------|-------|
| 3 different types (`TimeRangePreset`, `TimeFilter`, `TimeRange`) | 1 type: `TimeRangePreset` (+`ContributionsTimeRange` for API calls) |
| 3 different UIs (segmented, inline buttons, dropdown) | 1 UI: `TimeRangeSelector` + `DateRangePicker` |
| 3 different persistence strategies (URL+localStorage, local state, URL) | 1 strategy: `useTimeRange()` (URL + localStorage) |
| 3 different defaults (30d, all, week) | 1 default: `30d` |
| No custom date range on Sessions/Contributions | Custom date range everywhere (calendar picker preserved) |
| `mapPresetToRange` bridge function | Direct timestamp passthrough via `ContributionsTimeRange` |
| Bookmarked `/contributions?range=week` URLs | Legacy migration shim in `useTimeRange` |
| E2E tests assert 5 options | Updated to assert 6 options |
