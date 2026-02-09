---
status: pending
date: 2026-02-09
---

# Sidebar UX Overhaul: Three-Zone Architecture

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure the sidebar into three visually separated zones (Navigation, Scope, Quick Jump) that eliminate filter duplication, clarify the difference between scoping and navigating, and give users session-level precision without duplicating the Sessions page.

**Architecture:** The sidebar is split into three zones with distinct purposes: Zone 1 (nav tabs — unchanged), Zone 2 (scope panel — project/branch tree that sets `?project=`/`?branch=` URL params as global filters), Zone 3 (quick jump — shows 3-5 most recent sessions within the current scope for direct navigation). The Sessions page duplicate project filter is removed. Each zone has its own visual separator, section label, and interaction pattern.

**Tech Stack:** React 18, React Router v6 (`useSearchParams`), TanStack Query v5, Tailwind CSS, Lucide icons

---

## Background & Rationale

### Problems Being Solved

1. **Three competing filter systems**: Sidebar sets `?project=`/`?branch=`, Sessions page has its own local `selectedProjects` state + `showProjectFilter` dropdown + `SessionToolbar`, Contributions page reads from URL only. Users can't tell which filter is "active."

2. **Sidebar mixes navigation with filtering**: Clicking a project in the sidebar sets a filter (doesn't navigate) AND expands branches. Clicking a nav tab navigates. The tree items look the same but do different things.

3. **No session-level precision from sidebar**: To reach a specific session, users must go to Sessions page → scroll/search. There's no "quick jump" path for "I was just working on X."

4. **Duplicate project filter on Sessions page**: `HistoryView.tsx` lines 86-87 + 430-478 have a local `selectedProjects` Set and dropdown that duplicates sidebar scope. This causes confusion about which filter is "active."

### Design Principles

- **Spatial consistency**: Sidebar zones never change based on current page. Same tree, same state, every page.
- **Progressive disclosure**: Zone 3 (Quick Jump) only appears when a scope is active (project selected). When no project is selected, it's hidden — the user hasn't narrowed enough for quick-jump to be useful.
- **One source of truth per filter level**: Global scope = sidebar (`?project=`/`?branch=`). Page filters = content area. No duplication.
- **Navigation vs. Scoping are visually distinct**: Scope items (project/branch) use highlight + toggle behavior. Navigation items (nav tabs, quick jump sessions) navigate to URLs.

### Files That Will Be Changed

| File | Change Type | What Changes |
|------|------------|--------------|
| `src/components/Sidebar.tsx` | **Major rewrite** | Split into 3 zones, add Quick Jump, add section labels, add scope clear button |
| `src/components/HistoryView.tsx` | **Remove duplicate filter** | Delete `selectedProjects`, `showProjectFilter`, local project dropdown |
| `src/hooks/use-recent-sessions.ts` | **New file** | Hook to fetch recent sessions for Quick Jump zone |
| `src/components/Sidebar.test.tsx` | **New file** | Tests for the 3-zone sidebar |

### Files That Will NOT Be Changed

- `ContributionsPage.tsx` — Already reads from `?project=`/`?branch=` correctly
- `StatsDashboard.tsx` — Already reads from `?project=`/`?branch=` correctly
- `Header.tsx` — No changes needed
- `use-session-filters.ts` — Already handles URL param coordination correctly
- `use-contributions.ts` — No changes needed
- `router.tsx` — No route changes needed

---

## Task 1: Create `useRecentSessions` Hook (Quick Jump Data)

**Files:**
- Create: `src/hooks/use-recent-sessions.ts`
- Test: `src/hooks/use-recent-sessions.test.ts`

**Why:** Quick Jump zone needs recent sessions scoped to the current project+branch filter. We need a lightweight hook that reuses existing API infrastructure.

**Step 1: Write the failing test**

```ts
// src/hooks/use-recent-sessions.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useRecentSessions } from './use-recent-sessions'

// Mock fetch
const mockFetch = vi.fn()
global.fetch = mockFetch

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
}

describe('useRecentSessions', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('returns empty array when no project is selected', () => {
    const { result } = renderHook(
      () => useRecentSessions(null, null),
      { wrapper: createWrapper() }
    )
    expect(result.current.data).toEqual([])
    expect(result.current.isLoading).toBe(false)
    expect(mockFetch).not.toHaveBeenCalled()
  })

  it('fetches recent sessions when project is selected', async () => {
    const mockSessions = [
      { id: 's1', preview: 'Fix auth', modifiedAt: 1707400000 },
      { id: 's2', preview: 'Add tests', modifiedAt: 1707300000 },
    ]
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ sessions: mockSessions, total: 2 }),
    })

    const { result } = renderHook(
      () => useRecentSessions('my-project', null),
      { wrapper: createWrapper() }
    )

    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.data).toHaveLength(2)
    expect(result.current.data[0].id).toBe('s1')
  })

  it('includes branch filter in API call when provided', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ sessions: [], total: 0 }),
    })

    renderHook(
      () => useRecentSessions('my-project', 'feature/auth'),
      { wrapper: createWrapper() }
    )

    await waitFor(() => expect(mockFetch).toHaveBeenCalled())
    const url = mockFetch.mock.calls[0][0] as string
    expect(url).toContain('branch=feature%2Fauth')
  })

  it('limits to 5 results', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ sessions: Array(10).fill({ id: 'x', preview: 'y', modifiedAt: 0 }), total: 10 }),
    })

    const { result } = renderHook(
      () => useRecentSessions('my-project', null),
      { wrapper: createWrapper() }
    )

    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.data).toHaveLength(5)
  })
})
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/theme3-contributions && npx vitest run src/hooks/use-recent-sessions.test.ts`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```ts
// src/hooks/use-recent-sessions.ts
import { useQuery } from '@tanstack/react-query'

const QUICK_JUMP_LIMIT = 5

export interface RecentSession {
  id: string
  preview: string
  modifiedAt: number
  gitBranch?: string
  project?: string
}

/**
 * Fetch recent sessions for the Quick Jump sidebar zone.
 *
 * Returns the N most recent sessions scoped to the current
 * project + branch filter. Returns empty array when disabled
 * (no project selected).
 */
export function useRecentSessions(
  project: string | null,
  branch: string | null
) {
  return useQuery({
    queryKey: ['recent-sessions', project, branch],
    queryFn: async (): Promise<RecentSession[]> => {
      let url = `/api/projects/${encodeURIComponent(project!)}/sessions?limit=${QUICK_JUMP_LIMIT}&sort=recent`
      if (branch) {
        url += `&branch=${encodeURIComponent(branch)}`
      }
      const res = await fetch(url)
      if (!res.ok) throw new Error('Failed to fetch recent sessions')
      const data = await res.json()
      // API returns { sessions: [...], total: N }
      return (data.sessions ?? []).slice(0, QUICK_JUMP_LIMIT)
    },
    enabled: !!project,
    staleTime: 60 * 1000, // 1 min — Quick Jump should feel fresh
    gcTime: 5 * 60 * 1000,
    // When disabled (no project), return stable empty array
    placeholderData: [],
  })
}
```

**Step 4: Run test to verify it passes**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/theme3-contributions && npx vitest run src/hooks/use-recent-sessions.test.ts`
Expected: PASS (all 4 tests)

**Step 5: Commit**

```bash
git add src/hooks/use-recent-sessions.ts src/hooks/use-recent-sessions.test.ts
git commit -m "feat: add useRecentSessions hook for sidebar Quick Jump zone"
```

---

## Task 2: Restructure Sidebar into Three Visual Zones

**Files:**
- Modify: `src/components/Sidebar.tsx` (full rewrite of render structure, keep existing hooks/logic)

**Why:** The sidebar currently has two visual sections (nav tabs + project tree) with no clear separation. We need three clearly separated zones with distinct visual treatment and labels.

**Step 1: Write the failing test**

```tsx
// src/components/Sidebar.test.tsx
import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Sidebar } from './Sidebar'

