# Session Discovery UX - Full Rewrite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild Claude View's UX to fully implement the design spec with connected navigation, live search, rich stats, and proper routing.

**Architecture:** Single-page app with React Router for clean URL routing. Zustand store for global state (search, navigation). Command palette with live autocomplete powered by the same search engine. Sidebar shows contextual stats based on current view. All clickable elements are search entry points.

**Tech Stack:** React 19, TypeScript, TanStack Query, Tailwind CSS, Zustand (new), React Router (new)

**Design Spec Reference:** `docs/plans/2026-01-26-session-discovery-ux-design.md`

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                           App Shell                                  │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ Header: Logo | Breadcrumbs | Search Button (⌘K) | Settings   │   │
│  └──────────────────────────────────────────────────────────────┘   │
│  ┌─────────────┬────────────────────────────────────────────────┐   │
│  │   Sidebar   │              Main Content                       │   │
│  │ ┌─────────┐ │  Routes:                                        │   │
│  │ │Projects │ │  /           → StatsDashboard (global)          │   │
│  │ │  List   │ │  /project/:id → ProjectView (sessions list)     │   │
│  │ └─────────┘ │  /session/:id → ConversationView                │   │
│  │ ┌─────────┐ │  /search?q=   → SearchResults                   │   │
│  │ │ Context │ │                                                  │   │
│  │ │  Stats  │ │                                                  │   │
│  │ └─────────┘ │                                                  │   │
│  └─────────────┴────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ StatusBar: X projects · Y sessions · Last activity: ...       │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘

CommandPalette (⌘K) - Floating overlay with live results
```

---

## Task 1: Install Dependencies and Setup Router

**Files:**
- Modify: `package.json`
- Modify: `src/main.tsx`
- Create: `src/router.tsx`

### Step 1: Install dependencies

Run:
```bash
npm install zustand react-router-dom@7
```

### Step 2: Create router configuration

Create `src/router.tsx`:

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
      { path: 'search', element: <SearchResults /> },
      { path: 'session/:projectId/:sessionId', element: <ConversationView /> },
    ],
  },
])
```

### Step 3: Update main.tsx to use router

Modify `src/main.tsx`:

```tsx
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { router } from './router'
import './index.css'

const queryClient = new QueryClient()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>
)
```

### Step 4: Verify app still loads

Run: `npm run dev`

Expected: App loads (may have errors since components aren't updated yet)

### Step 5: Commit

```bash
git add package.json package-lock.json src/main.tsx src/router.tsx
git commit -m "feat: add react-router and zustand for navigation

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 2: Create Global Store with Zustand

**Files:**
- Create: `src/store/app-store.ts`

### Step 1: Create the store

Create `src/store/app-store.ts`:

```typescript
import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface AppState {
  // Search state
  searchQuery: string
  recentSearches: string[]
  isCommandPaletteOpen: boolean

  // Actions
  setSearchQuery: (query: string) => void
  addRecentSearch: (query: string) => void
  clearSearch: () => void
  openCommandPalette: () => void
  closeCommandPalette: () => void
  toggleCommandPalette: () => void
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      searchQuery: '',
      recentSearches: [],
      isCommandPaletteOpen: false,

      setSearchQuery: (query) => set({ searchQuery: query }),

      addRecentSearch: (query) => set((state) => ({
        recentSearches: [
          query,
          ...state.recentSearches.filter(s => s !== query)
        ].slice(0, 10)
      })),

      clearSearch: () => set({ searchQuery: '' }),

      openCommandPalette: () => set({ isCommandPaletteOpen: true }),
      closeCommandPalette: () => set({ isCommandPaletteOpen: false }),
      toggleCommandPalette: () => set((state) => ({
        isCommandPaletteOpen: !state.isCommandPaletteOpen
      })),
    }),
    {
      name: 'claude-view-storage',
      partialize: (state) => ({ recentSearches: state.recentSearches }),
    }
  )
)
```

### Step 2: Commit

```bash
git add src/store/app-store.ts
git commit -m "feat: add zustand store for global state management

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 3: Rebuild App Shell with Proper Layout

**Files:**
- Rewrite: `src/App.tsx`

### Step 1: Rewrite App.tsx as a shell with Outlet

```tsx
import { useEffect } from 'react'
import { Outlet, useNavigate, useLocation } from 'react-router-dom'
import { Loader2, FolderOpen } from 'lucide-react'
import { useProjects } from './hooks/use-projects'
import { useAppStore } from './store/app-store'
import { Header } from './components/Header'
import { Sidebar } from './components/Sidebar'
import { StatusBar } from './components/StatusBar'
import { CommandPalette } from './components/CommandPalette'

export default function App() {
  const { data: projects, isLoading, error } = useProjects()
  const { isCommandPaletteOpen, openCommandPalette, closeCommandPalette } = useAppStore()
  const navigate = useNavigate()
  const location = useLocation()

  // Global keyboard shortcut: ⌘K
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        openCommandPalette()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [openCommandPalette])

  // Loading state
  if (isLoading) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="flex items-center gap-3 text-gray-600">
          <Loader2 className="w-5 h-5 animate-spin" />
          <span>Loading sessions...</span>
        </div>
      </div>
    )
  }

  // Error state
  if (error) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-center text-red-600">
          <p className="font-medium">Failed to load projects</p>
          <p className="text-sm mt-1">{error.message}</p>
        </div>
      </div>
    )
  }

  // Empty state
  if (!projects || projects.length === 0) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-center text-gray-500">
          <FolderOpen className="w-12 h-12 mx-auto mb-4 text-gray-300" />
          <p className="font-medium">No Claude Code sessions found</p>
          <p className="text-sm mt-1">Start using Claude Code to see your sessions here</p>
        </div>
      </div>
    )
  }

  return (
    <div className="h-screen flex flex-col">
      <Header />

      <div className="flex-1 flex overflow-hidden">
        <Sidebar projects={projects} />

        <main className="flex-1 overflow-y-auto bg-gray-50">
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
}
```

