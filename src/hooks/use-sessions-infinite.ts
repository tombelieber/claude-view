// src/hooks/use-sessions-infinite.ts
import { useInfiniteQuery } from '@tanstack/react-query'
import type { SessionsListResponse } from '../types/generated'
import type { SessionFilters } from './use-session-filters'

const PAGE_SIZE = 30

/** Response type extended with hasMore (added by Task 3 route refactor). */
type SessionsPageResponse = SessionsListResponse & { hasMore?: boolean }

export interface SessionsQueryParams {
  filters: SessionFilters
  search: string
  timeAfter?: number
  timeBefore?: number
  sidebarProject?: string | null
  sidebarBranch?: string | null
}

function buildSearchParams(params: SessionsQueryParams, offset: number): URLSearchParams {
  const sp = new URLSearchParams()
  sp.set('limit', String(PAGE_SIZE))
  sp.set('offset', String(offset))
  sp.set('sort', params.filters.sort)

  if (params.search) sp.set('q', params.search)

  // Merge sidebar branch with filter branches
  const branches = [...params.filters.branches]
  if (params.sidebarBranch && !branches.includes(params.sidebarBranch)) {
    branches.push(params.sidebarBranch)
  }
  if (branches.length > 0) sp.set('branches', branches.join(','))

  if (params.filters.models.length > 0) sp.set('models', params.filters.models.join(','))

  if (params.filters.hasCommits === 'yes') sp.set('has_commits', 'true')
  if (params.filters.hasCommits === 'no') sp.set('has_commits', 'false')

  if (params.filters.hasSkills === 'yes') sp.set('has_skills', 'true')
  if (params.filters.hasSkills === 'no') sp.set('has_skills', 'false')

  if (params.filters.minDuration !== null) sp.set('min_duration', String(params.filters.minDuration))
  if (params.filters.minFiles !== null) sp.set('min_files', String(params.filters.minFiles))
  if (params.filters.minTokens !== null) sp.set('min_tokens', String(params.filters.minTokens))
  if (params.filters.highReedit === true) sp.set('high_reedit', 'true')

  if (params.timeAfter) sp.set('time_after', String(params.timeAfter))
  if (params.timeBefore) sp.set('time_before', String(params.timeBefore))

  return sp
}

async function fetchSessionsPage(
  params: SessionsQueryParams,
  offset: number,
): Promise<SessionsPageResponse> {
  const sp = buildSearchParams(params, offset)
  const response = await fetch(`/api/sessions?${sp}`)
  if (!response.ok) throw new Error('Failed to fetch sessions')
  return response.json()
}

export function useSessionsInfinite(params: SessionsQueryParams) {
  return useInfiniteQuery({
    queryKey: ['sessions-infinite', params],
    queryFn: ({ pageParam }) => fetchSessionsPage(params, pageParam),
    initialPageParam: 0,
    getNextPageParam: (lastPage, _allPages, lastPageParam) => {
      // Use server hasMore if available, otherwise compute from total
      const hasMore = lastPage.hasMore ?? (lastPageParam + PAGE_SIZE < lastPage.total)
      if (!hasMore) return undefined
      return lastPageParam + PAGE_SIZE
    },
    // Flatten all pages into a single sessions array for convenience
    select: (data) => ({
      sessions: data.pages.flatMap(p => p.sessions),
      total: data.pages[0]?.total ?? 0,
    }),
  })
}
