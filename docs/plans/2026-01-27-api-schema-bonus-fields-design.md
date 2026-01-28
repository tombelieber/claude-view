---
status: superseded
date: 2026-01-27
---

# API + Schema + UI Design: Bonus Fields & Response Optimization

> New fields from `sessions-index.json` (`summary`, `gitBranch`, `isSidechain`) plus API response optimization (676 KB → <50 KB initial load).

## Context

Two problems, one plan:

1. **New data**: `sessions-index.json` provides `summary`, `gitBranch`, and `isSidechain` — fields that enable new UI features but require schema, API, and frontend changes.

2. **Fat response**: `/api/projects` returns 676 KB for 542 sessions (10 projects). Every session includes `filesTouched: string[]`, `skillsUsed: string[]`, and `toolCounts` — even though the sidebar only needs project names and session counts. At 30 GB (5,000+ sessions), this response balloons to ~6 MB.

**Dependency:** This plan builds on top of the startup-ux-parallel-indexing plan (which adds the new columns to SQLite). This plan covers how those columns flow through API → frontend.

---

## Part 1: New Fields

### 1.1 Schema (additions from indexing plan)

The indexing plan already adds these columns:

```sql
ALTER TABLE sessions ADD COLUMN summary TEXT;
ALTER TABLE sessions ADD COLUMN git_branch TEXT;
ALTER TABLE sessions ADD COLUMN is_sidechain BOOLEAN DEFAULT FALSE;
ALTER TABLE sessions ADD COLUMN deep_indexed_at INTEGER;
```

**Additional indexes for query patterns:**

```sql
CREATE INDEX idx_sessions_git_branch ON sessions(project_id, git_branch);
CREATE INDEX idx_sessions_is_sidechain ON sessions(is_sidechain);
```

The compound index on `(project_id, git_branch)` supports filtering sessions by branch within a project. `is_sidechain` index supports the default filter that hides sub-agent sessions.

### 1.2 Rust Type Changes

```rust
// crates/core/src/types.rs
pub struct SessionInfo {
    // ... existing fields ...
    pub summary: Option<String>,      // Claude-generated session summary
    pub git_branch: Option<String>,   // Git branch during session
    pub is_sidechain: bool,           // Sub-agent session flag
    pub deep_indexed: bool,           // Whether Pass 2 extended fields are populated
}
```

```rust
// crates/db/src/queries.rs — SessionRow gains matching fields
struct SessionRow {
    // ... existing fields ...
    summary: Option<String>,
    git_branch: Option<String>,
    is_sidechain: bool,
    deep_indexed_at: Option<i64>,
}
```

### 1.3 API Response Changes

New fields appear in the existing `/api/projects` response:

```json
{
  "id": "abc-123",
  "summary": "Claude-view UI: Sidebar paths, SPA nav, active indicators",
  "gitBranch": "main",
  "isSidechain": false,
  "deepIndexed": true,
  "preview": "some feedback abt the @docs/plans/...",
  ...
}
```

### 1.4 TypeScript Interface

```typescript
// src/hooks/use-projects.ts
export interface SessionInfo {
  // ... existing fields ...
  summary: string | null       // Claude-generated summary
  gitBranch: string | null     // Git branch when session was active
  isSidechain: boolean         // Sub-agent session
  deepIndexed: boolean         // Whether extended fields are populated
}
```

### 1.5 UI Changes

**SessionCard** — Show `summary` as the primary descriptor:

| Current | New |
|---------|-----|
| `"some feedback abt the @docs/plans/..."` (firstPrompt) | **"Claude-view UI: Sidebar paths, SPA nav, active indicators"** (summary) |
| No branch info | `main` branch badge |
| Sub-agent sessions mixed in | Hidden by default |

Concrete changes to `SessionCard.tsx`:
- Primary text: `session.summary ?? session.preview` (summary when available, fall back to firstPrompt)
- Secondary text: `session.preview` shown below summary in lighter gray (only when summary exists and differs)
- Branch badge: small `<span>` next to timestamp showing `session.gitBranch` when present
- Deep-index indicator: subtle loading dot on tool counts section when `!session.deepIndexed` (tells user "more data coming")

