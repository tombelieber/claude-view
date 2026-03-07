import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import {
  type ActivitySummary,
  type DayActivity,
  type ProjectActivity,
  aggregateByDay,
  aggregateByProject,
  computeSummary,
} from '../lib/activity-utils'
import type { SessionInfo } from '../types/generated/SessionInfo'

const PAGE_SIZE = 200
const MAX_PAGES = 50 // Safety limit: 50 * 200 = 10,000 sessions max

export interface ActivityData {
  days: DayActivity[]
  projects: ProjectActivity[]
  summary: ActivitySummary
  sessions: SessionInfo[]
  /** Total sessions matching the query (from server) */
  totalCount: number
}

/**
 * Fetch ALL sessions for a time range (paginated) and compute activity aggregations.
 * Respects sidebar project/branch filters when provided (server-side).
 */
export function useActivityData(
  timeAfter: number | null,
  timeBefore: number | null,
  sidebarProject?: string | null,
  sidebarBranch?: string | null,
) {
  // Fork sessions (parent_id IS NOT NULL) are intentionally included. Each fork is an
  // independent working session with its own durationSeconds. Counting them gives accurate
  // total working time. The API returns them as part of kind=Conversation sessions.
  const query = useQuery<{ sessions: SessionInfo[]; total: number }>({
    queryKey: [
      'activity-sessions',
      timeAfter,
      timeBefore,
      sidebarProject ?? '',
      sidebarBranch ?? '',
    ],
    queryFn: async () => {
      const allSessions: SessionInfo[] = []
      let offset = 0
      let total = 0

      // Paginate until we have all sessions (with safety limit)
      for (let page = 0; page < MAX_PAGES; page++) {
        const sp = new URLSearchParams()
        sp.set('limit', String(PAGE_SIZE))
        sp.set('offset', String(offset))
        sp.set('sort', 'recent')
        if (timeAfter !== null && timeAfter > 0) sp.set('time_after', String(timeAfter))
        if (timeBefore !== null && timeBefore > 0) sp.set('time_before', String(timeBefore))
        if (sidebarProject) sp.set('project', sidebarProject)
        if (sidebarBranch) sp.set('branches', sidebarBranch)

        const resp = await fetch(`/api/sessions?${sp}`)
        if (!resp.ok) throw new Error('Failed to fetch activity sessions')
        const data = await resp.json()

        total = data.total ?? 0
        const sessions = data.sessions as SessionInfo[]
        allSessions.push(...sessions)

        // Check if we have all sessions
        if (allSessions.length >= total || sessions.length < PAGE_SIZE) {
          break
        }
        offset += PAGE_SIZE
      }

      return { sessions: allSessions, total }
    },
    staleTime: 60_000, // 1 minute
  })

  // Memoize on primitive key per CLAUDE.md rule: never raw parsed objects in useMemo deps.
  // Include first/last modifiedAt so content changes (not just count changes) invalidate the memo
  // after background refetches when session count is unchanged but data has updated.
  const sessionCount = query.data?.sessions.length ?? 0
  const totalCount = query.data?.total ?? 0
  const firstTs = query.data?.sessions[0]?.modifiedAt ?? 0
  const lastTs = query.data?.sessions[sessionCount - 1]?.modifiedAt ?? 0
  const memoKey = JSON.stringify([
    sessionCount,
    totalCount,
    firstTs,
    lastTs,
    timeAfter,
    timeBefore,
    sidebarProject,
    sidebarBranch,
  ])

  const activity = useMemo<ActivityData | null>(() => {
    if (!query.data) return null
    const { sessions } = query.data
    const { total } = query.data

    const days = aggregateByDay(sessions)
    const projects = aggregateByProject(sessions)
    const summary = computeSummary(sessions, days)
    return { days, projects, summary, sessions, totalCount: total }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [memoKey])

  return {
    data: activity,
    isLoading: query.isLoading,
    error: query.error,
  }
}
