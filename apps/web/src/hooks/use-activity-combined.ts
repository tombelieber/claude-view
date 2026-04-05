/**
 * Combined activity hook — uses server-side aggregation for summary/projects
 * (single SQL query) and keeps the existing session-level data only for
 * DailyTimeline drill-down.
 *
 * CalendarHeatmap now uses the server histogram (covers ALL sessions for the
 * selected time range) instead of the client-side session list (capped at 500).
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
  /** Server-computed daily breakdown for CalendarHeatmap (covers ALL sessions) */
  heatmapDays: DayActivity[]
  /** Client-computed daily breakdown (from paginated sessions) for DailyTimeline drill-down */
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
    busiestDay: null, // computed from heatmapDays below
    totalToolCalls: r.summary.totalToolCalls,
    totalAgentSpawns: r.summary.totalAgentSpawns,
    totalMcpCalls: r.summary.totalMcpCalls,
    uniqueSkills: r.summary.uniqueSkills,
  }
}

function toClientProjects(r: RichActivityResponse): ClientProjectActivity[] {
  return r.projects
    .map((p) => ({
      name: p.displayName,
      projectPath: p.projectPath,
      totalSeconds: p.totalSeconds,
      sessionCount: p.sessionCount,
    }))
    .sort((a, b) => b.totalSeconds - a.totalSeconds)
}

/**
 * Convert server histogram (ActivityPoint[]) → DayActivity[] for CalendarHeatmap.
 * The histogram covers ALL sessions for the time range (no 500-row cap).
 * DailyTimeline sessions are left empty — it uses the separate session query.
 */
function histogramToDays(r: RichActivityResponse): DayActivity[] {
  return r.histogram.map((pt) => ({
    date: pt.date,
    totalSeconds: pt.totalSeconds,
    sessionCount: pt.count,
    sessions: [], // heatmap doesn't need individual sessions
  }))
}

export function useActivityCombined(
  timeAfter: number | null,
  timeBefore: number | null,
  sidebarProject?: string | null,
  sidebarBranch?: string | null,
  /** When a heatmap day is clicked, pass the YYYY-MM-DD string to fetch that day's sessions on demand. */
  selectedDate?: string | null,
) {
  // 1. Server-side aggregation (1 request, <100ms for 10K sessions)
  const server = useServerActivity(timeAfter, timeBefore, sidebarProject, sidebarBranch)

  // 2. Default session list for DailyTimeline (recent 500) — used when no date is clicked.
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

  // 3. On-demand fetch for clicked day — fetches ALL sessions for that specific day.
  //    This solves the "click January day → empty timeline" bug: the 500-row default
  //    only covers recent sessions, but the heatmap (from server histogram) shows all dates.
  const dayQuery = useQuery<SessionInfo[]>({
    queryKey: [
      'activity-day-sessions',
      selectedDate ?? '',
      sidebarProject ?? '',
      sidebarBranch ?? '',
    ],
    queryFn: async () => {
      if (!selectedDate) return []
      // Convert YYYY-MM-DD to time_after (start of day) and time_before (end of day)
      const dayStart = new Date(selectedDate + 'T00:00:00')
      const dayEnd = new Date(selectedDate + 'T23:59:59')
      const sp = new URLSearchParams()
      sp.set('limit', '200') // max sessions in a single day
      sp.set('offset', '0')
      sp.set('sort', 'recent')
      sp.set('time_after', String(Math.floor(dayStart.getTime() / 1000)))
      sp.set('time_before', String(Math.floor(dayEnd.getTime() / 1000)))
      if (sidebarProject) sp.set('project', sidebarProject)
      if (sidebarBranch) sp.set('branches', sidebarBranch)

      const res = await fetch(`/api/sessions?${sp}`)
      if (!res.ok) throw new Error('Failed to fetch sessions for day')
      const data = await res.json()
      return data.sessions as SessionInfo[]
    },
    enabled: !!selectedDate, // only fetch when a day is clicked
    staleTime: 60_000,
  })

  const allSessions = sessionQuery.data ?? []

  // Use day-specific sessions when a date is clicked, otherwise fall back to the 500-row default.
  const effectiveSessions = selectedDate && dayQuery.data ? dayQuery.data : allSessions

  // Client-side day aggregation for DailyTimeline (needs actual session objects for drill-down)
  const timelineDays = useMemo(() => aggregateByDay(effectiveSessions), [effectiveSessions])

  // Compute combined data — use server histogram for heatmap, client sessions for timeline
  const data = useMemo<CombinedActivityData | null>(() => {
    if (!server.data) return null

    const summary = toClientSummary(server.data)
    const heatmapDays = histogramToDays(server.data)

    // Compute busiestDay from server histogram (covers ALL sessions, not just 500)
    let busiestDay: ClientActivitySummary['busiestDay'] = null
    let maxDaySeconds = 0
    for (const day of heatmapDays) {
      if (day.totalSeconds > maxDaySeconds) {
        maxDaySeconds = day.totalSeconds
        busiestDay = { date: day.date, totalSeconds: day.totalSeconds }
      }
    }
    summary.busiestDay = busiestDay

    return {
      summary,
      projects: toClientProjects(server.data),
      heatmapDays,
      days: timelineDays,
      totalCount: server.data.total,
    }
  }, [server.data, timelineDays])

  return {
    data,
    isLoading: server.isLoading || sessionQuery.isLoading,
    /** True while fetching sessions for a clicked heatmap day */
    isDayLoading: dayQuery.isLoading && !!selectedDate,
    error: server.error ?? sessionQuery.error ?? dayQuery.error,
  }
}
