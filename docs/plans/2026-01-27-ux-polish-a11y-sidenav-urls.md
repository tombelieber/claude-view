---
status: pending
date: 2026-01-27
depends_on:
  - 2026-01-27-api-schema-bonus-fields-design.md
---

# UX Polish: Accessibility, VSCode-Style Sidebar, Human-Readable URLs

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all accessibility violations, redesign the sidebar to feel like VSCode's explorer panel, make session URLs human-readable, and apply the information hierarchy from the bonus-fields design.

**Architecture:** Frontend-only changes (React components, CSS, routing). No backend API changes required — this plan consumes the API contract defined in `docs/plans/2026-01-27-api-schema-bonus-fields-design.md`. The URL slug generation is a pure frontend concern using data already available from the API.

**Tech Stack:** React 19, React Router 7, TanStack Query 5, Tailwind CSS 4, Zustand 5, Lucide icons, react-virtuoso.

---

## Current State Summary

| File | Lines | Role |
|------|-------|------|
| `src/index.css` | 6 | Global styles — system font stack, Tailwind import |
| `src/router.tsx` | 19 | React Router config — 4 routes |
| `src/App.tsx` | 87 | Shell — Header, Sidebar, main area, StatusBar, CommandPalette |
| `src/main.tsx` | 23 | React entry point with QueryClient |
| `src/store/app-store.ts` | 48 | Zustand store — search state, recent searches |
| `src/lib/utils.ts` | 6 | cn() helper |
| `src/lib/search.ts` | 183 | Client-side search engine |
| `src/hooks/use-projects.ts` | 45 | ProjectInfo/SessionInfo types + useProjects() |
| `src/hooks/use-session.ts` | 42 | useSession() hook for conversation view |
| `src/components/Header.tsx` | 97 | Top bar — logo, breadcrumbs, search, icon buttons |
| `src/components/Sidebar.tsx` | 208 | Project list + per-project stats panel |
| `src/components/ProjectView.tsx` | 59 | Session list for a project |
| `src/components/SessionCard.tsx` | 144 | Session card — preview, tools, skills |
| `src/components/ConversationView.tsx` | 158 | Full conversation reader (virtualized) |
| `src/components/SearchResults.tsx` | 90 | Search results page |
| `src/components/StatsDashboard.tsx` | 287 | Home dashboard — stats, heatmap |
| `src/components/CommandPalette.tsx` | 272 | ⌘K search modal |
| `src/components/StatusBar.tsx` | 33 | Bottom bar — project/session counts |
| `src/components/HealthIndicator.tsx` | 55 | Backend status dot |

---

## Task 1: Global Accessibility — Reduced Motion + Skip Link

**Files:**
- Modify: `src/index.css`
- Modify: `src/App.tsx:66-86`

**Step 1: Add reduced-motion media query and skip link styles to index.css**

Replace entire `src/index.css`:

```css
@import 'tailwindcss';

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
}

/* Respect reduced-motion preference */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

**Step 2: Add skip-to-content link and main landmark in App.tsx**

In `src/App.tsx`, add skip link as first child of the root div, and `id="main"` on the `<main>` element:

```tsx
// Inside the return of App(), change the wrapping div:
return (
  <div className="h-screen flex flex-col">
    <a
      href="#main"
      className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-[100] focus:px-4 focus:py-2 focus:bg-white focus:text-gray-900 focus:rounded-lg focus:shadow-lg focus:ring-2 focus:ring-blue-500"
    >
      Skip to main content
    </a>
    <Header />

    <div className="flex-1 flex overflow-hidden">
      <Sidebar projects={projects} />

      <main id="main" className="flex-1 overflow-hidden bg-gray-50">
        <Outlet context={{ projects }} />
      </main>
    </div>

    <StatusBar projects={projects} />

    <CommandPalette
      isOpen={isCommandPaletteOpen}
      onClose={closeCommandPalette}
      projects={projects}
    />
  </div>
)
```

**Step 3: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 4: Commit**

```bash
git add src/index.css src/App.tsx
git commit -m "a11y: add skip-to-content link and prefers-reduced-motion

Adds sr-only skip link that appears on focus, targets #main landmark.
Reduced-motion media query disables all animations/transitions."
```

---

## Task 2: Header Accessibility — aria-labels + focus-visible

**Files:**
- Modify: `src/components/Header.tsx:76-93`

**Step 1: Add aria-labels to icon-only buttons and focus-visible rings**

In `Header.tsx`, update the right-side buttons section (lines 76-93):

```tsx
{/* Right: Search + Actions */}
<div className="flex items-center gap-2">
  <button
    onClick={openCommandPalette}
    aria-label="Search sessions"
    className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-500 hover:text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none"
  >
    <Search className="w-4 h-4" />
    <span className="hidden sm:inline">Search</span>
    <kbd className="hidden sm:inline text-xs text-gray-400 bg-white px-1.5 py-0.5 rounded border border-gray-200">
      ⌘K
    </kbd>
  </button>

  <button
    aria-label="Help"
    className="p-2 text-gray-400 hover:text-gray-600 transition-colors rounded-lg focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none"
  >
    <HelpCircle className="w-5 h-5" />
  </button>

  <button
    aria-label="Settings"
    className="p-2 text-gray-400 hover:text-gray-600 transition-colors rounded-lg focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none"
  >
    <Settings className="w-5 h-5" />
  </button>
