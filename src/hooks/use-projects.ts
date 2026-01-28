import { useQuery, useQueries } from '@tanstack/react-query'
import type { ProjectSummary, SessionsPage, SessionInfo } from '../types/generated'

// Re-export for backward compatibility with existing imports
export type { ProjectSummary, SessionsPage, SessionInfo } from '../types/generated'
// Keep ProjectInfo re-export for components that still reference it
export type { ProjectInfo } from '../types/generated'

async function fetchProjectSummaries(): Promise<ProjectSummary[]> {
  const response = await fetch('/api/projects')
  if (!response.ok) throw new Error('Failed to fetch projects')
  return response.json()
}

export function useProjectSummaries() {
  return useQuery({
    queryKey: ['project-summaries'],
    queryFn: fetchProjectSummaries,
  })
}

export interface ProjectSessionsOptions {
  limit?: number
  offset?: number
  sort?: string
  branch?: string
  includeSidechains?: boolean
}

async function fetchProjectSessions(projectId: string, opts: ProjectSessionsOptions): Promise<SessionsPage> {
  const params = new URLSearchParams()
  if (opts.limit) params.set('limit', String(opts.limit))
  if (opts.offset) params.set('offset', String(opts.offset))
  if (opts.sort) params.set('sort', opts.sort)
  if (opts.branch) params.set('branch', opts.branch)
  if (opts.includeSidechains) params.set('includeSidechains', 'true')
  const response = await fetch(`/api/projects/${encodeURIComponent(projectId)}/sessions?${params}`)
  if (!response.ok) throw new Error('Failed to fetch sessions')
  return response.json()
}

export function useProjectSessions(projectId: string | undefined, opts: ProjectSessionsOptions = {}) {
  return useQuery({
    queryKey: ['project-sessions', projectId, opts],
    queryFn: () => fetchProjectSessions(projectId!, opts),
    enabled: !!projectId,
  })
}

export function useAllSessions(projectIds: string[]) {
  const queries = useQueries({
    queries: projectIds.map(id => ({
      queryKey: ['project-sessions', id, { limit: 1000 }],
      queryFn: () => fetchProjectSessions(id, { limit: 1000 }),
    })),
  })

  const isLoading = queries.some(q => q.isLoading)
  const sessions: SessionInfo[] = queries
    .flatMap(q => q.data?.sessions ?? [])
    .sort((a, b) => Number(b.modifiedAt) - Number(a.modifiedAt))

  return { sessions, isLoading }
}
