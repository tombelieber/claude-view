---
status: approved
date: 2026-02-07
---

# Navigation Restructure: Flat Top-Level Pages with Unified Project Filtering

## Problem Statement

Current navigation has inconsistent structure:

- **Fluency** and **Sessions** are top-level pages in the sidebar
- **Contributions** is nested under project routes with a top tab bar (`/project/:projectId/contributions`)
- Project selection in sidebar **navigates** to a project page rather than **filtering** the current view
- Users must navigate into a project to see its contributions, breaking the mental model

**Desired behavior:**

- All three pages (Fluency, Sessions, Contributions) should be **top-level siblings** in the left sidebar
- Project selection in sidebar should **filter** the current view, not navigate away
- Branch selection in sidebar should **filter** the current view (within selected project)
- No top tab bars - all navigation happens via sidebar
- Consistent filtering: `?project=foo&branches=main` works on all three pages

## Design Goals

1. **Flat navigation hierarchy:** Fluency | Sessions | Contributions (all same level)
2. **Unified project filtering:** Sidebar projects filter any page you're on (`?project=foo`)
3. **Unified branch filtering:** Sidebar branches filter any page you're on (`?project=foo&branches=main`)
4. **Remove redundant UI:** Eliminate ProjectLayout top tabs
5. **Preserve functionality:** All existing features still work (session viewing, search, etc.)
6. **Backwards compatibility:** Redirect old URLs to new structure

## Current State Analysis

### Current Router Structure

```tsx
/                           → StatsDashboard (Fluency)
/sessions                   → HistoryView
/project/:projectId         → ProjectLayout
  ├─ (index)                → ProjectView (sessions)
  └─ /contributions         → ContributionsPage
/project/:projectId/session/:slug → ConversationView
/search                     → SearchResults
/settings                   → SettingsPage

// Redirects
/history                    → /sessions
/contributions              → / (homepage)
/session/:projectId/:sessionId → /project/:projectId/session/:sessionId (legacy)
```

### Current Sidebar Behavior

- **Top section:** Fluency, Sessions (navigation links)
- **Bottom section:** Project tree (navigation to `/project/:projectId`)
- **Branch expansion:** Clicking a project expands to show branches
- **Branch selection:** Clicking a branch navigates to `/project/:projectId?branches=branchName`

### Current ProjectLayout Behavior

- Wraps ProjectView and ContributionsPage
- Shows top tab bar: "Sessions" | "Contributions"
- Validates project exists, redirects if not
- Preserves URL params when switching tabs

## Target State Design

### New Router Structure

```tsx
/                           → StatsDashboard (Fluency) - supports ?project=foo
/sessions                   → HistoryView - supports ?project=foo
/contributions              → ContributionsPage - supports ?project=foo
/session/:sessionId         → ConversationView (FLAT - just session ID)
/search                     → SearchResults
/settings                   → SettingsPage

// Redirects (for old bookmarks)
/project/:projectId         → /?project=:projectId (preserve project context)
/project/:projectId/contributions → /contributions?project=:projectId
/project/:projectId/session/:slug → /session/:sessionId (extract ID from slug)
/history                    → /sessions

// REMOVE these legacy routes entirely:
// ❌ /session/:projectId/:sessionId (old format - deprecate)
```

**Rationale for flattening session route:**

- Fully flat navigation - no project context in URL
- Session ID is globally unique, no need for project prefix
- Simpler mental model - all top-level pages are flat
- ConversationView fetches session data which includes project info

### New Sidebar Behavior

**Structure:**

```text
┌─────────────────────────┐
│ 🏠 Fluency             │ (navigation)
│ 🕐 Sessions            │ (navigation)
│ 📊 Contributions       │ (navigation)
├─────────────────────────┤
│ 📂 Projects            │ (filtering)
│   > claude-view  [42]  │
│   > claude-view  [230] │
│   v my-app       [15]  │ (expanded, selected when ?project=my-app)
│     ├ main       [10]  │ (clickable, adds ?branches=main)
│     └ feature-x  [5]   │
└─────────────────────────┘
```

