import { DollarSign, TrendingDown, ArrowRight } from 'lucide-react'
import { InsightLine } from './InsightLine'
import type { EfficiencyMetrics as EfficiencyMetricsType } from '../../types/generated'

interface EfficiencyMetricsSectionProps {
  efficiency: EfficiencyMetricsType
}

/**
 * EfficiencyMetrics displays ROI metrics: cost, lines, cost per line/commit.
 *
 * Shows cost trend over recent weeks to track efficiency improvement.
 */
export function EfficiencyMetricsSection({ efficiency }: EfficiencyMetricsSectionProps) {
  const {
    totalCost,
    totalLines,
    costPerLine,
    costPerCommit,
    costTrend,
    insight,
  } = efficiency

  const totalCommits = costPerCommit && totalCost > 0
    ? Math.round(totalCost / costPerCommit)
    : null

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex items-center gap-2 mb-4">
        <DollarSign className="w-4 h-4 text-emerald-500" aria-hidden="true" />
        <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          Efficiency
        </h2>
      </div>

      {/* Summary Flow */}
      <div className="flex flex-wrap items-center gap-2 text-lg mb-4">
        <span className="font-semibold text-gray-900 dark:text-gray-100">
          ${totalCost.toFixed(2)} spent
        </span>
        <ArrowRight className="w-4 h-4 text-gray-400" aria-hidden="true" />
        <span className="font-semibold text-gray-900 dark:text-gray-100">
          {formatNumber(Number(totalLines))} lines produced
        </span>
        {totalCommits !== null && (
          <>
            <ArrowRight className="w-4 h-4 text-gray-400" aria-hidden="true" />
            <span className="font-semibold text-gray-900 dark:text-gray-100">
              {totalCommits} commits shipped
            </span>
          </>
        )}
      </div>

      {/* Cost Breakdown */}
      <div className="flex flex-wrap gap-6 text-sm text-gray-600 dark:text-gray-400 mb-4">
        <div>
          <span className="font-medium">Cost per line:</span>{' '}
          <span className="text-gray-900 dark:text-gray-100 tabular-nums">
            {costPerLine !== null ? `$${costPerLine.toFixed(4)}` : '--'}
          </span>
        </div>
        <div>
          <span className="font-medium">Cost per commit:</span>{' '}
          <span className="text-gray-900 dark:text-gray-100 tabular-nums">
            {costPerCommit !== null ? `$${costPerCommit.toFixed(2)}` : '--'}
          </span>
        </div>
      </div>

      {/* Cost Trend */}
      {costTrend.length > 1 && (
        <div className="mb-4">
          <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400 mb-2">
            <TrendingDown className="w-4 h-4 text-green-500" aria-hidden="true" />
            <span className="font-medium">Trend (last {costTrend.length} weeks):</span>
          </div>
          <div className="flex items-center gap-2 text-sm tabular-nums">
            {costTrend.map((cost, i) => (
              <span key={i} className="flex items-center gap-2">
                <span
                  className={
                    i === costTrend.length - 1
                      ? 'font-semibold text-green-600 dark:text-green-400'
                      : 'text-gray-500 dark:text-gray-400'
                  }
                >
                  ${cost.toFixed(3)}
                </span>
                {i < costTrend.length - 1 && (
                  <ArrowRight className="w-3 h-3 text-gray-300" aria-hidden="true" />
                )}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Insight */}
      <InsightLine insight={insight} />
    </div>
  )
}

/**
 * Format large numbers with K/M suffixes.
 */
function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return n.toLocaleString()
}
