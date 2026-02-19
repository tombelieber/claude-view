# Page Reorganization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Mission Control the home page (`/`) and consolidate Fluency/Contributions/Insights into a single tabbed Analytics page (`/analytics`).

**Architecture:** Thin `AnalyticsPage` wrapper reads `?tab=` param and renders the corresponding existing component. Router changes swap the index route and add redirects for old URLs. Sidebar nav drops from 5 to 3 items.

**Tech Stack:** React Router v6, React, Tailwind CSS, Lucide icons

**Design doc:** `docs/plans/2026-02-19-page-reorg-design.md`

---

### Task 1: Create AnalyticsPage wrapper

**Files:**
- Create: `src/pages/AnalyticsPage.tsx`

**Step 1: Write the component**

```tsx
import { useSearchParams } from 'react-router-dom'
import { StatsDashboard } from '../components/StatsDashboard'
import { ContributionsPage } from './ContributionsPage'
import { InsightsPage } from '../components/InsightsPage'
import { cn } from '../lib/utils'

type AnalyticsTab = 'overview' | 'contributions' | 'insights'

const TABS: { id: AnalyticsTab; label: string }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'contributions', label: 'Contributions' },
  { id: 'insights', label: 'Insights' },
]

function isValidTab(value: string | null): value is AnalyticsTab {
  return value !== null && TABS.some(t => t.id === value)
}

export function AnalyticsPage() {
  const [searchParams, setSearchParams] = useSearchParams()
  const activeTab: AnalyticsTab = isValidTab(searchParams.get('tab'))
    ? (searchParams.get('tab') as AnalyticsTab)
    : 'overview'

  const handleTabChange = (tab: AnalyticsTab) => {
    const params = new URLSearchParams(searchParams)
    if (tab === 'overview') {
      params.delete('tab')
    } else {
      params.set('tab', tab)
    }
    setSearchParams(params)
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Tab bar */}
      <div className="flex items-center gap-1 px-6 pt-4 pb-0">
        {TABS.map(tab => (
          <button
            key={tab.id}
            type="button"
            onClick={() => handleTabChange(tab.id)}
            className={cn(
              'px-3 py-1.5 text-sm font-medium rounded-md transition-colors duration-150 cursor-pointer',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
              activeTab === tab.id
                ? 'bg-blue-500 text-white'
                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'overview' && <StatsDashboard />}
        {activeTab === 'contributions' && <ContributionsPage />}
        {activeTab === 'insights' && <InsightsPage />}
      </div>
    </div>
  )
}
```

**Step 2: Verify it compiles**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/mission-control-cde && npx tsc --noEmit src/pages/AnalyticsPage.tsx`

If tsc isn't configured for single files, just verify no red squiggles — this will be tested end-to-end in Task 4.

**Step 3: Commit**

```bash
git add src/pages/AnalyticsPage.tsx
git commit -m "feat: add AnalyticsPage wrapper with tab bar"
```

---

### Task 2: Update router

**Files:**
- Modify: `src/router.tsx`

**Step 1: Update imports and routes**

Add import for `AnalyticsPage`:
```tsx
import { AnalyticsPage } from './pages/AnalyticsPage'
```

Change the children array inside the router:

| What | Old | New |
|------|-----|-----|
| Index route | `{ index: true, element: <StatsDashboard /> }` | `{ index: true, element: <MissionControlPage /> }` |
| Add analytics | (none) | `{ path: 'analytics', element: <AnalyticsPage /> }` |
| Remove insights | `{ path: 'insights', ... }` | Delete this line |
| Remove standalone contributions | `{ path: 'contributions', element: <ContributionsPage /> }` | Replace with redirect (below) |
| Redirect /mission-control | `{ path: 'mission-control', element: <MissionControlPage /> }` | `{ path: 'mission-control', element: <Navigate to="/" replace /> }` |
| Redirect /contributions | (was standalone) | `{ path: 'contributions', element: <Navigate to="/analytics?tab=contributions" replace /> }` |
| Redirect /insights | (was standalone) | `{ path: 'insights', element: <Navigate to="/analytics?tab=insights" replace /> }` |

Keep the `StatsDashboard` import — it's still used inside `AnalyticsPage` (which imports it directly). You can remove it from `router.tsx` imports if it's no longer referenced there.

**Step 2: Verify no import errors**

Run: `bun run build` (or `npx vite build --mode development` for a quick check)

**Step 3: Commit**

```bash
git add src/router.tsx
git commit -m "feat: make Mission Control the index route, add /analytics"
```

---

### Task 3: Update sidebar nav

**Files:**
- Modify: `src/components/Sidebar.tsx`

**Step 1: Replace the 5 nav links with 3**

In the `<nav>` section (Zone 1, around lines 355-428), replace the 5 `<Link>` elements with:

```tsx
<Link
  to={`/${paramString ? `?${paramString}` : ""}`}
  className={cn(
    'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
    location.pathname === '/'
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
  )}