const mockProjects = [
  { name: 'project-a', displayName: 'Project A', sessionCount: 10, path: '/a' },
  { name: 'project-b', displayName: 'Project B', sessionCount: 5, path: '/b' },
]

function renderSidebar(initialEntries = ['/'], searchParams = '') {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[`${initialEntries[0]}${searchParams ? `?${searchParams}` : ''}`]}>
        <Sidebar projects={mockProjects} />
      </MemoryRouter>
    </QueryClientProvider>
  )
}

describe('Sidebar three-zone structure', () => {
  it('renders Zone 1: navigation tabs', () => {
    renderSidebar()
    expect(screen.getByRole('link', { name: /fluency/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /sessions/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /contributions/i })).toBeInTheDocument()
  })

  it('renders Zone 2: scope panel with section label', () => {
    renderSidebar()
    expect(screen.getByText(/scope/i)).toBeInTheDocument()
    expect(screen.getByRole('tree', { name: /projects/i })).toBeInTheDocument()
  })

  it('does NOT render Zone 3 (Quick Jump) when no project is selected', () => {
    renderSidebar()
    expect(screen.queryByText(/recent/i)).not.toBeInTheDocument()
  })

  it('renders Zone 3 (Quick Jump) when a project is selected', () => {
    renderSidebar(['/'], 'project=project-a')
    expect(screen.getByText(/recent/i)).toBeInTheDocument()
  })

  it('renders scope clear button when project is selected', () => {
    renderSidebar(['/'], 'project=project-a')
    expect(screen.getByRole('button', { name: /clear scope/i })).toBeInTheDocument()
  })
})
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/Sidebar.test.tsx`
Expected: FAIL — missing "Scope" label, missing "Recent" label, missing clear button

**Step 3: Rewrite Sidebar render structure**

The key changes in `Sidebar.tsx`:

1. **Add section labels**: "SCOPE" header above project tree, "RECENT" header above Quick Jump
2. **Add scope clear button**: Small "×" next to the SCOPE label when `?project=` is set
3. **Add Quick Jump zone**: New bottom section that renders `<QuickJumpList>` only when `selectedProjectId` is set
4. **Visual separators**: Each zone separated by a border-b with slightly different bg tone

**Structural changes to the JSX (preserve all existing hooks and logic above the return):**

Replace the current `<aside>` return with:

```tsx
return (
  <aside className="w-72 bg-gray-50/80 dark:bg-gray-900/80 border-r border-gray-200 dark:border-gray-700 flex flex-col overflow-hidden">
    {/* ─── Zone 1: Navigation Tabs ─── */}
    <nav className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 space-y-1" aria-label="Main navigation">
      {/* ...existing nav links (Fluency, Sessions, Contributions)... */}
      {/* UNCHANGED — keep the existing IIFE that builds preservedParams */}
    </nav>

    {/* ─── Zone 2: Scope Panel ─── */}
    <div className="flex flex-col min-h-0 flex-1">
      {/* Scope header + view mode toggle */}
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
            Scope
          </span>
          {selectedProjectId && (
            <button
              type="button"
              onClick={() => {
                const newParams = new URLSearchParams(searchParams)
                newParams.delete('project')
                newParams.delete('branch')
                setSearchParams(newParams)
              }}
              className="text-[10px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors flex items-center gap-0.5"
              aria-label="Clear scope"
            >
              <X className="w-3 h-3" />
              Clear
            </button>
          )}
        </div>
        {/* View mode + expand/collapse controls (existing) */}
        <div className="flex items-center gap-2">
          {/* ...existing view mode toggle + expand/collapse buttons... */}
        </div>
      </div>

      {/* Project tree (existing, scrollable) */}
      <div
        className="flex-1 overflow-y-auto py-1"
        role="tree"
        aria-label="Projects"
        onKeyDown={handleKeyDown}
      >
        {flattenedNodes.map((node, i) => renderTreeNode(node, i))}
      </div>
    </div>

    {/* ─── Zone 3: Quick Jump (only when scoped) ─── */}
    {selectedProjectId && (
      <QuickJumpZone project={selectedProjectId} branch={searchParams.get('branch')} />
    )}
  </aside>
)
```

**New `QuickJumpZone` component** (defined in same file, below `BranchList`):

```tsx
import { useRecentSessions } from '../hooks/use-recent-sessions'
import { buildSessionUrl } from '../lib/url-utils'
import { Link, useSearchParams } from 'react-router-dom'
import { Clock, ArrowRight } from 'lucide-react'

