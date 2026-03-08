import { useCallback, useMemo, useRef, useState } from 'react'
import type { KanbanGroupBy, KanbanSort } from './types'
import {
  KANBAN_COLLAPSE_STORAGE_KEY,
  KANBAN_GROUP_BY_STORAGE_KEY,
  KANBAN_SORT_STORAGE_KEY,
} from './types'
import { type LiveSession, sessionTotalCost } from './use-live-sessions'

export interface BranchGroup {
  branchName: string | null
  sessions: LiveSession[]
  sessionCount: number
  maxActivityAt: number
}

export interface ProjectGroup {
  projectName: string
  branches: BranchGroup[]
  totalSessionCount: number
  totalCostUsd: number
  maxActivityAt: number
}

/**
 * Pure function: groups sessions into Project > Branch hierarchy.
 *
 * `stableProjectOrder` and `stableBranchOrder` are optional maps that lock
 * in the display order of already-seen groups. When provided:
 * - Known projects/branches retain their existing position.
 * - New projects/branches are appended at the end (sorted by most-recent
 *   activity among themselves so the very first appearance is sensible).
 *
 * This prevents cards from jumping around as `lastActivityAt` updates on
 * every SSE tick while still surfacing newly-created groups promptly.
 */
export function groupSessionsByProjectBranch(
  sessions: LiveSession[],
  stableProjectOrder?: Map<string, number>,
  stableBranchOrder?: Map<string, Map<string, number>>,
  sort: KanbanSort = 'recent',
): ProjectGroup[] {
  if (sessions.length === 0) return []

  // Build map: project -> branch -> sessions
  const projectMap = new Map<string, Map<string, LiveSession[]>>()

  for (const s of sessions) {
    const projectKey = s.projectDisplayName || s.project
    const branchKey = s.effectiveBranch ?? '__null__'

    let branchMap = projectMap.get(projectKey)
    if (!branchMap) {
      branchMap = new Map()
      projectMap.set(projectKey, branchMap)
    }

    let branchSessions = branchMap.get(branchKey)
    if (!branchSessions) {
      branchSessions = []
      branchMap.set(branchKey, branchSessions)
    }

    branchSessions.push(s)
  }

  // Convert to ProjectGroup[]
  const groups: ProjectGroup[] = []

  for (const [projectName, branchMap] of projectMap) {
    const branches: BranchGroup[] = []
    let totalCostUsd = 0
    let totalSessionCount = 0
    let projectMaxActivity = 0

    for (const [branchKey, branchSessions] of branchMap) {
      const branchMaxActivity = Math.max(...branchSessions.map((s) => s.lastActivityAt))
      branches.push({
        branchName: branchKey === '__null__' ? null : branchKey,
        sessions: branchSessions,
        sessionCount: branchSessions.length,
        maxActivityAt: branchMaxActivity,
      })
      totalSessionCount += branchSessions.length
      totalCostUsd += branchSessions.reduce((sum, s) => sum + sessionTotalCost(s), 0)
      projectMaxActivity = Math.max(projectMaxActivity, branchMaxActivity)
    }

    // Sort branches: stable order for known, new ones ordered by `sort` preference
    const knownBranchOrder = stableBranchOrder?.get(projectName)
    branches.sort((a, b) => {
      const aKey = a.branchName ?? '__null__'
      const bKey = b.branchName ?? '__null__'
      const aIdx = knownBranchOrder?.get(aKey) ?? Number.POSITIVE_INFINITY
      const bIdx = knownBranchOrder?.get(bKey) ?? Number.POSITIVE_INFINITY
      if (aIdx !== bIdx) return aIdx - bIdx
      return newGroupTiebreak(a, b, sort)
    })

    groups.push({
      projectName,
      branches,
      totalSessionCount,
      totalCostUsd,
      maxActivityAt: projectMaxActivity,
    })
  }

  // Sort projects: stable order for known, new ones ordered by `sort` preference
  groups.sort((a, b) => {
    const aIdx = stableProjectOrder?.get(a.projectName) ?? Number.POSITIVE_INFINITY
    const bIdx = stableProjectOrder?.get(b.projectName) ?? Number.POSITIVE_INFINITY
    if (aIdx !== bIdx) return aIdx - bIdx
    return newGroupTiebreak(a, b, sort)
  })

  return groups
}

