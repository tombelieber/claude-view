---
status: approved
date: 2026-02-04
theme: "Theme 1: Session Discovery & Navigation"
reviewed: 2026-02-05
---

# Session Discovery & Navigation — Design

> **Problem:** Users accumulate hundreds of sessions across dozens of projects and branches. The current UI shows sessions chronologically with basic metrics, but doesn't help users answer: *"Where is the session with the work I'm looking for?"*

## Design System

- **Style:** Data-Dense Dashboard
- **Typography:** Fira Code / Fira Sans (existing)
- **Colors:** Blue data (#1E40AF / #3B82F6) + amber highlights (#F59E0B) (existing)
- **Icons:** Lucide (existing)
- **Key UX rules:** Hover tooltips, row highlighting on hover, smooth filter animations (150-300ms), skeleton loaders, cursor-pointer on all clickable elements, `prefers-reduced-motion` respected

## Approach: 3 Layers of Discovery

| Layer | Solves | Implementation |
|-------|--------|----------------|
| **Enhanced Session Cards** | "What is this session about?" | Branch tag, LOC impact, top files touched |
| **Smart Grouping & Filtering** | "Show me all sessions for feature X" | Group-by toggle, multi-facet filters, compact table view |
| **Sidebar Improvements** | "Navigate my project structure" | Branch list on expand, list/tree view toggle |

**Not included (YAGNI):** Kanban view (sessions aren't tasks), Gallery view (no visual content), Backlog view (same problem as kanban), more than 2 view modes.

---

## Layer 1: Enhanced Session Card

### Current State

Card shows: time range, duration, preview text, last message, prompts/tokens/files/re-edits, commits badge, skills badges.

### Problem

All sessions look the same. Can't distinguish a 2-hour deep refactor from a "what does this error mean?" quick ask.

### Design

```
+-----------------------------------------------------------+
|  +----------------+                                       |
|  | feature/auth   |  Today 2:30 PM -> 3:15 PM  |  45 min |
|  +----------------+                                       |
|                                                           |
|  "Add JWT authentication to the login endpoint..."        |
|  -> "All tests passing, auth middleware deployed"         |
|                                                           |
|  12 prompts . 45K tokens . 8 files . 3 re-edits          |
|  +342 / -89 lines                                         |
|                                                           |
|  auth.ts . middleware.ts . test.ts  +5 more               |
|  +-------------------------------------------+           |
|  | * 3 commits   tdd  commit                 |           |
|  +-------------------------------------------+           |
+-----------------------------------------------------------+
```

### New Elements

| Element | Data Source | UX Detail |
|---------|-----------|------------|
| **Branch badge** | `session.gitBranch` | `GitBranch` Lucide icon + truncated branch name, muted pill style. Hidden if `null`. Placed in header row before time range. |
| **LOC impact** | Phase 1: estimate from Edit/Write tool call content lengths. Phase 2: overlay with git diff stats when commits linked. | Green `+N` / red `-N` inline after metrics row. Uses `tabular-nums` for alignment. |
| **Top files touched** | `session.filesEdited` (already tracked) | Show up to 3 filenames (basename only via `path.split('/').pop()`), `+N more` overflow. `FileEdit` Lucide icon. New row between metrics and footer. |

### LOC Estimation Strategy

**Two-phase approach:**

1. **Tool-call estimate (always available):** During deep index, parse `Edit` and `Write` tool_use results to estimate lines changed. Edit tool calls contain `old_string` / `new_string` — diff those for lines added/removed. Write tool calls = lines added (new file). Approximate but works for all sessions including uncommitted work.

2. **Git diff stats (when commits linked):** During git sync, extract `insertions` / `deletions` from linked commits via `git diff --stat`. When available, override the tool-call estimate. Shown with a small `GitCommit` icon to indicate "verified from git."

---

## Layer 2: Smart Grouping & Filtering

### 2A: Group-By Control

New "Group by" dropdown in toolbar, next to existing Filter/Sort:

```
+----------+  +----------+  +-------------+  | All time  Today  7d  30d |
| v Filter |  | v Sort   |  | v Group by  |  |                          |
+----------+  +----------+  +-------------+  +--------------------------+
```

**Group-by options (all session fields):**

| Group | Source Field | Section Header Format |
|-------|-------------|----------------------|
| None (default) | — | Current date-grouped behavior |
| Branch | `git_branch` | `feature/auth-flow --- 12 sessions . 145K tokens . 23 files . +1.2K / -340 lines` |
| Project | `project` | Project display name with aggregate stats |
| Model | `primary_model` | Model name with aggregate stats |
| Day | `modified_at` | Same as current default grouping |
| Week | `modified_at` | "Week of Jan 27" with aggregate stats |
| Month | `modified_at` | "January 2026" with aggregate stats |

**Group section headers** show aggregate stats for that group:

```
--- feature/auth-flow ---------- 12 sessions . 145K tokens . 23 files . +1.2K / -340 lines ---
```

Sessions with `null` for the grouped field go into a `(no branch)` / `(no model)` section at the bottom.

Groups are **collapsible** — click the section header to collapse/expand. All start expanded.

### 2B: Extended Multi-Facet Filter Panel

Replace single Filter dropdown with a popover panel:

```
+-------------------------------------+
|  Filters                    Clear   |
|                                     |
|  Commits     * Any  * Has  * None  |
|  Duration    * Any  * >30m  * >1h  |
|  Branch      [ search branches... ] |
|               [ ] feature/auth      |
|               [ ] main              |
|               [ ] fix/login-bug     |
|  Model       [ ] opus-4  [ ] sonnet |
|  Has skills  * Any  * Yes  * No    |
|                                     |
|  Active: 2 filters        [Apply]  |
+-------------------------------------+
```

**UX decisions:**

- **Popover** (not inline) — keeps toolbar clean, doesn't push content down
- **Radio groups** for mutually exclusive filters (commits: any/has/none)
- **Checkboxes** for multi-select filters (branch, model, project)
- **Branch list is searchable** — text input at top to filter branch names (users may have many)
  - **Debounce**: 150ms debounce on search input to prevent jank
  - **Max height**: 200px with `overflow-y: auto` for scrollable list
  - **Empty state**: "No branches found" when search yields no results
- **Active filter count** on trigger button: `Filter (2)` with blue highlight
- **URL-persisted** via search params (existing pattern from `useFilterSort`)
- **Apply button** — filters don't apply until clicked (prevents jarring re-renders while selecting multiple checkboxes)
- **"Clear" link** in header resets all filters
- **Escape key** closes popover without applying (discard pending changes)
- **Focus trap**: Tab cycles within popover while open

**Every session field is filterable.** The filter panel covers:

| Filter | Type | Session Field |
|--------|------|--------------|
| Commits | Radio (any/has/none) | `commit_count` |
| Duration | Radio (any/>30m/>1h/>2h) | `duration_seconds` |
| Branch | Searchable checkbox list | `git_branch` |
| Model | Checkbox list | `primary_model` |
| Has skills | Radio (any/yes/no) | `skills_used.length` |
| Re-edit rate | Radio (any/high >20%) | `reedited_files_count / files_edited_count` |
| File count | Radio (any/>5/>10/>20) | `files_edited_count` |
| Token range | Radio (any/>10K/>50K/>100K) | total tokens |

### 2C: Two View Modes

Segmented control, right-aligned in toolbar:

```
+----------+ +--------+ +-------------+  | All time ... |  +------+-------+
| v Filter | | v Sort | | v Group by  |  |              |  | List | Table |
+----------+ +--------+ +-------------+  |              |  +------+-------+
```

| Mode | Icon | Best For |
|------|------|----------|
| **Timeline** (default) | `LayoutList` Lucide | Chronological browsing, flow of work |
| **Compact table** | `Table` Lucide | Power users scanning many sessions, comparing metrics |

**Timeline mode:** Current card-based layout with all enhancements from Layer 1.

**Compact table columns:**

| Column | Width | Content | Sortable |
|--------|-------|---------|----------|
| Time | 140px | Date + time range | Yes |
| Branch | 120px | Branch badge, truncated | Yes |
| Preview | flex | First line of preview, truncated | No |
| Prompts | 60px | Number | Yes |
| Tokens | 70px | Formatted (45K) | Yes |
| Files | 50px | Number | Yes |
| LOC | 80px | +N / -N | Yes |
| Commits | 60px | Number | Yes |
| Duration | 70px | Formatted (45m) | Yes |

- Row click navigates to session detail
- Row hover highlights with `bg-gray-50 dark:bg-gray-800`
- Column header click sorts (replaces Sort dropdown in table mode)
- `tabular-nums` on all numeric columns for alignment
- Horizontal scroll on mobile with `overflow-x-auto` wrapper

### 2D: Shared Filter Component (SessionToolbar)

Both `HistoryView` and `ProjectView` use the **same `SessionToolbar` component**:

```tsx
<SessionToolbar
  filter={filter}
  sort={sort}
  groupBy={groupBy}
  viewMode={viewMode}
  timeRange={timeRange}
  onFilterChange={setFilter}
  onSortChange={setSort}
  onGroupByChange={setGroupBy}
  onViewModeChange={setViewMode}
  onTimeRangeChange={setTimeRange}
  hideProjectFilter={isProjectView}  // hide in ProjectView since already scoped
/>
```

| Page | Default behavior |
|------|-----------------|
| HistoryView | All projects, all sessions |
| ProjectView | Pre-filtered to one project, project filter hidden |

---

## Layer 3: Sidebar Improvements

### 3A: Expanded Content Shows Branch List

**Current:** Expanding a project shows "N sessions" — redundant since count is on the right.

**New:** Expanding shows distinct branches with session counts:

```
v [folder] claude-view                          47
     main                                      28
     feature/auth-flow                          8
     fix/login-bug                              3
     feature/export-pdf                         5
     (no branch)                                3

> [folder] fluffy                               12
> [folder] fluffy/web                            6
```

- Clicking a branch navigates to `ProjectView` with branch pre-filtered
- Branch names truncated with `...` if too long, full name in `title` tooltip
- `(no branch)` shown in italic for sessions without `git_branch`
- Sorted by session count descending within each project

**New API endpoint:** `GET /api/projects/:id/branches`

Response:

```json
[
  { "branch": "main", "count": 28 },
  { "branch": "feature/auth-flow", "count": 8 },
  { "branch": null, "count": 3 }
]
```

### 3B: List / Tree View Toggle

Small segmented control at the top of the sidebar:

```
+---------------------------+
|  [List icon] | [Tree icon]|
+---------------------------+
```

Using Lucide `List` and `FolderTree` icons.

| Mode | Behavior |
|------|----------|
| **List** (default) | Flat alphabetical list of all projects. Current behavior. |
| **Tree** | Group by directory structure. Nested projects share parent nodes. |

**Tree mode example:**

```
v @vicky-ai
    > claude-view                          47
    > fluffy                               12
v personal
    > dotfiles                              3
    > blog                                  8
  standalone-project                        5
```

**Tree derivation algorithm (frontend only):**

1. Take all `project.path` values
2. Split by `/` to get directory segments
3. Find common prefixes — projects sharing a parent directory group under that parent
4. Single-child groups are flattened (don't show a parent with only one child)
5. Top-level projects (no shared parent) appear at root level

No backend changes needed — purely a frontend presentation layer.

**Session count stays on every row** in both modes.

---

## Backend Changes

| Change | Crate | Description |
|--------|-------|-------------|
| **LOC estimation fields** | `db` | Migration 13: add `lines_added INT DEFAULT 0`, `lines_removed INT DEFAULT 0`, `loc_source INT DEFAULT 0` to `sessions` table |
| **LOC from tool calls** | `core` | In `parse_bytes()`, extract Edit/Write tool_use content, estimate line diffs. New fields in `ParseResult`. |
| **LOC from git** | `db` | During git sync, extract insertions/deletions from linked commit diffs. Override session-level LOC with `loc_source = 2`. |
| **Branch list endpoint** | `server` | `GET /api/projects/:id/branches` — `SELECT git_branch, COUNT(*) FROM sessions WHERE project_id = ? GROUP BY git_branch` |
| **Extended filter params** | `server` | Extend `GET /api/sessions` to accept: `branches`, `models`, `has_skills`, `min_duration`, `min_files`, `min_tokens` query params |
| **Group-by** | Frontend | Client-side grouping with prefetch — fetch all sessions for current view (no pagination during grouping). |

**Note on existing infrastructure:**
- `GET /api/projects/:id/sessions?branch=X` already supports single-branch filtering (see `SessionsQuery` in `projects.rs:36`)
- `idx_sessions_project_branch` index already exists (Migration 7)
- `git_branch`, `files_edited`, `primary_model` fields already exist on `SessionInfo`

---

## Implementation Phases

| Phase | Scope | Impact | Dependencies |
|-------|-------|--------|--------------|
| **A: Session Card Enhancement** | Branch badge + top files on card | High — instant visual differentiation | Frontend only. `gitBranch` and `filesEdited` already exist. |
| **B: SessionToolbar + Group-by + Filters** | New shared toolbar component, group-by dropdown, extended filter panel | High — solves core "can't find session" problem | Frontend: new component. Backend: extend filter query params. |
| **C: LOC Estimation** | Parse tool calls for line counts | Medium — enables "deep work vs quick ask" triage | Backend: parse logic + Migration 13. Frontend: display on card. |
| **D: Compact Table View** | Table mode toggle in toolbar | Medium — power user productivity | Frontend only. Reuses same data + SessionToolbar. |
| **E: Sidebar Branch List + Tree View** | Branch list on expand, list/tree toggle | Medium — better project navigation | Frontend: tree derivation. Backend: branches endpoint. |
| **F: Git Diff Stats Overlay** | Accurate LOC from git for committed work | Low — refinement of Phase C | Backend: extract diff stats during git sync. |

**Recommended order:** A → B → C → D → E → F

Phases A and B are the highest impact and solve the core user pain. C-F are progressive enhancements.

---

## Accessibility Checklist (per UI/UX Pro Max)

- [ ] All new interactive elements have `cursor-pointer`
- [ ] Filter popover has `aria-expanded`, `aria-haspopup`
- [ ] Radio/checkbox groups use proper `role="radiogroup"` / `role="group"`
- [ ] Branch badges have sufficient contrast (4.5:1 min)
- [ ] Table view has proper `<table>` semantics with `<th scope="col">`
- [ ] Column sort buttons have `aria-sort` attribute
- [ ] Sidebar tree view uses `role="tree"` / `role="treeitem"` (already present)
- [ ] Group collapse/expand respects `prefers-reduced-motion`
- [ ] Focus-visible rings on all new interactive elements
- [ ] Skeleton loaders for branch list while loading

---

## Backend Specification

### Migration 13: LOC Estimation Fields

```sql
-- Migration 13: Add lines_added and lines_removed to sessions
ALTER TABLE sessions ADD COLUMN lines_added INTEGER NOT NULL DEFAULT 0 CHECK (lines_added >= 0);
ALTER TABLE sessions ADD COLUMN lines_removed INTEGER NOT NULL DEFAULT 0 CHECK (lines_removed >= 0);
-- Source tracking: 0 = not computed, 1 = tool-call estimate, 2 = git diff
ALTER TABLE sessions ADD COLUMN loc_source INTEGER NOT NULL DEFAULT 0 CHECK (loc_source IN (0, 1, 2));
```

**Why not store LOC on `session_commits`?** Sessions may have multiple commits. We want one aggregated LOC value per session displayed on the card. The `loc_source` column indicates provenance (tool estimate vs git verified).

### New Endpoint: GET /api/projects/:id/branches

**Location:** `crates/server/src/routes/projects.rs`

**Request:**
```
GET /api/projects/claude-view/branches
```

**Response:**
```json
{
  "branches": [
    { "branch": "main", "count": 28 },
    { "branch": "feature/auth-flow", "count": 8 },
    { "branch": null, "count": 3 }
  ]
}
```

**SQL:**
```sql
SELECT git_branch as branch, COUNT(*) as count
FROM sessions
WHERE project_id = ?1 AND is_sidechain = 0
GROUP BY git_branch
ORDER BY count DESC
```

**Rust types:**
```rust
// In crates/server/src/routes/projects.rs
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BranchCount {
    pub branch: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct BranchesResponse {
    pub branches: Vec<BranchCount>,
}
```

**Performance:** Single query, uses existing `idx_sessions_project_branch` index from Migration 7. Expected <5ms for 1000 sessions.

### Extended Filter Params on GET /api/sessions

**Current params:** `filter`, `sort`, `limit`, `offset`

**New params (all optional):**

| Param | Type | Example | SQL |
|-------|------|---------|-----|
| `branches` | comma-separated | `branches=main,feature/auth` | `git_branch IN (...)` |
| `models` | comma-separated | `models=claude-opus-4,claude-sonnet-4` | `primary_model IN (...)` |
| `has_commits` | boolean | `has_commits=true` | `commit_count > 0` |
| `has_skills` | boolean | `has_skills=true` | `json_array_length(skills_used) > 0` |
| `min_duration` | integer (seconds) | `min_duration=1800` | `duration_seconds >= ?` |
| `min_files` | integer | `min_files=5` | `files_edited_count >= ?` |
| `min_tokens` | integer | `min_tokens=10000` | `(total_input_tokens + total_output_tokens) >= ?` |
| `high_reedit` | boolean | `high_reedit=true` | `CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0) > 0.2` |
| `time_after` | unix timestamp | `time_after=1706400000` | `last_message_at >= ?` |
| `time_before` | unix timestamp | `time_before=1707004800` | `last_message_at <= ?` |

**Updated Rust struct:**
```rust
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SessionsListQuery {
    pub filter: Option<String>,  // kept for backward compat
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    // New multi-facet filters
    pub branches: Option<String>,       // comma-separated
    pub models: Option<String>,         // comma-separated
    pub has_commits: Option<bool>,
    pub has_skills: Option<bool>,
    pub min_duration: Option<i64>,
    pub min_files: Option<i64>,
    pub min_tokens: Option<i64>,
    pub high_reedit: Option<bool>,
    pub time_after: Option<i64>,
    pub time_before: Option<i64>,
}
```

**Query building pattern (existing in `list_sessions_for_project`):**
```rust
let mut conditions: Vec<String> = vec![];
let mut binds: Vec<Box<dyn ToSql>> = vec![];

if let Some(branches) = &params.branches {
    let branch_list: Vec<&str> = branches.split(',').collect();
    let placeholders = branch_list.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    conditions.push(format!("git_branch IN ({})", placeholders));
    for b in branch_list {
        binds.push(Box::new(b.to_string()));
    }
}
// ... similar for other filters
```

**Backward compatibility:** The existing `filter=has_commits` shorthand still works. New params take precedence if both specified.

### LOC Estimation in parse_bytes()

**Location:** `crates/core/src/parser.rs` → `parse_bytes()`

**Performance constraint:** Must follow existing SIMD-first pattern. Create Finders once outside loop (see CLAUDE.md rule: "memmem::Finder: create once, reuse").

**Algorithm:**
```rust
// SIMD pre-filter for Edit/Write tool_use — create ONCE outside loop
let edit_finder = memmem::Finder::new(b"\"name\":\"Edit\"");
let write_finder = memmem::Finder::new(b"\"name\":\"Write\"");
// Secondary filter for tool_use blocks (avoid parsing tool_result)
let tool_use_finder = memmem::Finder::new(b"\"type\":\"tool_use\"");

let mut lines_added: u32 = 0;
let mut lines_removed: u32 = 0;

for line in lines {
    // SIMD check first — ~98% of lines won't match, skip JSON parse
    let is_edit = edit_finder.find(line).is_some();
    let is_write = write_finder.find(line).is_some();

    if !is_edit && !is_write {
        continue; // Fast path: skip JSON parse entirely
    }

    // Only parse lines that are tool_use (not tool_result)
    if tool_use_finder.find(line).is_none() {
        continue;
    }

    // Slow path: parse JSON only for matching lines (~2% of total)
    let Ok(v) = serde_json::from_slice::<Value>(line) else { continue };

    if is_edit {
        if let Some(input) = v.get("input") {
            let old = input.get("old_string").and_then(|s| s.as_str()).unwrap_or("");
            let new = input.get("new_string").and_then(|s| s.as_str()).unwrap_or("");
            let old_lines = old.lines().count() as u32;
            let new_lines = new.lines().count() as u32;
            // Edit replaces old with new — compute net change
            lines_added += new_lines;
            lines_removed += old_lines;
        }
    } else if is_write {
        // Write = new file, all lines are added
        if let Some(content) = v.get("input").and_then(|i| i.get("content")).and_then(|c| c.as_str()) {
            lines_added += content.lines().count() as u32;
        }
    }
}
```

**Key implementation notes:**
1. **Input field, not params**: Claude Code JSONL uses `"input"` not `"params"` for tool arguments
2. **Net LOC, not delta**: Store total added/removed, not just the difference. This matches `git diff --stat` semantics.
3. **Skip tool_result**: Only count tool_use (the request), not tool_result (the response)
4. **Saturating math**: Use `.saturating_add()` in production to prevent overflow

**Extend ParseResult:**
```rust
pub struct ParseResult {
    // ... existing fields
    pub lines_added: u32,
    pub lines_removed: u32,
}
```

**Update update_session_deep_fields():** Add `lines_added`, `lines_removed`, `loc_source` params. Set `loc_source = 1` for tool-call estimate.

**Edge cases handled:**
| Case | Behavior |
|------|----------|
| Empty old_string (new content added) | All new lines counted as added |
| Empty new_string (content deleted) | All old lines counted as removed |
| Binary file content | `.lines()` still works, counts `\n` |
| Very large file (1MB+) | Capped at u32::MAX via saturating_add |
| Malformed JSON | Silently skipped (continue) |

### Git Diff Stats (Phase F)

During git sync in `crates/db/src/git_correlation.rs`:

```rust
// After linking a commit to a session, extract diff stats
let output = Command::new("git")
    .args(["diff", "--numstat", &format!("{}^..{}", commit_hash, commit_hash)])
    .current_dir(repo_path)
    .output()?;

// Parse numstat: "10\t5\tfile.rs" = 10 additions, 5 deletions
// Aggregate across all linked commits for the session
// Update sessions SET lines_added = ?, lines_removed = ?, loc_source = 2 WHERE id = ?
```

Only run this when a new commit link is created. Don't re-run on every git sync.

---

## Frontend State Management

### New Hooks

**`use-branches.ts`** — Fetch branch list for sidebar expansion
```typescript
import { useQuery } from '@tanstack/react-query'
import type { BranchesResponse } from '../types/generated'

async function fetchBranches(projectId: string): Promise<BranchesResponse> {
  const res = await fetch(`/api/projects/${encodeURIComponent(projectId)}/branches`)
  if (!res.ok) throw new Error('Failed to fetch branches')
  return res.json()
}

export function useBranches(projectId: string | undefined) {
  return useQuery({
    queryKey: ['branches', projectId],
    queryFn: () => fetchBranches(projectId!),
    enabled: !!projectId,
    staleTime: 60_000, // branches don't change often
  })
}
```

**`use-session-filters.ts`** — Extended filter state with URL persistence
```typescript
import { useSearchParams } from 'react-router-dom'
import { useMemo, useCallback } from 'react'

export interface SessionFilters {
  branches: string[]
  models: string[]
  hasCommits: boolean | null
  hasSkills: boolean | null
  minDuration: number | null
  minFiles: number | null
  minTokens: number | null
  highReedit: boolean | null
  timeRange: 'all' | 'today' | '7d' | '30d'
}

export interface FilterState {
  filters: SessionFilters
  sort: string
  groupBy: string
  viewMode: 'timeline' | 'table'
}

const DEFAULTS: FilterState = {
  filters: {
    branches: [],
    models: [],
    hasCommits: null,
    hasSkills: null,
    minDuration: null,
    minFiles: null,
    minTokens: null,
    highReedit: null,
    timeRange: 'all',
  },
  sort: 'recent',
  groupBy: 'none',
  viewMode: 'timeline',
}

export function useSessionFilters() {
  const [searchParams, setSearchParams] = useSearchParams()

  const state = useMemo<FilterState>(() => ({
    filters: {
      branches: searchParams.get('branches')?.split(',').filter(Boolean) ?? [],
      models: searchParams.get('models')?.split(',').filter(Boolean) ?? [],
      hasCommits: searchParams.get('hasCommits') === 'true' ? true
        : searchParams.get('hasCommits') === 'false' ? false : null,
      hasSkills: searchParams.get('hasSkills') === 'true' ? true
        : searchParams.get('hasSkills') === 'false' ? false : null,
      minDuration: searchParams.get('minDuration') ? Number(searchParams.get('minDuration')) : null,
      minFiles: searchParams.get('minFiles') ? Number(searchParams.get('minFiles')) : null,
      minTokens: searchParams.get('minTokens') ? Number(searchParams.get('minTokens')) : null,
      highReedit: searchParams.get('highReedit') === 'true' ? true : null,
      timeRange: (searchParams.get('timeRange') as FilterState['filters']['timeRange']) ?? 'all',
    },
    sort: searchParams.get('sort') ?? 'recent',
    groupBy: searchParams.get('groupBy') ?? 'none',
    viewMode: (searchParams.get('viewMode') as FilterState['viewMode']) ?? 'timeline',
  }), [searchParams])

  const updateState = useCallback((updates: Partial<FilterState>) => {
    const params = new URLSearchParams(searchParams)
    const merged = { ...state, ...updates }
    const filters = { ...state.filters, ...(updates.filters ?? {}) }

    // Serialize to URL params, omitting defaults
    if (filters.branches.length) params.set('branches', filters.branches.join(','))
    else params.delete('branches')

    if (filters.models.length) params.set('models', filters.models.join(','))
    else params.delete('models')

    // ... similar for other filters

    if (merged.sort !== 'recent') params.set('sort', merged.sort)
    else params.delete('sort')

    if (merged.groupBy !== 'none') params.set('groupBy', merged.groupBy)
    else params.delete('groupBy')

    if (merged.viewMode !== 'timeline') params.set('viewMode', merged.viewMode)
    else params.delete('viewMode')

    setSearchParams(params, { replace: true })
  }, [searchParams, setSearchParams, state])

  const clearFilters = useCallback(() => {
    updateState({ filters: DEFAULTS.filters })
  }, [updateState])

  const activeFilterCount = useMemo(() => {
    const f = state.filters
    let count = 0
    if (f.branches.length) count++
    if (f.models.length) count++
    if (f.hasCommits !== null) count++
    if (f.hasSkills !== null) count++
    if (f.minDuration !== null) count++
    if (f.minFiles !== null) count++
    if (f.minTokens !== null) count++
    if (f.highReedit !== null) count++
    if (f.timeRange !== 'all') count++
    return count
  }, [state.filters])

  return { ...state, updateState, clearFilters, activeFilterCount }
}
```

### Client-Side Grouping

**Grouping + Pagination Decision:**

Grouping requires all sessions in the current scope to compute aggregates. Two options:

| Option | Pros | Cons |
|--------|------|------|
| A: Load all, group client-side | Simple, instant group switching | Memory risk with 1000+ sessions |
| B: Server-side aggregation | Scales better | New API complexity, slower group switching |

**Decision: Option A with safeguards.**

Rationale: Most users have <500 sessions. Loading all is ~100KB JSON. For users with 1000+ sessions, we add:
1. **Pagination fallback**: If `total > 500`, disable grouping and show "Too many sessions for grouping. Use filters to narrow results."
2. **Memory-efficient rendering**: Use `react-window` virtualization for session lists (already planned for table view)
3. **Lazy group expansion**: Only render visible group's sessions, collapse others

**Implementation note:** When grouping is active, fetch with `limit=500`. If `total > 500`, show the warning and reset groupBy to 'none'.

Grouping is computed client-side since sessions are already loaded:

```typescript
// utils/group-sessions.ts
import type { SessionInfo } from '../types/generated'

export type GroupBy = 'none' | 'branch' | 'project' | 'model' | 'day' | 'week' | 'month'

export interface SessionGroup {
  key: string
  label: string
  sessions: SessionInfo[]
  stats: {
    count: number
    totalTokens: number
    totalFiles: number
    linesAdded: number
    linesRemoved: number
  }
}

export function groupSessions(sessions: SessionInfo[], groupBy: GroupBy): SessionGroup[] {
  if (groupBy === 'none') {
    // Return single group with all sessions
    return [{
      key: 'all',
      label: '',
      sessions,
      stats: computeStats(sessions),
    }]
  }

  const groups = new Map<string, SessionInfo[]>()

  for (const session of sessions) {
    const key = getGroupKey(session, groupBy)
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key)!.push(session)
  }

  // Sort groups: nullish keys at the end
  const sorted = Array.from(groups.entries()).sort(([a], [b]) => {
    if (a === '(none)') return 1
    if (b === '(none)') return -1
    return a.localeCompare(b)
  })

  return sorted.map(([key, sessions]) => ({
    key,
    label: key,
    sessions,
    stats: computeStats(sessions),
  }))
}

function getGroupKey(session: SessionInfo, groupBy: GroupBy): string {
  switch (groupBy) {
    case 'branch':
      return session.gitBranch ?? '(no branch)'
    case 'project':
      return session.project
    case 'model':
      return session.primaryModel ?? '(no model)'
    case 'day':
      return new Date(session.modifiedAt * 1000).toISOString().slice(0, 10)
    case 'week':
      const d = new Date(session.modifiedAt * 1000)
      const weekStart = new Date(d.setDate(d.getDate() - d.getDay()))
      return `Week of ${weekStart.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`
    case 'month':
      return new Date(session.modifiedAt * 1000).toLocaleDateString('en-US', { year: 'numeric', month: 'long' })
    default:
      return 'all'
  }
}

function computeStats(sessions: SessionInfo[]) {
  return {
    count: sessions.length,
    totalTokens: sessions.reduce((sum, s) => sum + (s.totalInputTokens ?? 0) + (s.totalOutputTokens ?? 0), 0),
    totalFiles: sessions.reduce((sum, s) => sum + s.filesEditedCount, 0),
    linesAdded: sessions.reduce((sum, s) => sum + (s.linesAdded ?? 0), 0),
    linesRemoved: sessions.reduce((sum, s) => sum + (s.linesRemoved ?? 0), 0),
  }
}
```

### TypeScript Types to Generate

Add to `SessionInfo` in `crates/core/src/types.rs`:
```rust
#[serde(default)]
pub lines_added: u32,
#[serde(default)]
pub lines_removed: u32,
#[serde(default)]
pub loc_source: u8,  // 0 = not computed, 1 = tool estimate, 2 = git verified
```

After updating Rust types, run:
```bash
cargo test -p core export  # generates TypeScript via ts-rs
```

---

## QA Acceptance Criteria

### AC-1: Branch Badge Display

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 1.1 | Session has `gitBranch = "feature/auth"` | Badge shows `feature/auth` with GitBranch icon | ☐ |
| 1.2 | Session has `gitBranch = null` | No badge rendered, no empty space | ☐ |
| 1.3 | Branch name > 20 chars | Truncated with `...`, full name in `title` tooltip | ☐ |
| 1.4 | Dark mode | Badge uses dark-mode-compatible colors (contrast ≥4.5:1) | ☐ |
| 1.5 | Click badge | Does NOT navigate (badge is informational only) | ☐ |

### AC-2: LOC Display

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 2.1 | Session has `linesAdded=100, linesRemoved=20` | Shows `+100 / -20` in green/red | ☐ |
| 2.2 | Session has `linesAdded=0, linesRemoved=0` | Shows `±0` in muted gray | ☐ |
| 2.3 | Session has `locSource=2` (git verified) | Shows small GitCommit icon next to LOC | ☐ |
| 2.4 | Session has `locSource=1` (tool estimate) | No git icon (estimate indicator TBD) | ☐ |
| 2.5 | Large numbers | Formats with K suffix: `+1.2K / -340` | ☐ |

### AC-3: Top Files Display

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 3.1 | Session has 3 files edited | Shows all 3 basenames: `auth.ts · middleware.ts · test.ts` | ☐ |
| 3.2 | Session has 8 files edited | Shows 3 + `+5 more` | ☐ |
| 3.3 | Session has 0 files edited | Row not rendered | ☐ |
| 3.4 | File path `/src/components/Button.tsx` | Shows `Button.tsx` (basename only) | ☐ |

### AC-4: Filter Popover

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 4.1 | Click Filter button | Popover opens with all filter sections | ☐ |
| 4.2 | Select `Has commits = Yes` | Radio selected, Apply button enabled | ☐ |
| 4.3 | Click Apply | Popover closes, sessions filtered, URL updated | ☐ |
| 4.4 | 2 filters active | Filter button shows `Filter (2)` with blue highlight | ☐ |
| 4.5 | Click Clear | All filters reset, URL params removed | ☐ |
| 4.6 | Escape key | Popover closes without applying | ☐ |
| 4.7 | Click outside | Popover closes without applying | ☐ |
| 4.8 | Page refresh with filters in URL | Filters restored from URL params | ☐ |

### AC-5: Branch Filter

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 5.1 | Type in branch search | List filters to matching branches | ☐ |
| 5.2 | Check `main` and `feature/auth` | Both checked, Apply shows both | ☐ |
| 5.3 | Apply branch filter | Sessions filtered to those branches | ☐ |
| 5.4 | No branches match search | Shows "No branches found" | ☐ |
| 5.5 | 50+ branches | List scrollable with max-height (200px) | ☐ |
| 5.6 | Rapid typing in search | Debounced (150ms), no jank | ☐ |
| 5.7 | Clear search input | Full branch list restored | ☐ |
| 5.8 | Branch name with special chars | Properly escaped in URL params | ☐ |

### AC-6: Group-By

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 6.1 | Select `Group by: Branch` | Sessions grouped by branch, headers shown | ☐ |
| 6.2 | Group header | Shows branch name + aggregate stats | ☐ |
| 6.3 | Click group header | Group collapses/expands | ☐ |
| 6.4 | Sessions with `gitBranch = null` | Appear in `(no branch)` group at bottom | ☐ |
| 6.5 | Switch to `Group by: None` | Returns to flat list | ☐ |
| 6.6 | Group by Month | Shows "January 2026", "February 2026", etc. | ☐ |
| 6.7 | Total > 500 sessions | Grouping disabled, shows warning message | ☐ |
| 6.8 | Filter reduces to < 500 | Grouping re-enabled | ☐ |
| 6.9 | Collapse all groups | "Expand All" button appears | ☐ |
| 6.10 | Group aggregates | LOC sums correctly across sessions in group | ☐ |

### AC-7: View Modes

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.1 | Default view | Timeline (card) mode | ☐ |
| 7.2 | Click Table icon | Switches to compact table view | ☐ |
| 7.3 | Table view columns | Time, Branch, Preview, Prompts, Tokens, Files, LOC, Commits, Duration | ☐ |
| 7.4 | Click table row | Navigates to session detail | ☐ |
| 7.5 | Click column header | Sorts by that column (toggles asc/desc) | ☐ |
| 7.6 | Column header shows sort arrow | `aria-sort` attribute set | ☐ |
| 7.7 | View mode persists in URL | `?viewMode=table` | ☐ |

### AC-8: Sidebar Branches

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 8.1 | Expand project in sidebar | Branch list loads (skeleton while loading) | ☐ |
| 8.2 | Branch list shows | Branch names with session counts | ☐ |
| 8.3 | Click branch | Navigates to ProjectView with `?branches=<branch>` | ☐ |
| 8.4 | Sessions with no branch | Shows `(no branch)` in italic | ☐ |
| 8.5 | API error | Shows error state, retry button | ☐ |

### AC-9: Sidebar Tree View

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 9.1 | Toggle to Tree view | Projects grouped by directory | ☐ |
| 9.2 | Projects in same dir | Share parent node | ☐ |
| 9.3 | Single-child parent | Flattened (no extra nesting) | ☐ |
| 9.4 | Session counts | Shown on every row | ☐ |
| 9.5 | Toggle back to List | Flat alphabetical list restored | ☐ |

### AC-10: Backend Filters

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 10.1 | `GET /api/sessions?branches=main` | Only sessions with `git_branch = 'main'` | ☐ |
| 10.2 | `GET /api/sessions?branches=main,feature/auth` | Sessions with either branch | ☐ |
| 10.3 | `GET /api/sessions?min_duration=1800` | Sessions ≥ 30 min | ☐ |
| 10.4 | `GET /api/sessions?high_reedit=true` | Sessions with re-edit rate > 20% | ☐ |
| 10.5 | Multiple filters combined | All conditions ANDed | ☐ |
| 10.6 | Invalid param value | 400 Bad Request with clear message | ☐ |
| 10.7 | Empty branches list | Ignores filter (returns all) | ☐ |

### AC-11: Branches Endpoint

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 11.1 | `GET /api/projects/foo/branches` | Returns array of {branch, count} | ☐ |
| 11.2 | Project has sessions with null branch | Includes `{"branch": null, "count": N}` | ☐ |
| 11.3 | Project doesn't exist | Returns empty array (not 404) | ☐ |
| 11.4 | Branches sorted by count DESC | Most-used branch first | ☐ |

### AC-12: LOC Parsing

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 12.1 | Session with Edit tool calls | `lines_added`, `lines_removed` populated | ☐ |
| 12.2 | Edit: old 5 lines → new 8 lines | +8 added, +5 removed (net +3) | ☐ |
| 12.3 | Edit: old 10 lines → new 3 lines | +3 added, +10 removed (net -7) | ☐ |
| 12.4 | Write tool call (new file) | All lines counted as lines_added | ☐ |
| 12.5 | No Edit/Write calls | `lines_added = 0, lines_removed = 0` | ☐ |
| 12.6 | `loc_source` set to 1 | For tool-call estimates | ☐ |
| 12.7 | Empty old_string in Edit | Only new lines counted as added | ☐ |
| 12.8 | Empty new_string in Edit | Only old lines counted as removed | ☐ |
| 12.9 | Malformed JSON line | Silently skipped, no crash | ☐ |
| 12.10 | tool_result line (not tool_use) | Ignored, not double-counted | ☐ |
| 12.11 | Very large session (10MB JSONL) | Completes without OOM, uses streaming | ☐ |

### AC-13: Performance

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 13.1 | `/api/projects/:id/branches` | < 10ms for 1000 sessions | ☐ |
| 13.2 | `/api/sessions` with all filters | < 50ms for 1000 sessions | ☐ |
| 13.3 | LOC parsing in deep index | < 5% overhead on parse phase | ☐ |
| 13.4 | Client-side grouping | < 50ms for 500 sessions | ☐ |
| 13.5 | Filter popover open | No jank, < 16ms frame time | ☐ |

### AC-14: Accessibility

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 14.1 | Filter popover | `aria-expanded`, `aria-haspopup` on trigger | ☐ |
| 14.2 | Radio groups | `role="radiogroup"` with `aria-labelledby` | ☐ |
| 14.3 | Table view | Proper `<table>` with `<th scope="col">` | ☐ |
| 14.4 | Sort column | `aria-sort="ascending"` or `"descending"` | ☐ |
| 14.5 | Branch badge | Sufficient color contrast (4.5:1) | ☐ |
| 14.6 | Focus visible | All interactive elements have focus ring | ☐ |
| 14.7 | Keyboard nav | Filter popover navigable with Tab/Enter | ☐ |
| 14.8 | prefers-reduced-motion | Animations disabled | ☐ |
| 14.9 | Focus trap | Tab cycles within open popover | ☐ |
| 14.10 | Screen reader | Filter changes announced via live region | ☐ |
| 14.11 | Group headers | Collapsible headers use `aria-expanded` | ☐ |
| 14.12 | Empty state | "No sessions match" has `role="status"` | ☐ |

### AC-15: Error Handling

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 15.1 | Branches endpoint fails | Sidebar shows error, collapse still works | ☐ |
| 15.2 | Sessions endpoint fails | Error state with retry button | ☐ |
| 15.3 | Invalid URL params | Gracefully ignored, defaults used | ☐ |
| 15.4 | Empty filter result | Shows "No sessions match filters" with clear filters button | ☐ |
| 15.5 | Grouping disabled (>500) | Shows info banner, not error state | ☐ |
| 15.6 | Network timeout | Shows "Taking longer than expected..." after 5s | ☐ |
| 15.7 | LOC parse failure | Session still displays, LOC shows "—" | ☐ |

### AC-16: Migration 13

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 16.1 | Fresh install | Migration 13 runs, columns exist | ☐ |
| 16.2 | Upgrade from Migration 12 | Migration 13 runs without error | ☐ |
| 16.3 | `lines_added` default | 0 for new sessions | ☐ |
| 16.4 | `lines_removed` default | 0 for new sessions | ☐ |
| 16.5 | `loc_source` default | 0 (not computed) for new sessions | ☐ |
| 16.6 | CHECK constraint | Negative values rejected | ☐ |
| 16.7 | Existing sessions | `lines_added=0`, `lines_removed=0`, `loc_source=0` | ☐ |

---

## Test Files to Create

| File | Coverage | Priority |
|------|----------|----------|
| `crates/server/src/routes/projects.rs` | AC-11 (branches endpoint) | P0 |
| `crates/server/src/routes/sessions.rs` | AC-10 (extended filter params) | P0 |
| `crates/core/src/parser.rs` | AC-12 (LOC parsing, edge cases 12.7-12.11) | P0 |
| `crates/db/src/migrations.rs` | AC-16 (Migration 13 columns + constraints) | P0 |
| `src/components/SessionCard.test.tsx` | AC-1, AC-2, AC-3 | P1 |
| `src/components/FilterPopover.test.tsx` | AC-4, AC-5 (incl. debounce 5.6) | P1 |
| `src/components/SessionToolbar.test.tsx` | AC-6, AC-7 (incl. safeguard 6.7) | P1 |
| `src/components/Sidebar.test.tsx` | AC-8, AC-9 | P1 |
| `src/utils/group-sessions.test.ts` | Grouping logic + 500-session safeguard | P1 |
| `src/hooks/use-session-filters.test.ts` | URL persistence + special chars | P1 |
| `src/hooks/use-branches.test.ts` | API integration + error states | P2 |
| `e2e/session-discovery.spec.ts` | AC-13 (performance), AC-14 (a11y) | P2 |

**Test commands:**
```bash
# Backend unit tests
cargo test -p server -- routes::projects
cargo test -p server -- routes::sessions
cargo test -p core -- parser::loc
cargo test -p db -- migrations

# Frontend unit tests
pnpm test src/components/SessionCard
pnpm test src/components/FilterPopover
pnpm test src/utils/group-sessions

# E2E (requires running server)
pnpm e2e session-discovery
```

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Time to find specific session | ~30s (scroll + scan) | < 10s (filter + click) |
| Sessions visible without scroll (table mode) | ~5 | ~20 |
| Filter options available | 4 | 10+ |
| Group-by options | 0 | 6 |
| Memory usage (500 sessions loaded) | N/A | < 50MB |
| Filter popover render time | N/A | < 16ms (60fps) |
| LOC parse overhead | N/A | < 5% of total parse time |

---

## Open Questions (Resolved)

| Question | Decision |
|----------|----------|
| Store LOC on session or session_commits? | **Session** — one aggregated value per session, simpler display |
| Group-by server-side or client-side? | **Client-side with 500-session safeguard** — prefetch all for grouping, disable if >500 |
| Filter apply on change or on button? | **On button** — prevents jarring re-renders during multi-select |
| How many filters is too many? | **10 filters max** — anything more needs search, not filters |
| LOC semantics: delta or gross? | **Gross (total added + total removed)** — matches `git diff --stat`, more informative |
| Branch filter: reuse existing or new? | **Extend existing** — `?branch=X` already works on project endpoint, add `?branches=X,Y` for multi-select |
| Migration number? | **Migration 13** — current schema is at Migration 12 |

---

## Verification Checklist (Pre-Implementation)

Before starting implementation, verify:

- [ ] Migration 12 is current latest (check `MIGRATIONS.len()` in `migrations.rs`)
- [ ] `idx_sessions_project_branch` index exists (Migration 7)
- [ ] `SessionInfo` has: `git_branch`, `files_edited`, `primary_model`, `duration_seconds`, `commit_count`
- [ ] `useFilterSort` hook exists at `src/hooks/use-filter-sort.ts`
- [ ] `FilterSortBar` component exists at `src/components/FilterSortBar.tsx`
- [ ] TypeScript types auto-generate via `cargo test -p core export`