>
  <Monitor className="w-4 h-4" />
  <span className="font-medium">Mission Control</span>
</Link>
<Link
  to={`/sessions${paramString ? `?${paramString}` : ""}`}
  className={cn(
    'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
    location.pathname.startsWith('/sessions')
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
  )}
>
  <Clock className="w-4 h-4" />
  <span className="font-medium">Sessions</span>
</Link>
<Link
  to={`/analytics${paramString ? `?${paramString}` : ""}`}
  className={cn(
    'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
    location.pathname === '/analytics'
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
  )}
>
  <BarChart3 className="w-4 h-4" />
  <span className="font-medium">Analytics</span>
</Link>
```

**Step 2: Clean up unused imports**

Remove from the import line: `Home`, `Lightbulb` (no longer referenced in Sidebar).
Keep: `Monitor`, `Clock`, `BarChart3` (already imported).

**Step 3: Commit**

```bash
git add src/components/Sidebar.tsx
git commit -m "feat: sidebar nav reduced to 3 items"
```

---

### Task 4: Update header breadcrumbs

**Files:**
- Modify: `src/components/Header.tsx`

**Step 1: Update getBreadcrumbs()**

Replace the breadcrumb cases for `/contributions` and `/insights` with a single `/analytics` case:

```tsx
if (location.pathname === '/analytics') {
  const tab = searchParams.get('tab')
  const tabLabel = tab === 'contributions' ? 'Contributions'
    : tab === 'insights' ? 'Insights'
    : 'Overview'
  crumbs.push({
    label: `Analytics — ${tabLabel}`,
    path: location.pathname + location.search
  })
}
```

Remove the old `/contributions` and `/insights` breadcrumb cases (they'll be redirected and never hit directly).

**Step 2: Update the header title/logo link**

In the `<Link to="/">` element (line 81-88), change the title text from "Claude View" to reflect the cockpit positioning. Actually — keep "Claude View" for now; renaming is a separate plan (`2026-02-07-rename-to-claude-score.md`).

**Step 3: Commit**

```bash
git add src/components/Header.tsx
git commit -m "feat: update breadcrumbs for /analytics route"
```

---

### Task 5: End-to-end verification

**Step 1: Start the dev server**

Run: `bun run dev`

**Step 2: Verify each route manually**

| URL | Expected |
|-----|----------|
| `http://localhost:5173/` | Mission Control (kanban/grid view, live sessions) |
| `http://localhost:5173/sessions` | Session history list |
| `http://localhost:5173/analytics` | Analytics page with Overview tab active |
| `http://localhost:5173/analytics?tab=contributions` | Analytics page with Contributions tab active |
| `http://localhost:5173/analytics?tab=insights` | Analytics page with Insights tab active |
| `http://localhost:5173/mission-control` | Redirects to `/` |
| `http://localhost:5173/contributions` | Redirects to `/analytics?tab=contributions` |
| `http://localhost:5173/insights` | Redirects to `/analytics?tab=insights` |

**Step 3: Verify sidebar**

- Only 3 nav items visible: Mission Control, Sessions, Analytics
- Active state highlights correctly for each route
- Project/branch scope filter still works

**Step 4: Verify no console errors**

Open browser DevTools → Console. No React errors or warnings.

**Step 5: Commit final state (if any fixups needed)**

```bash
git add -A
git commit -m "fix: page reorg cleanup from e2e verification"
```

---

### Task 6: Update PROGRESS.md

**Files:**
- Modify: `docs/plans/PROGRESS.md`

**Step 1: Add entry for this plan**

Add to the Plan File Index table:

```markdown
| 2026-02-19-page-reorg-design.md | Page Reorganization Design | done |
| 2026-02-19-page-reorg-impl.md | Page Reorganization Implementation | done |
```

**Step 2: Commit**

```bash
git add docs/plans/PROGRESS.md docs/plans/2026-02-19-page-reorg-design.md
git commit -m "docs: page reorganization plan complete"
```