### Step 2: Commit

```bash
git add src/App.tsx
git commit -m "refactor: rebuild App as router shell with Outlet

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 4: Create Header Component with Breadcrumbs

**Files:**
- Create: `src/components/Header.tsx`

### Step 1: Create Header component

Create `src/components/Header.tsx`:

```tsx
import { Link, useLocation, useParams } from 'react-router-dom'
import { Home, Search, HelpCircle, Settings, ChevronRight } from 'lucide-react'
import { useAppStore } from '../store/app-store'

export function Header() {
  const location = useLocation()
  const params = useParams()
  const { openCommandPalette, searchQuery } = useAppStore()

  // Build breadcrumbs based on current route
  const getBreadcrumbs = () => {
    const crumbs: { label: string; path: string }[] = []

    if (location.pathname.startsWith('/project/')) {
      crumbs.push({
        label: decodeURIComponent(params.projectId || '').split('/').pop() || 'Project',
        path: location.pathname
      })
    }

    if (location.pathname.startsWith('/session/')) {
      crumbs.push({
        label: decodeURIComponent(params.projectId || '').split('/').pop() || 'Project',
        path: `/project/${params.projectId}`
      })
      crumbs.push({
        label: 'Session',
        path: location.pathname
      })
    }

    if (location.pathname === '/search') {
      crumbs.push({ label: 'Search', path: '/search' })
    }

    return crumbs
  }

  const breadcrumbs = getBreadcrumbs()

  return (
    <header className="h-12 bg-white border-b border-gray-200 flex items-center justify-between px-4">
      {/* Left: Logo + Breadcrumbs */}
      <div className="flex items-center gap-2">
        <Link
          to="/"
          className="flex items-center gap-2 hover:opacity-70 transition-opacity"
        >
          <Home className="w-4 h-4 text-gray-400" />
          <h1 className="text-lg font-semibold text-gray-900">Claude View</h1>
        </Link>

        {breadcrumbs.map((crumb, i) => (
          <div key={crumb.path} className="flex items-center gap-2">
            <ChevronRight className="w-4 h-4 text-gray-300" />
            {i === breadcrumbs.length - 1 ? (
              <span className="text-sm text-gray-600 truncate max-w-[200px]">
                {crumb.label}
              </span>
            ) : (
              <Link
                to={crumb.path}
                className="text-sm text-gray-600 hover:text-gray-900 truncate max-w-[200px]"
              >
                {crumb.label}
              </Link>
            )}
          </div>
        ))}
      </div>

      {/* Right: Search + Actions */}
      <div className="flex items-center gap-2">
        <button
          onClick={openCommandPalette}
          className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-500 hover:text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
        >
          <Search className="w-4 h-4" />
          <span className="hidden sm:inline">Search</span>
          <kbd className="hidden sm:inline text-xs text-gray-400 bg-white px-1.5 py-0.5 rounded border border-gray-200">
            ⌘K
          </kbd>
        </button>

        <button className="p-2 text-gray-400 hover:text-gray-600 transition-colors">
          <HelpCircle className="w-5 h-5" />
        </button>

        <button className="p-2 text-gray-400 hover:text-gray-600 transition-colors">
          <Settings className="w-5 h-5" />
        </button>
      </div>
    </header>
  )
}
```

### Step 2: Commit

```bash
git add src/components/Header.tsx
git commit -m "feat: add Header component with breadcrumb navigation

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 5: Rebuild Sidebar with Full Per-Project Stats

**Files:**
- Rewrite: `src/components/Sidebar.tsx` (extract from App.tsx)

### Step 1: Create standalone Sidebar component

Create `src/components/Sidebar.tsx`:

