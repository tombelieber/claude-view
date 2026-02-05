import { useQuery } from '@tanstack/react-query'
import type { CategoriesResponse } from '../types/generated/CategoriesResponse'
import type { TimeRange } from './use-insights'

// ============================================================================
// Helpers
// ============================================================================

function timeRangeToTimestamps(timeRange: TimeRange): { from: number; to: number } {
  const now = Math.floor(Date.now() / 1000)

  switch (timeRange) {
    case '7d':
      return { from: now - 7 * 86400, to: now }
    case '30d':
      return { from: now - 30 * 86400, to: now }
    case '90d':
      return { from: now - 90 * 86400, to: now }
    case 'all':
      return { from: 0, to: now }
  }
}

// ============================================================================
// Hook
// ============================================================================

interface UseCategoriesOptions {
  timeRange: TimeRange
  enabled?: boolean
}

/**
 * Fetch category breakdown data from the API.
 * Uses React Query for caching with 1 minute stale time.
 */
export function useCategories({ timeRange, enabled = true }: UseCategoriesOptions) {
  const { from, to } = timeRangeToTimestamps(timeRange)

  return useQuery({
    queryKey: ['insights-categories', from, to],
    queryFn: async (): Promise<CategoriesResponse> => {
      const params = new URLSearchParams({
        from: from.toString(),
        to: to.toString(),
      })

      const response = await fetch(`/api/insights/categories?${params}`)
      if (!response.ok) {
        throw new Error('Failed to fetch categories')
      }

      return response.json()
    },
    staleTime: 60_000, // 1 minute
    refetchOnWindowFocus: false,
    enabled,
  })
}
