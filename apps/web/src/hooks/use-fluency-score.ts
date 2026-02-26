import { useQuery } from '@tanstack/react-query'

export interface FluencyScore {
  score: number | null
  achievementRate: number
  frictionRate: number
  costEfficiency: number
  satisfactionTrend: number
  consistency: number
  sessionsAnalyzed: number
}

/**
 * Fetch the current AI Fluency Score (0-100).
 *
 * Polls every 60s, considers data fresh for 30s.
 * Returns null score when insufficient data is available.
 */
export function useFluencyScore() {
  return useQuery<FluencyScore>({
    queryKey: ['fluency-score'],
    queryFn: async () => {
      const response = await fetch('/api/score')
      if (!response.ok) {
        const errorText = await response.text()
        throw new Error(`Failed to fetch fluency score: ${errorText}`)
      }
      return response.json()
    },
    refetchInterval: 60_000,
    staleTime: 30_000,
  })
}