```tsx
import { useMemo } from 'react'
import { Link, useParams, useLocation, useNavigate } from 'react-router-dom'
import { FolderOpen, Pencil, Eye, Terminal } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface SidebarProps {
  projects: ProjectInfo[]
}

export function Sidebar({ projects }: SidebarProps) {
  const params = useParams()
  const location = useLocation()
  const navigate = useNavigate()

  // Determine selected project from URL
  const selectedProjectId = params.projectId ? decodeURIComponent(params.projectId) : null
  const selectedProject = projects.find(p => p.name === selectedProjectId)

  // Calculate per-project stats when a project is selected
  const projectStats = useMemo(() => {
    if (!selectedProject) return null

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

    const maxTools = Math.max(totalEdits, totalReads, totalBash, 1)

    return {
      topSkills: Array.from(skillCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 5),
      topFiles: Array.from(fileCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 5),
      tools: {
        edits: totalEdits,
        reads: totalReads,
        bash: totalBash,
        maxTools,
      },
    }
  }, [selectedProject])

  const handleSkillClick = (skill: string) => {
    const query = selectedProject
      ? `project:${selectedProject.displayName} skill:${skill.replace('/', '')}`
      : `skill:${skill.replace('/', '')}`
    navigate(`/search?q=${encodeURIComponent(query)}`)
  }

  const handleFileClick = (file: string) => {
    const query = selectedProject
      ? `project:${selectedProject.displayName} path:${file}`
      : `path:${file}`
    navigate(`/search?q=${encodeURIComponent(query)}`)
  }

  return (
    <aside className="w-72 bg-gray-50/80 border-r border-gray-200 flex flex-col overflow-hidden">
      {/* Project List */}
      <div className="flex-1 overflow-y-auto py-2">
        {projects.map((project) => {
          const isSelected = selectedProjectId === project.name
          const parentPath = project.name.split('/').slice(0, -1).join('/')
          const hasActive = project.activeCount > 0

          return (
            <Link
              key={project.name}
              to={`/project/${encodeURIComponent(project.name)}`}
              className={cn(
                'w-full flex items-start gap-2.5 px-3 py-2 text-left transition-colors',
                isSelected
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-700 hover:bg-gray-200/70'
              )}
            >
              <FolderOpen className={cn(
                'w-4 h-4 flex-shrink-0 mt-0.5',
                isSelected ? 'text-white' : 'text-blue-400'
              )} />
              <div className="flex-1 min-w-0">
                <span className="truncate font-medium text-[13px] block">
                  {project.displayName}
                </span>
                {parentPath && (
                  <p className={cn(
                    'text-[11px] truncate mt-0.5',
                    isSelected ? 'text-blue-100' : 'text-gray-400'
                  )}>
                    {parentPath}
                  </p>
                )}
              </div>
              <div className="flex items-center gap-1.5 flex-shrink-0">
                {hasActive && (
                  <span className="flex items-center gap-1">
                    <span className={cn(
                      'w-1.5 h-1.5 rounded-full animate-pulse',
                      isSelected ? 'bg-green-300' : 'bg-green-500'
                    )} />
                    <span className={cn(
                      'text-xs tabular-nums',
                      isSelected ? 'text-green-200' : 'text-green-600'
                    )}>
                      {project.activeCount}
                    </span>
                  </span>
                )}
                <span className={cn(
                  'text-xs tabular-nums',
                  isSelected ? 'text-blue-100' : 'text-gray-400'
                )}>
                  {project.sessions.length}
                </span>
              </div>
            </Link>
          )
        })}
      </div>

      {/* Per-Project Stats Panel - Shows when project selected */}
      {projectStats && selectedProject && (
        <div className="border-t border-gray-200 p-3 space-y-4 bg-white">
          {/* Project Header */}
          <div>
            <h3 className="font-medium text-sm text-gray-900 truncate">
              {selectedProject.displayName}
            </h3>
            <p className="text-[11px] text-gray-400 truncate">
              {selectedProject.path}
            </p>
            <p className="text-xs text-gray-500 mt-1">
              {selectedProject.activeCount > 0 && (
                <span className="text-green-600">
                  ●{selectedProject.activeCount} active ·
                </span>
              )}
              {selectedProject.sessions.length} sessions
            </p>
          </div>

          {/* Skills */}
          {projectStats.topSkills.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-2">
                Skills
              </p>
              <div className="flex flex-wrap gap-1">
                {projectStats.topSkills.map(([skill, count]) => (
                  <button
                    key={skill}
                    onClick={() => handleSkillClick(skill)}
                    className="px-1.5 py-0.5 text-[11px] font-mono bg-gray-100 hover:bg-blue-500 hover:text-white text-gray-600 rounded transition-colors"
                  >
                    {skill} <span className="opacity-60">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Top Files */}
          {projectStats.topFiles.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-2">
                Top Files
              </p>
              <div className="space-y-0.5">
                {projectStats.topFiles.map(([file, count]) => (
                  <button
                    key={file}
                    onClick={() => handleFileClick(file)}
                    className="w-full flex items-center justify-between px-1.5 py-1 text-[11px] hover:bg-gray-100 rounded transition-colors text-left"
                  >
                    <span className="truncate text-gray-600 font-mono">{file}</span>
                    <span className="text-gray-400 tabular-nums ml-2">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Tool Usage Bars */}
          <div>
            <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-2">
              Tools
            </p>
            <div className="space-y-2">
              {[
                { label: 'Edit', value: projectStats.tools.edits, icon: Pencil, color: 'bg-blue-400' },
                { label: 'Read', value: projectStats.tools.reads, icon: Eye, color: 'bg-green-400' },
                { label: 'Bash', value: projectStats.tools.bash, icon: Terminal, color: 'bg-amber-400' },
              ].map(({ label, value, icon: Icon, color }) => (
                <div key={label} className="flex items-center gap-2">
                  <Icon className="w-3 h-3 text-gray-400" />
                  <span className="text-[11px] text-gray-600 w-8">{label}</span>
                  <div className="flex-1 h-1.5 bg-gray-100 rounded-full overflow-hidden">
                    <div
                      className={cn('h-full rounded-full transition-all', color)}
                      style={{ width: `${(value / projectStats.tools.maxTools) * 100}%` }}
                    />
                  </div>
                  <span className="text-[11px] text-gray-400 tabular-nums w-8 text-right">
                    {value}
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </aside>
  )
}
```

