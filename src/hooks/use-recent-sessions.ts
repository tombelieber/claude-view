import { useQuery } from '@tanstack/react-query'
import type { SessionsPage } from '../types/generated'

/**
 * Lightweight session info for the sidebar Quick Jump zone.
 */
export interface RecentSession {
  id: string
  preview: string
  modifiedAt: number
  gitBranch?: string
  project?: string
}

/**
 * Fetch the most recent sessions for a project, optionally filtered by branch.
 */
async function fetchRecentSessions(
  project: string,
  branch: string | null,
): Promise<RecentSession[]> {
  const params = new URLSearchParams()
  params.set('limit', '5')
  params.set('sort', 'recent')
  if (branch) {
    params.set('branch', branch)
  }
  const url = `/api/projects/${encodeURIComponent(project)}/sessions?${params}`
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error('Failed to fetch recent sessions')
  }
  const data: SessionsPage = await response.json()
  return data.sessions.slice(0, 5).map((s) => ({
    id: s.id,
    preview: s.preview,
    modifiedAt: Number(s.modifiedAt),
    gitBranch: s.gitBranch ?? undefined,
    project: s.project ?? undefined,
  }))
}

/**
 * Hook that returns the 5 most recent sessions for the current project+branch scope.
 *
 * Used by the sidebar Quick Jump zone.
 *
 * - When `project` is null the query is disabled and returns an empty array.
 * - When `branch` is set it is appended as a query param to scope the results.
 * - Results are defensively sliced to at most 5 items.
 */
export function useRecentSessions(
  project: string | null,
  branch: string | null,
) {
  return useQuery<RecentSession[]>({
    queryKey: ['recent-sessions', project, branch],
    queryFn: () => fetchRecentSessions(project!, branch),
    enabled: !!project,
    placeholderData: [],
    staleTime: 60 * 1000, // 1 min
    gcTime: 5 * 60 * 1000, // 5 min
  })
}
