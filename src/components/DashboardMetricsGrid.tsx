import { MetricCard } from './MetricCard'
import { formatNumber } from '../lib/format-utils'
import type { DashboardTrends, TrendMetric } from '../types/generated'

export interface DashboardMetricsGridProps {
  /** Week-over-week trends data from dashboard API */
  trends: DashboardTrends
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
 * 1. Sessions - session count this week
 * 2. Tokens - total tokens used this week
 * 3. Files Edited - files touched this week
 * 4. Commits Linked - git integration this week
 * 5. Tokens/Prompt - average tokens per user prompt this week
 * 6. Re-edit Rate - percentage of files re-edited this week
 *
 * Responsive:
 * - 3 columns on desktop (2 rows of 3)
 * - 2 columns on tablet
 * - 1 column on mobile
 */
export function DashboardMetricsGrid({ trends }: DashboardMetricsGridProps) {
  return (
    <section
      className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4"
      aria-label="Week-over-week metrics"
    >
      <MetricCard
        label="Sessions"
        value={formatNumber(trends.sessions.current)}
        trend={extractTrend(trends.sessions)}
      />
      <MetricCard
        label="Tokens"
        value={formatNumber(trends.tokens.current)}
        trend={extractTrend(trends.tokens)}
      />
      <MetricCard
        label="Files Edited"
        value={formatNumber(trends.filesEdited.current)}
        trend={extractTrend(trends.filesEdited)}
      />
      <MetricCard
        label="Commits Linked"
        value={formatNumber(trends.commits.current)}
        trend={extractTrend(trends.commits)}
      />
      <MetricCard
        label="Tokens/Prompt"
        value={formatNumber(trends.avgTokensPerPrompt.current)}
        trend={extractTrend(trends.avgTokensPerPrompt)}
      />
      <MetricCard
        label="Re-edit Rate"
        value={`${trends.avgReeditRate.current}%`}
        trend={extractTrend(trends.avgReeditRate)}
      />
    </section>
  )
}