**Interaction model:**

1. **Navigation links (top section):**
    - Click "Fluency" → navigate to `/`
    - Click "Sessions" → navigate to `/sessions`
    - Click "Contributions" → navigate to `/contributions`
    - Preserve existing `?project=` and `?branches=` params when navigating

2. **Project filtering (bottom section):**
    - Click a project → **toggle** project filter on current page
        - If not selected: add `?project=foo` to current URL
        - If already selected: remove `?project=foo` (return to global view)
    - Visual state: highlight project when `searchParams.get('project')` matches
    - Auto-expand project when selected to show branches

3. **Branch filtering (within projects):**
    - Only shown when project is expanded
    - Click a branch → add `?branches=bar` to current URL (preserves `?project=foo`)
    - Click again → remove `?branches=bar`
    - Visual state: highlight branch when params match

### Page Filtering Behavior

#### 1. Fluency Page (`StatsDashboard`)

**Current:** Shows global stats (all projects)

**New:**

- Default: Global stats when no `?project=` param
- Filtered: Project-specific stats when `?project=foo` is present

**Changes needed:**

- Read `project` from URL searchParams
- Pass to `useDashboardStats(project?)`
- Backend: filter all stats queries by project when provided
- UI adjustment: Hide or replace "Most Active Projects" card when filtered
  - Option A: Hide the card entirely
  - Option B: Show "Branch Activity" for the filtered project
  - **Recommendation:** Hide when filtered (keeps UI simple)

**Example URLs:**

- `/?project=claude-view` - Fluency for claude-view project (all branches)
- `/?project=claude-view&branches=main` - Fluency for claude-view main branch only

#### 2. Sessions Page (`HistoryView`)

**Current:** Shows all sessions, supports filtering via SessionToolbar

**New:**

- Default: All sessions when no `?project=` param
- Filtered: Project sessions when `?project=foo` is present

**Changes needed:**

- Verify existing filter logic respects `?project=` param
- Verify SessionToolbar doesn't conflict with sidebar project filter
- May need to add project filter chip in toolbar when active
- Ensure "Clear filters" button also clears project param

**Example URLs:**

- `/sessions?project=claude-view` - Sessions for claude-view
- `/sessions?project=claude-view&branches=main&sort=recent` - Filtered by project + branch + sort

#### 3. Contributions Page (`ContributionsPage`)

**Current:** Project-scoped via route param (`/project/:projectId/contributions`)

**New:**

- Default: Global contributions when no `?project=` param
- Filtered: Project contributions when `?project=foo` is present

**Changes needed:**

- Change from `useParams().projectId` to `useSearchParams().get('project')`
- Update `useContributions(range, project?)` call
- Verify all child components handle null project (for global view)
- Update ContributionsHeader to show "All Projects" vs "Project: X"

**Example URLs:**

- `/contributions` - Global contributions
- `/contributions?project=claude-view&range=month` - claude-view contributions for last month

#### 4. Session Detail Page (`ConversationView`)

**Current:** Wrapped in ProjectLayout, accessed via `/project/:projectId/session/:slug`

**New:** Keep the route structure, but remove ProjectLayout wrapper

**Changes needed:**

- Remove ProjectLayout wrapper from this route
- ConversationView needs to fetch/display project context independently
- Keep URL structure: `/project/:projectId/session/:slug`
- Sidebar should highlight the project if it matches the session's project

**Rationale:**

- Session detail is a focused view, not a dashboard
- Keeping project in URL provides context
- No conflict with new top-level navigation

### URL Parameter Coordination

**Core principle:** URL is the single source of truth for filters

**Parameters:**

- `?project=<name>` - Project filter (applies to Fluency, Sessions, Contributions)
- `?branches=<name>` - Branch filter (only meaningful when project is set)
- `?range=<week|month|all>` - Time range (Contributions only)
- `?sort=<recent|tokens|...>` - Sort order (Sessions only)
- Other filters: `?hasCommits=yes`, `?models=sonnet`, etc.