/** Tiebreak comparator for newly-seen groups (both have index = POSITIVE_INFINITY). */
function newGroupTiebreak(
  a: {
    maxActivityAt: number
    totalCostUsd?: number
    branchName?: string | null
    projectName?: string
  },
  b: {
    maxActivityAt: number
    totalCostUsd?: number
    branchName?: string | null
    projectName?: string
  },
  sort: KanbanSort,
): number {
  if (sort === 'alphabetical') {
    const aName = a.projectName ?? a.branchName ?? ''
    const bName = b.projectName ?? b.branchName ?? ''
    return aName.localeCompare(bName)
  }
  if (sort === 'cost') {
    return (b.totalCostUsd ?? 0) - (a.totalCostUsd ?? 0)
  }
  // 'recent'
  return b.maxActivityAt - a.maxActivityAt
}

// --- Collapse state ---

function loadCollapseState(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(KANBAN_COLLAPSE_STORAGE_KEY)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}

function saveCollapseState(state: Record<string, boolean>) {
  localStorage.setItem(KANBAN_COLLAPSE_STORAGE_KEY, JSON.stringify(state))
}

export function projectCollapseKey(projectName: string): string {
  return `project:${projectName}`
}

export function branchCollapseKey(projectName: string, branchName: string | null): string {
  return `branch:${projectName}:${branchName ?? '__null__'}`
}

// --- Group-by state ---

function loadGroupBy(): KanbanGroupBy {
  try {
    const raw = localStorage.getItem(KANBAN_GROUP_BY_STORAGE_KEY)
    if (raw === 'none') return 'none'
    return 'project-branch' // default
  } catch {
    return 'project-branch'
  }
}

// --- Sort state ---

function loadSort(): KanbanSort {
  try {
    const raw = localStorage.getItem(KANBAN_SORT_STORAGE_KEY)
    if (raw === 'alphabetical' || raw === 'cost') return raw
    return 'recent'
  } catch {
    return 'recent'
  }
}

// --- Hook ---

export function useKanbanGrouping(sessions: LiveSession[], urlGroupBy: KanbanGroupBy | null) {
  const [groupBy, setGroupByState] = useState<KanbanGroupBy>(() => urlGroupBy ?? loadGroupBy())
  const [sort, setSortState] = useState<KanbanSort>(loadSort)
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(loadCollapseState)

  // Stable order refs: remember the display index of each project/branch so
  // live SSE updates to lastActivityAt don't cause groups to re-sort and jump.
  const projectOrderRef = useRef<Map<string, number>>(new Map())
  const branchOrderRef = useRef<Map<string, Map<string, number>>>(new Map())

  const projectGroups = useMemo(() => {
    if (groupBy !== 'project-branch') return []

    const groups = groupSessionsByProjectBranch(
      sessions,
      projectOrderRef.current,
      branchOrderRef.current,
      sort,
    )

    // Update stable order maps: assign indices to any newly-seen projects/branches.
    for (let pi = 0; pi < groups.length; pi++) {
      const { projectName, branches } = groups[pi]
      if (!projectOrderRef.current.has(projectName)) {
        projectOrderRef.current.set(projectName, projectOrderRef.current.size)
      }
      if (!branchOrderRef.current.has(projectName)) {
        branchOrderRef.current.set(projectName, new Map())
      }
      const branchMap = branchOrderRef.current.get(projectName)
      if (branchMap) {
        for (let bi = 0; bi < branches.length; bi++) {
          const key = branches[bi].branchName ?? '__null__'
          if (!branchMap.has(key)) {
            branchMap.set(key, branchMap.size)
          }
        }
      }
    }

    return groups
  }, [sessions, groupBy, sort])

  const setGroupBy = useCallback((value: KanbanGroupBy) => {
    setGroupByState(value)
    localStorage.setItem(KANBAN_GROUP_BY_STORAGE_KEY, value)
  }, [])

  const setSort = useCallback((value: KanbanSort) => {
    setSortState(value)
    localStorage.setItem(KANBAN_SORT_STORAGE_KEY, value)
    // Reset stable order so groups re-sort immediately with the new preference.
    projectOrderRef.current = new Map()
    branchOrderRef.current = new Map()
  }, [])

  const toggleCollapse = useCallback((key: string) => {
    setCollapsed((prev) => {
      const next = { ...prev, [key]: !prev[key] }
      saveCollapseState(next)
      return next
    })
  }, [])

  const isCollapsed = useCallback((key: string) => collapsed[key] ?? false, [collapsed])

  return {
    groupBy,
    setGroupBy,
    sort,
    setSort,
    projectGroups,
    toggleCollapse,
    isCollapsed,
  }
}
