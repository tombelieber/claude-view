import { useQuery } from '@tanstack/react-query'
import type { ExtendedDashboardStats } from '../types/generated'

/**
 * Extended dashboard stats including current week metrics and trends.
 *
 * Response includes:
 * - Base stats: totalSessions, totalProjects, heatmap, topSkills, topProjects, toolTotals
 * - currentWeek: sessionCount, totalTokens, totalFilesEdited, commitCount
 * - trends: week-over-week changes for sessions, tokens, filesEdited, commits
 */
async function fetchDashboardStats(project?: string, branches?: string): Promise<ExtendedDashboardStats> {
  const params = new URLSearchParams()
  if (project) params.set('project', project)
  if (branches) params.set('branches', branches)
  const qs = params.toString()
  const url = qs ? `/api/stats/dashboard?${qs}` : '/api/stats/dashboard'
  const response = await fetch(url)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch dashboard stats: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch extended dashboard statistics.
 *
 * Returns ExtendedDashboardStats with:
 * - totalSessions, totalProjects (counts)
 * - heatmap (DayActivity[])
 * - topSkills (SkillStat[])
 * - topProjects (ProjectStat[])
 * - toolTotals (ToolCounts)
 * - currentWeek (CurrentWeekMetrics)
 * - trends (DashboardTrends)
 */
export function useDashboardStats(project?: string, branches?: string) {
  return useQuery({
    queryKey: ['dashboard-stats', project, branches],
    queryFn: () => fetchDashboardStats(project, branches),
    staleTime: 30_000, // Consider data fresh for 30 seconds
  })
}

// Re-export types for convenience
export type { ExtendedDashboardStats, CurrentWeekMetrics, DashboardTrends, TrendMetric } from '../types/generated'