**Navigation behavior:**

- Clicking sidebar links (Fluency/Sessions/Contributions) **preserves** `?project=` and `?branches=`
- Clicking a project **toggles** `?project=`, **preserves** other params
- Clicking a branch **toggles** `?branches=`, **preserves** `?project=` and other params
- Each page's filter controls **merge** with existing params (never replace entire query string)

**Example flow:**

1. User on Fluency, clicks "claude-view" project
    - URL: `/?project=claude-view`
2. User expands project, clicks "main" branch
    - URL: `/?project=claude-view&branches=main`
3. User clicks "Sessions" in sidebar
    - URL: `/sessions?project=claude-view&branches=main`
    - Sessions page shows filtered results
4. User clicks "claude-view" project again (deselect)
    - URL: `/sessions` (both project and branches cleared)

## Implementation Plan

### Phase 1: Router Restructuring ✅

**Goal:** Flatten routes, remove ProjectLayout from top-level pages

**Files to modify:**

- `src/router.tsx`

**Changes:**

1. Remove ProjectLayout wrapper for project routes
2. Move `/contributions` to top-level
3. Keep session detail route unchanged
4. Add redirects for old URLs

**New router.tsx:**

```tsx
export const router = createBrowserRouter([
    {
        path: "/",
        element: <App />,
        children: [
            { index: true, element: <StatsDashboard /> },
            { path: "sessions", element: <HistoryView /> },
            { path: "contributions", element: <ContributionsPage /> },
            { path: "settings", element: <SettingsPage /> },
            { path: "search", element: <SearchResults /> },

            // Session detail view (flat - just session ID)
            { path: "session/:sessionId", element: <ConversationView /> },

            // Redirects for old bookmarks
            { path: "history", element: <Navigate to="/sessions" replace /> },
            { path: "project/:projectId", element: <ProjectRedirect /> },
            {
                path: "project/:projectId/contributions",
                element: <ContributionsRedirect />,
            },
            {
                path: "project/:projectId/session/:slug",
                element: <OldSessionRedirect />,
            },
        ],
    },
]);

// Redirect old project pages to filtered dashboard
function ProjectRedirect() {
    const { projectId } = useParams();
    return (
        <Navigate to={`/?project=${encodeURIComponent(projectId!)}`} replace />
    );
}

// Redirect old contributions pages to filtered contributions
function ContributionsRedirect() {
    const { projectId } = useParams();
    return (
        <Navigate
            to={`/contributions?project=${encodeURIComponent(projectId!)}`}
            replace
        />
    );
}

// Redirect old session URLs to new flat structure
function OldSessionRedirect() {
    const { slug } = useParams();
    // Extract session ID from slug (format: "session-title-abc123" -> "abc123")
    const sessionId = slug?.split("-").pop() || slug;
    return <Navigate to={`/session/${sessionId}`} replace />;
}
```

**Test cases:**

- ✅ Old URL `/project/foo` redirects to `/?project=foo`
- ✅ Old URL `/project/foo/contributions` redirects to `/contributions?project=foo`
- ✅ Old URL `/project/foo/session/bar-123` redirects to `/session/123`
- ✅ New session URLs work: `/session/abc123`
- ✅ No broken links in existing components

### Phase 2: Sidebar Refactor 🔄

**Goal:** Add Contributions link, change project clicks to filter instead of navigate

**Files to modify:**

- `src/components/Sidebar.tsx`

**Changes:**

**Step 1:** Add Contributions link in top section:

```tsx
<Link
    to="/contributions"
    className={cn(
        "flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors",
        location.pathname === "/contributions"
            ? "bg-blue-500 text-white"
            : "text-gray-600 dark:text-gray-400 hover:bg-gray-200/70",
    )}
>
    <BarChart3 className="w-4 h-4" />
    <span className="font-medium">Contributions</span>
</Link>
```

**Step 2:** Change project click behavior from navigation to filtering:

**Current:**