</div>
```

**Step 2: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 3: Commit**

```bash
git add src/components/Header.tsx
git commit -m "a11y: add aria-labels and focus-visible to Header buttons"
```

---

## Task 3: SessionCard — Fix Nested Interactive + Add focus-visible

**Files:**
- Modify: `src/components/SessionCard.tsx:39-53`
- Modify: `src/components/ProjectView.tsx:37-49`
- Modify: `src/components/SearchResults.tsx:64-79`

**Step 1: Change SessionCard root element from `<button>` to `<article>`**

The card is currently a `<button>` but gets wrapped in `<Link>` (also interactive) — this is a WCAG violation (nested interactive elements). The `<Link>` is the correct interactive wrapper; the card should be a passive container.

In `SessionCard.tsx`, change the interface and root element:

```tsx
interface SessionCardProps {
  session: SessionInfo
  isSelected: boolean
}

export function SessionCard({ session, isSelected }: SessionCardProps) {
  const toolCounts = session.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
  const editCount = toolCounts.edit + toolCounts.write
  const totalTools = editCount + toolCounts.bash + toolCounts.read

  return (
    <article
      className={cn(
        'w-full text-left p-4 rounded-lg border transition-all',
        isSelected
          ? 'bg-blue-50 border-blue-500 shadow-[0_0_0_1px_#3b82f6]'
          : 'bg-white border-gray-200 group-hover:bg-gray-50 group-hover:border-gray-300 group-hover:shadow-sm'
      )}
    >
```

Note: `hover:` styles move to `group-hover:` because the parent `<Link>` is now the hover target.

**Step 2: Update ProjectView to use `<Link>` with group class**

In `ProjectView.tsx` (lines 37-49):

```tsx
<div className="space-y-3">
  {project.sessions.map((session) => (
    <Link
      key={session.id}
      to={`/session/${encodeURIComponent(session.project)}/${session.id}`}
      className="block group focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded-lg"
    >
      <SessionCard
        session={session}
        isSelected={false}
      />
    </Link>
  ))}
</div>
```

**Step 3: Update SearchResults similarly**

In `SearchResults.tsx` (lines 64-79):

```tsx
{results.map((session) => (
  <Link
    key={session.id}
    to={`/session/${encodeURIComponent(session.project)}/${session.id}`}
    className="block group focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded-lg"
  >
    <SessionCard
      session={session}
      isSelected={false}
    />
  </Link>
))}
```

**Step 4: Verify**

Run: `bun run typecheck`
Expected: No errors. The `onClick` prop removal may cause TS errors if any callers still pass it — fix by removing `onClick` from the interface.

**Step 5: Commit**

```bash
git add src/components/SessionCard.tsx src/components/ProjectView.tsx src/components/SearchResults.tsx
git commit -m "a11y: fix nested interactive elements in SessionCard

Change SessionCard root from <button> to <article>.
Parent <Link> is now the sole interactive element.
Add focus-visible rings and group-hover for card styles."
```

---

## Task 4: Sidebar Accessibility — focus-visible on project links

**Files:**
- Modify: `src/components/Sidebar.tsx:82-116`

**Step 1: Add focus-visible ring to sidebar project links**

In `Sidebar.tsx`, update the `<Link>` className (line 85):

```tsx
<Link
  key={project.name}
  to={`/project/${encodeURIComponent(project.name)}`}
  className={cn(
    'w-full flex items-start gap-2.5 px-3 py-2 text-left transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-inset focus-visible:outline-none',
    isSelected
      ? 'bg-blue-500 text-white'
      : 'text-gray-700 hover:bg-gray-200/70'
  )}
>
```

Note: `ring-inset` because sidebar items are flush against each other — an outset ring would overlap adjacent items.

**Step 2: Add focus-visible to sidebar stat buttons**

Update skill buttons (line 144) and file buttons (line 165):

Skill buttons:
```tsx
className="px-1.5 py-0.5 text-[11px] font-mono bg-gray-100 hover:bg-blue-500 hover:text-white text-gray-600 rounded transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none"
```

File buttons:
```tsx
className="w-full flex items-center justify-between px-1.5 py-1 text-[11px] hover:bg-gray-100 rounded transition-colors text-left focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none"
```

**Step 3: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 4: Commit**

```bash
git add src/components/Sidebar.tsx
git commit -m "a11y: add focus-visible rings to all Sidebar interactive elements"
```

---

## Task 5: StatsDashboard Accessibility — heatmap aria-labels + focus-visible

**Files:**
- Modify: `src/components/StatsDashboard.tsx:263-270, 108, 141`

**Step 1: Add aria-label to heatmap cells**

In `StatsDashboard.tsx`, the `ActivityHeatmap` component (around line 263):

```tsx
<button
  key={day.date.toISOString()}
  onClick={() => handleDayClick(day.date)}
  aria-label={`${day.date.toLocaleDateString('en-US', { weekday: 'short', month: 'short', day: 'numeric' })}: ${day.count} session${day.count !== 1 ? 's' : ''}`}
  className={cn(
    'w-3 h-3 rounded-sm transition-colors hover:ring-2 hover:ring-blue-400 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:outline-none',
    getColor(day.count)
  )}
  title={`${day.date.toLocaleDateString()}: ${day.count} sessions`}
/>
```

**Step 2: Add focus-visible to skill and project interactive elements**

Skill buttons (around line 108):
```tsx
className="w-full group text-left focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded-lg"
```

Project links (around line 141):
```tsx
className="w-full group block focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded-lg"
```

**Step 3: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 4: Commit**

```bash
git add src/components/StatsDashboard.tsx
git commit -m "a11y: add aria-labels to heatmap, focus-visible to dashboard"
```

---

## Task 6: ConversationView Accessibility — export button labels

**Files:**
- Modify: `src/components/ConversationView.tsx:115-128`

**Step 1: Add aria-labels to export buttons**

```tsx
<div className="flex items-center gap-2">
  <button
    onClick={handleExportHtml}
    aria-label="Export conversation as HTML"
    className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none"
  >
    <span>HTML</span>
    <Download className="w-4 h-4" />
  </button>
  <button
    onClick={handleExportPdf}
    aria-label="Export conversation as PDF"
    className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none"
  >
    <span>PDF</span>
    <Download className="w-4 h-4" />
  </button>
</div>
```

**Step 2: Add aria-label to back button**

```tsx
<button
  onClick={handleBack}
  aria-label="Back to sessions"
  className="flex items-center gap-1 text-gray-600 hover:text-gray-900 transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded"
>
  <ArrowLeft className="w-4 h-4" />
  <span className="text-sm">Back to sessions</span>
</button>
```

**Step 3: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "a11y: add aria-labels and focus-visible to ConversationView buttons"
```

---

## Task 7: VSCode-Style Sidebar Redesign

**Files:**
- Modify: `src/components/Sidebar.tsx` (major rewrite)

This redesigns the sidebar to follow VSCode Explorer conventions:

| VSCode Pattern | Our Equivalent |
|----------------|---------------|
| Explorer panel header with collapse/actions | "PROJECTS" header with session count |
| Tree items with indent + chevron for expand | Project items with expand chevron for session sub-list |
| Selection highlight = full-width bar | Same — blue-500 bar |
| Hover = subtle bg shift | Same — gray-200/70 |
| Item badge (file count) | Session count badge, right-aligned |
| Sticky section headers | "PROJECTS" header sticks to top |
| Compact density | 28px row height (matching VSCode's default) |
| Mono font for paths | Mono font for file paths in stats |
| Keyboard nav | Arrow keys to navigate project list |

**Step 1: Write the redesigned Sidebar component**

Replace the entire `Sidebar.tsx`:

```tsx
import { useState, useRef, useEffect } from 'react'
import { Link, useParams, useNavigate } from 'react-router-dom'
import { ChevronRight, ChevronDown, FolderOpen, Pencil, Eye, Terminal } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface SidebarProps {
  projects: ProjectInfo[]
}

export function Sidebar({ projects }: SidebarProps) {
  const params = useParams()
  const navigate = useNavigate()
  const listRef = useRef<HTMLDivElement>(null)
  const [focusedIndex, setFocusedIndex] = useState(-1)

  const selectedProjectId = params.projectId ? decodeURIComponent(params.projectId) : null

  // Keyboard navigation (VSCode-style arrow keys)
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setFocusedIndex(i => Math.min(i + 1, projects.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setFocusedIndex(i => Math.max(i - 1, 0))
    } else if (e.key === 'Enter' && focusedIndex >= 0) {
      e.preventDefault()
      const project = projects[focusedIndex]
      navigate(`/project/${encodeURIComponent(project.name)}`)
    }
  }

  // Focus the correct element when focusedIndex changes
  useEffect(() => {
    if (focusedIndex >= 0 && listRef.current) {
      const items = listRef.current.querySelectorAll('[data-project-item]')
      const item = items[focusedIndex] as HTMLElement | undefined
      item?.focus()
    }
  }, [focusedIndex])

  // Total session count
  const totalSessions = projects.reduce((sum, p) => sum + p.sessions.length, 0)

  return (
    <aside className="w-64 bg-[#f8f8f8] border-r border-gray-200 flex flex-col overflow-hidden select-none">
      {/* VSCode-style section header */}
      <div className="h-9 flex items-center justify-between px-3 border-b border-gray-200 bg-[#f8f8f8] sticky top-0 z-10">
        <span className="text-[11px] font-semibold text-gray-500 uppercase tracking-wider">
          Projects
        </span>
        <span className="text-[11px] text-gray-400 tabular-nums">
          {totalSessions}
        </span>
      </div>

      {/* Project tree */}
      <div
        ref={listRef}
        className="flex-1 overflow-y-auto py-0.5"
        role="tree"
        aria-label="Project list"
        onKeyDown={handleKeyDown}
      >
        {projects.map((project, index) => {
          const isSelected = selectedProjectId === project.name
          const parentPath = project.name.split('/').slice(0, -1).join('/')

          return (
            <Link
              key={project.name}
              to={`/project/${encodeURIComponent(project.name)}`}
              data-project-item
              role="treeitem"
              aria-selected={isSelected}
              tabIndex={index === focusedIndex ? 0 : -1}
              className={cn(
                'w-full flex items-center gap-1.5 pl-2 pr-3 h-[22px] text-[13px] transition-colors',
                'focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-inset focus-visible:outline-none',
                isSelected
                  ? 'bg-[#0060C0] text-white'
                  : 'text-gray-700 hover:bg-[#e8e8e8]'
              )}
            >
              {/* Expand chevron */}
              {isSelected ? (
                <ChevronDown className="w-3.5 h-3.5 flex-shrink-0 opacity-80" />
              ) : (
                <ChevronRight className="w-3.5 h-3.5 flex-shrink-0 opacity-50" />
              )}

              {/* Folder icon */}
              <FolderOpen className={cn(
                'w-4 h-4 flex-shrink-0',
                isSelected ? 'text-white/80' : 'text-[#C5A63C]'
              )} />

              {/* Project name */}
              <span className="flex-1 truncate">
                {project.displayName}
              </span>

              {/* Session count badge */}
              <span className={cn(
                'text-[11px] tabular-nums flex-shrink-0',
                isSelected ? 'text-white/70' : 'text-gray-400'
              )}>
                {project.sessions.length}
              </span>
            </Link>
          )
        })}
      </div>

      {/* Per-project stats panel — only when project selected */}
      {selectedProjectId && (() => {
        const selectedProject = projects.find(p => p.name === selectedProjectId)
        if (!selectedProject) return null

        // Compute stats from sessions
        const skillCounts = new Map<string, number>()
        const fileCounts = new Map<string, number>()
        let totalEdits = 0, totalReads = 0, totalBash = 0

        for (const session of selectedProject.sessions) {
          for (const skill of session.skillsUsed ?? []) {
            skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
          }
          for (const file of session.filesTouched ?? []) {
            fileCounts.set(file, (fileCounts.get(file) || 0) + 1)
          }
          const tc = session.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
          totalEdits += tc.edit + tc.write
          totalReads += tc.read
          totalBash += tc.bash
        }

        const topSkills = Array.from(skillCounts.entries())
          .sort((a, b) => b[1] - a[1])
          .slice(0, 5)
        const topFiles = Array.from(fileCounts.entries())
          .sort((a, b) => b[1] - a[1])
          .slice(0, 5)
        const maxTools = Math.max(totalEdits, totalReads, totalBash, 1)

        return (
          <div className="border-t border-gray-200 bg-white">
            {/* Collapsible section headers like VSCode's "OUTLINE" panel */}
            <div className="h-9 flex items-center px-3 border-b border-gray-100">
              <span className="text-[11px] font-semibold text-gray-500 uppercase tracking-wider">
                Details
              </span>
            </div>

            <div className="p-3 space-y-4">
              {/* Path */}
              <p className="text-[11px] text-gray-400 font-mono truncate" title={selectedProject.path}>
                {selectedProject.path}
              </p>

              {/* Skills */}
              {topSkills.length > 0 && (
                <div>
                  <p className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider mb-1.5">
                    Skills
                  </p>
                  <div className="flex flex-wrap gap-1">
                    {topSkills.map(([skill, count]) => (
                      <span
                        key={skill}
                        className="px-1.5 py-0.5 text-[11px] font-mono bg-gray-100 text-gray-600 rounded"
                      >
                        {skill} <span className="opacity-50">{count}</span>
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Top Files */}
              {topFiles.length > 0 && (
                <div>
                  <p className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider mb-1.5">
                    Files
                  </p>
                  <div className="space-y-0.5">
                    {topFiles.map(([file, count]) => (
                      <div
                        key={file}
                        className="flex items-center justify-between px-1 py-0.5 text-[11px] rounded"
                      >
                        <span className="truncate text-gray-600 font-mono">{file}</span>
                        <span className="text-gray-400 tabular-nums ml-2">{count}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Tool Usage */}
              <div>
                <p className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider mb-1.5">
                  Tools
                </p>
                <div className="space-y-1.5">
                  {[
                    { label: 'Edit', value: totalEdits, icon: Pencil, color: 'bg-blue-400' },
                    { label: 'Read', value: totalReads, icon: Eye, color: 'bg-green-400' },
                    { label: 'Bash', value: totalBash, icon: Terminal, color: 'bg-amber-400' },
                  ].map(({ label, value, icon: Icon, color }) => (
                    <div key={label} className="flex items-center gap-2">
                      <Icon className="w-3 h-3 text-gray-400" />
                      <span className="text-[11px] text-gray-500 w-7">{label}</span>
                      <div className="flex-1 h-1.5 bg-gray-100 rounded-full overflow-hidden">
                        <div
                          className={cn('h-full rounded-full', color)}
                          style={{ width: `${(value / maxTools) * 100}%` }}
                        />
                      </div>
                      <span className="text-[11px] text-gray-400 tabular-nums w-7 text-right">
                        {value}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        )
      })()}
    </aside>
  )
}
```

Key VSCode-style changes:
- **w-64** (256px) instead of w-72 (288px) — closer to VSCode's default
- **22px row height** — matches VSCode's compact tree items
- **Chevron + Folder icon** — expand/collapse hint like VSCode explorer
- **#0060C0 selection** — VSCode's exact blue selection color
- **#C5A63C folder icon** — VSCode's default folder icon color
- **#f8f8f8 background** — VSCode's sidebar background
- **#e8e8e8 hover** — VSCode's hover background
- **PROJECTS section header** — uppercase, sticky, matches VSCode's panel headers
- **Arrow key navigation** — focus moves through project list with ↑↓, Enter selects
- **role="tree" + role="treeitem"** — ARIA tree pattern for screen readers
- **tabIndex roving** — only focused item has tabIndex=0

**Step 2: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 3: Manual verification**

- Arrow keys navigate the project list
- Tab focuses the tree, arrows move within
- Selection is full-width blue bar
- Chevron rotates when project is selected
- Stats panel appears below when project is selected

**Step 4: Commit**

```bash
git add src/components/Sidebar.tsx
git commit -m "feat: redesign Sidebar with VSCode Explorer patterns

- 22px row height, chevron expand hints, folder icons
- VSCode selection blue (#0060C0), folder gold (#C5A63C)
- Arrow key navigation with roving tabIndex
- ARIA tree role with treeitem semantics
- Sticky section header, compact density"
```

---

## Task 8: Human-Readable Session URLs

**Files:**
- Create: `src/lib/url-slugs.ts`
- Modify: `src/router.tsx`
- Modify: `src/hooks/use-projects.ts`
- Modify: `src/components/ProjectView.tsx`
- Modify: `src/components/SearchResults.tsx`
- Modify: `src/components/ConversationView.tsx`
- Modify: `src/components/Header.tsx`

**Problem:** Current URLs look like:
```
/session/-Users-TBGor-dev--vicky-ai-vic-ai-mvp/974d98a2-2a04-49dc-b37e-db042a9d1345
```

This is not human-readable. Users see encoded project dirs and raw UUIDs.

**Solution:** Generate human-readable slugs from session metadata:
```
/project/vic-ai-mvp/session/fix-login-bug-974d98a2
```

Structure: `/project/:projectSlug/session/:sessionSlug`

Where:
- `projectSlug` = `displayName` slugified (e.g., `vic-ai-mvp`)
- `sessionSlug` = first 6 words of `preview` slugified + first 8 chars of UUID for uniqueness (e.g., `fix-the-login-bug-we-discussed-974d98a2`)

The UUID suffix ensures uniqueness even when multiple sessions have similar previews.

**Step 1: Create URL slug utility**

Create `src/lib/url-slugs.ts`:

```typescript
/**
 * Generate a URL-friendly slug from text.
 * Strips non-alphanumeric chars, lowercases, truncates to maxWords.
 */
export function slugify(text: string, maxWords = 6): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')  // Remove non-alphanumeric except spaces/hyphens
    .trim()
    .split(/\s+/)                   // Split on whitespace
    .slice(0, maxWords)             // Take first N words
    .join('-')
    .replace(/-+/g, '-')           // Collapse multiple hyphens
    .replace(/^-|-$/g, '')         // Trim leading/trailing hyphens
    || 'session'                   // Fallback if empty
}

/**
 * Generate a human-readable session slug from preview text and session ID.
 * Format: "fix-the-login-bug-974d98a2" (preview slug + UUID prefix)
 */
export function sessionSlug(preview: string, sessionId: string): string {
  const textPart = slugify(preview, 6)
  const idPart = sessionId.slice(0, 8)  // First 8 chars of UUID
  return `${textPart}-${idPart}`
}

/**
 * Extract the UUID prefix from a session slug.
 * The last 8 characters after the final hyphen group.
 */
export function extractSessionIdPrefix(slug: string): string {
  // The UUID prefix is always the last 8 chars
  return slug.slice(-8)
}

/**
 * Generate a project slug from display name.
 */
export function projectSlug(displayName: string): string {
  return slugify(displayName, 10) || 'project'
}
```

**Step 2: Update router to use new URL structure**

Update `src/router.tsx`:

```tsx
import { createBrowserRouter } from 'react-router-dom'
import App from './App'
import { StatsDashboard } from './components/StatsDashboard'
import { ProjectView } from './components/ProjectView'
import { SearchResults } from './components/SearchResults'
import { ConversationView } from './components/ConversationView'

export const router = createBrowserRouter([
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <StatsDashboard /> },
      { path: 'project/:projectId', element: <ProjectView /> },
      { path: 'project/:projectId/session/:sessionSlug', element: <ConversationView /> },
      { path: 'search', element: <SearchResults /> },
      // Legacy routes — redirect to new URLs
      { path: 'session/:projectId/:sessionId', element: <ConversationView /> },
    ],
  },
])
```

**Step 3: Add slug helpers to use-projects.ts**

Add to `src/hooks/use-projects.ts`, after the interfaces:

```typescript
import { projectSlug, sessionSlug } from '../lib/url-slugs'

/**
 * Build the URL path for a project.
 */
export function projectUrl(project: ProjectInfo): string {
  return `/project/${encodeURIComponent(project.name)}`
}

/**
 * Build the human-readable URL path for a session.
 */
export function sessionUrl(session: SessionInfo): string {
  const slug = sessionSlug(session.preview, session.id)
  return `/project/${encodeURIComponent(session.project)}/session/${slug}`
}
```

**Step 4: Update ProjectView links**

In `src/components/ProjectView.tsx`:

```tsx
import { sessionUrl } from '../hooks/use-projects'

// Inside the session map:
<Link
  key={session.id}
  to={sessionUrl(session)}
  className="block group focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded-lg"
>
```

**Step 5: Update SearchResults links**

In `src/components/SearchResults.tsx`:

```tsx
import { sessionUrl } from '../hooks/use-projects'

// Inside the results map:
<Link
  key={session.id}
  to={sessionUrl(session)}
  className="block group focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded-lg"
>
```

**Step 6: Update ConversationView to resolve slug → session**

In `src/components/ConversationView.tsx`, update the param extraction:

```tsx
import { extractSessionIdPrefix } from '../lib/url-slugs'

export function ConversationView() {
  const { projectId, sessionId, sessionSlug: slug } = useParams()
  const navigate = useNavigate()
  const { projects } = useOutletContext<{ projects: ProjectInfo[] }>()

  const projectDir = projectId ? decodeURIComponent(projectId) : ''
  const project = projects.find(p => p.name === projectDir)
  const projectName = project?.displayName || projectDir

  // Resolve session ID from either direct ID (legacy) or slug (new)
  const resolvedSessionId = (() => {
    // Legacy route: /session/:projectId/:sessionId
    if (sessionId) return sessionId

    // New route: /project/:projectId/session/:sessionSlug
    if (slug && project) {
      const idPrefix = extractSessionIdPrefix(slug)
      const match = project.sessions.find(s => s.id.startsWith(idPrefix))
      return match?.id || null
    }
    return null
  })()

  const handleBack = () => navigate(`/project/${encodeURIComponent(projectDir)}`)
  const { data: session, isLoading, error } = useSession(projectDir, resolvedSessionId || '')
```

**Step 7: Update Header breadcrumbs**

In `src/components/Header.tsx`, update the breadcrumb logic to show human-readable names:

Replace the `getBreadcrumbs` function (lines 12-38):

```tsx
const getBreadcrumbs = () => {
  const crumbs: { label: string; path: string }[] = []

  if (location.pathname.startsWith('/project/')) {
    const projectDir = decodeURIComponent(params.projectId || '')
    const projectName = projectDir.split('/').pop() || 'Project'
    crumbs.push({
      label: projectName,
      path: `/project/${params.projectId}`
    })

    // Session breadcrumb — extract readable name from slug
    if (params.sessionSlug) {
      const readableName = params.sessionSlug
        .replace(/-[a-f0-9]{8}$/, '')  // Strip UUID suffix
        .replace(/-/g, ' ')            // Hyphens to spaces
      crumbs.push({
        label: readableName || 'Session',
        path: location.pathname
      })
    } else if (params.sessionId) {
      crumbs.push({
        label: 'Session',
        path: location.pathname
      })
    }
  }

  if (location.pathname === '/search') {
    crumbs.push({ label: 'Search', path: '/search' })
  }

  return crumbs
}
```

**Step 8: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 9: Manual verification**

- Navigate to a project, click a session
- URL should be: `/project/-Users-TBGor-dev--vicky-ai-vic-ai-mvp/session/fix-the-login-bug-974d98a2`
- Breadcrumb should show: `vic-ai-mvp > fix the login bug`
- Refreshing the URL should load the correct session
- Legacy URLs (`/session/:projectId/:sessionId`) should still work

**Step 10: Commit**

```bash
git add src/lib/url-slugs.ts src/router.tsx src/hooks/use-projects.ts src/components/ProjectView.tsx src/components/SearchResults.tsx src/components/ConversationView.tsx src/components/Header.tsx
git commit -m "feat: human-readable session URLs with slugified previews

URLs change from:
  /session/-Users-TBGor-dev--vicky-ai-vic-ai-mvp/974d98a2-...
to:
  /project/.../session/fix-the-login-bug-974d98a2

Legacy /session/ routes still work for backward compat.
Breadcrumbs show readable session names extracted from slugs."
```

---

## Task 9: Virtualize ProjectView Session List

**Files:**
- Modify: `src/components/ProjectView.tsx`

**Step 1: Add react-virtuoso to the session list**

`react-virtuoso` is already a dependency (used in `ConversationView.tsx`).

Replace `ProjectView.tsx`:

```tsx
import { useParams, useOutletContext, Link } from 'react-router-dom'
import { Virtuoso } from 'react-virtuoso'
import type { ProjectInfo } from '../hooks/use-projects'
import { sessionUrl } from '../hooks/use-projects'
import { SessionCard } from './SessionCard'

interface OutletContext {
  projects: ProjectInfo[]
}

export function ProjectView() {
  const { projectId } = useParams()
  const { projects } = useOutletContext<OutletContext>()

  const decodedProjectId = projectId ? decodeURIComponent(projectId) : null
  const project = projects.find(p => p.name === decodedProjectId)

  if (!project) {
    return (
      <div className="p-6">
        <p className="text-gray-500">Project not found</p>
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <div className="px-6 pt-6 pb-4">
        <div className="max-w-3xl mx-auto">
          <h1 className="text-xl font-semibold text-gray-900">
            {project.displayName}
          </h1>
          <p className="text-sm text-gray-500 mt-1">
            {project.sessions.length} sessions
          </p>
        </div>
      </div>

      <Virtuoso
        data={project.sessions}
        itemContent={(_index, session) => (
          <div className="max-w-3xl mx-auto px-6 pb-3">
            <Link
              to={sessionUrl(session)}
              className="block group focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none rounded-lg"
            >
              <SessionCard
                session={session}
                isSelected={false}
              />
            </Link>
          </div>
        )}
        components={{
          Footer: () => (
            <div className="max-w-3xl mx-auto px-6 pb-6">
              <p className="text-center text-sm text-gray-400 py-4">
                {project.sessions.length} sessions total
              </p>
            </div>
          ),
        }}
        increaseViewportBy={{ top: 200, bottom: 200 }}
        className="flex-1 overflow-auto"
      />
    </div>
  )
}
```

**Step 2: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 3: Commit**

```bash
git add src/components/ProjectView.tsx
git commit -m "perf: virtualize ProjectView session list with react-virtuoso

Prevents DOM bloat for projects with 80+ sessions.
Replaces the space-y-3 div with windowed rendering."
```

---

## Task 10: URL State for Filters

**Files:**
- Modify: `src/components/ProjectView.tsx`

This task adds URL query params for the sort order, so filter state survives page refresh and is shareable.

**Step 1: Add sort query param to ProjectView**

Add to the already-modified `ProjectView.tsx`:

```tsx
import { useParams, useOutletContext, Link, useSearchParams } from 'react-router-dom'

// Inside ProjectView():
const [searchParams, setSearchParams] = useSearchParams()
const sort = searchParams.get('sort') || 'recent'

// Sort sessions
const sortedSessions = [...project.sessions].sort((a, b) => {
  switch (sort) {
    case 'oldest':
      return a.modifiedAt - b.modifiedAt
    case 'messages':
      return b.messageCount - a.messageCount
    case 'recent':
    default:
      return b.modifiedAt - a.modifiedAt
  }
})

// Add sort controls in the header area, after the session count:
<div className="flex items-center gap-2 mt-2">
  {(['recent', 'oldest', 'messages'] as const).map(option => (
    <button
      key={option}
      onClick={() => setSearchParams(option === 'recent' ? {} : { sort: option })}
      className={cn(
        'text-xs px-2 py-0.5 rounded transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none',
        sort === option
          ? 'bg-gray-200 text-gray-900'
          : 'text-gray-400 hover:text-gray-600'
      )}
    >
      {option === 'recent' ? 'Recent' : option === 'oldest' ? 'Oldest' : 'Most messages'}
    </button>
  ))}
</div>
```

Pass `sortedSessions` to `Virtuoso` instead of `project.sessions`.

**Step 2: Verify**

- Click "Oldest" — URL becomes `/project/...?sort=oldest`
- Refresh page — sort persists
- Click "Recent" — URL removes `?sort` param (clean default)

**Step 3: Commit**

```bash
git add src/components/ProjectView.tsx
git commit -m "feat: URL-persisted sort order for session list

Sort state reflected in ?sort=recent|oldest|messages query param.
Default (recent) omits param for clean URLs."
```

---

## Task 11: Final Typecheck + Test

**Files:** None (verification only)

**Step 1: Run full typecheck**

Run: `bun run typecheck`
Expected: 0 errors.

**Step 2: Run Rust tests**

Run: `cargo test --workspace`
Expected: All pass (no backend changes in this plan).

**Step 3: Manual smoke test**

1. Load app → sidebar shows projects with VSCode styling
2. Tab → skip link appears → Enter → focus jumps to main content
3. Arrow keys navigate sidebar project list
4. Click project → sessions load with virtualized list
5. Click session → URL is human-readable slug
6. Refresh → same session loads correctly
7. Browser back → returns to project view
8. Sort buttons change URL query param
9. ⌘K → command palette opens → Esc closes
10. Resize window → sidebar and content adapt

**Step 4: Commit final pass**

```bash
git add -A
git commit -m "chore: final verification of UX polish changes"
```

---

## Task 12: Copy-to-Clipboard Button on Each Message

**Files:**
- Modify: `src/components/Message.tsx`

**Problem:** Users want to copy individual Claude or user messages to paste into Notion, Slack, or other tools. Currently there's no copy button — users have to manually select text.

**Solution:** Add a copy button in the message header (next to the timestamp) that copies the raw markdown content. On hover over the message, the button appears. Two copy modes:
- **Default click:** Copy as raw markdown (paste into markdown-aware apps like Notion preserves formatting)
- The browser's Clipboard API with `text/plain` format is sufficient — Notion and most apps auto-detect markdown

**Step 1: Add copy button to Message component**

In `src/components/Message.tsx`, add a `useState` for copy feedback and a copy button in the header:

Add imports at the top:
```tsx
import { useState, useCallback } from 'react'
import { Copy, Check } from 'lucide-react'
```

Add state and handler inside the `Message` component, before the return:
```tsx
const [copied, setCopied] = useState(false)

const handleCopy = useCallback(async () => {
  try {
    await navigator.clipboard.writeText(message.content)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  } catch {
    // Fallback for older browsers
    const textarea = document.createElement('textarea')
    textarea.value = message.content
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.appendChild(textarea)
    textarea.select()
    document.execCommand('copy')
    document.body.removeChild(textarea)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }
}, [message.content])
```

Update the root `<div>` to add `group` class for hover detection:
```tsx
<div
  className={cn(
    'group/msg p-4 rounded-lg',
    isUser ? 'bg-white border border-gray-200' : 'bg-gray-50'
  )}
>
```

Add the copy button in the header, after the timestamp (inside the `flex items-center justify-between` div):
```tsx
<div className="flex items-center justify-between gap-2">
  <span className="font-medium text-gray-900">
    {isUser ? 'You' : 'Claude'}
  </span>
  <div className="flex items-center gap-2">
    {/* Copy button — visible on hover */}
    <button
      onClick={handleCopy}
      aria-label={copied ? 'Copied!' : 'Copy message as markdown'}
      className={cn(
        'p-1 rounded transition-all focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none',
        copied
          ? 'text-green-500'
          : 'text-gray-300 opacity-0 group-hover/msg:opacity-100 hover:text-gray-500'
      )}
    >
      {copied ? (
        <Check className="w-3.5 h-3.5" />
      ) : (
        <Copy className="w-3.5 h-3.5" />
      )}
    </button>
    {time && (
      <span className="text-xs text-gray-400">{time}</span>
    )}
  </div>
</div>
```

**Step 2: Verify**

Run: `bun run typecheck`
Expected: No errors.

**Step 3: Manual verification**

- Hover over a message → copy icon appears (subtle gray)
- Click copy → icon changes to green checkmark for 2 seconds
- Paste into Notion → markdown renders correctly (headers, lists, code blocks preserved)
- Paste into plain text editor → raw markdown appears
- Keyboard: Tab to copy button, Enter to copy

**Step 4: Commit**

```bash
git add src/components/Message.tsx
git commit -m "feat: add copy-to-clipboard button on each conversation message

Hover to reveal copy icon, click to copy raw markdown.
Green checkmark feedback for 2 seconds after copy.
Fallback to document.execCommand for older browsers.
Content copies as markdown — Notion auto-renders formatting."
```

---

## Summary of All Changes

| Task | Scope | Files |
|------|-------|-------|
| 1. Reduced motion + skip link | a11y | `index.css`, `App.tsx` |
| 2. Header aria-labels | a11y | `Header.tsx` |
| 3. SessionCard nested interactive | a11y | `SessionCard.tsx`, `ProjectView.tsx`, `SearchResults.tsx` |
| 4. Sidebar focus-visible | a11y | `Sidebar.tsx` |
| 5. Dashboard aria-labels | a11y | `StatsDashboard.tsx` |
| 6. ConversationView labels | a11y | `ConversationView.tsx` |
| 7. VSCode-style Sidebar | UX | `Sidebar.tsx` (major rewrite) |
| 8. Human-readable URLs | UX | `url-slugs.ts` (new), `router.tsx`, `use-projects.ts`, `ProjectView.tsx`, `SearchResults.tsx`, `ConversationView.tsx`, `Header.tsx` |
| 9. Virtualized session list | perf | `ProjectView.tsx` |
| 10. URL state for filters | UX | `ProjectView.tsx` |
| 11. Final verification | QA | — |
| 12. Copy-to-clipboard on messages | UX | `Message.tsx` |

Total: **12 tasks, 13 files modified, 1 file created, 12 commits.**
