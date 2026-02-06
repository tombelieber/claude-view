import { useQuery } from '@tanstack/react-query'
import type { ExtendedDashboardStats } from '../types/generated'
import type { TimeRangeParams } from '../types/time-range'

export type { TimeRangeParams } from '../types/time-range'

/**
 * Fetch dashboard stats with optional time range filter.
 *
 * Response includes:
 * - Base stats: totalSessions, totalProjects, heatmap, topSkills, topProjects, toolTotals
 * - currentWeek: sessionCount, totalTokens, totalFilesEdited, commitCount
 * - trends: period-over-period changes (null for all-time view)
 * - periodStart, periodEnd, comparisonPeriodStart, comparisonPeriodEnd
 * - dataStartDate: earliest session in database
 */
async function fetchDashboardStats(params?: TimeRangeParams): Promise<ExtendedDashboardStats> {
  let url = '/api/stats/dashboard'

  // Add time range params if provided (not all-time)
  if (params?.from != null && params?.to != null) {
    const searchParams = new URLSearchParams()
    searchParams.set('from', params.from.toString())
    searchParams.set('to', params.to.toString())
    url += `?${searchParams.toString()}`
  }

  const response = await fetch(url)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch dashboard stats: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch extended dashboard statistics with optional time range filter.
 *
 * @param timeRange - Optional time range filter. If null/undefined or both from/to are null,
 *                    returns all-time stats with no trends.
 *
 * Returns ExtendedDashboardStats with:
 * - totalSessions, totalProjects (counts)
 * - heatmap (DayActivity[]) - always 90 days, not affected by time range
 * - topSkills, topCommands, topMcpTools, topAgents (SkillStat[])
 * - topProjects (ProjectStat[])
 * - toolTotals (ToolCounts)
 * - currentWeek (CurrentWeekMetrics) - metrics for selected period
 * - trends (DashboardTrends | null) - null for all-time view
 * - periodStart, periodEnd, comparisonPeriodStart, comparisonPeriodEnd
 * - dataStartDate - earliest session date ("since [date]")
 */
export function useDashboardStats(timeRange?: TimeRangeParams | null) {
  return useQuery({
    queryKey: ['dashboard-stats', timeRange?.from, timeRange?.to],
    queryFn: () => fetchDashboardStats(timeRange ?? undefined),
    staleTime: 30_000, // Consider data fresh for 30 seconds
  })
}

// Re-export types for convenience
export type { ExtendedDashboardStats, CurrentWeekMetrics, DashboardTrends, TrendMetric } from '../types/generated'