function QuickJumpZone({ project, branch }: { project: string; branch: string | null }) {
  const [searchParams] = useSearchParams()
  const { data: sessions, isLoading } = useRecentSessions(project, branch)

  if (isLoading) {
    return (
      <div className="border-t border-gray-200 dark:border-gray-700 px-3 py-2">
        <div className="h-4 w-16 bg-gray-200 dark:bg-gray-700 rounded animate-pulse mb-2" />
        {[1, 2, 3].map(i => (
          <div key={i} className="h-6 bg-gray-200 dark:bg-gray-700 rounded animate-pulse mb-1.5" style={{ width: `${60 + i * 10}%` }} />
        ))}
      </div>
    )
  }

  if (!sessions || sessions.length === 0) return null

  return (
    <div className="border-t border-gray-200 dark:border-gray-700 px-3 py-2">
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
          Recent
        </span>
        <Link
          to={`/sessions?project=${encodeURIComponent(project)}${branch ? `&branch=${encodeURIComponent(branch)}` : ''}`}
          className="text-[10px] text-gray-400 hover:text-blue-500 transition-colors flex items-center gap-0.5"
        >
          All <ArrowRight className="w-2.5 h-2.5" />
        </Link>
      </div>
      <div className="space-y-0.5">
        {sessions.map(session => (
          <Link
            key={session.id}
            to={buildSessionUrl(session.id, searchParams)}
            className={cn(
              'flex items-center gap-2 px-2 py-1 h-6 rounded text-[11px] transition-colors',
              'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70',
              'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 focus-visible:outline-none'
            )}
            title={session.preview}
          >
            <Clock className="w-3 h-3 flex-shrink-0 text-gray-400 dark:text-gray-500" />
            <span className="truncate flex-1">{session.preview || '(untitled)'}</span>
            <span className="text-[10px] text-gray-400 dark:text-gray-500 tabular-nums flex-shrink-0">
              {formatRelativeTimeShort(session.modifiedAt)}
            </span>
          </Link>
        ))}
      </div>
    </div>
  )
}

