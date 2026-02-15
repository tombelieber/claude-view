import type { LiveSession } from './use-live-sessions'
import { GROUP_ORDER } from './types'

export type LiveSortField = 'status' | 'last_active' | 'cost' | 'turns' | 'context' | 'project'
export type LiveSortDirection = 'asc' | 'desc'

export interface LiveSessionFilters {
  statuses: string[] // agent state groups to include (needs_you, autonomous, delivered)
  projects: string[] // project display names
  branches: string[] // git branch names
  search: string // text search query
  sort: LiveSortField
  sortDir: LiveSortDirection
}

export const DEFAULT_LIVE_FILTERS: LiveSessionFilters = {
  statuses: [],
  projects: [],
  branches: [],
  search: '',
  sort: 'status',
  sortDir: 'asc',
}

export function sortLiveSessions(
  sessions: LiveSession[],
  field: LiveSortField,
  dir: LiveSortDirection
): LiveSession[] {
  return [...sessions].sort((a, b) => {
    let cmp = 0

    switch (field) {
      case 'status': {
        const aOrder = GROUP_ORDER[a.agentState.group]
        const bOrder = GROUP_ORDER[b.agentState.group]
        cmp = aOrder - bOrder
        break
      }
      case 'last_active':
        cmp = a.lastActivityAt - b.lastActivityAt
        break
      case 'cost':
        cmp = a.cost.totalUsd - b.cost.totalUsd
        break
      case 'turns':
        cmp = a.turnCount - b.turnCount
        break
      case 'context':
        cmp = a.contextWindowTokens - b.contextWindowTokens
        break
      case 'project':
        cmp = (a.projectDisplayName || a.project).localeCompare(
          b.projectDisplayName || b.project
        )
        break
    }

    // Tiebreaker: lastActivityAt descending
    if (cmp === 0) {
      cmp = b.lastActivityAt - a.lastActivityAt
    }

    return dir === 'desc' ? -cmp : cmp
  })
}

export function filterLiveSessions(
  sessions: LiveSession[],
  filters: LiveSessionFilters
): LiveSession[] {
  let result = sessions

  // Status filter
  if (filters.statuses.length > 0) {
    result = result.filter((s) =>
      filters.statuses.includes(s.agentState.group)
    )
  }

  // Project filter
  if (filters.projects.length > 0) {
    result = result.filter((s) =>
      filters.projects.includes(s.projectDisplayName || s.project)
    )
  }

  // Branch filter
  if (filters.branches.length > 0) {
    result = result.filter(
      (s) => s.gitBranch !== null && filters.branches.includes(s.gitBranch)
    )
  }

  // Text search
  const query = filters.search.trim().toLowerCase()
  if (query) {
    result = result.filter((s) => {
      const project = (s.projectDisplayName || s.project).toLowerCase()
      const branch = (s.gitBranch ?? '').toLowerCase()
      const message = s.lastUserMessage.toLowerCase()
      return (
        project.includes(query) ||
        branch.includes(query) ||
        message.includes(query)
      )
    })
  }

  // Sort
  return sortLiveSessions(result, filters.sort, filters.sortDir)
}
