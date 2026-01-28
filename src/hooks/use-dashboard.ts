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
async function fetchDashboardStats(): Promise<ExtendedDashboardStats> {
  const response = await fetch('/api/stats/dashboard')
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
export function useDashboardStats() {
  return useQuery({
    queryKey: ['dashboard-stats'],
    queryFn: fetchDashboardStats,
    staleTime: 30_000, // Consider data fresh for 30 seconds
  })
}

// Re-export types for convenience
export type { ExtendedDashboardStats, CurrentWeekMetrics, DashboardTrends, TrendMetric } from '../types/generated'
