import { useQuery } from '@tanstack/react-query'
import type { InsightsResponse } from '../types/generated/InsightsResponse'
import type { GeneratedInsight } from '../types/generated/GeneratedInsight'

// ============================================================================
// Time Range Types
// ============================================================================

export type TimeRange = '7d' | '30d' | '90d' | 'all'

export type TabId = 'patterns' | 'trends' | 'categories' | 'benchmarks'

// ============================================================================
// UI Data Types (mapped from API response)
// ============================================================================

export interface HeroInsightData {
  id: string
  title: string
  description: string
  impactScore: number
  category: string
  metric: {
    value: number
    comparison: number
    unit: string
    improvement: number
  }
  sampleSize: number
}

export interface WorkBreakdownData {
  total: number
  withCommits: number
  exploration: number
  avgMinutes: number
}

export interface EfficiencyData {
  editsPerFile: number
  trend: number
  reeditRate: number
  trendDirection: 'improving' | 'stable' | 'declining'
}

export interface PatternStatsData {
  bestTime: string
  bestDay: string
  improvementPct: number
}

export interface InsightsData {
  heroInsight: HeroInsightData | null
  quickStats: {
    workBreakdown: WorkBreakdownData | null
    efficiency: EfficiencyData | null
    patterns: PatternStatsData | null
  }
  patternGroups: {
    high: GeneratedInsight[]
    medium: GeneratedInsight[]
    observations: GeneratedInsight[]
  }
  meta: {
    totalSessions: number
    patternsReturned: number
    minSessionsRequired: number
    hasEnoughData: boolean
  }
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Convert TimeRange to unix timestamps (seconds) for API.
 */
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
      return { from: 1, to: now }
  }
}

/**
 * Map insights API response to UI data structure.
 */
function mapApiToUi(api: InsightsResponse): InsightsData {
  const totalSessions = api.overview.workBreakdown.totalSessions
  const hasEnoughData = totalSessions >= 20

  const trendStr = api.overview.efficiency.trend
  const trendDirection: 'improving' | 'stable' | 'declining' =
    trendStr === 'improving' ? 'improving' : trendStr === 'declining' ? 'declining' : 'stable'

  return {
    heroInsight: api.topInsight
      ? {
          id: api.topInsight.patternId,
          title: api.topInsight.title,
          description: api.topInsight.body,
          impactScore: api.topInsight.impactScore,
          category: api.topInsight.category,
          metric: {
            value: api.topInsight.evidence?.comparisonValues?.['value'] ?? 0,
            comparison: api.topInsight.evidence?.comparisonValues?.['comparison'] ?? 0,
            unit: 're-edit rate',
            improvement:
              api.topInsight.evidence?.comparisonValues?.['improvement_pct'] ?? 0,
          },
          sampleSize: api.topInsight.evidence.sampleSize,
        }
      : null,

    quickStats: {
      workBreakdown: {
        total: api.overview.workBreakdown.totalSessions,
        withCommits: api.overview.workBreakdown.withCommits,
        exploration: api.overview.workBreakdown.exploration,
        avgMinutes: api.overview.workBreakdown.avgSessionMinutes,
      },
      efficiency: {
        editsPerFile: api.overview.efficiency.avgEditVelocity,
        trend: api.overview.efficiency.trendPct,
        reeditRate: api.overview.efficiency.avgReeditRate,
        trendDirection,
      },
      patterns:
        api.overview.bestTime.dayOfWeek || api.overview.bestTime.timeSlot
          ? {
              bestTime: api.overview.bestTime.timeSlot,
              bestDay: api.overview.bestTime.dayOfWeek,
              improvementPct: api.overview.bestTime.improvementPct,
            }
          : null,
    },

    patternGroups: {
      high: api.patterns.high,
      medium: api.patterns.medium,
      observations: api.patterns.observations,
    },

    meta: {
      totalSessions,
      patternsReturned: api.patterns.high.length + api.patterns.medium.length + api.patterns.observations.length,
      minSessionsRequired: 20,
      hasEnoughData,
    },
  }
}

// ============================================================================
// Hook
// ============================================================================

interface UseInsightsOptions {
  timeRange: TimeRange
}

/**
 * Fetch insights data from the API with time range filtering.
 *
 * Uses React Query for caching with 1 minute stale time.
 */
export function useInsights({ timeRange }: UseInsightsOptions) {
  const { from, to } = timeRangeToTimestamps(timeRange)

  return useQuery({
    queryKey: ['insights', from, to],
    queryFn: async (): Promise<InsightsData> => {
      const params = new URLSearchParams({
        from: from.toString(),
        to: to.toString(),
      })

      const response = await fetch(`/api/insights?${params}`)
      if (!response.ok) {
        const errorText = await response.text()
        throw new Error(`Failed to fetch insights: ${errorText}`)
      }

      const apiResponse: InsightsResponse = await response.json()
      return mapApiToUi(apiResponse)
    },
    staleTime: 60_000, // 1 minute
    refetchOnWindowFocus: false,
  })
}
