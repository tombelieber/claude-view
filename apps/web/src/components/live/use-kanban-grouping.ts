import { useCallback, useMemo, useState } from 'react'
import type { KanbanGroupBy } from './types'
import { KANBAN_COLLAPSE_STORAGE_KEY, KANBAN_GROUP_BY_STORAGE_KEY } from './types'
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
 * Projects sorted by most-recent-activity descending.
 * Branches within each project sorted by most-recent-activity descending.
 */
export function groupSessionsByProjectBranch(sessions: LiveSession[]): ProjectGroup[] {
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

    // Sort branches: most recent activity first
    branches.sort((a, b) => b.maxActivityAt - a.maxActivityAt)

    groups.push({
      projectName,
      branches,
      totalSessionCount,
      totalCostUsd,
      maxActivityAt: projectMaxActivity,
    })
  }

  // Sort projects: most recent activity first
  groups.sort((a, b) => b.maxActivityAt - a.maxActivityAt)

  return groups
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
    if (raw === 'project-branch') return 'project-branch'
    return 'none'
  } catch {
    return 'none'
  }
}

// --- Hook ---

export function useKanbanGrouping(sessions: LiveSession[], urlGroupBy: KanbanGroupBy | null) {
  const [groupBy, setGroupByState] = useState<KanbanGroupBy>(() => urlGroupBy ?? loadGroupBy())
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(loadCollapseState)

  const projectGroups = useMemo(
    () => (groupBy === 'project-branch' ? groupSessionsByProjectBranch(sessions) : []),
    [sessions, groupBy],
  )

  const setGroupBy = useCallback((value: KanbanGroupBy) => {
    setGroupByState(value)
    localStorage.setItem(KANBAN_GROUP_BY_STORAGE_KEY, value)
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
    projectGroups,
    toggleCollapse,
    isCollapsed,
  }
}
