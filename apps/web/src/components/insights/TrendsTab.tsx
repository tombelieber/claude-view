import { AlertTriangle, RefreshCw } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import type { TimeRange } from '../../hooks/use-insights'
import {
  type TrendsGranularity,
  type TrendsMetric,
  useTrendsData,
} from '../../hooks/use-trends-data'
import { ActivityHeatmapGrid } from './ActivityHeatmapGrid'
import { CategoryEvolutionChart } from './CategoryEvolutionChart'
import { TrendsChart } from './TrendsChart'

type ScopeValue = 'primary_sessions_only' | 'primary_plus_subagent_work'

type ScopeMeta = {
  dataScope?: {
    sessions?: ScopeValue
    workload?: ScopeValue
  }
  sessionBreakdown?: {
    primarySessions?: number
    sidechainSessions?: number
    otherSessions?: number
    totalObservedSessions?: number
  }
}

function scopeLabel(scope: ScopeValue | undefined): string {
  return scope === 'primary_plus_subagent_work'
    ? 'primary + subagent work'
    : 'primary sessions only'
}

function resolveSessionBreakdown(meta: ScopeMeta | undefined, primaryFallback: number) {
  const primarySessions = meta?.sessionBreakdown?.primarySessions ?? primaryFallback
  const sidechainSessions = meta?.sessionBreakdown?.sidechainSessions ?? 0
  const otherSessions = meta?.sessionBreakdown?.otherSessions ?? 0
  const totalObservedSessions =
    meta?.sessionBreakdown?.totalObservedSessions ??
    primarySessions + sidechainSessions + otherSessions

  return {
    primarySessions,
    sidechainSessions,
    otherSessions,
    totalObservedSessions,
  }
}

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
 * Translate Insights page time range into trends query params.
 * Non-all ranges use explicit from/to to avoid hidden widening.
 */
function mapTimeRangeToQuery(
  timeRange: TimeRange,
): { range: 'all' } | { from: number; to: number } {
  const now = Math.floor(Date.now() / 1000)

  switch (timeRange) {
    case '7d':
      return { from: now - 7 * 86400, to: now }
    case '30d':
      return { from: now - 30 * 86400, to: now }
    case '90d':
      return { from: now - 90 * 86400, to: now }
    case 'all':
      return { range: 'all' }
  }
}

/**
 * Pick a default granularity based on time range.
 */
function defaultGranularity(timeRange: TimeRange): TrendsGranularity {
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
  const [granularity, setGranularity] = useState<TrendsGranularity>(defaultGranularity(timeRange))

  const query = useMemo(() => mapTimeRangeToQuery(timeRange), [timeRange])

  useEffect(() => {
    setGranularity(defaultGranularity(timeRange))
  }, [timeRange])

  const { data, isLoading, error, refetch } = useTrendsData({
    metric,
    granularity,
    ...query,
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

  const scopeMeta = data.meta as ScopeMeta | undefined
  const sessionBreakdown = resolveSessionBreakdown(scopeMeta, data.totalSessions)
  const sessionsScope = scopeLabel(scopeMeta?.dataScope?.sessions)
  const workloadScope = scopeLabel(scopeMeta?.dataScope?.workload)

  return (
    <div className="space-y-6 pt-4">
      <p className="text-xs text-gray-500 dark:text-gray-400">
        Session counts show {sessionsScope}. Workload metrics include {workloadScope}. Observed
        sessions: {sessionBreakdown.primarySessions.toLocaleString()} primary,{' '}
        {sessionBreakdown.sidechainSessions.toLocaleString()} sidechain,{' '}
        {sessionBreakdown.otherSessions.toLocaleString()} other,{' '}
        {sessionBreakdown.totalObservedSessions.toLocaleString()} total.
      </p>

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
      <ActivityHeatmapGrid data={data.activityHeatmap} insight={data.heatmapInsight} />

      {/* Summary footer */}
      <div className="text-center text-xs text-gray-400 dark:text-gray-500 pb-2">
        Showing data from {data.periodStart} to {data.periodEnd} (primary sessions:{' '}
        {data.totalSessions.toLocaleString()})
      </div>
    </div>
  )
}
