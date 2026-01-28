import { MetricCard } from './MetricCard'
import { formatNumber, formatPercent } from '../lib/format-utils'
import type { WeekTrends, TrendMetric } from '../types/generated'

export interface DashboardMetricsGridProps {
  /** Week-over-week trends data */
  trends: WeekTrends
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
 * 1. Sessions - session count
 * 2. Tokens - total tokens used
 * 3. Files Edited - files touched
 * 4. Tokens/Prompt - efficiency metric
 * 5. Re-edit Rate - code quality signal
 * 6. Commits Linked - git integration
 *
 * Responsive:
 * - 3 columns on desktop
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
        value={formatNumber(trends.sessionCount.current)}
        trend={extractTrend(trends.sessionCount)}
      />
      <MetricCard
        label="Tokens"
        value={formatNumber(trends.totalTokens.current)}
        trend={extractTrend(trends.totalTokens)}
      />
      <MetricCard
        label="Files Edited"
        value={formatNumber(trends.totalFilesEdited.current)}
        trend={extractTrend(trends.totalFilesEdited)}
      />
      <MetricCard
        label="Tokens/Prompt"
        value={formatNumber(trends.avgTokensPerPrompt.current)}
        trend={extractTrend(trends.avgTokensPerPrompt)}
      />
      <MetricCard
        label="Re-edit Rate"
        value={formatPercent(
          trends.avgReeditRate.current !== 0n
            ? Number(trends.avgReeditRate.current)
            : null
        )}
        trend={extractTrend(trends.avgReeditRate)}
      />
      <MetricCard
        label="Commits Linked"
        value={formatNumber(trends.commitLinkCount.current)}
        trend={extractTrend(trends.commitLinkCount)}
      />
    </section>
  )
}
