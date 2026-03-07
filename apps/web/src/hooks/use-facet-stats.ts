import { useQuery } from '@tanstack/react-query'

// ============================================================================
// Types
// ============================================================================

export interface FacetStats {
  totalWithFacets: number
  totalWithoutFacets: number
  achievementRate: number
  frustratedCount: number
  satisfiedOrAboveCount: number
  frictionSessionCount: number
}

// ============================================================================
// Fetch
// ============================================================================

async function fetchFacetStats(): Promise<FacetStats> {
  const response = await fetch('/api/facets/stats')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch facet stats: ${errorText}`)
  }
  return response.json()
}

// ============================================================================
// Hook
// ============================================================================

/**
 * Fetch aggregate facet statistics from the API.
 *
 * Uses React Query for caching with 1 minute stale time.
 */
export function useFacetStats() {
  return useQuery({
    queryKey: ['facetStats'],
    queryFn: fetchFacetStats,
    staleTime: 60_000, // Cache for 1 minute
    refetchOnWindowFocus: false,
  })
}
