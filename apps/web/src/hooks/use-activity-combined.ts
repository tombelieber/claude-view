/**
 * Combined activity hook — uses server-side aggregation for summary/projects
 * (single SQL query) and keeps the existing session-level data only for
 * DailyTimeline + CalendarHeatmap drill-down.
 *
 * This replaces `useActivityData` which fetched up to 10,000 sessions
 * purely for client-side aggregation.
 */
import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import type { DayActivity, ProjectActivity as ClientProjectActivity } from '../lib/activity-utils'
import { aggregateByDay } from '../lib/activity-utils'
import type { ActivitySummary as ClientActivitySummary } from '../lib/activity-utils'
import type { SessionInfo } from '../types/generated/SessionInfo'
import { useServerActivity, type RichActivityResponse } from './use-server-activity'

export interface CombinedActivityData {
  /** Server-computed summary (no 10K client loop) */
  summary: ClientActivitySummary
  /** Server-computed project breakdown */
  projects: ClientProjectActivity[]
  /** Client-computed daily breakdown (from paginated sessions) for heatmap/timeline */
  days: DayActivity[]
  /** Total count from server */
  totalCount: number
}

/**
 * Maps server `RichActivityResponse` summary to the client `ActivitySummary` shape
 * expected by `<SummaryStats>`.
 */
function toClientSummary(r: RichActivityResponse): ClientActivitySummary {
  const timedCount =
    r.summary.sessionCount > 0 && r.summary.totalSeconds > 0
      ? Math.max(1, r.summary.sessionCount) // avoid div-by-zero
      : 0
  return {
    totalSeconds: r.summary.totalSeconds,
    sessionCount: r.summary.sessionCount,
    avgSessionSeconds: timedCount > 0 ? Math.round(r.summary.totalSeconds / timedCount) : 0,
    longestSession: r.summary.longestSessionId
      ? {
          seconds: r.summary.longestSessionSeconds,
          project: r.summary.longestSessionProject ?? '',
          title: r.summary.longestSessionTitle ?? '(untitled)',
        }
      : null,
    busiestDay: null, // computed from days below if available
    totalToolCalls: r.summary.totalToolCalls,
    totalAgentSpawns: r.summary.totalAgentSpawns,
    totalMcpCalls: r.summary.totalMcpCalls,
    uniqueSkills: r.summary.uniqueSkills,
  }
}

function toClientProjects(r: RichActivityResponse): ClientProjectActivity[] {
  return r.projects.map((p) => ({
    name: p.displayName,
    projectPath: p.projectPath,
    totalSeconds: p.totalSeconds,
    sessionCount: p.sessionCount,
  }))
}

export function useActivityCombined(
  timeAfter: number | null,
  timeBefore: number | null,
  sidebarProject?: string | null,
  sidebarBranch?: string | null,
) {
  // 1. Server-side aggregation (1 request, <100ms for 10K sessions)
  const server = useServerActivity(timeAfter, timeBefore, sidebarProject, sidebarBranch)

  // 2. Session list for day-level drill-down (single request, max 500 for heatmap)
  //    This is much smaller than the old 10K fetch — we only need enough for
  //    CalendarHeatmap + DailyTimeline day-level display.
  const sessionQuery = useQuery<SessionInfo[]>({
    queryKey: [
      'activity-sessions-light',
      timeAfter,
      timeBefore,
      sidebarProject ?? '',
      sidebarBranch ?? '',
    ],
    queryFn: async () => {
      const sp = new URLSearchParams()
      sp.set('limit', '500')
      sp.set('offset', '0')
      sp.set('sort', 'recent')
      if (timeAfter !== null && timeAfter > 0) sp.set('time_after', String(timeAfter))
      if (timeBefore !== null && timeBefore > 0) sp.set('time_before', String(timeBefore))
      if (sidebarProject) sp.set('project', sidebarProject)
      if (sidebarBranch) sp.set('branches', sidebarBranch)

      const res = await fetch(`/api/sessions?${sp}`)
      if (!res.ok) throw new Error('Failed to fetch sessions for activity')
      const data = await res.json()
      return data.sessions as SessionInfo[]
    },
    staleTime: 60_000,
  })

  const allSessions = sessionQuery.data ?? []

  // Compute days from session data (for heatmap/timeline)
  const days = useMemo(() => aggregateByDay(allSessions), [allSessions])

  // Compute busiest day from client-side data and merge into server summary
  const data = useMemo<CombinedActivityData | null>(() => {
    if (!server.data) return null

    const summary = toClientSummary(server.data)
    // Compute busiestDay from days (needs session-level data)
    let busiestDay: ClientActivitySummary['busiestDay'] = null
    let maxDaySeconds = 0
    for (const day of days) {
      if (day.totalSeconds > maxDaySeconds) {
        maxDaySeconds = day.totalSeconds
        busiestDay = { date: day.date, totalSeconds: day.totalSeconds }
      }
    }
    summary.busiestDay = busiestDay

    return {
      summary,
      projects: toClientProjects(server.data),
      days,
      totalCount: server.data.total,
    }
  }, [server.data, days])

  return {
    data,
    isLoading: server.isLoading || sessionQuery.isLoading,
    error: server.error ?? sessionQuery.error,
  }
}
