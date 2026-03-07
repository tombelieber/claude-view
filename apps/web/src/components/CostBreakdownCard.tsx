import { DollarSign } from 'lucide-react'
import { formatCostUsd } from '../lib/format-utils'
import { COST_CATEGORY_COLORS, COST_SEGMENT_CONFIG } from '../theme'
import type { AggregateCostBreakdown } from '../types/generated/AggregateCostBreakdown'
import { MetricCard, StackedBar } from './ui'
import type { StackedBarSegment } from './ui/StackedBar'

interface CostBreakdownCardProps {
  cost: AggregateCostBreakdown
}

export function CostBreakdownCard({ cost }: CostBreakdownCardProps) {
  if (cost.totalCostUsd === 0) return null

  const segments: StackedBarSegment[] = COST_SEGMENT_CONFIG.map((s) => ({
    label: s.label,
    value: cost[s.key],
    color: s.color.light,
    darkColor: s.color.dark,
  }))

  return (
    <div className="space-y-3">
      {/* Hero card with stacked bar */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
        <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2 flex items-center gap-1.5">
          <DollarSign className="w-3.5 h-3.5" />
          Total Cost
        </p>
        <p className="text-3xl sm:text-4xl font-semibold text-blue-800 dark:text-blue-300 tabular-nums mb-4">
          {formatCostUsd(cost.totalCostUsd)}
        </p>
        <StackedBar segments={segments} />

        {/* Cache savings callout */}
        {cost.cacheSavingsUsd > 0 && (
          <p className={`mt-3 text-sm ${COST_CATEGORY_COLORS.savings.text}`}>
            Saved {formatCostUsd(cost.cacheSavingsUsd)} via prompt caching
          </p>
        )}
      </div>

      {/* Detail cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        {COST_SEGMENT_CONFIG.map((s) => {
          const value = cost[s.key]
          const pct = cost.totalCostUsd > 0 ? ((value / cost.totalCostUsd) * 100).toFixed(1) : '0.0'
          return (
            <MetricCard
              key={s.key}
              label={s.label}
              value={formatCostUsd(value)}
              subValue={`${pct}% of total`}
            />
          )
        })}
      </div>
    </div>
  )
}