### Step 2: Commit

```bash
git add src/components/Sidebar.tsx
git commit -m "feat: rebuild Sidebar with full per-project stats panel

- Skills badges with click-to-search
- Top files with counts
- Tool usage bar charts (Edit/Read/Bash)
- Project header with active count

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 6: Rebuild StatsDashboard as Route Component

**Files:**
- Rewrite: `src/components/StatsDashboard.tsx`

### Step 1: Rewrite as route component with heatmap

```tsx
import { useMemo } from 'react'
import { useOutletContext, useNavigate, Link } from 'react-router-dom'
import { BarChart3, Zap, FolderOpen, Calendar, Pencil, Eye, Terminal } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface OutletContext {
  projects: ProjectInfo[]
}

export function StatsDashboard() {
  const { projects } = useOutletContext<OutletContext>()
  const navigate = useNavigate()

  const stats = useMemo(() => {
    const allSessions = projects.flatMap(p => p.sessions)

    // Aggregate skills
    const skillCounts = new Map<string, number>()
    let totalEdits = 0, totalReads = 0, totalBash = 0

    for (const session of allSessions) {
      for (const skill of session.skillsUsed ?? []) {
        skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
      }
      const tc = session.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
      totalEdits += tc.edit + tc.write
      totalReads += tc.read
      totalBash += tc.bash
    }

    const topSkills = Array.from(skillCounts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
    const maxSkillCount = topSkills[0]?.[1] || 1

    // Project stats
    const projectStats = projects
      .map(p => ({
        name: p.displayName,
        fullName: p.name,
        sessions: p.sessions.length,
        activeCount: p.activeCount,
      }))
      .sort((a, b) => b.sessions - a.sessions)
      .slice(0, 5)
    const maxProjectSessions = projectStats[0]?.sessions || 1

    // Date range
    const dates = allSessions.map(s => new Date(s.modifiedAt))
    const earliest = dates.reduce((min, d) => d < min ? d : min, new Date())

    // Activity heatmap (last 30 days)
    const heatmap = generateHeatmap(allSessions)

    return {
      totalSessions: allSessions.length,
      totalProjects: projects.length,
      since: earliest.toLocaleDateString('en-US', { month: 'short', year: 'numeric' }),
      topSkills,
      maxSkillCount,
      projectStats,
      maxProjectSessions,
      tools: { edits: totalEdits, reads: totalReads, bash: totalBash },
      heatmap,
    }
  }, [projects])

  const handleSkillClick = (skill: string) => {
    navigate(`/search?q=${encodeURIComponent(`skill:${skill.replace('/', '')}`)}`)
  }

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      {/* Header Card */}
      <div className="bg-white rounded-xl border border-gray-200 p-6">
        <div className="flex items-center gap-2 mb-4">
          <BarChart3 className="w-5 h-5 text-[#7c9885]" />
          <h1 className="text-xl font-semibold text-gray-900">Your Claude Code Usage</h1>
        </div>

        <div className="flex items-center gap-6 text-sm text-gray-600">
          <div>
            <span className="text-2xl font-bold text-gray-900 tabular-nums">{stats.totalSessions}</span>
            <span className="ml-1">sessions</span>
          </div>
          <div className="w-px h-8 bg-gray-200" />
          <div>
            <span className="text-2xl font-bold text-gray-900 tabular-nums">{stats.totalProjects}</span>
            <span className="ml-1">projects</span>
          </div>
          <div className="w-px h-8 bg-gray-200" />
          <div className="text-gray-500">
            since {stats.since}
          </div>
        </div>
      </div>

      <div className="grid md:grid-cols-2 gap-6">
        {/* Top Skills */}
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5">
            <Zap className="w-4 h-4" />
            Top Skills
          </h2>
          <div className="space-y-3">
            {stats.topSkills.map(([skill, count]) => (
              <button
                key={skill}
                onClick={() => handleSkillClick(skill)}
                className="w-full group text-left"
              >
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="font-mono text-gray-700 group-hover:text-blue-600 transition-colors">
                    {skill}
                  </span>
                  <span className="tabular-nums text-gray-400">{count}</span>
                </div>
                <div className="h-2 bg-gray-100 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-[#7c9885] group-hover:bg-blue-500 transition-colors rounded-full"
                    style={{ width: `${(count / stats.maxSkillCount) * 100}%` }}
                  />
                </div>
              </button>
            ))}
            {stats.topSkills.length === 0 && (
              <p className="text-sm text-gray-400 italic">No skills used yet</p>
            )}
          </div>
        </div>

        {/* Most Active Projects */}
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5">
            <FolderOpen className="w-4 h-4" />
            Most Active Projects
          </h2>
          <div className="space-y-3">
            {stats.projectStats.map((project) => (
              <Link
                key={project.fullName}
                to={`/project/${encodeURIComponent(project.fullName)}`}
                className="w-full group block"
              >
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="flex items-center gap-2">
                    <span className="text-gray-700 group-hover:text-blue-600 transition-colors">
                      {project.name}
                    </span>
                    {project.activeCount > 0 && (
                      <span className="flex items-center gap-1 text-xs text-green-600">
                        <span className="w-1.5 h-1.5 bg-green-500 rounded-full animate-pulse" />
                        {project.activeCount}
                      </span>
                    )}
                  </span>
                  <span className="tabular-nums text-gray-400">{project.sessions}</span>
                </div>
                <div className="h-2 bg-gray-100 rounded-full overflow-hidden">
                  <div
                    className={cn(
                      "h-full rounded-full transition-colors",
                      project.activeCount > 0
                        ? "bg-green-400 group-hover:bg-green-500"
                        : "bg-gray-300 group-hover:bg-blue-500"
                    )}
                    style={{ width: `${(project.sessions / stats.maxProjectSessions) * 100}%` }}
                  />
                </div>
              </Link>
            ))}
          </div>
        </div>
      </div>

      {/* Activity Heatmap */}
      <div className="bg-white rounded-xl border border-gray-200 p-6">
        <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5">
          <Calendar className="w-4 h-4" />
          Activity (Last 30 Days)
        </h2>
        <ActivityHeatmap data={stats.heatmap} navigate={navigate} />
      </div>

      {/* Global Tool Usage */}
      <div className="bg-white rounded-xl border border-gray-200 p-6">
        <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4">
          Tool Usage
        </h2>
        <div className="grid grid-cols-3 gap-4">
          {[
            { label: 'Edits', value: stats.tools.edits, icon: Pencil, color: 'text-blue-500' },
            { label: 'Reads', value: stats.tools.reads, icon: Eye, color: 'text-green-500' },
            { label: 'Bash', value: stats.tools.bash, icon: Terminal, color: 'text-amber-500' },
          ].map(({ label, value, icon: Icon, color }) => (
            <div key={label} className="text-center p-4 bg-gray-50 rounded-lg">
              <Icon className={cn('w-6 h-6 mx-auto mb-2', color)} />
              <p className="text-2xl font-bold text-gray-900 tabular-nums">{value}</p>
              <p className="text-xs text-gray-500">{label}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

// Helper: Generate heatmap data for last 30 days
function generateHeatmap(sessions: { modifiedAt: string }[]) {
  const days: { date: Date; count: number }[] = []
  const now = new Date()

  for (let i = 29; i >= 0; i--) {
    const date = new Date(now)
    date.setDate(date.getDate() - i)
    date.setHours(0, 0, 0, 0)
    days.push({ date, count: 0 })
  }

  for (const session of sessions) {
    const sessionDate = new Date(session.modifiedAt)
    sessionDate.setHours(0, 0, 0, 0)
    const dayEntry = days.find(d => d.date.getTime() === sessionDate.getTime())
    if (dayEntry) dayEntry.count++
  }

  return days
}

// Activity Heatmap Component
function ActivityHeatmap({
  data,
  navigate
}: {
  data: { date: Date; count: number }[]
  navigate: (path: string) => void
}) {
  const maxCount = Math.max(...data.map(d => d.count), 1)

  const getColor = (count: number) => {
    if (count === 0) return 'bg-gray-100'
    const intensity = count / maxCount
    if (intensity > 0.66) return 'bg-green-500'
    if (intensity > 0.33) return 'bg-green-300'
    return 'bg-green-200'
  }

  const handleDayClick = (date: Date) => {
    const dateStr = date.toISOString().split('T')[0]
    const nextDay = new Date(date)
    nextDay.setDate(nextDay.getDate() + 1)
    const nextDateStr = nextDay.toISOString().split('T')[0]
    navigate(`/search?q=${encodeURIComponent(`after:${dateStr} before:${nextDateStr}`)}`)
  }

  // Group by week
  const weeks: { date: Date; count: number }[][] = []
  let currentWeek: { date: Date; count: number }[] = []

  for (const day of data) {
    if (currentWeek.length === 7) {
      weeks.push(currentWeek)
      currentWeek = []
    }
    currentWeek.push(day)
  }
  if (currentWeek.length > 0) weeks.push(currentWeek)

  return (
    <div className="flex gap-1">
      {weeks.map((week, wi) => (
        <div key={wi} className="flex flex-col gap-1">
          {week.map((day) => (
            <button
              key={day.date.toISOString()}
              onClick={() => handleDayClick(day.date)}
              className={cn(
                'w-3 h-3 rounded-sm transition-colors hover:ring-2 hover:ring-blue-400',
                getColor(day.count)
              )}
              title={`${day.date.toLocaleDateString()}: ${day.count} sessions`}
            />
          ))}
        </div>
      ))}
      <div className="ml-2 flex items-center gap-2 text-xs text-gray-400">
        <span>Less</span>
        <div className="flex gap-0.5">
          <div className="w-3 h-3 rounded-sm bg-gray-100" />
          <div className="w-3 h-3 rounded-sm bg-green-200" />
          <div className="w-3 h-3 rounded-sm bg-green-300" />
          <div className="w-3 h-3 rounded-sm bg-green-500" />
        </div>
        <span>More</span>
      </div>
    </div>
  )
}
```

### Step 2: Commit

```bash
git add src/components/StatsDashboard.tsx
git commit -m "feat: rebuild StatsDashboard with heatmap and tool usage

- Activity heatmap for last 30 days
- Clickable days trigger date search
- Global tool usage cards
- Two-column layout for skills/projects

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 7: Create ProjectView Route Component

**Files:**
- Create: `src/components/ProjectView.tsx`

### Step 1: Create ProjectView component

Create `src/components/ProjectView.tsx`:

```tsx
import { useParams, useOutletContext, Link } from 'react-router-dom'
import type { ProjectInfo } from '../hooks/use-projects'
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

  const activeSessionId = project.sessions[0]?.id

  return (
    <div className="p-6 max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-xl font-semibold text-gray-900">
          {project.displayName}
        </h1>
        <p className="text-sm text-gray-500 mt-1">
          {project.sessions.length} sessions
          {project.activeCount > 0 && (
            <span className="text-green-600 ml-2">
              · {project.activeCount} active
            </span>
          )}
        </p>
      </div>

      <div className="space-y-3">
        {project.sessions.map((session) => (
          <Link
            key={session.id}
            to={`/session/${encodeURIComponent(project.name)}/${session.id}`}
          >
            <SessionCard
              session={session}
              isSelected={false}
              isActive={session.id === activeSessionId}
              onClick={() => {}}
            />
          </Link>
        ))}
      </div>

      {project.sessions.length >= 20 && (
        <button className="w-full mt-4 py-3 text-sm text-gray-500 hover:text-gray-700 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors">
          Load more sessions...
        </button>
      )}
    </div>
  )
}
```

### Step 2: Commit

```bash
git add src/components/ProjectView.tsx
git commit -m "feat: add ProjectView route component

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 8: Create SearchResults Route with URL Query Params

**Files:**
- Rewrite: `src/components/SearchResults.tsx`

### Step 1: Rewrite as route component

```tsx
import { useMemo, useEffect } from 'react'
import { useSearchParams, useOutletContext, Link, useNavigate } from 'react-router-dom'
import { X } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { parseQuery, filterSessions } from '../lib/search'
import { SessionCard } from './SessionCard'
import { useAppStore } from '../store/app-store'

interface OutletContext {
  projects: ProjectInfo[]
}

export function SearchResults() {
  const [searchParams] = useSearchParams()
  const { projects } = useOutletContext<OutletContext>()
  const navigate = useNavigate()
  const { addRecentSearch } = useAppStore()

  const query = searchParams.get('q') || ''

  // Add to recent searches when query changes
  useEffect(() => {
    if (query) {
      addRecentSearch(query)
    }
  }, [query, addRecentSearch])

  const results = useMemo(() => {
    if (!query) return []
    const allSessions = projects.flatMap(p => p.sessions)
    const parsed = parseQuery(query)
    return filterSessions(allSessions, projects, parsed)
  }, [projects, query])

  const handleClearSearch = () => {
    navigate('/')
  }

  // Find project for each session (for linking)
  const getSessionProject = (sessionId: string) => {
    return projects.find(p => p.sessions.some(s => s.id === sessionId))
  }

  return (
    <div className="p-6 max-w-3xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-xl font-semibold text-gray-900">Search Results</h1>
          <p className="text-sm text-gray-500 mt-1">
            {results.length} sessions matching "<span className="font-mono">{query}</span>"
          </p>
        </div>
        <button
          onClick={handleClearSearch}
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors"
        >
          <X className="w-4 h-4" />
          Clear
        </button>
      </div>

      {results.length > 0 ? (
        <div className="space-y-3">
          {results.map((session) => {
            const project = getSessionProject(session.id)
            return (
              <Link
                key={session.id}
                to={`/session/${encodeURIComponent(project?.name || session.project)}/${session.id}`}
              >
                <SessionCard
                  session={session}
                  isSelected={false}
                  isActive={false}
                  onClick={() => {}}
                />
              </Link>
            )
          })}
        </div>
      ) : (
        <div className="text-center py-12 text-gray-500">
          <p>No sessions match your search.</p>
          <p className="text-sm mt-1">Try different keywords or filters.</p>
        </div>
      )}
    </div>
  )
}
```

### Step 2: Commit

```bash
git add src/components/SearchResults.tsx
git commit -m "feat: rebuild SearchResults as route with URL query params

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 9: Rebuild CommandPalette with Live Autocomplete

**Files:**
- Rewrite: `src/components/CommandPalette.tsx`

### Step 1: Rewrite with live results

```tsx
import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { Search, X, Zap, FolderOpen, FileText, Clock } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { parseQuery, filterSessions } from '../lib/search'
import { useAppStore } from '../store/app-store'
import { cn } from '../lib/utils'

interface CommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  projects: ProjectInfo[]
}

type SuggestionType = 'project' | 'skill' | 'file' | 'recent' | 'session'

interface Suggestion {
  type: SuggestionType
  label: string
  query: string
  count?: number
}

export function CommandPalette({ isOpen, onClose, projects }: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const navigate = useNavigate()
  const { recentSearches, addRecentSearch } = useAppStore()

  // Reset when opened
  useEffect(() => {
    if (isOpen) {
      inputRef.current?.focus()
      setQuery('')
      setSelectedIndex(0)
    }
  }, [isOpen])

  // Generate suggestions based on query
  const suggestions = useMemo((): Suggestion[] => {
    const results: Suggestion[] = []
    const q = query.toLowerCase().trim()

    if (!q) {
      // Show recent searches and quick filters
      for (const recent of recentSearches.slice(0, 3)) {
        results.push({ type: 'recent', label: recent, query: recent })
      }
      return results
    }

    // Autocomplete project names
    if (q.startsWith('project:') || !q.includes(':')) {
      const searchTerm = q.startsWith('project:') ? q.slice(8) : q
      for (const project of projects) {
        if (project.displayName.toLowerCase().includes(searchTerm)) {
          results.push({
            type: 'project',
            label: project.displayName,
            query: `project:${project.displayName}`,
            count: project.sessions.length,
          })
        }
        if (results.length >= 3) break
      }
    }

    // Autocomplete skills
    if (q.startsWith('skill:') || !q.includes(':')) {
      const searchTerm = q.startsWith('skill:') ? q.slice(6) : q
      const skillCounts = new Map<string, number>()
      for (const project of projects) {
        for (const session of project.sessions) {
          for (const skill of session.skillsUsed ?? []) {
            if (skill.toLowerCase().includes(searchTerm)) {
              skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
            }
          }
        }
      }
      const topSkills = Array.from(skillCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3)
      for (const [skill, count] of topSkills) {
        results.push({
          type: 'skill',
          label: skill,
          query: `skill:${skill.replace('/', '')}`,
          count,
        })
      }
    }

    // Show matching sessions (preview)
    if (q.length >= 2) {
      const allSessions = projects.flatMap(p => p.sessions)
      const parsed = parseQuery(q)
      const matches = filterSessions(allSessions, projects, parsed).slice(0, 3)
      for (const session of matches) {
        results.push({
          type: 'session',
          label: session.preview.slice(0, 60) + (session.preview.length > 60 ? '...' : ''),
          query: q, // Full search
        })
      }
    }

    return results.slice(0, 8)
  }, [query, projects, recentSearches])

  // Handle keyboard navigation
  useEffect(() => {
    if (!isOpen) return

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      } else if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelectedIndex(i => Math.min(i + 1, suggestions.length - 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelectedIndex(i => Math.max(i - 1, 0))
      } else if (e.key === 'Enter') {
        e.preventDefault()
        if (suggestions[selectedIndex]) {
          handleSelect(suggestions[selectedIndex].query)
        } else if (query.trim()) {
          handleSelect(query.trim())
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose, suggestions, selectedIndex, query])

  const handleSelect = useCallback((searchQuery: string) => {
    addRecentSearch(searchQuery)
    onClose()
    navigate(`/search?q=${encodeURIComponent(searchQuery)}`)
  }, [addRecentSearch, onClose, navigate])

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    if (query.trim()) {
      handleSelect(query.trim())
    }
  }, [query, handleSelect])

  const insertFilter = useCallback((filter: string) => {
    setQuery(prev => {
      const trimmed = prev.trim()
      return trimmed ? `${trimmed} ${filter}` : filter
    })
    inputRef.current?.focus()
  }, [])

  const getIcon = (type: SuggestionType) => {
    switch (type) {
      case 'project': return FolderOpen
      case 'skill': return Zap
      case 'file': return FileText
      case 'recent': return Clock
      case 'session': return Search
    }
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[12vh]">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-xl bg-[#111113] rounded-xl shadow-2xl border border-[#2a2a2e] overflow-hidden">
        {/* Search input */}
        <form onSubmit={handleSubmit}>
          <div className="flex items-center gap-3 px-4 py-3 border-b border-[#2a2a2e]">
            <Search className="w-5 h-5 text-[#6e6e76]" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value)
                setSelectedIndex(0)
              }}
              placeholder="Search sessions..."
              className="flex-1 bg-transparent text-[#ececef] placeholder-[#6e6e76] outline-none font-mono text-sm"
              spellCheck={false}
              autoComplete="off"
            />
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-[#6e6e76] hover:text-[#ececef] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </form>

        {/* Suggestions */}
        {suggestions.length > 0 && (
          <div className="py-2 border-b border-[#2a2a2e]">
            {suggestions.map((suggestion, i) => {
              const Icon = getIcon(suggestion.type)
              return (
                <button
                  key={`${suggestion.type}-${suggestion.label}-${i}`}
                  onClick={() => handleSelect(suggestion.query)}
                  className={cn(
                    'w-full flex items-center gap-3 px-4 py-2 text-sm transition-colors',
                    selectedIndex === i
                      ? 'bg-[#1c1c1f] text-[#ececef]'
                      : 'text-[#9b9ba0] hover:bg-[#1c1c1f] hover:text-[#ececef]'
                  )}
                >
                  <Icon className="w-4 h-4 text-[#6e6e76]" />
                  <span className="flex-1 truncate font-mono">{suggestion.label}</span>
                  {suggestion.count !== undefined && (
                    <span className="text-xs text-[#6e6e76] tabular-nums">{suggestion.count}</span>
                  )}
                </button>
              )
            })}
          </div>
        )}

        {/* Filter hints */}
        <div className="px-4 py-3">
          <p className="text-xs font-medium text-[#6e6e76] uppercase tracking-wider mb-2">
            Filters
          </p>
          <div className="flex flex-wrap gap-2">
            {['project:', 'path:', 'skill:', 'after:', 'before:', '"phrase"'].map(filter => (
              <button
                key={filter}
                onClick={() => insertFilter(filter)}
                className="px-2 py-1 text-xs font-mono text-[#7c9885] bg-[#1c1c1f] hover:bg-[#252525] rounded border border-[#2a2a2e] transition-colors"
              >
                {filter}
              </button>
            ))}
          </div>
        </div>

        {/* Keyboard hints */}
        <div className="px-4 py-2 border-t border-[#2a2a2e] flex items-center gap-4 text-xs text-[#6e6e76]">
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">↑↓</kbd>
            Navigate
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">Enter</kbd>
            Search
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">Esc</kbd>
            Close
          </span>
        </div>
      </div>
    </div>
  )
}
```

### Step 2: Commit

```bash
git add src/components/CommandPalette.tsx
git commit -m "feat: rebuild CommandPalette with live autocomplete

- Real-time suggestions for projects, skills
- Session preview matches
- Arrow key navigation with selection highlight
- Recent searches shown when empty

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 10: Create StatusBar Component

**Files:**
- Create: `src/components/StatusBar.tsx`

### Step 1: Extract StatusBar to its own file

Create `src/components/StatusBar.tsx`:

```tsx
import type { ProjectInfo } from '../hooks/use-projects'

interface StatusBarProps {
  projects: ProjectInfo[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const totalSessions = projects.reduce((sum, p) => sum + p.sessions.length, 0)
  const totalActive = projects.reduce((sum, p) => sum + p.activeCount, 0)
  const latestActivity = projects[0]?.sessions[0]?.modifiedAt

  const formatLastActivity = (dateString: string) => {
    const date = new Date(dateString)
    return date.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    })
  }

  return (
    <footer className="h-8 bg-white border-t border-gray-200 px-4 flex items-center justify-between text-xs text-gray-500">
      <span>
        {projects.length} projects · {totalSessions} sessions
        {totalActive > 0 && (
          <span className="text-green-600 ml-2">
            · {totalActive} active
          </span>
        )}
      </span>
      {latestActivity && (
        <span>
          Last activity: {formatLastActivity(latestActivity)}
        </span>
      )}
    </footer>
  )
}
```

### Step 2: Commit

```bash
git add src/components/StatusBar.tsx
git commit -m "feat: extract StatusBar component

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 11: Update ConversationView for Router

**Files:**
- Modify: `src/components/ConversationView.tsx`

### Step 1: Update to use router params

The ConversationView needs to get params from the router instead of props. Update the component to use `useParams` and `useNavigate`:

```tsx
// At the top of ConversationView.tsx, add:
import { useParams, useNavigate, useOutletContext } from 'react-router-dom'

// Replace props-based params with:
export function ConversationView() {
  const { projectId, sessionId } = useParams()
  const navigate = useNavigate()
  const { projects } = useOutletContext<{ projects: ProjectInfo[] }>()

  const projectDir = projectId ? decodeURIComponent(projectId) : ''
  const project = projects.find(p => p.name === projectDir)
  const projectName = project?.displayName || projectDir

  const handleBack = () => navigate(-1)

  // ... rest of component uses projectDir, sessionId, handleBack
}
```

### Step 2: Commit

```bash
git add src/components/ConversationView.tsx
git commit -m "refactor: update ConversationView to use router params

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 12: Update Router with All Components

**Files:**
- Modify: `src/router.tsx`

### Step 1: Update router with correct imports

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
      { path: 'search', element: <SearchResults /> },
      { path: 'session/:projectId/:sessionId', element: <ConversationView /> },
    ],
  },
])
```

### Step 2: Commit

```bash
git add src/router.tsx
git commit -m "feat: finalize router configuration

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 13: Type Check and Build Verification

### Step 1: Run type check

Run: `npm run typecheck`

Expected: No type errors

### Step 2: Run build

Run: `npm run build`

Expected: Build succeeds

### Step 3: Manual testing

Run: `npm run dev`

Test checklist:
- [ ] Home (`/`) shows StatsDashboard with heatmap
- [ ] Click project in dashboard → navigates to `/project/:id`
- [ ] Click session → navigates to `/session/:projectId/:sessionId`
- [ ] ⌘K opens command palette with live suggestions
- [ ] Type `project:` shows autocomplete
- [ ] Arrow keys navigate suggestions
- [ ] Enter executes search → `/search?q=...`
- [ ] Sidebar shows project list
- [ ] Selecting project shows per-project stats in sidebar footer
- [ ] Browser back/forward works
- [ ] Breadcrumbs update correctly

### Step 4: Final commit

```bash
git add .
git commit -m "feat: complete Session Discovery UX rewrite

Full implementation of design spec:
- React Router for clean URL navigation
- Zustand for global state
- StatsDashboard with activity heatmap
- CommandPalette with live autocomplete
- Sidebar with per-project stats panel
- Proper breadcrumb navigation
- All elements clickable as search entry points

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Summary

This plan fully implements the design spec:

| Feature | Status |
|---------|--------|
| Routing with React Router | ✅ Clean URLs |
| Global state with Zustand | ✅ Search, recent searches |
| Stats Dashboard | ✅ With heatmap + tool usage |
| Per-Project Stats | ✅ Skills, files, tool bars |
| Command Palette | ✅ Live autocomplete |
| Breadcrumb navigation | ✅ Context-aware |
| Everything clickable | ✅ All stats → search |
