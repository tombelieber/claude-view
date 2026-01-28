import { useQuery } from '@tanstack/react-query'
import type { WeekTrends } from '../types/generated'

/**
 * Fetch week-over-week trend metrics from /api/trends.
 *
 * Returns trends for:
 * - sessionCount
 * - totalTokens
 * - avgTokensPerPrompt
 * - totalFilesEdited
 * - avgReeditRate
 * - commitLinkCount
 */
async function fetchTrends(): Promise<WeekTrends> {
  const response = await fetch('/api/trends')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch trends: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch week-over-week trend metrics.
 *
 * Each TrendMetric contains:
 * - current: Current period value
 * - previous: Previous period value
 * - delta: Absolute change (current - previous)
 * - deltaPercent: Percentage change (null if previous == 0)
 */
export function useTrends() {
  return useQuery({
    queryKey: ['trends'],
    queryFn: fetchTrends,
    staleTime: 60_000, // Trends change slowly, cache for 1 minute
  })
}

// Re-export types for convenience
export type { WeekTrends, TrendMetric } from '../types/generated'