```tsx
const handleProjectClick = useCallback(
    (node: ProjectTreeNode) => {
        // Toggle expand/collapse + navigate
        navigate(`/project/${encodeURIComponent(node.name)}`);
    },
    [navigate],
);
```

**New:**

```tsx
const handleProjectClick = useCallback(
    (node: ProjectTreeNode) => {
        if (node.type !== "project") return;

        const currentProject = searchParams.get("project");
        const isSelected = currentProject === node.name;

        // Toggle project filter
        const newParams = new URLSearchParams(searchParams);
        if (isSelected) {
            // Deselect: clear project and branches
            newParams.delete("project");
            newParams.delete("branches");
        } else {
            // Select: set project, clear branches (will re-populate from new project)
            newParams.set("project", node.name);
            newParams.delete("branches");
        }

        // Update URL preserving other params
        setSearchParams(newParams);

        // Toggle expand/collapse
        setExpandedProjects((prev) => {
            const next = new Set(prev);
            if (next.has(node.name)) {
                next.delete(node.name);
            } else {
                next.add(node.name);
            }
            return next;
        });
    },
    [searchParams, setSearchParams],
);
```

**Step 3:** Update project selection highlighting:

**Current:**

```tsx
const selectedProjectId = params.projectId
    ? decodeURIComponent(params.projectId)
    : null;
```

**New:**

```tsx
const selectedProjectId = searchParams.get("project");
```

**Step 4:** Update branch click behavior to preserve project param:

**Current (in BranchList component):**

```tsx
const handleBranchClick = useCallback(
    (branch: string | null) => {
        const params = new URLSearchParams(window.location.search);
        if (branch) {
            params.set("branches", branch);
        } else {
            params.delete("branches");
        }
        navigate(`/project/${encodeURIComponent(projectName)}?${params}`);
    },
    [projectName, navigate],
);
```

**New:**

```tsx
const handleBranchClick = useCallback(
    (branch: string | null) => {
        const params = new URLSearchParams(window.location.search);
        if (branch) {
            params.set("branches", branch);
        } else {
            params.delete("branches");
        }
        // Stay on current page, just update params
        navigate(`${location.pathname}?${params}`);
    },
    [location.pathname, navigate],
);
```

**Edge cases:**

- When user navigates between pages (Fluency → Sessions), preserve selected project
- When project is selected but not expanded, auto-expand it
- When user is on `/project/:id/session/:slug`, highlight that project in sidebar

**Test cases:**

- ✅ Clicking project toggles filter on current page
- ✅ Clicking project again deselects it
- ✅ Branch filtering works within selected project
- ✅ Navigating between pages preserves project/branch selection
- ✅ URL is always source of truth

### Phase 3: StatsDashboard (Fluency) Project Filtering 📊

**Goal:** Add project filtering to dashboard

**Files to modify:**

- `src/components/StatsDashboard.tsx`
- `src/hooks/use-dashboard.ts` (or similar)
- `crates/server/src/routes/stats.rs` (backend)

**Frontend changes:**

1. Read project from URL:

```tsx
export function StatsDashboard() {
    const [searchParams] = useSearchParams();
    const projectFilter = searchParams.get("project") || undefined;

    const {
        data: stats,
        isLoading,
        error,
        refetch,
    } = useDashboardStats(projectFilter);
    // ... rest of component
}
```

**Step 2:** Update header to show filter state:

```tsx
<div className="flex items-center justify-between mb-4">
    <div className="flex items-center gap-2">
        <BarChart3 className="w-5 h-5 text-[#7c9885]" />
        <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
            {projectFilter
                ? `${projectFilter} Usage`
                : "Your Claude Code Usage"}
        </h1>
    </div>
    {projectFilter && (
        <button
            onClick={() => {
                const params = new URLSearchParams(searchParams);
                params.delete("project");
                setSearchParams(params);
            }}
            className="text-xs text-gray-500 hover:text-gray-700"
        >
            Clear filter
        </button>
    )}
</div>
```

**Step 3:** Conditionally hide "Most Active Projects" when filtered:

