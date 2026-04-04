import { useQuery } from '@tanstack/react-query'

/** Project-level aggregation from the server. */
export interface ProjectActivity {
  projectPath: string
  displayName: string
  sessionCount: number
  totalSeconds: number
  totalCostUsd: number
}

/** Summary stats from the server. */
export interface ActivitySummary {
  totalSeconds: number
  sessionCount: number
  totalToolCalls: number
  totalAgentSpawns: number
  totalMcpCalls: number
  uniqueSkills: number
  longestSessionId: string | null
  longestSessionSeconds: number
  longestSessionProject: string | null
  longestSessionTitle: string | null
}

/** Histogram point. */
export interface ActivityPoint {
  date: string
  count: number
}

/** Full server-side activity response. */
export interface RichActivityResponse {
  histogram: ActivityPoint[]
  bucket: string
  projects: ProjectActivity[]
  summary: ActivitySummary
  total: number
}

/**
 * Server-side activity aggregation — replaces `useActivityData`'s 50-page client loop.
 * Single SQL query returns histogram, project breakdown, and summary stats.
 */
export function useServerActivity(
  timeAfter: number | null,
  timeBefore: number | null,
  project?: string | null,
  branch?: string | null,
) {
  return useQuery<RichActivityResponse>({
    queryKey: ['server-activity', timeAfter, timeBefore, project ?? '', branch ?? ''],
    queryFn: async () => {
      const sp = new URLSearchParams()
      if (timeAfter !== null && timeAfter > 0) sp.set('time_after', String(timeAfter))
      if (timeBefore !== null && timeBefore > 0) sp.set('time_before', String(timeBefore))
      if (project) sp.set('project', project)
      if (branch) sp.set('branch', branch)

      const res = await fetch(`/api/sessions/activity/rich?${sp}`)
      if (!res.ok) throw new Error('Failed to fetch activity')
      return res.json() as Promise<RichActivityResponse>
    },
    staleTime: 60_000,
  })
}
