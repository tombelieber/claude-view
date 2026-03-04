import { useQuery } from '@tanstack/react-query'
import type { CategoriesResponse } from '../types/generated/CategoriesResponse'
import type { TimeRange } from './use-insights'

// ============================================================================
// Helpers
// ============================================================================

function timeRangeToBounds(timeRange: TimeRange): { from?: number; to?: number } {
  if (timeRange === 'all') {
    return {}
  }

  const now = Math.floor(Date.now() / 1000)

  switch (timeRange) {
    case '7d':
      return { from: now - 7 * 86400, to: now }
    case '30d':
      return { from: now - 30 * 86400, to: now }
    case '90d':
      return { from: now - 90 * 86400, to: now }
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
  return useQuery({
    queryKey: ['insights-categories', timeRange],
    queryFn: async (): Promise<CategoriesResponse> => {
      const { from, to } = timeRangeToBounds(timeRange)
      const params = new URLSearchParams()
      if (from != null) params.set('from', from.toString())
      if (to != null) params.set('to', to.toString())

      const query = params.toString()

      const response = await fetch(
        query ? `/api/insights/categories?${query}` : '/api/insights/categories',
      )
      if (!response.ok) {
        const errorText = await response.text()
        throw new Error(`Failed to fetch categories: ${errorText}`)
      }

      return response.json()
    },
    staleTime: 60_000, // 1 minute
    refetchOnWindowFocus: false,
    enabled,
  })
}