```tsx
{
    !projectFilter && (
        <div className="bg-white dark:bg-gray-900 rounded-xl ...">
            <h2>Most Active Projects</h2>
            {/* ... */}
        </div>
    );
}
```

**Backend changes:**

Update `/api/stats` endpoint to accept `?project=` and `?branches=` query params:

```rust
#[derive(Deserialize)]
pub struct StatsQuery {
    pub project: Option<String>,
    pub branches: Option<String>,
}

pub async fn stats_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<StatsOverview>, ApiError> {
    let stats = state.db.get_dashboard_stats(
        query.project.as_deref(),
        query.branches.as_deref()
    ).await?;
    Ok(Json(stats))
}
```

Update `db/src/queries.rs`:

```rust
pub async fn get_dashboard_stats(
    &self,
    project_filter: Option<&str>,
    branch_filter: Option<&str>
) -> Result<StatsOverview> {
    // Build WHERE clause based on filters
    let mut conditions = vec![];
    if let Some(p) = project_filter {
        conditions.push(format!("project = '{}'", p));
    }
    if let Some(b) = branch_filter {
        conditions.push(format!("git_branch = '{}'", b));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Apply to all stat queries...
}
```

**Test cases:**

- ✅ Dashboard shows global stats by default
- ✅ Dashboard shows project-specific stats when `?project=foo`
- ✅ Dashboard shows project+branch stats when `?project=foo&branches=main`
- ✅ "Most Active Projects" card hidden when filtered
- ✅ All metrics (trends, tool usage, heatmap) respect both filters
- ✅ Clear filter button removes both `?project=` and `?branches=` params

### Phase 4: HistoryView (Sessions) Project Filtering 📜

**Goal:** Ensure Sessions page respects project filter from sidebar

**Files to check:**

- `src/components/HistoryView.tsx` (or equivalent)
- Existing filter logic

**Changes:**

1. Verify HistoryView reads `?project=` param:

```tsx
const [searchParams] = useSearchParams();
const projectFilter = searchParams.get("project");
```

**Step 2:** Verify SessionToolbar doesn't conflict:
    - If SessionToolbar has its own project filter, merge with sidebar filter
    - Or hide toolbar's project filter when sidebar has one selected

**Step 3:** Show active filter chip when project is selected:

```tsx
{
    projectFilter && (
        <div className="flex items-center gap-2 mb-4">
            <span className="text-sm text-gray-600">Filtered by project:</span>
            <span className="px-2 py-1 bg-blue-100 rounded text-sm">
                {projectFilter}
            </span>
            <button onClick={clearProjectFilter}>×</button>
        </div>
    );
}
```

**Test cases:**

- ✅ Sessions page shows all sessions by default
- ✅ Sessions page shows project sessions when `?project=foo`
- ✅ Sessions page shows project+branch sessions when `?project=foo&branches=main`
- ✅ Other filters (model, commits, skills) work with project+branch filter
- ✅ Sorting works with project+branch filter
- ✅ Pagination works with project+branch filter

### Phase 5: ContributionsPage Project Filtering 📊

**Goal:** Change from route-based project scope to query param filter

**Files to modify:**

- `src/pages/ContributionsPage.tsx`

**Changes:**

**Step 1:** Change data source from route param to query param:

**Current:**

```tsx
const { projectId: rawProjectId } = useParams();
const projectId = rawProjectId ? decodeURIComponent(rawProjectId) : null;
```

**New:**

```tsx
const [searchParams] = useSearchParams();
const projectId = searchParams.get("project");
```

**Step 2:** Update header to reflect global vs filtered view:

**Current:**

```tsx
<ContributionsHeader
    range={range}
    onRangeChange={handleRangeChange}
    sessionCount={sessionCount}
/>
```

**New:**

```tsx
<ContributionsHeader
    range={range}
    onRangeChange={handleRangeChange}
    sessionCount={sessionCount}
    projectFilter={projectId}
    onClearProjectFilter={() => {
        const params = new URLSearchParams(searchParams);
        params.delete("project");
        setSearchParams(params);
    }}
/>
```