function formatRelativeTimeShort(timestamp: number): string {
  const diff = Date.now() / 1000 - timestamp
  if (diff < 3600) return `${Math.floor(diff / 60)}m`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`
  return `${Math.floor(diff / 86400)}d`
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/components/Sidebar.test.tsx`
Expected: PASS

**Step 5: Run existing tests to check for regressions**

Run: `npx vitest run --reporter=verbose`
Expected: All existing tests still pass

**Step 6: Commit**

```bash
git add src/components/Sidebar.tsx src/components/Sidebar.test.tsx
git commit -m "feat: restructure sidebar into three zones (nav, scope, quick jump)"
```

---

## Task 3: Remove Duplicate Project Filter from Sessions Page

**Files:**
- Modify: `src/components/HistoryView.tsx`

**Why:** The Sessions page has its own `selectedProjects` state and dropdown that duplicates what the sidebar Scope zone already does. This is the primary source of "which filter is active?" confusion. Remove it.

**Step 1: Write the test that validates the removal**

Add to existing `CompactSessionTable.test.tsx` or create a new test:

```tsx
// Add to existing HistoryView tests or create src/components/HistoryView.test.tsx
describe('HistoryView filter consolidation', () => {
  it('does NOT render a local project filter dropdown', () => {
    // Render HistoryView
    // Assert: no element with text "Projects" in a button context
    // Assert: no dropdown with project checkboxes
    expect(screen.queryByRole('button', { name: /^projects$/i })).not.toBeInTheDocument()
  })

  it('respects sidebar project scope from URL params', () => {
    // Render with ?project=my-project in URL
    // Assert: sessions are filtered to my-project only
  })
})
```

**Step 2: Remove the following from `HistoryView.tsx`**

Delete these state declarations (around lines 86-91):
```diff
- const [selectedProjects, setSelectedProjects] = useState<Set<string>>(new Set())
- const [showProjectFilter, setShowProjectFilter] = useState(false)
```

Delete the `filterRef` (line 91):
```diff
- const filterRef = useRef<HTMLDivElement>(null)
```

Delete the `showProjectFilter` effect (lines 106-116):
```diff
- // Close project filter on outside click
- useEffect(() => {
-   function handleClick(e: MouseEvent) { ... }
-   if (showProjectFilter) { ... }
- }, [showProjectFilter])
```

Delete the `sortedProjects` memo (lines 290-292):
```diff
- const sortedProjects = useMemo(() => {
-   return [...(summaries ?? [])].sort((a, b) => b.sessionCount - a.sessionCount)
- }, [summaries])
```

Delete `toggleProject` function (lines 294-304):
```diff
- function toggleProject(name: string) { ... }
```

Remove `selectedProjects` from `filteredSessions` filter (around line 164):
```diff
-     // Local project filter (from session toolbar dropdown)
-     if (selectedProjects.size > 0 && !selectedProjects.has(s.project)) return false
```

Remove `selectedProjects` from the `filteredSessions` dependency array (line 248):
```diff
- }, [allSessions, searchText, selectedProjects, sidebarProject, sidebarBranch, timeFilter, selectedDate, filters])
+ }, [allSessions, searchText, sidebarProject, sidebarBranch, timeFilter, selectedDate, filters])
```

Remove `selectedProjects` from `isFiltered` check (line 250):
```diff
- const isFiltered = searchText || selectedProjects.size > 0 || sidebarProject || ...
+ const isFiltered = searchText || sidebarProject || ...
```

Remove `selectedProjects` from `clearAll` (lines 306-312):
```diff
  function clearAll() {
    setSearchText('')
-   setSelectedProjects(new Set())
    setTimeFilter('all')
    setSelectedDate(null)
    setFilters(DEFAULT_FILTERS)
  }
```

Delete the entire project filter dropdown JSX block (lines 430-478):
```diff
-           {/* Project filter dropdown */}
-           <div className="relative" ref={filterRef}>
-             <button onClick={() => setShowProjectFilter(!showProjectFilter)} ...>
-               ...
-             </button>
-             {showProjectFilter && ( ... dropdown JSX ... )}
-           </div>
```

**Clean up imports**: Remove `FolderOpen` from lucide-react imports if no longer used.

**Step 3: Run tests**

Run: `npx vitest run --reporter=verbose`
Expected: PASS — no regressions

**Step 4: Commit**

```bash
git add src/components/HistoryView.tsx
git commit -m "refactor: remove duplicate project filter from Sessions page (sidebar is single source of truth)"
```

---

## Task 4: Add Active Scope Indicator to Sessions Page

**Files:**
- Modify: `src/components/HistoryView.tsx`

**Why:** After removing the local project filter, we need to show users that the sidebar scope is active. Add a small read-only indicator when `?project=` is set, with a link to clear it.

**Step 1: Add scope indicator JSX**

In `HistoryView.tsx`, after the search input and before the filter row, add:

```tsx
{/* Sidebar scope indicator (read-only — scope is controlled by sidebar) */}
{sidebarProject && (
  <div className="flex items-center gap-2 px-3 py-1.5 bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800 rounded-lg text-xs">
    <FolderOpen className="w-3.5 h-3.5 text-blue-500" />
    <span className="text-blue-700 dark:text-blue-300 font-medium truncate">
      {sidebarProject.split('/').pop()}
    </span>
    {sidebarBranch && (
      <>
        <span className="text-blue-300 dark:text-blue-600">/</span>
        <span className="text-blue-600 dark:text-blue-400 truncate">{sidebarBranch}</span>
      </>
    )}
    <button
      onClick={() => {
        const params = new URLSearchParams(searchParams)
        params.delete('project')
        params.delete('branch')
        setSearchParams(params)
      }}
      className="ml-auto text-blue-400 hover:text-blue-600 dark:text-blue-500 dark:hover:text-blue-300 transition-colors"
      aria-label="Clear project scope"
    >
      <X className="w-3.5 h-3.5" />
    </button>
  </div>
)}
```

**Step 2: Re-add `FolderOpen` to imports** if removed in Task 3.

**Step 3: Run tests and visual check**

Run: `npx vitest run --reporter=verbose`
Expected: PASS

**Step 4: Commit**

```bash
git add src/components/HistoryView.tsx
git commit -m "feat: add read-only scope indicator to Sessions page when sidebar project is active"
```

---

## Task 5: Accessibility & Keyboard Navigation Audit

**Files:**
- Modify: `src/components/Sidebar.tsx`

**Why:** The three zones need proper ARIA landmark roles and keyboard behavior:
- Zone 1 (nav): already a `<nav>` — good
- Zone 2 (scope): tree role — already present, but needs `aria-label` update
- Zone 3 (quick jump): needs `<nav aria-label="Recent sessions">` wrapper
- The Quick Jump links need to be reachable via Tab without being trapped in the tree

**Step 1: Verify ARIA structure**

Check that:
1. Zone 1 has `<nav aria-label="Main navigation">`
2. Zone 2 tree has `<div role="tree" aria-label="Projects">`
3. Zone 3 has `<nav aria-label="Recent sessions">`
4. Tab key moves between zones (not trapped in tree)
5. Arrow keys work within Zone 2 tree (existing behavior, keep it)

**Step 2: Add landmark roles to Quick Jump zone**

In the `QuickJumpZone` component, wrap in `<nav aria-label="Recent sessions">`:

```tsx
<nav aria-label="Recent sessions" className="border-t ...">
  {/* ...existing content... */}
</nav>
```

**Step 3: Test keyboard flow**

Manual test:
1. Tab into sidebar → focuses first nav link (Fluency)
2. Tab → cycles through nav links
3. Tab → enters project tree
4. Arrow up/down → navigates tree items
5. Tab → exits tree, enters Quick Jump zone
6. Tab → cycles through Quick Jump session links
7. Tab → exits sidebar entirely

**Step 4: Run all tests**

Run: `npx vitest run --reporter=verbose`
Expected: PASS

**Step 5: Commit**

```bash
git add src/components/Sidebar.tsx
git commit -m "a11y: add landmark roles and keyboard navigation for three-zone sidebar"
```

---

## Task 6: Final Integration Test & Edge Cases

**Files:**
- Modify: `src/components/Sidebar.test.tsx`

**Why:** Test the full integration of all three zones, including edge cases.

**Step 1: Add edge case tests**

```tsx
describe('Sidebar edge cases', () => {
  it('Quick Jump zone hides when project scope is cleared', () => {
    // Render with ?project=foo → Quick Jump visible
    // Simulate clearing scope → Quick Jump disappears
  })

  it('Quick Jump zone updates when branch scope changes', () => {
    // Render with ?project=foo → shows project-scoped sessions
    // Change to ?project=foo&branch=main → shows branch-scoped sessions
  })

  it('nav links preserve current scope params', () => {
    // Render with ?project=foo&branch=main
    // Click Sessions tab → navigates to /sessions?project=foo&branch=main
  })

  it('scope clear button removes both project and branch params', () => {
    // Render with ?project=foo&branch=main
    // Click clear → both params gone
  })

  it('handles project with zero sessions gracefully', () => {
    // Select a project with 0 sessions
    // Quick Jump shows empty state or stays hidden
  })
})
```

**Step 2: Run full test suite**

Run: `npx vitest run --reporter=verbose`
Expected: ALL PASS

**Step 3: Final commit**

```bash
git add -A
git commit -m "test: add integration tests for sidebar three-zone architecture"
```

---

## Dependency Graph

```
Task 1 (hook) ────────────┐
                           ├──→ Task 2 (sidebar rewrite) ──→ Task 5 (a11y)
                           │                                       │
Task 3 (remove dup) ──────┤                                       ├──→ Task 6 (integration)
                           │                                       │
Task 4 (scope indicator) ──┘                                       │
                                                                   │
```

**Parallelizable:**
- Task 1 and Task 3 can run in parallel (no shared files)
- Task 2 depends on Task 1 (imports the hook)
- Task 4 depends on Task 3 (modifies same file after removals)
- Task 5 depends on Task 2
- Task 6 depends on all others

## Success Criteria

- [ ] Sidebar has three visually distinct zones with section labels
- [ ] Zone 1 (nav tabs) unchanged, preserves scope params on navigation
- [ ] Zone 2 (scope) shows project/branch tree with clear button
- [ ] Zone 3 (quick jump) appears only when scoped, shows 3-5 recent sessions
- [ ] Quick Jump items navigate to session detail (not filter)
- [ ] Sessions page has NO local project dropdown
- [ ] Sessions page shows read-only scope indicator when sidebar scope is active
- [ ] All existing tests pass
- [ ] Keyboard navigation works across all three zones
- [ ] URL params are properly preserved (copy-then-modify, never `new URLSearchParams()`)
