import { useState } from 'react'
import { RefreshCw, AlertTriangle } from 'lucide-react'
import {
  useTrendsData,
  type TrendsMetric,
  type TrendsGranularity,
} from '../../hooks/use-trends-data'
import type { TimeRange } from '../../hooks/use-insights'
import { TrendsChart } from './TrendsChart'
import { CategoryEvolutionChart } from './CategoryEvolutionChart'
import { ActivityHeatmapGrid } from './ActivityHeatmapGrid'

// ============================================================================
// Types
// ============================================================================

interface TrendsTabProps {
  timeRange: TimeRange
}

// ============================================================================
// Time range mapping
// ============================================================================

/**
 * Map the Phase 5 TimeRange to the trends API range parameter.
 */
function mapTimeRange(timeRange: TimeRange): '3mo' | '6mo' | '1yr' | 'all' {
  switch (timeRange) {
    case '7d':
      return '3mo' // 7d is too short; use 3mo to show context
    case '30d':
      return '3mo'
    case '90d':
      return '6mo'
    case 'all':
      return 'all'
  }
}

/**
 * Pick a default granularity based on time range.
 */
function defaultGranularity(
  timeRange: TimeRange
): TrendsGranularity {
  switch (timeRange) {
    case '7d':
      return 'day'
    case '30d':
      return 'day'
    case '90d':
      return 'week'
    case 'all':
      return 'month'
  }
}

// ============================================================================
// Skeleton
// ============================================================================

function TrendsSkeleton() {
  return (
    <div className="animate-pulse space-y-6 pt-4">
      {/* Line chart skeleton */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="h-5 w-40 bg-gray-200 dark:bg-gray-700 rounded" />
          <div className="flex items-center gap-2">
            <div className="h-8 w-28 bg-gray-200 dark:bg-gray-700 rounded" />
            <div className="h-8 w-20 bg-gray-200 dark:bg-gray-700 rounded" />
          </div>
        </div>
        <div className="h-[300px] bg-gray-100 dark:bg-gray-800 rounded" />
        <div className="h-4 w-3/4 bg-gray-200 dark:bg-gray-700 rounded mt-4" />
      </div>

      {/* Category chart skeleton */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <div className="h-5 w-36 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
        <div className="h-[300px] bg-gray-100 dark:bg-gray-800 rounded" />
      </div>

      {/* Heatmap skeleton */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <div className="h-5 w-32 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
        <div className="h-[200px] bg-gray-100 dark:bg-gray-800 rounded" />
      </div>
    </div>
  )
}

// ============================================================================
// Component
// ============================================================================

export function TrendsTab({ timeRange }: TrendsTabProps) {
  const [metric, setMetric] = useState<TrendsMetric>('reedit_rate')
  const [granularity, setGranularity] = useState<TrendsGranularity>(
    defaultGranularity(timeRange)
  )

  const apiRange = mapTimeRange(timeRange)

  const { data, isLoading, error, refetch } = useTrendsData({
    metric,
    range: apiRange,
    granularity,
  })

  // Error state
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-16">
        <AlertTriangle className="w-8 h-8 text-amber-400 mb-3" />
        <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100 mb-1">
          Unable to Load Trends
        </h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mb-4 text-center max-w-md">
          Something went wrong while loading trend data.
        </p>
        <button
          onClick={() => refetch()}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 transition-colors cursor-pointer"
        >
          <RefreshCw className="w-4 h-4" />
          Retry
        </button>
      </div>
    )
  }

  // Loading state
  if (isLoading || !data) {
    return <TrendsSkeleton />
  }

  return (
    <div className="space-y-6 pt-4">
      {/* 7.2 Efficiency Over Time Line Chart */}
      <TrendsChart
        data={data.dataPoints}
        metric={metric}
        average={data.average}
        trend={data.trend}
        trendDirection={data.trendDirection}
        insight={data.insight}
        onMetricChange={setMetric}
        granularity={granularity}
        onGranularityChange={setGranularity}
      />

      {/* 7.3 Category Evolution Stacked Area Chart */}
      <CategoryEvolutionChart
        data={data.categoryEvolution}
        insight={data.categoryInsight}
        classificationRequired={data.classificationRequired}
      />

      {/* 7.4 Activity Heatmap */}
      <ActivityHeatmapGrid
        data={data.activityHeatmap}
        insight={data.heatmapInsight}
      />

      {/* Summary footer */}
      <div className="text-center text-xs text-gray-400 dark:text-gray-500 pb-2">
        Showing data from {data.periodStart} to {data.periodEnd} ({data.totalSessions} sessions)
      </div>
    </div>
  )
}