**Step 3:** Verify all child components handle null projectId:
    - OverviewCards
    - TrendChart
    - EfficiencyMetricsSection
    - ModelComparison
    - BranchList
    - All should show global data when projectId is null

**Test cases:**

- ✅ Contributions page shows global data by default
- ✅ Contributions page shows project data when `?project=foo`
- ✅ Contributions page shows project+branch data when `?project=foo&branches=main`
- ✅ All metrics (overview, trends, efficiency) respect both filters
- ✅ Branch breakdown shows all branches when no branch filter, single branch when filtered
- ✅ Time range filter works with project+branch filters
- ✅ Empty state shows appropriate message for filtered vs global view

### Phase 6: Link Updates 🔗

**Goal:** Update all links throughout the app to use new URL structure

**Files to search:**

- All components linking to projects
- All components linking to sessions

**Changes:**

**Step 1:** Project links - Change from navigation to filtering

**Find all:**

```tsx
<Link to={`/project/${projectName}`}>
navigate(`/project/${projectName}`)
```

**Replace with:** (context-dependent)

- If in sidebar: change to filter toggle (already done in Phase 2)
- If in dashboard "Most Active Projects": remove or change to filter action
- If elsewhere: decide case-by-case (some may still want to navigate)

**Step 2:** Session links - Update to new flat structure

**Find all:**

```tsx
<Link to={`/project/${encodeURIComponent(session.project)}/session/${sessionSlug(session.preview, session.id)}`}>
```

**Replace with:**

```tsx
<Link to={`/session/${session.id}`}>
```

**Affected files:**

- `src/components/ProjectView.tsx` (session cards)
- `src/components/SessionCard.tsx` (if used elsewhere)
- `src/components/StatsDashboard.tsx` (longest sessions)
- Any other components linking to sessions

**Step 3:** Contributions links - Change to top-level

**Find all:**

```tsx
<Link to={`/project/${project}/contributions`}>
```

**Replace with:**

```tsx
<Link to={`/contributions?project=${encodeURIComponent(project)}`}>
```

**Step 4:** "Open Full Session" button in ContributionsPage - CRITICAL FIX

**Current (BROKEN):**

```tsx
// src/pages/ContributionsPage.tsx
onOpenFullSession={(sessionId) => {
  // Navigate to full session view (if implemented)
  window.location.href = `/sessions/${sessionId}`  // ❌ WRONG - plural
}
```

**Fix to:**

```tsx
onOpenFullSession={(sessionId) => {
  navigate(`/session/${sessionId}`)  // ✅ Correct - singular, flat structure
}
```

**Also update SessionDrillDown modal navigation:**

- Ensure all session navigation uses `/session/:id` format
- Remove any project context from URLs

**Test cases:**

- ✅ No broken links after changes
- ✅ All project references use filter pattern
- ✅ All session references use flat `/session/:id` structure
- ✅ "Open Full Session" button in contributions drill-down works
- ✅ All contributions references use new URL

### Phase 7: Remove ProjectLayout Component 🗑️

**Goal:** Clean up unused ProjectLayout component

**Files to remove/modify:**

- `src/components/ProjectLayout.tsx` (remove entirely)
- `src/components/ProjectLayout.test.tsx` (remove if exists)
- `src/router.tsx` (already removed usage in Phase 1)

**Verification:**

```bash
# Ensure no remaining references
grep -r "ProjectLayout" src/
```

**Test cases:**

- ✅ No imports of ProjectLayout remain
- ✅ No test failures from removal
- ✅ Build succeeds without component

### Phase 8: ConversationView Independence 🔍

**Goal:** Ensure session detail page works with flat URL structure

**Files to modify:**

- `src/components/ConversationView.tsx`

**Changes:**

**Step 1:** Change route param:

```tsx
// OLD
const { projectId, slug } = useParams();
const sessionId = extractIdFromSlug(slug);

// NEW
const { sessionId } = useParams();
```

**Step 2:** Fetch session data independently:

```tsx
const { data: session } = useSession(sessionId);
// session includes project info
```

**Step 3:** Display project context in the view:

```tsx
<div className="text-sm text-gray-500">
    Project: {session.project}
    {session.gitBranch && ` • Branch: ${session.gitBranch}`}
</div>
```

**Step 4:** Sidebar highlighting:
    - Read project from session data
    - Highlight that project in sidebar (visual feedback only, no filtering)

**Test cases:**

- ✅ Can access session via `/session/abc123`
- ✅ Old URLs redirect: `/project/foo/session/bar-123` → `/session/123`
- ✅ Session view displays project context
- ✅ Sidebar highlights correct project when viewing session
- ✅ Back navigation works correctly

### Phase 9: Testing & Edge Cases 🧪

**Test matrix:**

| Test Case               | Fluency            | Sessions             | Contributions               |
| ----------------------- | ------------------ | -------------------- | --------------------------- |
| No filters (global)     | ✅                 | ✅                   | ✅                          |
| Project filter          | ✅                 | ✅                   | ✅                          |
| Project + branch filter | ✅                 | ✅                   | ✅                          |
| Navigate between pages  | Preserves filters  | Preserves filters    | Preserves filters           |
| Deselect project        | Returns to global  | Returns to global    | Returns to global           |
| Bookmark URL            | Works              | Works                | Works                       |
| Old URL redirect        | ✅                 | N/A                  | ✅                          |
| Empty state             | Shows global empty | Shows filtered empty | Shows global/filtered empty |

**Edge cases to test:**

1. **Filter persistence:**
    - Select project on Fluency → navigate to Sessions → project still selected ✅
    - Select project + branch → navigate → both preserved ✅

2. **URL bookmarking:**
    - `/?project=foo` loads with filter applied ✅
    - `/sessions?project=foo&branches=main` works ✅
    - Old URL `/project/foo` redirects correctly ✅

3. **Project deletion:**
    - If project in URL no longer exists, show error or clear filter ✅

4. **Multiple browser tabs:**
    - URL is source of truth, each tab has independent state ✅

5. **Session detail navigation:**
    - Viewing session `/session/abc123` highlights project in sidebar (visual only) ✅
    - Clicking project in sidebar while viewing session navigates to Fluency with filter ✅
    - Old session URLs redirect correctly ✅

6. **Empty results:**
    - Project filter with no sessions shows "No sessions in this project" ✅
    - Project filter with no contributions shows appropriate message ✅

7. **Performance:**
    - Large project lists don't slow down sidebar ✅
    - Filter changes are instant (no loading flicker) ✅

8. **Contributions page session navigation:**
    - "Open Full Session" button in SessionDrillDown modal navigates correctly ✅
    - Session drill-down from branch cards works ✅
    - All session links use flat `/session/:id` format ✅

## Backend API Contract

### Required Endpoints

All endpoints must support optional `?project=` and `?branches=` query parameters:

1. **GET /api/stats**
    - Query params: `?project=<name>&branches=<name>`
    - Returns: StatsOverview (filtered by project and/or branch if provided)
    - Note: `branches` param only meaningful when `project` is also provided

2. **GET /api/sessions**
    - Query params: `?project=<name>&branches=<name>&limit=50&sort=recent`
    - Returns: Paginated sessions list
    - Note: Existing `branch` param may need rename to `branches` for consistency

3. **GET /api/contributions**
    - Query params: `?project=<name>&branches=<name>&range=<week|month|all>`
    - Returns: ContributionsResponse

### Backend Verification Checklist

- ✅ `/api/stats` supports `?project=` and `?branches=` params
- ✅ `/api/sessions` supports `?project=` and `?branches=` params (verify naming consistency)
- ✅ `/api/contributions` supports `?project=` and `?branches=` params (already implemented)
- ✅ All queries use parameterized SQL (no injection risk)
- ✅ Project and branch names are URL-encoded/decoded correctly
- ✅ Missing project/branch returns empty results, not error
- ✅ Branch filter without project filter is handled gracefully (ignore or error)

