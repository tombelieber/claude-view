import { MetricCard } from './ui'
import { formatNumber } from '../lib/format-utils'
import type { DashboardTrends, TrendMetric } from '../types/generated'

export interface DashboardMetricsGridProps {
  /** Period-over-period trends data from dashboard API */
  trends: DashboardTrends | null | undefined
  /** Comparison label (e.g., "vs prev 7d") - shown in footer */
  comparisonLabel?: string | null
}

/** Helper to extract trend data from TrendMetric */
function extractTrend(metric: TrendMetric): { delta: number; deltaPercent: number | null } {
  return {
    delta: Number(metric.delta),
    deltaPercent: metric.deltaPercent,
  }
}

/**
 * DashboardMetricsGrid displays 6 key metrics in a responsive grid.
 *
 * Metrics:
 * 1. Sessions - session count for the selected period
 * 2. Tokens - total tokens used for the selected period
 * 3. Files Edited - files touched for the selected period
 * 4. Commits Linked - git integration for the selected period
 * 5. Tokens/Prompt - average tokens per user prompt for the selected period
 * 6. Re-edit Rate - percentage of files re-edited for the selected period
 *
 * Responsive:
 * - 3 columns on desktop (2 rows of 3)
 * - 2 columns on tablet
 * - 1 column on mobile
 *
 * Null safety:
 * - Gracefully handles null/undefined trends data
 * - Shows placeholder when data is unavailable
 */
export function DashboardMetricsGrid({ trends, comparisonLabel }: DashboardMetricsGridProps) {
  // Handle null/undefined trends gracefully
  if (!trends) {
    return (
      <section
        className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 sm:gap-4"
        aria-label="Week-over-week metrics (loading)"
      >
        {Array.from({ length: 6 }).map((_, i) => (
          <div key={i} className="h-24 bg-gray-100 dark:bg-gray-800 rounded animate-pulse" />
        ))}
      </section>
    )
  }

  // Build footer label if comparison label is provided
  const footer = comparisonLabel || undefined

  return (
    <section
      className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 sm:gap-4"
      aria-label="Period metrics"
    >
      <MetricCard
        label="Sessions"
        value={formatNumber(trends.sessions?.current || 0)}
        trend={trends.sessions ? extractTrend(trends.sessions) : { delta: 0, deltaPercent: null }}
        footer={footer}
      />
      <MetricCard
        label="Tokens"
        value={formatNumber(trends.tokens?.current || 0)}
        trend={trends.tokens ? extractTrend(trends.tokens) : { delta: 0, deltaPercent: null }}
        footer={footer}
      />
      <MetricCard
        label="Files Edited"
        value={formatNumber(trends.filesEdited?.current || 0)}
        trend={trends.filesEdited ? extractTrend(trends.filesEdited) : { delta: 0, deltaPercent: null }}
        footer={footer}
      />
      <MetricCard
        label="Commits Linked"
        value={formatNumber(trends.commits?.current || 0)}
        trend={trends.commits ? extractTrend(trends.commits) : { delta: 0, deltaPercent: null }}
        footer={footer}
      />
      <MetricCard
        label="Tokens/Prompt"
        value={formatNumber(trends.avgTokensPerPrompt?.current || 0)}
        trend={trends.avgTokensPerPrompt ? extractTrend(trends.avgTokensPerPrompt) : { delta: 0, deltaPercent: null }}
        footer={footer}
      />
      <MetricCard
        label="Re-edit Rate"
        value={`${trends.avgReeditRate?.current || 0}%`}
        trend={trends.avgReeditRate ? extractTrend(trends.avgReeditRate) : { delta: 0, deltaPercent: null }}
        footer={footer}
      />
    </section>
  )
}
