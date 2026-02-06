import { useQuery } from '@tanstack/react-query'
import type { InsightsTrendsResponse } from '../types/generated/InsightsTrendsResponse'

// ============================================================================
// Types
// ============================================================================

export type TrendsMetric = 'reedit_rate' | 'sessions' | 'lines' | 'cost_per_line' | 'prompts'
export type TrendsRange = '3mo' | '6mo' | '1yr' | 'all'
export type TrendsGranularity = 'day' | 'week' | 'month'

export interface TrendsParams {
  metric: TrendsMetric
  range: TrendsRange
  granularity: TrendsGranularity
  from?: number
  to?: number
}

// ============================================================================
// Metric display info
// ============================================================================

export const METRIC_OPTIONS: { value: TrendsMetric; label: string; isLowerBetter: boolean }[] = [
  { value: 'reedit_rate', label: 'Re-edit Rate', isLowerBetter: true },
  { value: 'sessions', label: 'Session Count', isLowerBetter: false },
  { value: 'lines', label: 'Lines Produced', isLowerBetter: false },
  { value: 'cost_per_line', label: 'Cost per Line', isLowerBetter: true },
  { value: 'prompts', label: 'Prompts / Session', isLowerBetter: true },
]

export const GRANULARITY_OPTIONS: { value: TrendsGranularity; label: string }[] = [
  { value: 'day', label: 'Day' },
  { value: 'week', label: 'Week' },
  { value: 'month', label: 'Month' },
]

// ============================================================================
// Fetcher
// ============================================================================

async function fetchTrends(params: TrendsParams): Promise<InsightsTrendsResponse> {
  const searchParams = new URLSearchParams()
  searchParams.set('metric', params.metric)
  searchParams.set('range', params.range)
  searchParams.set('granularity', params.granularity)
  if (params.from) searchParams.set('from', params.from.toString())
  if (params.to) searchParams.set('to', params.to.toString())

  const response = await fetch(`/api/insights/trends?${searchParams}`)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch trends data: ${errorText}`)
  }
  return response.json()
}

// ============================================================================
// Hook
// ============================================================================

/**
 * Fetch trends data from the API with metric, range, and granularity params.
 *
 * Uses React Query for caching with 5 minute stale time.
 */
export function useTrendsData(params: TrendsParams) {
  return useQuery({
    queryKey: ['insights', 'trends', params],
    queryFn: () => fetchTrends(params),
    staleTime: 5 * 60 * 1000,   // 5 minutes
    gcTime: 30 * 60 * 1000,     // 30 minutes cache
    refetchOnWindowFocus: false,
  })
}