**Sidebar** — Filter controls:
- Session count excludes sidechains by default (matches user's mental model of "my sessions")
- No branch filter in sidebar (defer to Part 2 query params)

**ProjectView** — Sidechain toggle:
- Default: hides `isSidechain === true` sessions
- Toggle button: "Show sub-agent sessions" reveals them with a visual indicator (indented or dimmed)

---

## Part 2: API Response Optimization

### 2.1 The Problem

Current response shape — every field for every session:

```
GET /api/projects → 676 KB (542 sessions × ~1.2 KB each)
```

The sidebar only needs: `name`, `displayName`, `path`, `sessions.length`, `activeCount`. That's ~50 bytes per project, not 1.2 KB per session.

The session list (ProjectView) needs session-level fields, but only for the selected project (typically 20-80 sessions), not all 542.

### 2.2 Split into Two Endpoints

**Endpoint 1: Project list (lightweight)**

```
GET /api/projects
```

Returns project summaries without session details:

```json
[
  {
    "name": "-Users-user-dev--myorg-claude-view",
    "displayName": "claude-view",
    "path": "/Users/user/dev/@myorg/claude-view",
    "sessionCount": 87,
    "activeCount": 2,
    "lastActivityAt": "2026-01-27T14:30:00Z"
  }
]
```

**Response size:** ~200 bytes × 10 projects = **~2 KB** (vs 676 KB today).

This is what the sidebar needs. SQL query:

```sql
SELECT
    project_id,
    project_display_name,
    project_path,
    COUNT(*) AS session_count,
    COUNT(*) FILTER (WHERE last_message_at > ?1) AS active_count,
    MAX(last_message_at) AS last_activity_at
FROM sessions
WHERE is_sidechain = FALSE
GROUP BY project_id
ORDER BY last_activity_at DESC
```

**Endpoint 2: Project sessions (on demand)**

```
GET /api/projects/:projectId/sessions
```

Returns sessions for a single project:

```json
{
  "sessions": [
    {
      "id": "abc-123",
      "summary": "Claude-view UI: Sidebar paths, SPA nav, active indicators",
      "preview": "some feedback abt the @docs/plans/...",
      "gitBranch": "main",
      "isSidechain": false,
      "deepIndexed": true,
      "modifiedAt": "2026-01-27T14:30:00Z",
      "messageCount": 60,
      "turnCount": 28,
      "toolCounts": { "edit": 5, "read": 10, "bash": 3, "write": 2 },
      "filesTouched": ["src/main.rs", "Cargo.toml"],
      "skillsUsed": ["/commit"],
      "lastMessage": "Here is the result"
    }
  ],
  "total": 87
}
```

**Response size:** ~1 KB × 87 sessions = **~87 KB** for the largest project (loaded only when user clicks into it).

### 2.3 Query Parameters

**`GET /api/projects`:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `include_sidechains` | bool | `false` | Include sub-agent sessions in counts |

**`GET /api/projects/:projectId/sessions`:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `branch` | string | — | Filter by git branch |
| `include_sidechains` | bool | `false` | Include sub-agent sessions |
| `limit` | int | `50` | Page size |
| `offset` | int | `0` | Pagination offset |
| `sort` | string | `recent` | Sort order: `recent`, `oldest`, `messages` |

### 2.4 Pagination

Default page size of 50 sessions. The ProjectView "Load more sessions..." button (already in the UI at line 51-55 of `ProjectView.tsx`) becomes functional:

```typescript
const { data, fetchNextPage, hasNextPage } = useInfiniteQuery({
  queryKey: ['sessions', projectId],
  queryFn: ({ pageParam = 0 }) =>
    fetch(`/api/projects/${projectId}/sessions?offset=${pageParam}&limit=50`).then(r => r.json()),
  getNextPageParam: (lastPage, pages) =>
    pages.flatMap(p => p.sessions).length < lastPage.total
      ? pages.flatMap(p => p.sessions).length
      : undefined,
})
```

### 2.5 Sidebar Stats

The current sidebar computes per-project stats (skills, files, tools) client-side by iterating all sessions. With the split API, this data is only available after the user clicks a project. Two options:

**Option A: Compute server-side** — Add aggregated stats to the project sessions response:

```json
{
  "sessions": [...],
  "total": 87,
  "stats": {
    "topSkills": [["commit", 42], ["review-pr", 15]],
    "topFiles": [["src/main.rs", 30], ["Cargo.toml", 22]],
    "toolTotals": { "edit": 150, "read": 300, "bash": 80, "write": 40 }
  }
}
```

**Option B: Compute client-side from loaded sessions** — Same as today, but only from the loaded page.

**Recommendation:** Option A. Server has all sessions; client may only have first 50. Aggregation in SQL is trivial and accurate.

### 2.6 Backend Implementation

**New files:**

| File | Purpose |
|------|---------|
| `crates/server/src/routes/sessions.rs` | `GET /api/projects/:id/sessions` endpoint |

**Modified files:**

| File | Changes |
|------|---------|
| `crates/core/src/types.rs` | Add `ProjectSummary` (lightweight), update `SessionInfo` with new fields |
| `crates/db/src/queries.rs` | Add `list_project_summaries()`, `list_sessions_for_project()`, `get_project_stats()` |
| `crates/server/src/routes/projects.rs` | Return `ProjectSummary` instead of `ProjectInfo` with embedded sessions |
| `crates/server/src/lib.rs` | Register sessions route |
| `src/hooks/use-projects.ts` | Split into `useProjects()` (lightweight) + `useProjectSessions(projectId)` |
| `src/components/Sidebar.tsx` | Use `ProjectSummary` (no sessions array) |
| `src/components/ProjectView.tsx` | Fetch sessions on mount, pagination |
| `src/components/SessionCard.tsx` | Render summary, branch badge, sidechain indicator |

### 2.7 New Rust Types

```rust
/// Lightweight project summary for the sidebar (no session details).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub name: String,
    pub display_name: String,
    pub path: String,
    pub session_count: usize,
    pub active_count: usize,
    #[serde(with = "unix_to_iso")]
    pub last_activity_at: i64,
}

/// Aggregated stats for a project's sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStats {
    pub top_skills: Vec<(String, usize)>,
    pub top_files: Vec<(String, usize)>,
    pub tool_totals: ToolCounts,
}

/// Paginated sessions response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
    pub stats: ProjectStats,
}
```

---

## Part 3: Frontend Data Flow

### Current (single fetch, all data)

```
App mount → GET /api/projects (676 KB) → Sidebar + ProjectView rendered
```

### Proposed (two fetches, lazy)

```
App mount → GET /api/projects (2 KB) → Sidebar rendered immediately

User clicks project → GET /api/projects/:id/sessions (87 KB max) → ProjectView rendered
                      ↑ cached by react-query, subsequent clicks instant
```

### React Query Setup

```typescript
// Lightweight project list — cached indefinitely, refetched on window focus
export function useProjects() {
  return useQuery({
    queryKey: ['projects'],
    queryFn: () => fetch('/api/projects').then(r => r.json()),
  })
}

// Sessions for a specific project — fetched on demand
export function useProjectSessions(projectId: string | undefined) {
  return useInfiniteQuery({
    queryKey: ['sessions', projectId],
    queryFn: ({ pageParam = 0 }) =>
      fetch(`/api/projects/${encodeURIComponent(projectId!)}/sessions?offset=${pageParam}&limit=50`)
        .then(r => r.json()),
    enabled: !!projectId,
    getNextPageParam: (lastPage, pages) => {
      const loaded = pages.flatMap(p => p.sessions).length
      return loaded < lastPage.total ? loaded : undefined
    },
  })
}
```

---

## Implementation Steps

| Step | Depends on | Deliverable |
|------|-----------|-------------|
| 1. Add `summary`, `git_branch`, `is_sidechain`, `deep_indexed` to `SessionInfo` | Indexing plan Step 4 | `types.rs` |
| 2. Add `ProjectSummary`, `ProjectStats`, `SessionsResponse` types | — | `types.rs` |
| 3. Add `list_project_summaries()` query | Indexing plan Step 3 (migration) | `queries.rs` |
| 4. Add `list_sessions_for_project()` with filters + pagination | Step 1 | `queries.rs` |
| 5. Add `get_project_stats()` aggregation query | Step 1 | `queries.rs` |
| 6. Update `/api/projects` to return `ProjectSummary` | Steps 2, 3 | `routes/projects.rs` |
| 7. Add `GET /api/projects/:id/sessions` endpoint | Steps 2, 4, 5 | `routes/sessions.rs` |
| 8. Add indexes on `git_branch`, `is_sidechain` | Indexing plan Step 3 | Migration SQL |
| 9. Update `SessionInfo` TypeScript interface | Step 1 | `use-projects.ts` |
| 10. Split `useProjects` / add `useProjectSessions` hook | Steps 6, 7 | `use-projects.ts` |
| 11. Update `Sidebar` to use `ProjectSummary` | Steps 9, 10 | `Sidebar.tsx` |
| 12. Update `ProjectView` with pagination + sidechain toggle | Step 10 | `ProjectView.tsx` |
| 13. Update `SessionCard` with summary, branch badge | Step 9 | `SessionCard.tsx` |
| 14. Update existing tests | Steps 6, 7 | `routes/projects.rs`, `queries.rs` |
| 15. Add new endpoint tests | Step 7 | `routes/sessions.rs` |

---

## Acceptance Criteria

### AC-1: `/api/projects` returns lightweight summaries

```rust
#[tokio::test]
async fn test_projects_returns_summaries_not_sessions() {
    let db = setup_db_with_sessions(50).await;
    let app = build_app(db);
    let (status, body) = get(app, "/api/projects").await;

    assert_eq!(status, StatusCode::OK);
    let projects: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();

    // Should have sessionCount, NOT sessions array
    assert!(projects[0].get("sessionCount").is_some());
    assert!(projects[0].get("sessions").is_none());
    assert!(projects[0].get("lastActivityAt").is_some());
}
```

### AC-2: `/api/projects/:id/sessions` returns paginated sessions

```rust
#[tokio::test]
async fn test_sessions_endpoint_paginated() {
    let db = setup_db_with_sessions_for_project("proj-a", 80).await;
    let app = build_app(db);

    // First page
    let (_, body) = get(app.clone(), "/api/projects/proj-a/sessions?limit=50&offset=0").await;
    let resp: SessionsResponse = serde_json::from_str(&body).unwrap();
    assert_eq!(resp.sessions.len(), 50);
    assert_eq!(resp.total, 80);

    // Second page
    let (_, body) = get(app, "/api/projects/proj-a/sessions?limit=50&offset=50").await;
    let resp: SessionsResponse = serde_json::from_str(&body).unwrap();
    assert_eq!(resp.sessions.len(), 30);
    assert_eq!(resp.total, 80);
}
```

### AC-3: Sidechain filtering works

```rust
#[tokio::test]
async fn test_sidechains_excluded_by_default() {
    let db = setup_db_with_sidechains("proj-a", 10, 5).await; // 10 normal, 5 sidechain
    let app = build_app(db);

    // Default: exclude sidechains
    let (_, body) = get(app.clone(), "/api/projects").await;
    let projects: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(projects[0]["sessionCount"], 10);

    // Include sidechains
    let (_, body) = get(app.clone(), "/api/projects?include_sidechains=true").await;
    let projects: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(projects[0]["sessionCount"], 15);

    // Sessions endpoint also excludes by default
    let (_, body) = get(app.clone(), "/api/projects/proj-a/sessions").await;
    let resp: SessionsResponse = serde_json::from_str(&body).unwrap();
    assert_eq!(resp.total, 10);
}
```

### AC-4: Branch filtering works

```rust
#[tokio::test]
async fn test_branch_filter() {
    let db = setup_db_with_branches("proj-a", &[
        ("main", 5), ("feature/auth", 3), ("fix/bug", 2),
    ]).await;
    let app = build_app(db);

    let (_, body) = get(app, "/api/projects/proj-a/sessions?branch=feature/auth").await;
    let resp: SessionsResponse = serde_json::from_str(&body).unwrap();
    assert_eq!(resp.total, 3);
    assert!(resp.sessions.iter().all(|s| s.git_branch.as_deref() == Some("feature/auth")));
}
```

### AC-5: New fields in session response

```rust
#[tokio::test]
async fn test_new_fields_present() {
    let db = setup_db_with_new_fields().await;
    let app = build_app(db);

    let (_, body) = get(app, "/api/projects/proj-a/sessions").await;
    let resp: SessionsResponse = serde_json::from_str(&body).unwrap();
    let session = &resp.sessions[0];

    assert!(session.summary.is_some());
    assert!(session.git_branch.is_some());
    assert!(!session.is_sidechain);
    assert!(session.deep_indexed);
}
```

### AC-6: Project stats are server-computed

```rust
#[tokio::test]
async fn test_project_stats_aggregated() {
    let db = setup_db_with_varied_sessions("proj-a").await;
    let app = build_app(db);

    let (_, body) = get(app, "/api/projects/proj-a/sessions").await;
    let resp: SessionsResponse = serde_json::from_str(&body).unwrap();

    assert!(!resp.stats.top_skills.is_empty());
    assert!(!resp.stats.top_files.is_empty());
    assert!(resp.stats.tool_totals.total() > 0);
}
```

### AC-7: Response size < 5 KB for project list

```rust
#[tokio::test]
async fn test_project_list_small_response() {
    let db = setup_db_with_sessions(500).await; // 500 sessions across projects
    let app = build_app(db);

    let (_, body) = get(app, "/api/projects").await;
    assert!(body.len() < 5_000, "Project list should be <5 KB, got {} bytes", body.len());
}
```

### AC-8: Existing tests pass

`cargo test --workspace` passes with zero failures after all changes.

### AC-9: Frontend renders summary over preview

Manual verification:
- Session card shows `summary` as primary text when available
- Falls back to `preview` when `summary` is null
- Branch badge appears next to timestamp
- Sidechain sessions hidden by default in ProjectView

---

## Migration Path (Backward Compatibility)

The frontend currently calls `GET /api/projects` and expects `sessions[]` in the response. To avoid a hard cutover:

**Phase 1 (this plan):** Add the new `/api/projects/:id/sessions` endpoint. Keep the old `/api/projects` response shape but add `sessionCount` and `lastActivityAt` fields alongside `sessions[]`.

**Phase 2 (follow-up):** Update frontend to use split endpoints. Remove `sessions[]` from `/api/projects` response.

This allows frontend and backend to be deployed independently.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Breaking existing frontend | Phase 1 adds fields without removing any; Phase 2 removes after frontend migrated |
| N+1 for project stats | Single SQL aggregation query, not per-session iteration |
| Branch filter on NULL values | Sessions without `git_branch` excluded from branch-filtered queries (expected) |
| Sidechain flag wrong | Falls back to `false` (safe default); user can toggle to see all |

---

## Part 4: UX Audit & Redesign Recommendations

Full review of all frontend components against [Web Interface Guidelines](https://github.com/vercel-labs/web-interface-guidelines) and production-grade design standards.

### 4.1 Accessibility Violations (must fix)

| File | Line | Issue | Fix |
|------|------|-------|-----|
| `Header.tsx` | 87 | Icon-only `<button>` (HelpCircle) has no `aria-label` | Add `aria-label="Help"` |
| `Header.tsx` | 91 | Icon-only `<button>` (Settings) has no `aria-label` | Add `aria-label="Settings"` |
| `ProjectView.tsx` | 38-48 | `<Link>` wraps `<SessionCard>` which is a `<button>` — nested interactive elements | Change `SessionCard` root to `<div>`, let `<Link>` be the interactive wrapper |
| `SearchResults.tsx` | 69-79 | Same nested `<Link>` > `<button>` pattern | Same fix |
| `StatsDashboard.tsx` | 263 | Heatmap `<button>` cells have `title` but no `aria-label` — screen readers get nothing useful | Add `aria-label={`${day.date.toLocaleDateString()}: ${day.count} sessions`}` |
| `ConversationView.tsx` | 116 | Export buttons have no `aria-label` and rely on ambiguous icon | Add `aria-label="Export as HTML"` / `"Export as PDF"` |

### 4.2 Focus & Keyboard Issues (must fix)

| File | Line | Issue | Fix |
|------|------|-------|-----|
| `Sidebar.tsx` | 85 | `<Link>` elements lack visible focus indicator | Add `focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1` |
| `SessionCard.tsx` | 47 | `<button>` has no `focus-visible` ring — only hover style | Add `focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none` |
| `Header.tsx` | 77 | Search button lacks `focus-visible` | Add focus ring |
| `StatsDashboard.tsx` | 108 | Skill buttons, project links — no focus indicators | Add focus rings to all interactive elements |
| All components | — | No skip-to-content link exists in `App.tsx` | Add `<a href="#main" className="sr-only focus:not-sr-only">Skip to main content</a>` and `id="main"` on `<main>` |

### 4.3 URL State Not Reflected (should fix)

| Feature | Current | Guideline |
|---------|---------|-----------|
| Sidechain toggle | Local React state | Should be `?sidechains=true` in URL |
| Branch filter | Not implemented yet | Must be `?branch=feature/auth` from day 1 |
| Pagination offset | Not implemented yet | Must be `?page=2` or `?offset=50` |
| Sort order | Not implemented yet | Must be `?sort=oldest` |
| Selected project | Already in URL (`/project/:id`) | Correct |

**Rule:** "URL reflects state — filters, tabs, pagination, expanded panels in query params." Every filter and pagination state must survive a page refresh and be shareable as a link.

### 4.4 Performance Concerns (should fix)

| File | Line | Issue | Fix |
|------|------|-------|-----|
| `ProjectView.tsx` | 36-49 | Session list renders all sessions without virtualization. At 80+ sessions, this causes layout thrashing. | Use `react-virtuoso` (already a dependency — `ConversationView.tsx` uses it) for session list |
| `Sidebar.tsx` | 20-57 | `projectStats` recomputes skill/file aggregation from all sessions on every render when selected | With API split, this moves server-side — no client computation |
| `StatsDashboard.tsx` | 15-66 | Aggregates all sessions client-side into stats. With 5,000+ sessions this is slow. | After API split, dashboard should fetch from a dedicated stats endpoint |
| `CommandPalette.tsx` | 96-107 | `filterSessions` runs on every keystroke across ALL sessions | Debounce input (150ms); with API split, search should hit server |

### 4.5 Motion & Reduced Motion (should fix)

No component respects `prefers-reduced-motion`. The `animate-spin` on `Loader2` and any future transitions should be wrapped:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

Add to `index.css`. This is a one-line fix that covers the entire app.

### 4.6 Typography & Visual Identity (design upgrade)

**Current state:** System font stack (`-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto`) with entirely gray palette and `blue-500` as the only accent. This looks like an unfinished Tailwind starter. The CommandPalette dark theme (`#111113`) is disconnected from the light app shell. The sage green `#7c9885` in StatsDashboard appears once and nowhere else.

**Recommended direction: "Developer Terminal" aesthetic** — lean into the tool's identity as a developer's session viewer. Clean, dense, monospace-forward where data matters, proportional where prose matters.

| Element | Current | Recommended |
|---------|---------|-------------|
| **Body font** | System sans-serif | `"Inter Variable"` or `"Geist Sans"` — designed for UI density |
| **Mono font** | Browser default | `"Geist Mono"` or `"JetBrains Mono"` — for session IDs, branch names, skills, file paths |
| **Color accent** | `blue-500` (generic) | `#7c9885` sage green (already in StatsDashboard — promote to primary accent) |
| **Session summary** | Not yet implemented | `text-[15px] leading-snug font-medium text-gray-900` — the most readable text on the card |
| **First prompt** | Currently primary | Demote to `text-[13px] text-gray-500 font-mono` — shows user's raw input below summary |
| **Branch badge** | Not yet implemented | `font-mono text-[11px] px-1.5 py-0.5 bg-emerald-50 text-emerald-700 rounded` — git-green |
| **Sidechain indicator** | Not yet implemented | Thin left-border `border-l-2 border-amber-300` + slight indent — visually subordinate |

### 4.7 Information Hierarchy Redesign

The current `SessionCard` treats all data equally. With `summary` available, the hierarchy should be:

```
┌─────────────────────────────────────────────────────────┐
│  Claude-view UI: Sidebar paths, SPA nav,        Today,  │  ← summary (bold, primary)
│  active indicators                              2:30 PM  │
│                                                          │
│  "some feedback abt the @docs/plans/..."                 │  ← firstPrompt (mono, secondary)
│                                                          │
│  main ← branch badge         ⚙ not yet indexed ← deep  │
│                                                          │
│  src/main.rs, Cargo.toml                                 │  ← files touched
│  ──────────────────────────────────────────────────────  │
│  ✏ 5  💻 3  👁 10    60 msgs · 28 turns    /commit +1   │  ← footer
└─────────────────────────────────────────────────────────┘
```

vs current (all text looks the same weight, no summary, no branch):

```
┌─────────────────────────────────────────────────────────┐
│  "some feedback abt the @docs/plans/..."  Today, 2:30PM │
│  → "Here is the result"                                  │
│  src/main.rs, Cargo.toml                                 │
│  ──────────────────────────────────────────────────────  │
│  ✏ 5  💻 3  👁 10    60 msgs · 28 turns    /commit +1   │
└─────────────────────────────────────────────────────────┘
```

Key differences:
1. **Summary is the headline** — descriptive, scannable. Users can skim a project's sessions and understand what each one was about without reading raw prompts.
2. **First prompt becomes supporting context** — shown in mono, dimmer. It's the user's raw input, useful for identification but not for scanning.
3. **Branch badge** — immediately tells you "this work happened on `feature/auth`". Colored green to echo git semantics.
4. **Deep-index indicator** — when `!deepIndexed`, show a subtle "indexing..." note instead of tool counts. Avoids showing `0` for all tools (which looks like nothing happened) vs "not yet counted."
5. **Sidechain visual treatment** — left amber border + slightly indented, so these sessions are clearly subordinate without needing a toggle to hide them entirely. The toggle still hides them, but when visible they're visually distinct.

### 4.8 Sidebar Simplification

With `ProjectSummary` (no sessions array), the sidebar becomes leaner:

**Current sidebar (with sessions loaded):**
- Project list
- Per-project stats panel (skills, files, tools) computed client-side from all sessions

**Proposed sidebar:**
- Project list with `sessionCount` badge (from lightweight API)
- Stats panel appears only after clicking a project (loaded with sessions response)
- Stats panel shows server-computed aggregates (accurate across all sessions, not just page 1)

This eliminates the biggest data dependency: the sidebar no longer needs all 542 sessions just to show project-level stats.

### 4.9 Empty & Loading States

| State | Current | Proposed |
|-------|---------|----------|
| **First launch (empty DB)** | Generic "No Claude Code sessions found" | "Indexing your sessions..." with progress from SSE (ties into indexing plan) |
| **Project selected, sessions loading** | No loading state in ProjectView | Skeleton cards (3-4 gray rectangles pulsing) matching SessionCard layout |
| **Deep indexing in progress** | Not implemented | Tool counts section shows `⟳ Analyzing...` instead of zeros |
| **Search with no results** | "No sessions match your search" | Add suggestion: "Try searching by summary, branch, or skill" |
| **Error loading sessions** | Not implemented in ProjectView | Error banner with retry button |

### 4.10 Command Palette Upgrades

The CommandPalette currently searches `preview` (first prompt). With `summary` and `gitBranch`:

1. **Search summaries** — `summary` field is human-readable and more searchable than raw prompts
2. **Branch autocomplete** — typing `branch:` shows available branches with session counts
3. **Sidechain filter** — add `sidechain:true` / `sidechain:false` to filter hints

Update the filter hints section (`CommandPalette.tsx:242`):
```
project:  path:  skill:  branch:  after:  before:  "phrase"  sidechain:
```

### 4.11 StatsDashboard Integration

With `summary` and `gitBranch` available, the dashboard gains:

1. **Branch activity chart** — which branches get the most sessions (bar chart alongside "Most Active Projects")
2. **Summary word cloud** or **topic clusters** — group sessions by common summary themes (defer to Phase 2)
3. **Sidechain ratio** — "23% of sessions are sub-agent" gives users a sense of how agentic their usage is

For this plan, only the branch activity chart is in scope. Word cloud and topic clustering are out of scope (require Tantivy or client-side NLP).

---

## Summary of UX Changes by Component

| Component | Changes |
|-----------|---------|
| `index.css` | Add `prefers-reduced-motion` media query; add Geist font imports |
| `App.tsx` | Add skip-to-content link; `id="main"` on `<main>` |
| `Header.tsx` | `aria-label` on icon buttons; focus-visible rings |
| `Sidebar.tsx` | Use `ProjectSummary` type; session count from API; remove client-side stats computation; focus-visible on links |
| `ProjectView.tsx` | Fetch sessions via `useProjectSessions`; virtualize session list; sidechain toggle as URL param; skeleton loading state |
| `SessionCard.tsx` | Change root to `<div>` (fix nested interactive); summary as primary text; firstPrompt as secondary mono; branch badge; deep-index indicator; focus-visible |
| `SearchResults.tsx` | Fix nested interactive; search summaries; branch filter hint |
| `CommandPalette.tsx` | Add `branch:` and `sidechain:` filters; search against summary field |
| `StatsDashboard.tsx` | aria-label on heatmap; branch activity section; focus-visible on all buttons |
| `StatusBar.tsx` | Use `ProjectSummary` counts directly (no session iteration) |

---

## Out of Scope

- Full-text search across summaries (needs Tantivy integration from `crates/search/`)
- `stats-cache.json` integration (dashboard analytics — separate plan)
- `history.jsonl` integration (prompt search — separate plan)
- Real-time updates via WebSocket (Phase 4 v2 roadmap)
- Summary word cloud / topic clustering (requires NLP)
- Font change to Geist (evaluate separately — adds ~100 KB to bundle)