## Migration & Rollout

### User-Facing Changes

**Before:**

- Contributions is hidden inside project pages
- Must navigate to project to see its stats
- Top tabs for switching views

**After:**

- Contributions is a top-level page
- Sidebar filters any page by project
- No top tabs, cleaner navigation

### Migration Strategy

1. **No data migration needed** - all data stays the same
2. **URL redirects handle old bookmarks** - users won't hit 404s
3. **Gradual rollout:** Can deploy as single release (no breaking changes)

### User Education

Consider adding:

- **First-time tooltip:** "Projects now filter instead of navigate"
- **Help doc update:** Explain new navigation model
- **Changelog entry:** Document the restructure

## Success Metrics

- ✅ All three pages support project filtering
- ✅ No broken links or 404s
- ✅ Old URLs redirect correctly
- ✅ Navigation is consistent and intuitive
- ✅ Performance is maintained or improved

## Open Questions

### Q1: Should branch filtering work without project selection?

**Option A:** Branch filter only works when project is selected

- Simpler logic
- Current behavior

**Option B:** Allow branch filter across all projects

- More flexible
- Requires backend changes

**Recommendation:** Option A (current behavior)

### Q2: What happens when navigating with both ?project and route /project/:id?

**Answer:** This won't happen - we're removing `/project/:id` routes except for session detail. Session detail route doesn't conflict with query params.

### Q3: Should the dashboard "Most Active Projects" card show anything when filtered?

**Options:**

- A: Hide completely (recommended - simplest)
- B: Show "Branch Activity" for filtered project
- C: Show grayed out message "Filtered to project"

**Recommendation:** Option A (hide when filtered)

### Q4: Should we keep the `/project/:projectId/session/:slug` URL or flatten it?

**Answer:** Flatten to `/session/:sessionId`. Reasons:

- Fully flat navigation - consistent with top-level design
- Session IDs are globally unique
- Simpler mental model
- ConversationView can fetch and display project context from session data
- Old URLs redirect automatically for backwards compatibility

## Rollback Plan

If issues arise post-deployment:

1. **Revert router.tsx** - restore ProjectLayout routes
2. **Revert Sidebar.tsx** - restore navigation behavior
3. **Keep redirects** - old URLs still work

Risk: Low - changes are mostly frontend routing, no data changes.

## Appendix: File Checklist

### Must Modify

- [ ] `src/router.tsx` - flatten routes, add redirects
- [ ] `src/components/Sidebar.tsx` - add Contributions link, change project behavior
- [ ] `src/components/StatsDashboard.tsx` - add project filtering
- [ ] `src/pages/ContributionsPage.tsx` - change from route param to query param
- [ ] `crates/server/src/routes/stats.rs` - add project filter support (if not exists)

### Must Remove

- [ ] `src/components/ProjectLayout.tsx` - no longer needed
- [ ] `src/components/ProjectLayout.test.tsx` - if exists

### Verify/Update

- [ ] `src/components/HistoryView.tsx` - ensure project+branch filtering works
- [ ] `src/components/ConversationView.tsx` - update to flat `/session/:id` structure
- [ ] `src/components/SessionCard.tsx` - update session links to flat structure
- [ ] `src/components/contributions/SessionDrillDown.tsx` - fix "Open Full Session" button
- [ ] `src/pages/ContributionsPage.tsx` - fix onOpenFullSession callback
- [ ] `src/components/StatsDashboard.tsx` - update longest session links to flat structure
- [ ] `src/hooks/use-dashboard.ts` - add project+branch param support
- [ ] All components with project links - update to filter pattern
- [ ] All components with session links - update to flat `/session/:id` pattern

### Backend (if needed)

- [ ] `crates/server/src/routes/stats.rs` - project filtering
- [ ] `crates/db/src/queries.rs` - filtered stat queries
- [ ] Integration tests for filtered endpoints

---

**Status:** Draft - awaiting review and validation
**Next Steps:** Review design, validate completeness, begin Phase 1 implementation
